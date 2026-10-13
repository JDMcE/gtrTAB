//! A recording the score is played along with, such as the song a part was taken down
//! from: its place in the mix, and where the score's bars fall in it.

use serde::{Deserialize, Serialize};

use crate::Mix;

/// The score's audio track. The recording itself is kept beside the score, not in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AudioTrack {
    /// The name of the file the recording came from.
    pub name: String,
    #[serde(default)]
    pub mix: Mix,
    /// Where bars start in the recording, in the order of the bars they name. Between two
    /// of them the bars are played evenly faster or slower than written to fit; before the
    /// first and after the last, as fast as the nearest two have them.
    #[serde(default)]
    pub sync: Vec<SyncPoint>,
}

/// A bar that starts at a place in the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SyncPoint {
    /// The bar's place in the order the score is played through from the start, repeats
    /// and jumps followed, counted from 0: a bar played twice has two places.
    pub bar: usize,
    /// Milliseconds into the recording the bar starts.
    pub millis: u32,
}

impl AudioTrack {
    /// The most sync points a recording can have.
    pub const MAX_SYNC: usize = 10_000;
    pub const MAX_NAME: usize = 200;

    /// A recording with its first bar at its start, and the bars after it as written.
    pub fn new(name: impl Into<String>) -> AudioTrack {
        AudioTrack {
            name: name.into(),
            mix: Mix::default(),
            sync: vec![SyncPoint { bar: 0, millis: 0 }],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.name.chars().count() <= AudioTrack::MAX_NAME
            && self.mix.is_valid()
            && self.sync.len() <= AudioTrack::MAX_SYNC
            && self
                .sync
                .windows(2)
                .all(|p| p[0].bar < p[1].bar && p[0].millis < p[1].millis)
    }

    /// The sync points with the one for `bar` put in, in place of any it had, and any
    /// that would no longer be in order with it taken out: a bar cannot start before one
    /// that comes ahead of it.
    pub fn with_sync(&self, point: SyncPoint) -> Vec<SyncPoint> {
        let mut sync: Vec<SyncPoint> = (self.sync.iter().copied())
            .filter(|p| {
                (p.bar < point.bar && p.millis < point.millis)
                    || (p.bar > point.bar && p.millis > point.millis)
            })
            .collect();
        let at = sync.partition_point(|p| p.bar < point.bar);
        sync.insert(at, point);
        sync
    }
}

/// Where each bar starts in the recording, in seconds, and how many times longer than
/// written it lasts there, from the seconds each lasts as written (`lengths`, in the order
/// the score is played through) and the sync points. A bar before the recording starts
/// starts at a time under zero.
pub fn synced(lengths: &[f64], sync: &[SyncPoint]) -> Vec<(f64, f64)> {
    let mut starts = Vec::with_capacity(lengths.len() + 1);
    let mut start = 0.0;
    for length in lengths {
        starts.push(start);
        start += length;
    }
    starts.push(start);
    let points: Vec<(usize, f64)> = (sync.iter())
        .filter(|p| p.bar <= lengths.len())
        .map(|p| (p.bar, f64::from(p.millis) / 1000.0))
        .collect();
    // How much longer than written the bars between two points are, if they can be made
    // to fit at all.
    let factor = |a: (usize, f64), b: (usize, f64)| {
        let written = starts[b.0] - starts[a.0];
        let heard = b.1 - a.1;
        if written > 0.0 && heard > 0.0 {
            heard / written
        } else {
            1.0
        }
    };
    (0..lengths.len())
        .map(|bar| {
            let after = points.partition_point(|p| p.0 <= bar);
            // The segment the bar is in, or the nearest one for a bar outside them all.
            let (from, factor) = match (after.checked_sub(1), points.get(after)) {
                (None, None) => ((0, 0.0), 1.0),
                (Some(i), Some(&next)) => (points[i], factor(points[i], next)),
                (Some(i), None) if i > 0 => (points[i], factor(points[i - 1], points[i])),
                (None, Some(_)) if points.len() > 1 => (points[0], factor(points[0], points[1])),
                // One point alone: the bars are as written from it.
                (Some(i), None) => (points[i], 1.0),
                (None, Some(_)) => (points[0], 1.0),
            };
            (from.1 + (starts[bar] - starts[from.0]) * factor, factor)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn at(bar: usize, millis: u32) -> SyncPoint {
        SyncPoint { bar, millis }
    }

    fn close(found: &[(f64, f64)], expected: &[(f64, f64)]) {
        assert_eq!(found.len(), expected.len());
        for (f, e) in found.iter().zip(expected) {
            assert!(
                (f.0 - e.0).abs() < 1e-9 && (f.1 - e.1).abs() < 1e-9,
                "{found:?}"
            );
        }
    }

    #[test]
    fn bars_are_fitted_between_the_sync_points() {
        let lengths = [2.0; 6];
        // Nothing to go by: as written, from the start of the recording.
        close(
            &synced(&lengths, &[]),
            &[
                (0.0, 1.0),
                (2.0, 1.0),
                (4.0, 1.0),
                (6.0, 1.0),
                (8.0, 1.0),
                (10.0, 1.0),
            ],
        );
        // Bar 1 at a second in, and the rest as written from there; bar 0 before it.
        close(
            &synced(&lengths, &[at(1, 1_000)]),
            &[
                (-1.0, 1.0),
                (1.0, 1.0),
                (3.0, 1.0),
                (5.0, 1.0),
                (7.0, 1.0),
                (9.0, 1.0),
            ],
        );
        // Bars 1 to 3 played in five seconds where they are written to take four, and bars
        // 3 and 4 in two: the bars after the last point go on as fast as the last two
        // have them, and the bar before the first as fast as the first two.
        close(
            &synced(&lengths, &[at(1, 1_000), at(3, 6_000), at(5, 8_000)]),
            &[
                (-1.5, 1.25),
                (1.0, 1.25),
                (3.5, 1.25),
                (6.0, 0.5),
                (7.0, 0.5),
                (8.0, 0.5),
            ],
        );
        // A point past the end, and two that cannot be fitted, are no help.
        close(
            &synced(&lengths[..2], &[at(0, 0), at(9, 1_000)]),
            &[(0.0, 1.0), (2.0, 1.0)],
        );
    }

    #[test]
    fn a_sync_point_put_in_takes_out_those_out_of_order_with_it() {
        let mut track = AudioTrack::new("song.mp3");
        assert!(track.is_valid());
        track.sync = track.with_sync(at(4, 8_000));
        track.sync = track.with_sync(at(8, 16_000));
        assert_eq!(track.sync, [at(0, 0), at(4, 8_000), at(8, 16_000)]);
        // Bar 4 moved later than bar 8 was: bar 8's point goes.
        assert_eq!(track.with_sync(at(4, 17_000)), [at(0, 0), at(4, 17_000)]);
        // Bar 6 between them, in order: it is put in its place.
        assert_eq!(
            track.with_sync(at(6, 12_000)),
            [at(0, 0), at(4, 8_000), at(6, 12_000), at(8, 16_000)]
        );
        track.sync = vec![at(4, 8_000), at(2, 9_000)];
        assert!(!track.is_valid());
    }
}
