//! The sounds themselves: each voice is one note, hit or click, and adds its samples to a
//! buffer a block at a time, carrying on where the block before left off.

use std::f32::consts::TAU;

use crate::soundfont::SampleVoice;
use crate::{NoteEvent, Timbre, Tone};

const NOTE_GAIN: f32 = 0.22;
const CLICK_GAIN: f32 = 0.35;
/// Seconds a string keeps ringing after its note ends.
const RELEASE: f32 = 0.06;
/// Seconds a metronome click lasts.
const CLICK_LENGTH: f32 = 0.04;
/// Samples between updates of a bending note's pitch.
const CONTROL_PERIOD: usize = 32;
const VIBRATO_HZ: f32 = 5.5;
/// Seconds into a note before vibrato starts.
const VIBRATO_DELAY: f32 = 0.08;

/// One sounding note of a track.
pub(crate) enum Voice {
    String(StringVoice),
    Drum(DrumHit),
    /// A sample from a SoundFont, which is the only kind of voice with two sides. It
    /// carries all its bank says about it, and is far the largest of the three.
    Sampled(Box<SampleVoice>),
}

impl Voice {
    /// The modelled voice for a note on a track of the given timbre. `limit` is the most samples it
    /// may sound for, and `seed` picks its noise. Returns nothing for a note with no length.
    pub(crate) fn new(
        note: &NoteEvent,
        timbre: Timbre,
        rate: f32,
        limit: usize,
        seed: u32,
    ) -> Option<Voice> {
        // A drum rings for as long as the drum does, however long the note is written.
        let seconds = match timbre {
            Timbre::Drums => drum_voice(note.pitch).ring,
            _ => note.duration as f32 + RELEASE,
        };
        let length = ((seconds * rate) as usize).min(limit);
        (length > 0).then(|| match timbre {
            Timbre::Drums => Voice::Drum(DrumHit::new(note, rate, length, seed)),
            _ => Voice::String(StringVoice::new(note, timbre, rate, length, seed)),
        })
    }

    /// Adds the voice's next samples to `mono`, or if it has two sides to `left` and
    /// `right`. Returns false once it has nothing left.
    pub(crate) fn render(&mut self, mono: &mut [f32], left: &mut [f32], right: &mut [f32]) -> bool {
        match self {
            Voice::String(voice) => voice.render(mono),
            Voice::Drum(hit) => hit.render(mono),
            Voice::Sampled(voice) => voice.render(left, right),
        }
    }
}

/// Runs a block of a track through its amplifier: overdriven sounds are clipped, which is
/// what makes chords growl, and then filtered the way a speaker cabinet would. `filtered`
/// carries the filter from one block to the next.
pub(crate) fn shape(bus: &mut [f32], timbre: Timbre, rate: f32, filtered: &mut f32) {
    let (drive, level) = match timbre {
        Timbre::Overdrive => (5.0, 0.42),
        Timbre::Distortion => (16.0, 0.3),
        _ => return,
    };
    let smoothing = 1.0 - (-TAU * 3_800.0 / rate).exp();
    for sample in bus {
        *filtered += smoothing * ((*sample * drive).tanh() * level - *filtered);
        *sample = *filtered;
    }
}

/// Evens out a block of a driven track that is played from samples, as the amplifier the
/// samples were recorded through would have. A bank has each note recorded alone, and added
/// up a chord of six is several times as loud as one of them; through an amplifier it is
/// hardly louder at all, and a muted note is not far under an open one. `level` carries how
/// loud the track has been from one block to the next.
pub(crate) fn squeeze(
    left: &mut [f32],
    right: &mut [f32],
    timbre: Timbre,
    rate: f32,
    level: &mut f32,
) {
    // The level above which the track is held back, by how much, and what it is then
    // brought up by, so that a single note is as loud as it was.
    let (threshold, ratio, makeup) = match timbre {
        Timbre::Overdrive => (0.2, 4.0, 1.0),
        Timbre::Distortion => (0.12, 8.0, 1.5),
        _ => return,
    };
    let attack = 1.0 - (-1.0 / (0.0005 * rate)).exp();
    let release = 1.0 - (-1.0 / (0.12 * rate)).exp();
    for (left, right) in left.iter_mut().zip(right) {
        let peak = left.abs().max(right.abs());
        let glide = if peak > *level { attack } else { release };
        *level += glide * (peak - *level);
        if *level < 1e-9 {
            *level = 0.0;
        }
        let gain = makeup * (threshold / level.max(threshold)).powf(1.0 - 1.0 / ratio);
        *left *= gain;
        *right *= gain;
    }
}

/// Pitch offset in semitones at `t` seconds into a note, from its bend or slide curve.
pub(crate) fn curve_at(curve: &[(f32, f32)], t: f32) -> f32 {
    let Some(&(first_time, first_value)) = curve.first() else {
        return 0.0;
    };
    if t <= first_time {
        return first_value;
    }
    for pair in curve.windows(2) {
        let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
        if t < t1 {
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0).max(1e-6);
        }
    }
    curve[curve.len() - 1].1
}

/// A source of white noise in `-1.0..1.0` that gives the same samples for the same seed.
struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Noise {
        Noise(seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1)
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

/// A metronome click: a short, high beep.
pub(crate) struct ClickVoice {
    frequency: f32,
    rate: f32,
    at: usize,
}

impl ClickVoice {
    pub(crate) fn new(accent: bool, rate: f32) -> ClickVoice {
        ClickVoice {
            frequency: if accent { 1760.0 } else { 1320.0 },
            rate,
            at: 0,
        }
    }

    /// Adds the click's next samples to `out`. Returns false once it is over.
    pub(crate) fn render(&mut self, out: &mut [f32]) -> bool {
        let length = (CLICK_LENGTH * self.rate) as usize;
        for sample in out.iter_mut().take(length.saturating_sub(self.at)) {
            let t = self.at as f32 / self.rate;
            *sample += CLICK_GAIN * (TAU * self.frequency * t).sin() * (-t * 120.0).exp();
            self.at += 1;
        }
        self.at < length
    }
}

/// How a piece of the drum kit is made up: a pitched body and a burst of noise.
struct DrumVoice {
    /// Seconds until the piece has died away.
    ring: f32,
    /// Pitch of the body in Hz where it ends up, and how many times higher it starts.
    tone: f32,
    sweep: f32,
    /// Loudness of the body and of the noise.
    body: f32,
    noise: f32,
    /// How much of the noise's low end is kept: 1.0 for a full hiss, 0.0 for only the
    /// sizzle of a cymbal.
    dark: f32,
}

/// The make-up of a piece of the kit, by its General MIDI percussion number.
fn drum_voice(piece: u8) -> DrumVoice {
    let voice = |ring, tone, sweep, body, noise, dark| DrumVoice {
        ring,
        tone,
        sweep,
        body,
        noise,
        dark,
    };
    match piece {
        35 | 36 => voice(0.3, 48.0, 3.0, 1.6, 0.1, 1.0),
        37 => voice(0.06, 800.0, 1.0, 0.5, 0.5, 0.3),
        38 | 40 => voice(0.22, 190.0, 1.4, 0.7, 1.0, 0.5),
        39 => voice(0.12, 1_000.0, 1.0, 0.0, 1.0, 0.3),
        41 => voice(0.4, 80.0, 1.6, 1.3, 0.1, 1.0),
        43 => voice(0.4, 95.0, 1.6, 1.3, 0.1, 1.0),
        45 => voice(0.35, 115.0, 1.6, 1.3, 0.1, 1.0),
        47 => voice(0.35, 140.0, 1.6, 1.2, 0.1, 1.0),
        48 => voice(0.3, 165.0, 1.6, 1.2, 0.1, 1.0),
        50 => voice(0.3, 195.0, 1.6, 1.2, 0.1, 1.0),
        42 => voice(0.06, 0.0, 1.0, 0.0, 0.7, 0.0),
        44 => voice(0.04, 0.0, 1.0, 0.0, 0.5, 0.0),
        46 => voice(0.45, 0.0, 1.0, 0.0, 0.7, 0.0),
        49 | 57 | 52 => voice(1.5, 0.0, 1.0, 0.0, 0.8, 0.05),
        55 => voice(0.7, 0.0, 1.0, 0.0, 0.7, 0.0),
        51 | 59 => voice(0.9, 3_400.0, 1.0, 0.12, 0.35, 0.0),
        53 => voice(0.7, 2_600.0, 1.0, 0.5, 0.1, 0.0),
        54 => voice(0.15, 0.0, 1.0, 0.0, 0.6, 0.0),
        56 => voice(0.2, 560.0, 1.0, 0.8, 0.0, 0.0),
        _ => voice(0.08, 400.0, 1.0, 0.4, 0.4, 0.3),
    }
}

/// One drum hit: a sine that drops in pitch for the body of a drum, and filtered noise for
/// snares and cymbals, both dying away over the piece's ring time.
pub(crate) struct DrumHit {
    voice: DrumVoice,
    gain: f32,
    rate: f32,
    noise: Noise,
    phase: f32,
    previous: f32,
    low: f32,
    at: usize,
    length: usize,
}

impl DrumHit {
    fn new(note: &NoteEvent, rate: f32, length: usize, seed: u32) -> DrumHit {
        DrumHit {
            voice: drum_voice(note.pitch),
            gain: NOTE_GAIN * 1.6 * note.velocity,
            rate,
            noise: Noise::new(seed),
            phase: 0.0,
            previous: 0.0,
            low: 0.0,
            at: 0,
            length,
        }
    }

    fn render(&mut self, out: &mut [f32]) -> bool {
        let voice = &self.voice;
        for sample in out.iter_mut().take(self.length - self.at) {
            let t = self.at as f32 / self.rate;
            let fade = (-5.0 * t / voice.ring).exp();
            // The body starts sharp and settles within a few hundredths of a second.
            let frequency = voice.tone * (1.0 + (voice.sweep - 1.0) * (-t * 40.0).exp());
            self.phase += TAU * frequency / self.rate;
            let white = self.noise.next();
            // The difference between successive samples keeps only the top of the noise.
            let bright = white - self.previous;
            self.previous = white;
            self.low += 0.25 * (white - self.low);
            let hiss = bright * (1.0 - voice.dark) * 0.6 + self.low * voice.dark;
            *sample += self.gain * fade * (voice.body * self.phase.sin() + voice.noise * hiss);
            self.at += 1;
        }
        self.at < self.length
    }
}

/// One plucked note, using the Karplus-Strong string model: a burst of noise circulating in
/// a delay line one period long, losing its high frequencies on each pass. Changing the
/// length of the delay while the note sounds bends its pitch.
pub(crate) struct StringVoice {
    line: Vec<f32>,
    write: usize,
    filtered: f32,
    /// Samples back along the line that the string is read from.
    delay: f32,
    base_period: f32,
    /// How much of the string's energy survives each trip round the loop.
    decay: f32,
    gain: f32,
    rate: f32,
    pitch_curve: Vec<(f32, f32)>,
    vibrato: f32,
    at: usize,
    length: usize,
    release_start: usize,
}

impl StringVoice {
    fn new(note: &NoteEvent, timbre: Timbre, rate: f32, length: usize, seed: u32) -> StringVoice {
        let frequency = 440.0 * 2f32.powf((f32::from(note.pitch) - 69.0) / 12.0);
        let base_period = rate / frequency;

        // Room for the longest period the note can reach when bent or slid downwards.
        let lowest = (note.pitch_curve.iter())
            .map(|&(_, semitones)| semitones)
            .fold(0.0f32, f32::min)
            - note.vibrato;
        let size = (base_period * 2f32.powf(-lowest / 12.0)).ceil() as usize + 4;

        // The excitation: bright noise for a picked note, duller for a hammered or muted
        // one, and a single sine cycle for the bell-like tone of a harmonic.
        let smoothing = match (note.tone, note.soft) {
            (Tone::PalmMute, _) => 4,
            (_, true) => 3,
            _ => 1,
        } + match timbre {
            Timbre::Steel | Timbre::Drums | Timbre::Banjo | Timbre::Mandolin => 0,
            Timbre::Clean | Timbre::Overdrive | Timbre::Distortion => 1,
            Timbre::Nylon | Timbre::Bass | Timbre::Piano | Timbre::Ukulele => 2,
        };
        let mut noise = Noise::new(seed);
        let mut line: Vec<f32> = (0..size)
            .map(|i| match note.tone {
                Tone::Harmonic => (TAU * i as f32 / base_period).sin(),
                _ => noise.next(),
            })
            .collect();
        for _ in 0..smoothing {
            let mut previous = 0.0;
            for sample in &mut line {
                previous = 0.5 * (*sample + previous);
                *sample = previous;
            }
        }

        // What goes round the string is averaged at every turn, which wears away
        // everything but a steady offset: left in, that outlasts the note itself, and on
        // a short string, which holds few samples to average out, is most of what sounds.
        let offset = line.iter().sum::<f32>() / line.len() as f32;
        line.iter_mut().for_each(|sample| *sample -= offset);

        // Lower strings ring longer; muting damps the string hard.
        let decay = match note.tone {
            Tone::Dead => 0.80,
            Tone::PalmMute => 0.955 - 0.01 * (frequency / 400.0).min(1.0),
            Tone::Harmonic => 0.9992,
            Tone::Normal => match timbre {
                Timbre::Nylon => 0.997 - 0.006 * (frequency / 1500.0).min(1.0),
                // Short strings on small bodies, and a banjo's skin, give the note up fast.
                Timbre::Ukulele => 0.9945 - 0.008 * (frequency / 1500.0).min(1.0),
                Timbre::Mandolin => 0.996 - 0.007 * (frequency / 1500.0).min(1.0),
                Timbre::Banjo => 0.992 - 0.01 * (frequency / 1500.0).min(1.0),
                // An amplifier's compression holds a driven note up.
                Timbre::Overdrive | Timbre::Distortion | Timbre::Bass | Timbre::Piano => {
                    0.9992 - 0.002 * (frequency / 1500.0).min(1.0)
                }
                Timbre::Steel | Timbre::Clean | Timbre::Drums => {
                    0.9985 - 0.004 * (frequency / 1500.0).min(1.0)
                }
            },
        };

        StringVoice {
            line,
            write: 0,
            filtered: 0.0,
            // The averaging filter itself delays by half a sample.
            delay: base_period - 0.5,
            base_period,
            decay,
            gain: NOTE_GAIN * note.velocity * if note.soft { 0.8 } else { 1.0 },
            rate,
            pitch_curve: note.pitch_curve.clone(),
            vibrato: note.vibrato,
            at: 0,
            length,
            release_start: length.saturating_sub((RELEASE * rate) as usize),
        }
    }

    fn render(&mut self, out: &mut [f32]) -> bool {
        let steady = self.pitch_curve.is_empty() && self.vibrato == 0.0;
        let size = self.line.len();
        let release_length = (self.length - self.release_start).max(1) as f32;
        for sample in out.iter_mut().take(self.length - self.at) {
            if !steady && self.at.is_multiple_of(CONTROL_PERIOD) {
                let t = self.at as f32 / self.rate;
                let mut semitones = curve_at(&self.pitch_curve, t);
                if self.vibrato > 0.0 && t > VIBRATO_DELAY {
                    let swell = ((t - VIBRATO_DELAY) * 6.0).min(1.0);
                    semitones += self.vibrato * swell * (TAU * VIBRATO_HZ * t).sin();
                }
                self.delay = self.base_period * 2f32.powf(-semitones / 12.0) - 0.5;
            }

            // Read the line `delay` samples back, between two stored samples.
            let position = (self.write as f32 - self.delay).rem_euclid(size as f32);
            let index = position as usize % size;
            let fraction = position - position.floor();
            let current =
                self.line[index] + (self.line[(index + 1) % size] - self.line[index]) * fraction;

            let envelope = match self.at.checked_sub(self.release_start) {
                Some(into) => 1.0 - into as f32 / release_length,
                None => 1.0,
            };
            *sample += self.gain * envelope * current;

            self.line[self.write] = self.decay * 0.5 * (current + self.filtered);
            self.filtered = current;
            self.write = (self.write + 1) % size;
            self.at += 1;
        }
        self.at < self.length
    }
}
