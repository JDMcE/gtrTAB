//! The pedals a track's sound, and then the whole mix, goes through: each a few filters,
//! delays and curves run over a block of stereo samples, carrying their state from one
//! block to the next.

use std::f32::consts::{PI, TAU};

use gtr_model::{Pedal, PedalKind};

/// Gain for a change of level in decibels.
fn db(decibels: f32) -> f32 {
    10f32.powf(decibels / 20.0)
}

/// A level knob: at its middle the level is as it was, and at its ends 12 dB down or up.
fn level(knob: f32) -> f32 {
    db((knob - 0.5) * 24.0)
}

/// From `low` at 0.0 to `high` at 1.0 on a logarithmic scale, as frequencies and times are
/// heard.
fn sweep(knob: f32, low: f32, high: f32) -> f32 {
    low * (high / low).powf(knob.clamp(0.0, 1.0))
}

/// How far a one-pole smoother moves towards its target each sample, for a time constant
/// of `seconds`.
fn glide(seconds: f32, rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * rate).max(1.0)).exp()
}

/// A second-order filter in transposed direct form II, with the state of both sides.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    state: [[f32; 2]; 2],
}

impl Biquad {
    /// A filter of the shapes in Robert Bristow-Johnson's cookbook.
    fn new(shape: Shape, frequency: f32, q: f32, rate: f32) -> Biquad {
        let w = TAU * frequency.clamp(10.0, rate * 0.49) / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let (b, a) = match shape {
            Shape::LowPass => {
                let b1 = 1.0 - cos;
                (
                    [b1 / 2.0, b1, b1 / 2.0],
                    [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
                )
            }
            Shape::HighPass => {
                let b1 = 1.0 + cos;
                (
                    [b1 / 2.0, -b1, b1 / 2.0],
                    [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
                )
            }
            Shape::Peak(gain) => {
                let a = db(gain / 2.0);
                (
                    [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
                    [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
                )
            }
            Shape::LowShelf(gain) | Shape::HighShelf(gain) => {
                let a = db(gain / 2.0);
                let root = 2.0 * a.sqrt() * alpha;
                let s = if matches!(shape, Shape::LowShelf(_)) {
                    1.0
                } else {
                    -1.0
                };
                (
                    [
                        a * ((a + 1.0) - s * (a - 1.0) * cos + root),
                        s * 2.0 * a * ((a - 1.0) - s * (a + 1.0) * cos),
                        a * ((a + 1.0) - s * (a - 1.0) * cos - root),
                    ],
                    [
                        (a + 1.0) + s * (a - 1.0) * cos + root,
                        -s * 2.0 * ((a - 1.0) + s * (a + 1.0) * cos),
                        (a + 1.0) + s * (a - 1.0) * cos - root,
                    ],
                )
            }
        };
        Biquad {
            b: [b[0] / a[0], b[1] / a[0], b[2] / a[0]],
            a: [a[1] / a[0], a[2] / a[0]],
            state: [[0.0; 2]; 2],
        }
    }

    /// Takes another filter's settings and keeps its own state, so that turning a knob
    /// does not start the filter afresh.
    fn retune(&mut self, other: Biquad) {
        (self.b, self.a) = (other.b, other.a);
    }

    fn run(&mut self, side: usize, x: f32) -> f32 {
        let [z1, z2] = &mut self.state[side];
        let y = self.b[0] * x + *z1;
        *z1 = self.b[1] * x - self.a[0] * y + *z2;
        *z2 = self.b[2] * x - self.a[1] * y;
        // Below anything that can be heard, a filter's state is let go of, so that it does
        // not fall into numbers too small to work with quickly.
        if z1.abs() < 1e-20 {
            *z1 = 0.0;
        }
        if z2.abs() < 1e-20 {
            *z2 = 0.0;
        }
        y
    }
}

#[derive(Clone, Copy)]
enum Shape {
    LowPass,
    HighPass,
    /// A peak or dip of so many decibels.
    Peak(f32),
    LowShelf(f32),
    HighShelf(f32),
}

/// A row of filters run one after the other.
#[derive(Clone, Default)]
struct Filters(Vec<Biquad>);

impl Filters {
    fn set(&mut self, filters: Vec<Biquad>) {
        if self.0.len() == filters.len() {
            for (old, new) in self.0.iter_mut().zip(filters) {
                old.retune(new);
            }
        } else {
            self.0 = filters;
        }
    }

    fn run(&mut self, side: usize, x: f32) -> f32 {
        self.0.iter_mut().fold(x, |x, filter| filter.run(side, x))
    }
}

/// Runs a curve at twice the sample rate, so that the overtones it adds above half the
/// rate are filtered off instead of folding back down as harsh tones that are not in
/// the music.
#[derive(Clone)]
struct Oversampled {
    up: Filters,
    down: Filters,
}

impl Oversampled {
    fn new(rate: f32) -> Oversampled {
        // Two poles twice over, just under the rate's half, at twice the rate.
        let filters = || {
            Filters(vec![
                Biquad::new(Shape::LowPass, rate * 0.45, 0.54, rate * 2.0),
                Biquad::new(Shape::LowPass, rate * 0.45, 1.31, rate * 2.0),
            ])
        };
        Oversampled {
            up: filters(),
            down: filters(),
        }
    }

    fn run(&mut self, side: usize, x: f32, curve: impl Fn(f32) -> f32) -> f32 {
        let a = curve(self.up.run(side, 2.0 * x));
        let b = curve(self.up.run(side, 0.0));
        self.down.run(side, a);
        self.down.run(side, b)
    }
}

/// A line of past samples for each side, read at a distance that may fall between them.
#[derive(Clone)]
struct DelayLine {
    buffer: [Vec<f32>; 2],
    at: usize,
}

impl DelayLine {
    fn new(longest: usize) -> DelayLine {
        DelayLine {
            buffer: [vec![0.0; longest + 2], vec![0.0; longest + 2]],
            at: 0,
        }
    }

    fn len(&self) -> usize {
        self.buffer[0].len()
    }

    /// The sample written `delay` samples before the next one is.
    fn read(&self, side: usize, delay: f32) -> f32 {
        let delay = delay.clamp(1.0, (self.len() - 2) as f32);
        let whole = delay.floor();
        let part = delay - whole;
        let line = &self.buffer[side];
        let len = self.len();
        let i = (self.at + len - whole as usize) % len;
        let j = (i + len - 1) % len;
        line[i] + (line[j] - line[i]) * part
    }

    fn write(&mut self, side: usize, x: f32) {
        self.buffer[side][self.at] = x;
    }

    /// Moves on a sample, once both sides have been written.
    fn advance(&mut self) {
        self.at = (self.at + 1) % self.len();
    }
}

/// Follows how loud a signal is: quick to rise and slow to fall.
#[derive(Clone, Copy, Default)]
struct Envelope {
    level: f32,
    attack: f32,
    release: f32,
}

impl Envelope {
    fn run(&mut self, x: f32) -> f32 {
        let peak = x.abs();
        let rate = if peak > self.level {
            self.attack
        } else {
            self.release
        };
        self.level += rate * (peak - self.level);
        if self.level < 1e-9 {
            self.level = 0.0;
        }
        self.level
    }
}

struct Compressor {
    threshold: f32,
    ratio: f32,
    makeup: f32,
    envelope: Envelope,
}

impl Compressor {
    fn set(&mut self, pedal: &Pedal, rate: f32) {
        self.threshold = db(-40.0 + 40.0 * pedal.knob(0));
        self.ratio = 1.0 + 19.0 * pedal.knob(1).powi(2);
        self.envelope.attack = glide(sweep(pedal.knob(2), 0.0001, 0.1), rate);
        self.envelope.release = glide(sweep(pedal.knob(3), 0.02, 1.0), rate);
        // Brought back up by half of what a note at full level loses, so that the pedal
        // does not simply make the track quieter.
        let lost = (1.0 / self.threshold).powf(1.0 - 1.0 / self.ratio);
        self.makeup = level(pedal.knob(4)) * lost.sqrt();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right) {
            let peak = self.envelope.run(l.abs().max(r.abs()));
            let over = (self.threshold / peak.max(self.threshold)).powf(1.0 - 1.0 / self.ratio);
            let gain = over * self.makeup;
            *l *= gain;
            *r *= gain;
        }
    }
}

struct Wah {
    /// The pedal's position from heel to toe.
    pedal: f32,
    resonance: f32,
    /// How far how hard the strings are played moves the pedal.
    auto: f32,
    mix: f32,
    envelope: Envelope,
    /// The filter's state for each side: the two integrators of a state-variable filter.
    state: [[f32; 2]; 2],
    rate: f32,
}

impl Wah {
    fn set(&mut self, pedal: &Pedal, rate: f32) {
        self.pedal = pedal.knob(0);
        self.resonance = sweep(pedal.knob(1), 1.0, 12.0);
        self.auto = pedal.knob(2) * 8.0;
        self.mix = pedal.knob(3);
        self.envelope.attack = glide(0.005, rate);
        self.envelope.release = glide(0.12, rate);
        self.rate = rate;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right) {
            let swell = self.envelope.run(l.abs().max(r.abs())) * self.auto;
            let frequency = sweep(self.pedal + swell, 350.0, 2_400.0);
            // A state-variable filter with its frequency warped, which stays steady however
            // fast the pedal moves.
            let g = (PI * frequency / self.rate).tan();
            let k = 1.0 / self.resonance;
            let a1 = 1.0 / (1.0 + g * (g + k));
            for (side, x) in [l, r].into_iter().enumerate() {
                let [ic1, ic2] = &mut self.state[side];
                let v1 = a1 * (*ic1 + g * (*x - *ic2));
                let v2 = *ic2 + g * v1;
                *ic1 = 2.0 * v1 - *ic1;
                *ic2 = 2.0 * v2 - *ic2;
                // The band around the peak, at the level the signal had, and louder the
                // higher the resonance as a wah is.
                let band = v1 * k * self.resonance.sqrt() * 1.5;
                *x += self.mix * (band - *x);
            }
        }
    }
}

/// A drive pedal: overdrive or distortion.
struct Drive {
    hard: bool,
    gain: f32,
    output: f32,
    before: Filters,
    after: Filters,
    curve: Oversampled,
}

impl Drive {
    fn new(hard: bool, rate: f32) -> Drive {
        Drive {
            hard,
            gain: 1.0,
            output: 1.0,
            before: Filters::default(),
            after: Filters::default(),
            curve: Oversampled::new(rate),
        }
    }

    fn set(&mut self, pedal: &Pedal, rate: f32) {
        let tone = pedal.knob(1);
        if self.hard {
            self.gain = sweep(pedal.knob(0), 4.0, 600.0);
            self.output = 0.22 * level(pedal.knob(2));
            self.before
                .set(vec![Biquad::new(Shape::HighPass, 120.0, 0.7, rate)]);
            self.after.set(vec![
                Biquad::new(Shape::Peak(-6.0), 650.0, 0.8, rate),
                Biquad::new(Shape::LowPass, sweep(tone, 1_800.0, 9_000.0), 0.7, rate),
                Biquad::new(Shape::LowPass, 11_000.0, 0.7, rate),
            ]);
        } else {
            self.gain = sweep(pedal.knob(0), 1.5, 80.0);
            self.output = 0.3 * level(pedal.knob(2));
            // The hump in the middle: the lows are cut before the clip, so that chords
            // stay clear.
            self.before.set(vec![
                Biquad::new(Shape::HighPass, 500.0, 0.6, rate),
                Biquad::new(Shape::Peak(4.0), 900.0, 0.8, rate),
            ]);
            self.after.set(vec![Biquad::new(
                Shape::LowPass,
                sweep(tone, 1_200.0, 7_000.0),
                0.7,
                rate,
            )]);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let (gain, hard) = (self.gain, self.hard);
        let curve = move |x: f32| match hard {
            // Clipped twice over, so that it is all but square.
            true => (3.0 * (gain * x).tanh()).tanh(),
            // Clipped more on one side than the other, as a diode pair that is not quite
            // matched does, which adds the even overtones of a valve.
            false => {
                let x = gain * x;
                (x + 0.25).tanh() - 0.25f32.tanh()
            }
        };
        for (side, samples) in [left, right].into_iter().enumerate() {
            for x in samples.iter_mut() {
                let driven = self.curve.run(side, self.before.run(side, *x), curve);
                // An overdrive keeps some of what it was given, under the clip.
                let clean = if hard { 0.0 } else { 0.3 * *x };
                *x = self.output * self.after.run(side, driven + clean);
            }
        }
    }
}

struct Amp {
    gain: f32,
    master: f32,
    input: Filters,
    tone: Filters,
    presence: Filters,
    curve: Oversampled,
}

impl Amp {
    fn new(rate: f32) -> Amp {
        Amp {
            gain: 1.0,
            master: 1.0,
            input: Filters::default(),
            tone: Filters::default(),
            presence: Filters::default(),
            curve: Oversampled::new(rate),
        }
    }

    fn set(&mut self, pedal: &Pedal, rate: f32) {
        let cut = |knob: f32| (knob - 0.5) * 24.0;
        self.gain = sweep(pedal.knob(0), 1.0, 150.0);
        self.master = 0.35 * level(pedal.knob(5));
        self.input
            .set(vec![Biquad::new(Shape::HighPass, 90.0, 0.7, rate)]);
        self.tone.set(vec![
            Biquad::new(Shape::LowShelf(cut(pedal.knob(1))), 120.0, 0.7, rate),
            Biquad::new(Shape::Peak(cut(pedal.knob(2))), 650.0, 0.7, rate),
            Biquad::new(Shape::HighShelf(cut(pedal.knob(3))), 2_800.0, 0.7, rate),
        ]);
        self.presence.set(vec![Biquad::new(
            Shape::HighShelf((pedal.knob(4) - 0.5) * 16.0),
            4_500.0,
            0.7,
            rate,
        )]);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let gain = self.gain;
        // The preamplifier's valve, biased so that it clips one side first.
        let preamp = move |x: f32| (gain * x + 0.2).tanh() - 0.2f32.tanh();
        for (side, samples) in [left, right].into_iter().enumerate() {
            for x in samples.iter_mut() {
                let pre = self.curve.run(side, self.input.run(side, *x), preamp);
                // The tone stack comes after the preamplifier, as in most valve amplifiers,
                // and the power stage rounds off what is left.
                let shaped = self.tone.run(side, pre);
                *x = self.presence.run(side, (1.5 * shaped).tanh()) * self.master;
            }
        }
    }
}

struct Cabinet {
    gain: f32,
    filters: Filters,
}

impl Cabinet {
    fn set(&mut self, pedal: &Pedal, rate: f32) {
        // From a small open combo to a closed four-by-twelve: the bigger the cabinet the
        // lower it goes, and the bigger the thump where it resonates.
        let size = pedal.knob(0);
        let resonance = 130.0 - 55.0 * size;
        // The microphone from off the cone's axis, dark, to on it, bright.
        let mic = pedal.knob(1);
        let top = sweep(mic, 2_800.0, 7_500.0);
        self.gain = level(pedal.knob(2)) * 1.4;
        self.filters.set(vec![
            Biquad::new(Shape::HighPass, resonance * 0.7, 0.8, rate),
            Biquad::new(Shape::Peak(2.0 + 4.0 * size), resonance, 1.4, rate),
            Biquad::new(Shape::Peak(-5.0), 400.0 + 200.0 * size, 1.0, rate),
            Biquad::new(Shape::Peak(2.0 + 3.0 * mic), top * 0.55, 1.5, rate),
            Biquad::new(Shape::LowPass, top, 0.9, rate),
            Biquad::new(Shape::LowPass, top * 1.2, 0.6, rate),
        ]);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (side, samples) in [left, right].into_iter().enumerate() {
            for x in samples.iter_mut() {
                *x = self.filters.run(side, *x) * self.gain;
            }
        }
    }
}

struct Eq {
    gain: f32,
    filters: Filters,
}

impl Eq {
    fn set(&mut self, pedal: &Pedal, rate: f32) {
        let cut = |knob: f32| (knob - 0.5) * 24.0;
        self.gain = level(pedal.knob(4));
        self.filters.set(vec![
            Biquad::new(Shape::LowShelf(cut(pedal.knob(0))), 100.0, 0.7, rate),
            Biquad::new(
                Shape::Peak(cut(pedal.knob(1))),
                sweep(pedal.knob(3), 200.0, 4_000.0),
                1.0,
                rate,
            ),
            Biquad::new(Shape::HighShelf(cut(pedal.knob(2))), 4_000.0, 0.7, rate),
        ]);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (side, samples) in [left, right].into_iter().enumerate() {
            for x in samples.iter_mut() {
                *x = self.filters.run(side, *x) * self.gain;
            }
        }
    }
}

/// Seconds of delay a chorus sweeps around, and the furthest it sweeps from it.
const CHORUS_DELAY: f32 = 0.012;
const CHORUS_DEPTH: f32 = 0.006;

struct Chorus {
    /// Cycles of the sweep per sample, how far it sweeps in samples, and how much of it is
    /// heard.
    step: f32,
    depth: f32,
    mix: f32,
    phase: f32,
    line: DelayLine,
    rate: f32,
}

impl Chorus {
    fn new(rate: f32) -> Chorus {
        Chorus {
            step: 0.0,
            depth: 0.0,
            mix: 0.0,
            phase: 0.0,
            line: DelayLine::new(((CHORUS_DELAY + CHORUS_DEPTH) * rate) as usize + 2),
            rate,
        }
    }

    fn set(&mut self, pedal: &Pedal) {
        self.step = sweep(pedal.knob(0), 0.1, 6.0) / self.rate;
        self.depth = pedal.knob(1) * CHORUS_DEPTH * self.rate;
        self.mix = pedal.knob(2);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let centre = CHORUS_DELAY * self.rate;
        // Louder for the copy added, as much less as it adds.
        let scale = 1.0 / (1.0 + 0.5 * self.mix);
        for (l, r) in left.iter_mut().zip(right) {
            self.phase = (self.phase + self.step).fract();
            // The two sides sweep a quarter of a turn apart, which spreads the sound.
            for (side, x) in [l, r].into_iter().enumerate() {
                let turn = TAU * (self.phase + 0.25 * side as f32);
                let wet = self.line.read(side, centre + self.depth * turn.sin());
                self.line.write(side, *x);
                *x = (*x + self.mix * wet) * scale;
            }
            self.line.advance();
        }
    }
}

/// The longest a delay can be set to, in seconds.
const LONGEST_DELAY: f32 = 1.5;

struct Delay {
    /// The delay heading for and the one reached, in samples: a change of time glides, as
    /// a tape echo's does, instead of jumping with a click.
    target: f32,
    time: f32,
    feedback: f32,
    mix: f32,
    tone: Filters,
    line: DelayLine,
    glide: f32,
}

impl Delay {
    fn new(rate: f32) -> Delay {
        Delay {
            target: 0.0,
            time: 0.0,
            feedback: 0.0,
            mix: 0.0,
            tone: Filters::default(),
            line: DelayLine::new((LONGEST_DELAY * rate) as usize + 2),
            glide: glide(0.08, rate),
        }
    }

    /// Seconds a delay pedal's time knob stands for.
    fn seconds(pedal: &Pedal) -> f32 {
        0.02 + (LONGEST_DELAY - 0.02) * pedal.knob(0).powf(1.5)
    }

    fn set(&mut self, pedal: &Pedal, rate: f32) {
        let first = self.target == 0.0;
        self.target = Delay::seconds(pedal) * rate;
        if first {
            self.time = self.target;
        }
        self.feedback = 0.95 * pedal.knob(1);
        self.mix = pedal.knob(3);
        let tone = sweep(pedal.knob(2), 1_000.0, 14_000.0);
        self.tone
            .set(vec![Biquad::new(Shape::LowPass, tone, 0.6, rate)]);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right) {
            self.time += self.glide * (self.target - self.time);
            for (side, x) in [l, r].into_iter().enumerate() {
                let echo = self.tone.run(side, self.line.read(side, self.time));
                self.line.write(side, *x + self.feedback * echo);
                *x += self.mix * echo;
            }
            self.line.advance();
        }
    }
}

/// Freeverb's lengths in samples at 44.1 kHz: the combs, the all-passes, and how much
/// longer each is on the right than the left.
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

/// One comb of the reverb: a delay fed back through a low-pass filter.
struct Comb {
    buffer: Vec<f32>,
    at: usize,
    filtered: f32,
}

impl Comb {
    fn run(&mut self, x: f32, feedback: f32, damping: f32) -> f32 {
        let out = self.buffer[self.at];
        self.filtered = out * (1.0 - damping) + self.filtered * damping;
        if self.filtered.abs() < 1e-20 {
            self.filtered = 0.0;
        }
        self.buffer[self.at] = x + self.filtered * feedback;
        self.at = (self.at + 1) % self.buffer.len();
        out
    }
}

struct AllPass {
    buffer: Vec<f32>,
    at: usize,
}

impl AllPass {
    fn run(&mut self, x: f32) -> f32 {
        let delayed = self.buffer[self.at];
        let out = delayed - x;
        self.buffer[self.at] = x + delayed * 0.5;
        self.at = (self.at + 1) % self.buffer.len();
        out
    }
}

/// Jezar's Freeverb: eight combs in parallel and four all-passes after them, for each side.
struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<AllPass>; 2],
    feedback: f32,
    damping: f32,
    mix: f32,
}

impl Reverb {
    fn new(rate: f32) -> Reverb {
        let scale = |length: usize| ((length as f32 * rate / 44_100.0) as usize).max(1);
        let side = |spread: usize| {
            let combs = (COMBS.iter())
                .map(|&length| Comb {
                    buffer: vec![0.0; scale(length + spread)],
                    at: 0,
                    filtered: 0.0,
                })
                .collect();
            let allpasses = (ALLPASSES.iter())
                .map(|&length| AllPass {
                    buffer: vec![0.0; scale(length + spread)],
                    at: 0,
                })
                .collect();
            (combs, allpasses)
        };
        let (left, right) = (side(0), side(SPREAD));
        Reverb {
            combs: [left.0, right.0],
            allpasses: [left.1, right.1],
            feedback: 0.0,
            damping: 0.0,
            mix: 0.0,
        }
    }

    /// Seconds a reverb's tail can be heard for, from its size.
    fn tail(pedal: &Pedal) -> f64 {
        1.0 + 7.0 * f64::from(pedal.knob(0))
    }

    fn set(&mut self, pedal: &Pedal) {
        self.feedback = 0.7 + 0.28 * pedal.knob(0);
        self.damping = 0.4 * pedal.knob(1);
        self.mix = pedal.knob(2);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        // Freeverb's own levels: what goes in is brought well down, as eight combs add it
        // up, and what comes out back up.
        const INPUT: f32 = 0.015;
        const WET: f32 = 3.0;
        for (l, r) in left.iter_mut().zip(right) {
            let input = (*l + *r) * INPUT;
            for (side, x) in [l, r].into_iter().enumerate() {
                let mut wet: f32 = (self.combs[side].iter_mut())
                    .map(|comb| comb.run(input, self.feedback, self.damping))
                    .sum();
                for allpass in &mut self.allpasses[side] {
                    wet = allpass.run(wet);
                }
                *x = *x * (1.0 - 0.5 * self.mix) + wet * WET * self.mix;
            }
        }
    }
}

/// A pedal at work: its settings and what it is carrying from block to block.
enum Unit {
    Compressor(Compressor),
    Wah(Wah),
    Drive(Drive),
    Amp(Amp),
    Cabinet(Cabinet),
    Eq(Eq),
    Chorus(Chorus),
    Delay(Delay),
    Reverb(Box<Reverb>),
}

impl Unit {
    fn new(kind: PedalKind, rate: f32) -> Unit {
        match kind {
            PedalKind::Compressor => Unit::Compressor(Compressor {
                threshold: 1.0,
                ratio: 1.0,
                makeup: 1.0,
                envelope: Envelope::default(),
            }),
            PedalKind::Wah => Unit::Wah(Wah {
                pedal: 0.5,
                resonance: 1.0,
                auto: 0.0,
                mix: 1.0,
                envelope: Envelope::default(),
                state: [[0.0; 2]; 2],
                rate,
            }),
            PedalKind::Overdrive => Unit::Drive(Drive::new(false, rate)),
            PedalKind::Distortion => Unit::Drive(Drive::new(true, rate)),
            PedalKind::Amp => Unit::Amp(Amp::new(rate)),
            PedalKind::Cabinet => Unit::Cabinet(Cabinet {
                gain: 1.0,
                filters: Filters::default(),
            }),
            PedalKind::Eq => Unit::Eq(Eq {
                gain: 1.0,
                filters: Filters::default(),
            }),
            PedalKind::Chorus => Unit::Chorus(Chorus::new(rate)),
            PedalKind::Delay => Unit::Delay(Delay::new(rate)),
            PedalKind::Reverb => Unit::Reverb(Box::new(Reverb::new(rate))),
        }
    }

    fn set(&mut self, pedal: &Pedal, rate: f32) {
        match self {
            Unit::Compressor(unit) => unit.set(pedal, rate),
            Unit::Wah(unit) => unit.set(pedal, rate),
            Unit::Drive(unit) => unit.set(pedal, rate),
            Unit::Amp(unit) => unit.set(pedal, rate),
            Unit::Cabinet(unit) => unit.set(pedal, rate),
            Unit::Eq(unit) => unit.set(pedal, rate),
            Unit::Chorus(unit) => unit.set(pedal),
            Unit::Delay(unit) => unit.set(pedal, rate),
            Unit::Reverb(unit) => unit.set(pedal),
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        match self {
            Unit::Compressor(unit) => unit.process(left, right),
            Unit::Wah(unit) => unit.process(left, right),
            Unit::Drive(unit) => unit.process(left, right),
            Unit::Amp(unit) => unit.process(left, right),
            Unit::Cabinet(unit) => unit.process(left, right),
            Unit::Eq(unit) => unit.process(left, right),
            Unit::Chorus(unit) => unit.process(left, right),
            Unit::Delay(unit) => unit.process(left, right),
            Unit::Reverb(unit) => unit.process(left, right),
        }
    }
}

/// A chain of pedals at work.
#[derive(Default)]
pub(crate) struct Chain {
    pedals: Vec<(Pedal, Unit)>,
}

impl Chain {
    /// Sets the chain to `pedals`. A pedal of the same kind in the same place as before
    /// keeps what it was carrying, so that turning a knob while the music plays does not
    /// cut off an echo or the tail of the reverb.
    pub(crate) fn set(&mut self, pedals: &[Pedal], rate: f32) {
        let mut old = std::mem::take(&mut self.pedals).into_iter();
        for pedal in pedals {
            let mut unit = match old.next() {
                Some((was, unit)) if was.kind == pedal.kind => unit,
                _ => Unit::new(pedal.kind, rate),
            };
            unit.set(pedal, rate);
            self.pedals.push((pedal.clone(), unit));
        }
    }

    /// Runs a block of samples through every pedal that is switched on.
    pub(crate) fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (pedal, unit) in &mut self.pedals {
            if pedal.on {
                unit.process(left, right);
            }
        }
    }
}

/// Seconds a chain can still be heard for after the last note has stopped: the echoes of
/// a delay until they are 60 dB down, or the tail of a reverb.
pub(crate) fn tail(pedals: &[Pedal]) -> f64 {
    let mut longest: f64 = 0.0;
    for pedal in pedals.iter().filter(|p| p.on) {
        let tail = match pedal.kind {
            PedalKind::Delay => {
                let feedback = f64::from(0.95 * pedal.knob(1)).max(0.01);
                let repeats = (0.001f64.ln() / feedback.ln()).ceil().max(1.0);
                f64::from(Delay::seconds(pedal)) * (repeats + 1.0)
            }
            PedalKind::Reverb => Reverb::tail(pedal),
            _ => 0.0,
        };
        // What follows a delay or a reverb rings on with it.
        longest = longest.max(tail).min(10.0);
    }
    longest
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    fn sine(frequency: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * RATE) as usize)
            .map(|i| amplitude * (TAU * frequency * i as f32 / RATE).sin())
            .collect()
    }

    /// A note with every overtone, as a plucked string has.
    fn saw(frequency: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * RATE) as usize)
            .map(|i| amplitude * (2.0 * (frequency * i as f32 / RATE).fract() - 1.0))
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    /// Runs `input` on both sides through one pedal, in blocks of 128 as the synth does.
    fn through(pedal: Pedal, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut chain = Chain::default();
        chain.set(&[pedal], RATE);
        let (mut left, mut right) = (input.to_vec(), input.to_vec());
        for (l, r) in left.chunks_mut(128).zip(right.chunks_mut(128)) {
            chain.process(l, r);
        }
        (left, right)
    }

    #[test]
    fn every_pedal_at_its_defaults_keeps_a_note_near_its_level_and_stays_finite() {
        let note = saw(220.0, 1.0, 0.2);
        for kind in PedalKind::ALL {
            let (left, right) = through(Pedal::new(kind), &note);
            assert!(left.iter().chain(&right).all(|s| s.is_finite()), "{kind:?}");
            // A pedal changes the sound, but does not take it away or blow it up.
            let ratio = rms(&left[4_410..]) / rms(&note[4_410..]);
            assert!((0.3..3.0).contains(&ratio), "{kind:?} {ratio}");
        }
    }

    #[test]
    fn a_pedal_that_is_off_leaves_the_sound_alone() {
        let note = sine(330.0, 0.2, 0.2);
        for kind in PedalKind::ALL {
            let mut pedal = Pedal::new(kind);
            pedal.on = false;
            assert_eq!(through(pedal, &note).0, note);
        }
    }

    #[test]
    fn the_compressor_brings_loud_and_quiet_together() {
        let compressed = |amplitude| {
            let note = sine(220.0, 0.5, amplitude);
            rms(&through(Pedal::with(PedalKind::Compressor, &[30, 80]), &note).0[11_025..])
        };
        let (quiet, loud) = (compressed(0.05), compressed(0.4));
        // Eight times as loud going in is not much more than twice coming out.
        assert!(loud / quiet < 2.5, "{}", loud / quiet);
    }

    #[test]
    fn drive_adds_overtones_and_more_drive_adds_more() {
        // The share of a note's energy that is not at its own frequency.
        let overtones = |pedal| {
            let note = sine(200.0, 0.5, 0.2);
            let (out, _) = through(pedal, &note);
            let out = &out[4_410..];
            let (mut sin, mut cos) = (0.0, 0.0);
            for (i, s) in out.iter().enumerate() {
                let phase = TAU * 200.0 * (i + 4_410) as f32 / RATE;
                sin += s * phase.sin();
                cos += s * phase.cos();
            }
            let n = out.len() as f32;
            // The fundamental's amplitude, and its power against the whole note's.
            let amplitude = 2.0 * (sin * sin + cos * cos).sqrt() / n;
            let whole = out.iter().map(|s| s * s).sum::<f32>() / n;
            1.0 - amplitude * amplitude / 2.0 / whole
        };
        let light = overtones(Pedal::with(PedalKind::Overdrive, &[10, 100]));
        let heavy = overtones(Pedal::with(PedalKind::Overdrive, &[90, 100]));
        let distortion = overtones(Pedal::with(PedalKind::Distortion, &[90, 100]));
        assert!(
            light < heavy && heavy < distortion,
            "{light} {heavy} {distortion}"
        );
        // A square wave, as far as clipping can go, has a fifth of its energy in overtones.
        assert!(distortion > 0.08);
        let amp = |gain| overtones(Pedal::with(PedalKind::Amp, &[gain]));
        assert!(amp(10) < amp(90));
    }

    #[test]
    fn the_wah_moves_its_peak_with_the_pedal() {
        // A low note comes through heel down more than toe down, and a high one the other
        // way round.
        let level = |frequency, position| {
            let note = sine(frequency, 0.3, 0.2);
            rms(&through(Pedal::with(PedalKind::Wah, &[position]), &note).0[4_410..])
        };
        assert!(level(400.0, 0) > level(400.0, 100) * 2.0);
        assert!(level(2_000.0, 100) > level(2_000.0, 0) * 2.0);
    }

    #[test]
    fn the_eq_and_the_cabinet_shape_the_sound() {
        let at = |pedal: &Pedal, frequency| {
            let note = sine(frequency, 0.3, 0.1);
            rms(&through(pedal.clone(), &note).0[4_410..]) / rms(&note[4_410..])
        };
        let bass_up = Pedal::with(PedalKind::Eq, &[100, 50, 0]);
        assert!(at(&bass_up, 60.0) > 3.0 && at(&bass_up, 8_000.0) < 0.3);
        // A speaker gives nothing like the fizz above its range.
        let cabinet = Pedal::new(PedalKind::Cabinet);
        assert!(at(&cabinet, 12_000.0) < 0.05 && at(&cabinet, 1_000.0) > 0.2);
    }

    #[test]
    fn echoes_and_reverb_ring_on_after_the_note() {
        // A short burst, then silence.
        let mut burst = sine(440.0, 0.05, 0.3);
        burst.resize((RATE * 2.0) as usize, 0.0);
        let delay = Pedal::with(PedalKind::Delay, &[20, 50, 100, 50]);
        let time = Delay::seconds(&delay);
        let (out, _) = through(delay.clone(), &burst);
        let window = |from: f32| {
            let start = (from * RATE) as usize;
            rms(&out[start..start + 2_000])
        };
        // An echo at the delay's time and another at twice it, each quieter.
        assert!(window(time) > 0.05 && window(2.0 * time) > 0.02);
        assert!(window(2.0 * time) < window(time));
        assert!(window(time * 0.5 + 0.03) < 0.001);

        let (out, _) = through(Pedal::with(PedalKind::Reverb, &[80, 30, 60]), &burst);
        assert!(rms(&out[(0.5 * RATE) as usize..(0.6 * RATE) as usize]) > 0.002);
        // The tail the engine waits for covers what can be heard.
        assert!(tail(&[delay]) > f64::from(time) * 3.0);
        assert!(tail(&[Pedal::with(PedalKind::Reverb, &[100])]) >= 7.0);
        assert_eq!(tail(&[Pedal::new(PedalKind::Eq)]), 0.0);
    }

    #[test]
    fn the_chorus_spreads_the_sides() {
        let note = sine(330.0, 0.5, 0.2);
        let (left, right) = through(Pedal::with(PedalKind::Chorus, &[50, 100, 100]), &note);
        let apart = rms(&(left.iter().zip(&right))
            .map(|(l, r)| l - r)
            .collect::<Vec<_>>());
        assert!(apart > 0.02, "{apart}");
    }

    #[test]
    fn turning_a_knob_keeps_the_echoes_already_made() {
        let mut burst = sine(440.0, 0.05, 0.3);
        burst.resize((RATE * 0.6) as usize, 0.0);
        let mut chain = Chain::default();
        let delay = Pedal::with(PedalKind::Delay, &[20, 60, 100, 60]);
        chain.set(std::slice::from_ref(&delay), RATE);
        let (mut left, mut right) = (burst.clone(), burst.clone());
        let half = left.len() / 4;
        chain.process(&mut left[..half], &mut right[..half]);
        // More feedback: the echoes go on, where a new pedal would have been silent.
        chain.set(&[Pedal::with(PedalKind::Delay, &[20, 80, 100, 60])], RATE);
        chain.process(&mut left[half..], &mut right[half..]);
        assert!(rms(&left[half..]) > 0.01);
        // Another kind in its place starts afresh.
        chain.set(&[Pedal::new(PedalKind::Eq)], RATE);
        let (mut l, mut r) = (vec![0.0; 1_000], vec![0.0; 1_000]);
        chain.process(&mut l, &mut r);
        assert_eq!(rms(&l), 0.0);
    }
}
