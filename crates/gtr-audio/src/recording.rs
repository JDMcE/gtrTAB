//! The score's recording, played along with it: as recorded at the speed it was made, and
//! faster or slower with its pitch as it is.

use std::f32::consts::TAU;

/// A recording, as the samples of two sides at the synth's rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    /// Left and right in turn, as sixteen bits each, which is half the room of floats and
    /// as good as any recording is.
    samples: Vec<i16>,
}

impl Recording {
    /// A recording of the samples of its two sides, which are made the same length.
    pub fn new(left: &[f32], right: &[f32]) -> Recording {
        let quantise = |s: f32| (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        let frames = left.len().max(right.len());
        let side = |samples: &[f32], i: usize| quantise(samples.get(i).copied().unwrap_or(0.0));
        Recording {
            samples: (0..frames)
                .flat_map(|i| [side(left, i), side(right, i)])
                .collect(),
        }
    }

    /// A recording of samples already in turn, left and right, as sixteen bits each.
    pub fn interleaved(mut samples: Vec<i16>) -> Recording {
        if samples.len() % 2 == 1 {
            samples.push(0);
        }
        Recording { samples }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Both sides at `frame`; silence outside the recording.
    pub(crate) fn at(&self, frame: i64) -> (f32, f32) {
        let Some(i) = usize::try_from(frame).ok().filter(|&f| f < self.frames()) else {
            return (0.0, 0.0);
        };
        let scale = 1.0 / f32::from(i16::MAX);
        (
            f32::from(self.samples[2 * i]) * scale,
            f32::from(self.samples[2 * i + 1]) * scale,
        )
    }

    /// Both sides added, at `frame`, for comparing one stretch with another.
    fn mono(&self, frame: i64) -> f32 {
        let (left, right) = self.at(frame);
        left + right
    }
}

/// Plays a recording at another speed with its pitch as it is, by WSOLA: grains of it
/// half a grain apart, each faded in and out, taken from where the speed says it has got
/// to, moved a little to where they best carry on from the grain before so that the
/// waves meet in step.
pub(crate) struct Stretch {
    grain: usize,
    hop: usize,
    /// How far a grain can be moved from where the speed puts it.
    reach: i64,
    window: Vec<f32>,
    /// The grains added up, from the next sample to come out on.
    sum: [Vec<f32>; 2],
    /// How many of the samples at the front of `sum` have come out.
    out: usize,
    /// Where in the recording the last grain started, if there is one to carry on from.
    last: Option<i64>,
}

impl Stretch {
    pub(crate) fn new(sample_rate: f32) -> Stretch {
        // Grains of 40 ms: long enough for a low note's wave, short enough not to smear
        // a strum.
        let grain = ((0.04 * sample_rate) as usize / 2 * 2).max(64);
        let hop = grain / 2;
        // A periodic Hann window, whose halves add up to one at half a grain apart.
        let window = (0..grain)
            .map(|i| 0.5 - 0.5 * (TAU * i as f32 / grain as f32).cos())
            .collect();
        Stretch {
            grain,
            hop,
            reach: (grain / 4) as i64,
            window,
            sum: [vec![0.0; grain], vec![0.0; grain]],
            out: hop,
            last: None,
        }
    }

    /// Starts afresh, as after playback has jumped to another place.
    pub(crate) fn reset(&mut self) {
        self.last = None;
        self.out = self.hop;
        for side in &mut self.sum {
            side.fill(0.0);
        }
    }

    /// The next sample, where playback has got to `target` frames into the recording.
    pub(crate) fn next(&mut self, recording: &Recording, target: f64) -> (f32, f32) {
        if self.out == self.hop {
            self.add_grain(recording, target.round() as i64);
        }
        let sample = (self.sum[0][self.out], self.sum[1][self.out]);
        self.out += 1;
        sample
    }

    fn add_grain(&mut self, recording: &Recording, ideal: i64) {
        // What would carry on from the last grain without a break: the stretch after it.
        let natural = self.last.map(|last| last + self.hop as i64);
        let start = match natural {
            // Near enough to carry on from: the grain is moved to where it matches.
            Some(natural) if (ideal - natural).abs() <= 2 * self.grain as i64 => {
                self.best(recording, natural, ideal)
            }
            // Playback has jumped: start again where it has gone.
            _ => {
                for side in &mut self.sum {
                    side.fill(0.0);
                }
                ideal
            }
        };
        // What has come out goes, and the new grain is added over what is left.
        for side in &mut self.sum {
            side.copy_within(self.hop.., 0);
            side[self.grain - self.hop..].fill(0.0);
        }
        for (i, &weight) in self.window.iter().enumerate() {
            let (left, right) = recording.at(start + i as i64);
            self.sum[0][i] += left * weight;
            self.sum[1][i] += right * weight;
        }
        self.last = Some(start);
        self.out = 0;
    }

    /// Where near `ideal` a grain should start to be most like the stretch at `natural`,
    /// over the half a grain the two overlap.
    fn best(&self, recording: &Recording, natural: i64, ideal: i64) -> i64 {
        // Every fourth sample is enough to tell a wave in step from one out of it.
        const STRIDE: usize = 4;
        let overlap = self.grain - self.hop;
        let reference: Vec<f32> = (0..overlap)
            .step_by(STRIDE)
            .map(|i| recording.mono(natural + i as i64))
            .collect();
        let mut best = (f32::MIN, ideal);
        for offset in -self.reach..=self.reach {
            let from = ideal + offset;
            let score: f32 = (reference.iter().enumerate())
                .map(|(k, r)| r * recording.mono(from + (k * STRIDE) as i64))
                .sum();
            if score > best.0 {
                best = (score, from);
            }
        }
        best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    fn tone(frequency: f32, seconds: f32) -> Recording {
        let samples: Vec<f32> = (0..(seconds * RATE) as usize)
            .map(|i| 0.5 * (TAU * frequency * i as f32 / RATE).sin())
            .collect();
        Recording::new(&samples, &samples)
    }

    /// The frequency of a stretch of samples, from how often it crosses zero going up.
    fn frequency(samples: &[f32]) -> f32 {
        let ups = samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        ups as f32 * RATE / samples.len() as f32
    }

    #[test]
    fn a_recording_keeps_its_samples_and_is_silent_outside_them() {
        let recording = Recording::new(&[0.5, -1.0], &[0.25]);
        assert_eq!(recording.frames(), 2);
        let (left, right) = recording.at(1);
        assert!((left + 1.0).abs() < 1e-4 && right == 0.0);
        assert_eq!(recording.at(-1), (0.0, 0.0));
        assert_eq!(recording.at(2), (0.0, 0.0));
        assert_eq!(Recording::interleaved(vec![1, 2, 3]).frames(), 2);
    }

    #[test]
    fn played_slower_or_faster_the_pitch_stays_and_the_level_holds() {
        let recording = tone(220.0, 3.0);
        for rate in [0.5, 0.75, 1.25] {
            let mut stretch = Stretch::new(RATE);
            let frames = (1.5 * RATE) as usize;
            let left: Vec<f32> = (0..frames)
                .map(|i| {
                    stretch
                        .next(&recording, 0.2 * f64::from(RATE) + i as f64 * rate)
                        .0
                })
                .collect();
            let steady = &left[4_410..];
            let heard = frequency(steady);
            assert!((heard - 220.0).abs() < 3.0, "{rate}: {heard} Hz");
            let level = (steady.iter().map(|s| s * s).sum::<f32>() / steady.len() as f32).sqrt();
            // A sine of 0.5 has a level of 0.35, and the grains meet in step without a dip.
            assert!((level - 0.354).abs() < 0.04, "{rate}: {level}");
        }
    }

    #[test]
    fn a_jump_starts_the_grains_afresh() {
        let recording = tone(220.0, 3.0);
        let mut stretch = Stretch::new(RATE);
        for i in 0..5_000 {
            stretch.next(&recording, i as f64 * 0.5);
        }
        // Two seconds on: the next grains come from there, not from where it was.
        let far: Vec<f32> = (0..5_000)
            .map(|i| stretch.next(&recording, 88_200.0 + i as f64 * 0.5).0)
            .collect();
        assert!(far[2_000..].iter().any(|s| s.abs() > 0.3));
        stretch.reset();
        assert_eq!(
            stretch.next(&recording, 3.0 * f64::from(RATE) + 10.0),
            (0.0, 0.0)
        );
    }
}
