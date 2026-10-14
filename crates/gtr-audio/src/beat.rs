//! Listening for the beat of a recording, so that the score's bars can be fitted to it:
//! where the notes start (how much louder each band of the spectrum gets from one moment
//! to the next), how far apart the beats are (the spacing at which those starts repeat
//! most, near the tempo the score is written at), and then the beats themselves, the run
//! of starts that are strongest and most evenly spaced (Ellis, "Beat tracking by dynamic
//! programming", 2007).

use std::f32::consts::PI;

/// The rate the recording is brought down to before it is listened to: plenty for where
/// notes start, and a quarter of the work of 44.1 kHz.
const LISTEN_RATE: u32 = 11_025;
/// Samples looked at at once, and how far apart each look is.
const FRAME: usize = 512;
const HOP: usize = 128;
/// The slowest and fastest beats looked for, in beats a minute.
const SLOWEST: f64 = 30.0;
const FASTEST: f64 = 300.0;
/// How far, in octaves, the beat is likely to be from the one written: the tempo found is
/// the spacing that repeats most, weighed by this.
const OCTAVES: f64 = 0.6;
/// How much an uneven spacing between two beats costs against a strong start.
const TIGHTNESS: f32 = 100.0;

/// Where the beats fall in a recording of `samples` at `rate`, in seconds, from its first
/// to its last; `expected` is how many seconds apart they are as the score is written,
/// which decides between a beat and one twice or half as fast. Empty for a recording too
/// short, or too quiet, to have one.
pub fn beats(samples: &[f32], rate: u32, expected: f64) -> Vec<f64> {
    let (samples, rate) = brought_down(samples, rate);
    let onsets = onsets(&samples);
    let fps = f64::from(rate) / HOP as f64;
    let Some(period) = period(&onsets, fps, expected) else {
        return Vec::new();
    };
    // A start is heard at the end of the look whose growth it is, at its middle.
    let delay = FRAME as f64 / 2.0 / f64::from(rate);
    track(&onsets, period)
        .into_iter()
        .map(|frame| frame as f64 / fps + delay)
        .collect()
}

/// The samples averaged down to about [`LISTEN_RATE`], and the rate they are then at.
fn brought_down(samples: &[f32], rate: u32) -> (Vec<f32>, u32) {
    let by = (rate / LISTEN_RATE).max(1) as usize;
    let down = samples
        .chunks_exact(by)
        .map(|chunk| chunk.iter().sum::<f32>() / by as f32)
        .collect();
    (down, rate / by as u32)
}

/// How strongly notes start at each look: by how much each band of the spectrum, heard as
/// loudness rather than power, grows from the look before. Evened out around its local
/// average and scaled to a spread of one, so that loud passages and quiet ones count alike.
fn onsets(samples: &[f32]) -> Vec<f32> {
    if samples.len() < FRAME {
        return Vec::new();
    }
    let window: Vec<f32> = (0..FRAME)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / FRAME as f32).cos())
        .collect();
    let looks = (samples.len() - FRAME) / HOP + 1;
    let mut before = vec![0.0f32; FRAME / 2];
    let mut flux = Vec::with_capacity(looks);
    let mut re = vec![0.0f32; FRAME];
    let mut im = vec![0.0f32; FRAME];
    for look in 0..looks {
        let at = look * HOP;
        for i in 0..FRAME {
            re[i] = samples[at + i] * window[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        let mut grown = 0.0;
        for (bin, last) in before.iter_mut().enumerate() {
            let level = (1.0 + 100.0 * re[bin].hypot(im[bin])).ln();
            grown += (level - *last).max(0.0);
            *last = level;
        }
        flux.push(if look == 0 { 0.0 } else { grown });
    }
    // Take away the level around each look, about a quarter of a second either side.
    let reach = (LISTEN_RATE as usize / HOP / 4).max(1);
    let mut sums = vec![0.0f64; flux.len() + 1];
    for (i, f) in flux.iter().enumerate() {
        sums[i + 1] = sums[i] + f64::from(*f);
    }
    let evened: Vec<f32> = (0..flux.len())
        .map(|i| {
            let (from, to) = (i.saturating_sub(reach), (i + reach + 1).min(flux.len()));
            let mean = (sums[to] - sums[from]) / (to - from) as f64;
            (flux[i] - mean as f32).max(0.0)
        })
        .collect();
    let spread = (evened.iter().map(|o| o * o).sum::<f32>() / evened.len() as f32).sqrt();
    if spread <= f32::EPSILON {
        return Vec::new();
    }
    evened.into_iter().map(|o| o / spread).collect()
}

/// How many looks apart the beats are: the spacing at which the starts repeat most,
/// weighed towards the `expected` seconds apart, refined between whole looks.
fn period(onsets: &[f32], fps: f64, expected: f64) -> Option<f64> {
    let shortest = (fps * 60.0 / FASTEST).floor().max(1.0) as usize;
    let longest = ((fps * 60.0 / SLOWEST).ceil() as usize).min(onsets.len() / 2);
    if longest <= shortest + 2 {
        return None;
    }
    let likely = (expected * fps).clamp(shortest as f64, longest as f64);
    let repeats = |lag: usize| -> f64 {
        let sum: f32 = (onsets.iter().zip(&onsets[lag..]))
            .map(|(a, b)| a * b)
            .sum();
        f64::from(sum) / (onsets.len() - lag) as f64
    };
    let weighed: Vec<f64> = (shortest - 1..=longest + 1)
        .map(|lag| {
            let octaves = (lag as f64 / likely).log2() / OCTAVES;
            repeats(lag) * (-0.5 * octaves * octaves).exp()
        })
        .collect();
    let best = (1..weighed.len() - 1).max_by(|&a, &b| weighed[a].total_cmp(&weighed[b]))?;
    if weighed[best] <= 0.0 {
        return None;
    }
    // The top of the parabola through the best spacing and the two beside it.
    let (a, b, c) = (weighed[best - 1], weighed[best], weighed[best + 1]);
    let bend = a - 2.0 * b + c;
    let shift = if bend < 0.0 {
        0.5 * (a - c) / bend
    } else {
        0.0
    };
    Some((best + shortest - 1) as f64 + shift.clamp(-0.5, 0.5))
}

/// The looks the beats fall on: of every run of starts, the one whose starts are strongest
/// less what it costs for the spaces between them to differ from `period`.
fn track(onsets: &[f32], period: f64) -> Vec<usize> {
    let n = onsets.len();
    let mut score = vec![0.0f32; n];
    let mut from: Vec<Option<usize>> = vec![None; n];
    let (nearest, furthest) = (
        (period / 2.0).round() as usize,
        (period * 2.0).round() as usize,
    );
    for t in 0..n {
        let mut best: Option<(f32, usize)> = None;
        for gap in nearest.max(1)..=furthest.min(t) {
            let p = t - gap;
            let off = (gap as f64 / period).ln() as f32;
            let value = score[p] - TIGHTNESS * off * off;
            if best.is_none_or(|(v, _)| value > v) {
                best = Some((value, p));
            }
        }
        score[t] = onsets[t] + best.map_or(0.0, |(v, _)| v.max(0.0));
        from[t] = best.filter(|(v, _)| *v > 0.0).map(|(_, p)| p);
    }
    // The last beat is the best of those in the last beat's length, and the rest are
    // followed back from it.
    let last_from = n.saturating_sub(period.round() as usize);
    let Some(mut t) = (last_from..n).max_by(|&a, &b| score[a].total_cmp(&score[b])) else {
        return Vec::new();
    };
    let mut found = vec![t];
    while let Some(p) = from[t] {
        found.push(p);
        t = p;
    }
    found.reverse();
    // The run is carried on over silence at either end, where nothing starts: those beats
    // are not heard, and are left out.
    let mut strengths: Vec<f32> = found.iter().map(|&t| onsets[t]).collect();
    strengths.sort_by(f32::total_cmp);
    let heard = 0.2 * strengths[strengths.len() / 2];
    let first = found.iter().position(|&t| onsets[t] > heard);
    let last = found.iter().rposition(|&t| onsets[t] > heard);
    match (first, last) {
        (Some(first), Some(last)) => found[first..=last].to_vec(),
        _ => Vec::new(),
    }
}

/// A Fourier transform in place, of a length that is a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut length = 2;
    while length <= n {
        let angle = -2.0 * PI / length as f32;
        for start in (0..n).step_by(length) {
            for k in 0..length / 2 {
                let (sin, cos) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + length / 2);
                let (tr, ti) = (re[b] * cos - im[b] * sin, re[b] * sin + im[b] * cos);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        length <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `seconds` of clicks at the times given, each a short ringing burst, over noise.
    fn clicks(seconds: f64, times: impl Iterator<Item = f64>, rate: u32) -> Vec<f32> {
        let mut samples = vec![0.0f32; (seconds * f64::from(rate)) as usize];
        let mut seed = 1u32;
        for s in &mut samples {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s = (seed >> 8) as f32 / (1 << 24) as f32 * 0.04 - 0.02;
        }
        for time in times {
            let at = (time * f64::from(rate)) as usize;
            for i in 0..2_000.min(samples.len().saturating_sub(at)) {
                let t = i as f32 / rate as f32;
                samples[at + i] += 0.6 * (2.0 * PI * 1_000.0 * t).sin() * (-t * 120.0).exp();
            }
        }
        samples
    }

    fn near(found: &[f64], expected: &[f64]) {
        assert_eq!(found.len(), expected.len(), "{found:?}");
        for (f, e) in found.iter().zip(expected) {
            assert!((f - e).abs() < 0.025, "{f} is not {e}: {found:?}");
        }
    }

    #[test]
    fn the_beats_of_clicks_are_found_where_they_are() {
        let times: Vec<f64> = (0..20).map(|k| 0.25 + 0.5 * k as f64).collect();
        let found = beats(&clicks(10.2, times.iter().copied(), 44_100), 44_100, 0.5);
        near(&found, &times);
        // At another rate, and a little off the beat written.
        let found = beats(&clicks(10.2, times.iter().copied(), 48_000), 48_000, 0.45);
        near(&found, &times);
    }

    #[test]
    fn the_beat_written_decides_between_one_and_one_twice_as_slow() {
        let times: Vec<f64> = (0..20).map(|k| 0.25 + 0.5 * k as f64).collect();
        let slow = beats(&clicks(10.2, times.iter().copied(), 44_100), 44_100, 1.0);
        assert!((slow.len() as i32 - 10).abs() <= 1, "{slow:?}");
        for pair in slow.windows(2) {
            assert!((pair[1] - pair[0] - 1.0).abs() < 0.03, "{slow:?}");
        }
    }

    #[test]
    fn a_beat_that_drifts_is_followed() {
        // Getting faster: from 100 beats a minute to about 130.
        let mut times = Vec::new();
        let (mut time, mut gap) = (0.3, 0.6);
        while time < 12.0 {
            times.push(time);
            time += gap;
            gap *= 0.985;
        }
        let found = beats(&clicks(12.2, times.iter().copied(), 44_100), 44_100, 0.55);
        near(&found, &times);
    }

    #[test]
    fn silence_has_no_beat() {
        assert!(beats(&vec![0.0; 44_100 * 4], 44_100, 0.5).is_empty());
        assert!(beats(&[0.1; 100], 44_100, 0.5).is_empty());
    }
}
