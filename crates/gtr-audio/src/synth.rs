use std::f32::consts::TAU;

use crate::{NoteEvent, Timeline, Tone};

const NOTE_GAIN: f32 = 0.22;
const CLICK_GAIN: f32 = 0.35;
/// Seconds a string keeps ringing after its note ends.
const RELEASE: f32 = 0.06;
/// Seconds of silence appended so the last notes can die away.
const TAIL: f64 = 0.3;
/// Samples between updates of a bending note's pitch.
const CONTROL_PERIOD: usize = 32;
const VIBRATO_HZ: f32 = 5.5;
/// Seconds into a note before vibrato starts.
const VIBRATO_DELAY: f32 = 0.08;

/// Renders a timeline to mono samples in `-1.0..=1.0`.
pub fn render(timeline: &Timeline, sample_rate: u32) -> Vec<f32> {
    let rate = sample_rate as f32;
    let total = ((timeline.duration + TAIL) * f64::from(sample_rate)).ceil() as usize;
    let mut out = vec![0.0f32; total];

    for (index, note) in timeline.notes.iter().enumerate() {
        let start = (note.time * f64::from(sample_rate)) as usize;
        let length = ((note.duration as f32 + RELEASE) * rate) as usize;
        let end = (start + length).min(total);
        if start < end {
            pluck(&mut out[start..end], note, rate, index as u32);
        }
    }
    for click in &timeline.clicks {
        let start = (click.time * f64::from(sample_rate)) as usize;
        let frequency = if click.accent { 1760.0 } else { 1320.0 };
        for (i, sample) in out
            .iter_mut()
            .skip(start)
            .take((0.04 * rate) as usize)
            .enumerate()
        {
            let t = i as f32 / rate;
            *sample += CLICK_GAIN * (TAU * frequency * t).sin() * (-t * 120.0).exp();
        }
    }
    for sample in &mut out {
        *sample = sample.tanh();
    }
    out
}

/// Pitch offset in semitones at `t` seconds into a note, from its bend or slide curve.
fn curve_at(curve: &[(f32, f32)], t: f32) -> f32 {
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

/// Adds one plucked note to `out` using the Karplus-Strong string model: a burst of noise
/// circulating in a delay line one period long, losing its high frequencies on each pass.
/// Changing the length of the delay while the note sounds bends its pitch.
fn pluck(out: &mut [f32], note: &NoteEvent, rate: f32, seed: u32) {
    let frequency = 440.0 * 2f32.powf((f32::from(note.pitch) - 69.0) / 12.0);
    let base_period = rate / frequency;
    let steady = note.pitch_curve.is_empty() && note.vibrato == 0.0;

    // Room for the longest period the note can reach when bent or slid downwards.
    let lowest = (note.pitch_curve.iter())
        .map(|&(_, semitones)| semitones)
        .fold(0.0f32, f32::min)
        - note.vibrato;
    let size = (base_period * 2f32.powf(-lowest / 12.0)).ceil() as usize + 4;

    // The excitation: bright noise for a picked note, duller for a hammered or muted one,
    // and a single sine cycle for the bell-like tone of a harmonic.
    let smoothing = match (note.tone, note.soft) {
        (Tone::PalmMute, _) => 4,
        (_, true) => 3,
        _ => 1,
    };
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1;
    let mut line: Vec<f32> = (0..size)
        .map(|i| {
            if note.tone == Tone::Harmonic {
                return (TAU * i as f32 / base_period).sin();
            }
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 23) as f32 - 1.0
        })
        .collect();
    for _ in 0..smoothing {
        let mut previous = 0.0;
        for sample in &mut line {
            previous = 0.5 * (*sample + previous);
            *sample = previous;
        }
    }

    // How much of the string's energy survives each trip round the loop. Lower strings
    // ring longer; muting damps the string hard.
    let decay = match note.tone {
        Tone::Dead => 0.80,
        Tone::PalmMute => 0.955 - 0.01 * (frequency / 400.0).min(1.0),
        Tone::Harmonic => 0.9992,
        Tone::Normal => 0.9985 - 0.004 * (frequency / 1500.0).min(1.0),
    };
    let gain = NOTE_GAIN * note.velocity * if note.soft { 0.8 } else { 1.0 };

    let release_start = out.len().saturating_sub((RELEASE * rate) as usize);
    let release_length = (out.len() - release_start).max(1) as f32;
    // The averaging filter itself delays by half a sample.
    let mut delay = base_period - 0.5;
    let mut filtered = 0.0;
    let mut write = 0;
    for (i, sample) in out.iter_mut().enumerate() {
        if !steady && i % CONTROL_PERIOD == 0 {
            let t = i as f32 / rate;
            let mut semitones = curve_at(&note.pitch_curve, t);
            if note.vibrato > 0.0 && t > VIBRATO_DELAY {
                let swell = ((t - VIBRATO_DELAY) * 6.0).min(1.0);
                semitones += note.vibrato * swell * (TAU * VIBRATO_HZ * t).sin();
            }
            delay = base_period * 2f32.powf(-semitones / 12.0) - 0.5;
        }

        // Read the line `delay` samples back, between two stored samples.
        let position = (write as f32 - delay).rem_euclid(size as f32);
        let index = position as usize % size;
        let fraction = position - position.floor();
        let current = line[index] + (line[(index + 1) % size] - line[index]) * fraction;

        let envelope = match i.checked_sub(release_start) {
            Some(into) => 1.0 - into as f32 / release_length,
            None => 1.0,
        };
        *sample += gain * envelope * current;

        line[write] = decay * 0.5 * (current + filtered);
        filtered = current;
        write = (write + 1) % size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_one(note: NoteEvent) -> Vec<f32> {
        let timeline = Timeline {
            duration: note.duration,
            notes: vec![note],
            clicks: Vec::new(),
            beats: Vec::new(),
        };
        render(&timeline, 44_100)
    }

    /// Estimates the fundamental of `window` from its autocorrelation peak near `expected`.
    fn measured_frequency(window: &[f32], expected: f32) -> f32 {
        let span = window.len() / 2;
        let correlation = |lag: usize| -> f32 {
            (window[..span].iter().zip(&window[lag..]))
                .map(|(a, b)| a * b)
                .sum()
        };
        let guess = 44_100.0 / expected;
        let lags = (guess * 0.9) as usize..=(guess * 1.1) as usize + 1;
        let best = lags
            .max_by(|&a, &b| correlation(a).total_cmp(&correlation(b)))
            .unwrap();
        let (l, c, r) = (
            correlation(best - 1),
            correlation(best),
            correlation(best + 1),
        );
        let shift = 0.5 * (l - r) / (l - 2.0 * c + r);
        44_100.0 / (best as f32 + shift)
    }

    fn cents(actual: f32, expected: f32) -> f32 {
        1200.0 * (actual / expected).log2()
    }

    #[test]
    fn notes_are_in_tune() {
        // Low E, A440 and the 24th fret of the high E string.
        for (pitch, expected) in [(40, 82.41), (69, 440.0), (88, 1318.5)] {
            let samples = render_one(NoteEvent::plain(0.0, 1.0, pitch));
            let off = cents(
                measured_frequency(&samples[2_000..14_000], expected),
                expected,
            );
            assert!(off.abs() < 6.0, "pitch {pitch}: {off} cents off");
        }
    }

    #[test]
    fn a_bend_raises_the_pitch() {
        let mut note = NoteEvent::plain(0.0, 1.0, 64);
        note.pitch_curve = vec![(0.0, 0.0), (0.2, 2.0)];
        let samples = render_one(note);
        // E4 at the start, bent up a whole tone to F#4 once the bend has landed.
        let early = cents(measured_frequency(&samples[..1_200], 329.63), 329.63);
        let late = cents(measured_frequency(&samples[20_000..30_000], 369.99), 369.99);
        assert!(early.abs() < 60.0, "start is {early} cents from E");
        assert!(late.abs() < 8.0, "bent note is {late} cents from F#");
    }

    #[test]
    fn harmonics_are_in_tune_and_muted_notes_die_quickly() {
        let mut harmonic = NoteEvent::plain(0.0, 1.0, 64);
        harmonic.tone = Tone::Harmonic;
        let samples = render_one(harmonic);
        let off = cents(measured_frequency(&samples[2_000..12_000], 329.63), 329.63);
        assert!(off.abs() < 6.0, "harmonic is {off} cents off");

        let energy = |samples: &[f32]| samples[8_000..12_000].iter().map(|s| s * s).sum::<f32>();
        let open = energy(&render_one(NoteEvent::plain(0.0, 1.0, 45)));
        let mut muted = NoteEvent::plain(0.0, 1.0, 45);
        muted.tone = Tone::PalmMute;
        let palm = energy(&render_one(muted.clone()));
        muted.tone = Tone::Dead;
        let dead = energy(&render_one(muted));
        assert!(palm < open * 0.2, "palm mute {palm} vs open {open}");
        assert!(dead < palm * 0.01, "dead {dead} vs palm mute {palm}");
    }

    #[test]
    fn output_is_bounded_and_ends_quietly() {
        let mut loud = NoteEvent::plain(0.0, 1.0, 40);
        loud.velocity = 1.7;
        loud.vibrato = 0.5;
        let samples = render_one(loud);
        assert_eq!(samples.len(), (1.3f64 * 44_100.0).ceil() as usize);
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        assert!(samples.iter().any(|s| s.abs() > 0.02));
        assert!(samples[samples.len() - 4_000..].iter().all(|&s| s == 0.0));
    }
}
