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

/// The sync points moved onto the same bars, and the same time through each, after the
/// play-through has changed from `before` to `after`, each the bars in the order they are
/// played. `bars` gives each bar of before the edit its number after it, or `None` for
/// a bar taken away. A point whose bar is gone, or is played fewer times now, is let go,
/// as is one that would now come before a point ahead of it in the recording.
pub fn moved(
    sync: &[SyncPoint],
    before: &[usize],
    after: &[usize],
    bars: &[Option<usize>],
) -> Vec<SyncPoint> {
    let mut kept: Vec<SyncPoint> = Vec::with_capacity(sync.len());
    for point in sync {
        let Some(&bar) = before.get(point.bar) else {
            continue;
        };
        let time = before[..point.bar].iter().filter(|&&b| b == bar).count();
        let Some(Some(now)) = bars.get(bar) else {
            continue;
        };
        let place = (after.iter().enumerate())
            .filter(|&(_, &b)| b == *now)
            .nth(time)
            .map(|(place, _)| place);
        match place {
            Some(place) if kept.last().is_none_or(|last| last.bar < place) => {
                kept.push(SyncPoint {
                    bar: place,
                    millis: point.millis,
                })
            }
            _ => {}
        }
    }
    kept
}

/// Sync points holding each bar of the play-through to the beat it starts on, from the
/// `beats` heard in the recording, in seconds, and how many beats each bar has (`counts`,
/// in the order the bars are played). `anchor` is a bar's place and where in the recording
/// it is held now: it starts on the beat nearest there, and the bars before and after it
/// are counted out from it, beat by beat. Bars that start before the first beat heard or
/// after the last are not held.
pub fn on_beats(beats: &[f64], counts: &[usize], anchor: (usize, f64)) -> Vec<SyncPoint> {
    let Some(nearest) = (0..beats.len()).min_by(|&a, &b| {
        (beats[a] - anchor.1)
            .abs()
            .total_cmp(&(beats[b] - anchor.1).abs())
    }) else {
        return Vec::new();
    };
    let (place, at) = (anchor.0.min(counts.len()), nearest as i64);
    // The beat each bar starts on, counted back and on from the anchor's.
    let mut starts = vec![0i64; counts.len()];
    for p in (0..place).rev() {
        starts[p] = starts.get(p + 1).copied().unwrap_or(at) - counts[p] as i64;
    }
    let mut beat = at;
    for (p, count) in counts.iter().enumerate().skip(place) {
        starts[p] = beat;
        beat += *count as i64;
    }
    (starts.iter().enumerate())
        .filter_map(|(bar, &start)| {
            let time = beats.get(usize::try_from(start).ok()?)?;
            Some(SyncPoint {
                bar,
                millis: (time * 1000.0).round().max(0.0) as u32,
            })
        })
        .take(AudioTrack::MAX_SYNC)
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

    #[test]
    fn held_bars_stay_held_when_the_play_through_changes() {
        let identity = |n: usize| (0..n).map(Some).collect::<Vec<_>>();
        // Bars 0 to 3, then the same with bars 1 and 2 repeated: 0 1 2 1 2 3.
        let plain = [0, 1, 2, 3];
        let repeated = [0, 1, 2, 1, 2, 3];
        let held = [at(0, 0), at(1, 2_000), at(3, 6_000)];
        // The repeat added: bar 3 is now played sixth, not fourth.
        assert_eq!(
            moved(&held, &plain, &repeated, &identity(4)),
            [at(0, 0), at(1, 2_000), at(5, 6_000)]
        );
        // The second time through a repeat is held as the second time, and taken away
        // with the repeat.
        let second = [at(0, 0), at(3, 6_000), at(5, 10_000)];
        assert_eq!(
            moved(&second, &repeated, &plain, &identity(4)),
            [at(0, 0), at(3, 10_000)]
        );
        // A bar put in before them moves them on; the bar taken away lets its point go.
        let inserted = [Some(0), Some(2), Some(3), Some(4)];
        assert_eq!(
            moved(&held, &plain, &[0, 1, 2, 3, 4], &inserted),
            [at(0, 0), at(2, 2_000), at(4, 6_000)]
        );
        let removed = [Some(0), None, Some(1), Some(2)];
        assert_eq!(
            moved(&held, &plain, &[0, 1, 2], &removed),
            [at(0, 0), at(2, 6_000)]
        );
    }

    #[test]
    fn points_out_of_order_after_a_change_are_let_go() {
        // A jump that now plays bar 3 before bar 1: the later point cannot come first.
        let held = [at(1, 2_000), at(3, 6_000)];
        let identity = [Some(0), Some(1), Some(2), Some(3)];
        assert_eq!(
            moved(&held, &[0, 1, 2, 3], &[0, 3, 1, 2], &identity),
            [at(2, 2_000)]
        );
    }

    #[test]
    fn bars_are_held_to_the_beats_counted_out_from_the_one_held() {
        // A beat every half second from a quarter of a second in.
        let beats: Vec<f64> = (0..14).map(|k| 0.25 + 0.5 * k as f64).collect();
        // Bars of four, then one of three, then of four.
        let counts = [4, 4, 3, 4, 4];
        assert_eq!(
            on_beats(&beats, &counts, (0, 0.0)),
            [at(0, 250), at(1, 2_250), at(2, 4_250), at(3, 5_750)]
        );
        // Held further in, the bars before it are counted back; those before the first
        // beat heard, and after the last, are not held.
        assert_eq!(
            on_beats(&beats, &counts, (1, 1.3)),
            [at(1, 1_250), at(2, 3_250), at(3, 4_750), at(4, 6_750)]
        );
        assert!(on_beats(&[], &counts, (0, 0.0)).is_empty());
    }
}
