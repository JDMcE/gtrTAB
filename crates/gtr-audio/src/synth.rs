use std::f32::consts::TAU;

use crate::Timeline;

const NOTE_GAIN: f32 = 0.22;
const CLICK_GAIN: f32 = 0.35;
/// Seconds a string keeps ringing after its note ends.
const RELEASE: f32 = 0.06;
/// Seconds of silence appended so the last notes can die away.
const TAIL: f64 = 0.3;

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
            pluck(&mut out[start..end], note.pitch, rate, index as u32);
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

/// Adds one plucked note to `out` using the Karplus-Strong string model: a burst of noise
/// circulating in a delay line one period long, losing its high frequencies on each pass.
fn pluck(out: &mut [f32], pitch: u8, rate: f32, seed: u32) {
    let frequency = 440.0 * 2f32.powf((f32::from(pitch) - 69.0) / 12.0);
    let period = rate / frequency;
    // The averaging filter delays by half a sample; an all-pass filter supplies the
    // remaining fraction so that high notes stay in tune.
    let whole = (period - 0.6).floor().max(2.0);
    let fraction = period - 0.5 - whole;
    let allpass = (1.0 - fraction) / (1.0 + fraction);

    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1;
    let mut previous = 0.0;
    let mut line: Vec<f32> = (0..whole as usize)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (state >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
            // Soften the pick attack.
            previous = 0.5 * (noise + previous);
            previous
        })
        .collect();

    // Lower strings ring longer.
    let decay = 0.9985 - 0.004 * (frequency / 1500.0).min(1.0);
    let release_start = out.len().saturating_sub((RELEASE * rate) as usize);
    let release_length = (out.len() - release_start).max(1) as f32;
    let (mut filtered, mut ap_in, mut ap_out) = (0.0, 0.0, 0.0);
    let mut position = 0;
    for (i, sample) in out.iter_mut().enumerate() {
        let current = line[position];
        let gain = match i.checked_sub(release_start) {
            Some(into) => 1.0 - into as f32 / release_length,
            None => 1.0,
        };
        *sample += NOTE_GAIN * gain * current;

        let averaged = decay * 0.5 * (current + filtered);
        filtered = current;
        ap_out = allpass * averaged + ap_in - allpass * ap_out;
        ap_in = averaged;
        line[position] = ap_out;
        position = (position + 1) % line.len();
    }
}

#[cfg(test)]
mod tests {
    use crate::{NoteEvent, Timeline};

    use super::*;

    fn single_note(pitch: u8) -> Vec<f32> {
        let timeline = Timeline {
            notes: vec![NoteEvent {
                time: 0.0,
                duration: 1.0,
                pitch,
            }],
            clicks: Vec::new(),
            beats: Vec::new(),
            duration: 1.0,
        };
        render(&timeline, 44_100)
    }

    /// Estimates the fundamental from the autocorrelation peak, with parabolic interpolation.
    fn measured_frequency(samples: &[f32], expected: f32) -> f32 {
        let window = &samples[2_000..12_000];
        let correlation = |lag: usize| -> f32 {
            (window.iter().zip(&window[lag..]))
                .map(|(a, b)| a * b)
                .sum()
        };
        let guess = 44_100.0 / expected;
        let lags = (guess * 0.9) as usize..=(guess * 1.1) as usize + 1;
        let best = (lags.clone())
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

    #[test]
    fn notes_are_in_tune() {
        // Low E, A440 and the 24th fret of the high E string.
        for (pitch, expected) in [(40, 82.41), (69, 440.0), (88, 1318.5)] {
            let actual = measured_frequency(&single_note(pitch), expected);
            let cents = 1200.0 * (actual / expected).log2();
            assert!(
                cents.abs() < 6.0,
                "pitch {pitch}: {actual} Hz, {cents} cents off"
            );
        }
    }

    #[test]
    fn output_is_bounded_and_ends_quietly() {
        let samples = single_note(40);
        assert_eq!(samples.len(), (1.3f64 * 44_100.0).ceil() as usize);
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        assert!(samples.iter().any(|s| s.abs() > 0.02));
        assert!(samples[samples.len() - 4_000..].iter().all(|&s| s == 0.0));
    }
}
