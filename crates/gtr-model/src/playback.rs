//! The route playback takes through a score, and how long it takes over each bar.

use crate::{Feel, MasterBar, Rational, Score, Target};

/// How many times its written length a beat under a fermata is held for.
const FERMATA_HOLD: f64 = 2.0;

/// The order in which bars are played once repeats, alternate endings and jumps such as
/// D.S. al Coda are followed. A jump back is taken once. After it the repeats it lands among
/// are passed straight through, on their last ending, as convention has it, unless
/// `replay_repeats` asks for them to be played again.
pub fn play_order(masters: &[MasterBar], replay_repeats: bool) -> Vec<usize> {
    let find = |target: Target| masters.iter().position(|m| m.targets.contains(&target));
    // The pass a repeat reached from `bar` starts on: its first, or once a jump has been
    // taken its last, which is the one that leads out of it.
    let first_pass = |bar: usize, jumped: bool| match jumped && !replay_repeats {
        true => (masters[bar..].iter())
            .find(|m| m.repeat_count >= 2)
            .map_or(1, |m| m.repeat_count),
        false => 1,
    };
    let mut order = Vec::with_capacity(masters.len());
    let mut bar = 0;
    // Where the current repeat goes back to, and which time through it this is.
    let mut repeat_start = 0;
    let mut pass: u8 = 1;
    // A repeat has just been completed: its last ending may still follow.
    let mut finished = false;
    // The jumps back that have been taken, each by its bar and its place among the
    // bar's jumps: a jump is followed once.
    let mut taken: Vec<(usize, usize)> = Vec::new();
    // What the last jump back said to go on until: the fine, or one of the codas.
    let mut until: Option<Target> = None;
    // Scores that loop for ever are cut short rather than followed for ever.
    'bars: while bar < masters.len() && order.len() < 20_000 {
        let master = &masters[bar];
        let jumped = !taken.is_empty();
        if master.repeat_start && repeat_start != bar {
            repeat_start = bar;
            pass = first_pass(bar, jumped);
            finished = false;
        }
        if master.alternate_endings == 0 && finished {
            pass = first_pass(bar, jumped);
            finished = false;
        }
        let ending = master.alternate_endings;
        if ending != 0 && ending & (1 << (pass - 1).min(7)) == 0 {
            bar += 1;
            continue;
        }
        order.push(bar);
        if until == Some(Target::Fine) && master.targets.contains(&Target::Fine) {
            break;
        }
        for (index, &jump) in master.jumps.iter().enumerate() {
            if let Some(coda) = jump.onward() {
                // On to a coda, if that is where the last jump back said to go.
                if until == Some(coda)
                    && let Some(to) = find(coda)
                {
                    until = None;
                    bar = to;
                    continue 'bars;
                }
            } else if !taken.contains(&(bar, index)) {
                taken.push((bar, index));
                until = jump.until();
                bar = jump.back_to().and_then(find).unwrap_or(0);
                repeat_start = bar;
                pass = first_pass(bar, true);
                finished = false;
                continue 'bars;
            }
        }
        if master.repeat_count >= 2 {
            if pass < master.repeat_count {
                pass += 1;
                bar = repeat_start;
                continue;
            }
            finished = true;
            repeat_start = bar + 1;
        }
        bar += 1;
    }
    order
}

/// The order in which the bars `from..to` are played on their own: their repeats are
/// followed, but no jump is taken, since it may lead out of them.
pub fn range_order(masters: &[MasterBar], from: usize, to: usize) -> Vec<usize> {
    let to = to.min(masters.len());
    let mut range = masters[from.min(to)..to].to_vec();
    for master in &mut range {
        master.jumps.clear();
        master.targets.clear();
    }
    (play_order(&range, true).into_iter())
        .map(|bar| bar + from)
        .collect()
}

/// Moves a position within a bar for a feel: of each pair of short notes, the first
/// takes its share of the pair's time, two thirds of it in swing, and the second the rest.
fn swing(feel: Feel, position: Rational) -> Rational {
    let Some((unit, share)) = feel.pair() else {
        return position;
    };
    let start = (position / unit).floor() * unit;
    let within = position - start;
    let half = unit / 2;
    let one = Rational::from_integer(1);
    start
        + if within <= half {
            within * share * 2
        } else {
            unit * share + (within - half) * (one - share) * 2
        }
}

fn float(value: Rational) -> f64 {
    *value.numer() as f64 / *value.denom() as f64
}

/// A stretch of a bar played at one rate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    /// Where it starts: whole notes from the start of the bar, after any swing.
    from: Rational,
    /// Seconds from the start of the bar to there.
    start: f64,
    /// Seconds a whole note lasts at the tempo here.
    whole: f64,
    /// Seconds a whole note takes here, which is longer under a fermata.
    rate: f64,
}

/// How one bar is timed when it is played: its feel, the tempo at each point in it, and
/// the beats in it that a fermata holds.
#[derive(Debug, Clone, PartialEq)]
pub struct BarTiming {
    /// Index of the bar in the score.
    pub bar: usize,
    feel: Feel,
    /// The bar from start to end; the last span has no end.
    spans: Vec<Span>,
    /// Seconds the bar lasts.
    pub length: f64,
}

impl BarTiming {
    /// `tempos` gives the seconds a whole note lasts from each position on, starting with
    /// the start of the bar; `held` the stretches under a fermata. Both are in order.
    fn new(
        bar: usize,
        feel: Feel,
        tempos: &[(Rational, f64)],
        held: &[(Rational, Rational)],
        bar_length: Rational,
    ) -> BarTiming {
        let mut edges: Vec<Rational> = (tempos.iter().map(|t| t.0))
            .chain(held.iter().flat_map(|&(from, to)| [from, to]))
            .collect();
        edges.sort();
        edges.dedup();
        let mut spans: Vec<Span> = Vec::with_capacity(edges.len());
        for from in edges {
            let whole = (tempos.iter().rev())
                .find(|t| t.0 <= from)
                .map_or(tempos[0].1, |t| t.1);
            let hold = match held.iter().any(|&(start, end)| start <= from && from < end) {
                true => FERMATA_HOLD,
                false => 1.0,
            };
            let start = (spans.last()).map_or(0.0, |s| s.start + float(from - s.from) * s.rate);
            spans.push(Span {
                from,
                start,
                whole,
                rate: whole * hold,
            });
        }
        let mut timing = BarTiming {
            bar,
            feel,
            spans,
            length: 0.0,
        };
        timing.length = timing.at(bar_length);
        timing
    }

    fn span(&self, position: Rational) -> &Span {
        let after = self.spans.partition_point(|s| s.from <= position);
        &self.spans[after.saturating_sub(1)]
    }

    /// Seconds from the start of the bar to `position`, in whole notes as written; the
    /// bar's feel decides when it is played.
    pub fn seconds(&self, position: Rational) -> f64 {
        self.at(swing(self.feel, position))
    }

    /// Seconds from the start of the bar to a `position` that no feel moves, such as a
    /// click of the metronome.
    pub fn at(&self, position: Rational) -> f64 {
        let span = self.span(position);
        span.start + float(position - span.from) * span.rate
    }

    /// The bar played `factor` times as long, as when it is fitted to a recording.
    pub fn scaled(&self, factor: f64) -> BarTiming {
        let spans = (self.spans.iter())
            .map(|span| Span {
                start: span.start * factor,
                whole: span.whole * factor,
                rate: span.rate * factor,
                ..*span
            })
            .collect();
        BarTiming {
            spans,
            length: self.length * factor,
            ..self.clone()
        }
    }

    /// Seconds a whole note lasts at the tempo the bar starts in.
    pub fn whole(&self) -> f64 {
        self.spans[0].whole
    }

    /// Seconds a whole note lasts at the tempo in force at `position`, as written.
    pub fn whole_at(&self, position: Rational) -> f64 {
        self.span(swing(self.feel, position)).whole
    }
}

/// The tempo and feel each bar starts in. A change lasts for the bars played after it, and
/// a bar that a repeat or jump comes back to is played as it was the first time.
fn in_force(score: &Score) -> Vec<(u32, Feel)> {
    let masters = &score.master_bars;
    let start = (score.tempo, Feel::Straight);
    let entering = |master: &MasterBar, (tempo, feel): (u32, Feel)| {
        (master.tempo.unwrap_or(tempo), master.feel.unwrap_or(feel))
    };
    // What a bar leaves in force for the next: its last change of tempo, if it has one.
    let leaving = |master: &MasterBar, (tempo, feel): (u32, Feel)| {
        (master.tempo_changes.last().map_or(tempo, |c| c.tempo), feel)
    };
    let mut found: Vec<Option<(u32, Feel)>> = vec![None; masters.len()];
    let mut current = start;
    for bar in play_order(masters, false) {
        let state = *found[bar].get_or_insert(entering(&masters[bar], current));
        current = leaving(&masters[bar], state);
    }
    // A bar that is never reached takes what the bar before it on the page has.
    let mut current = start;
    (found.iter().zip(masters))
        .map(|(found, master)| {
            let state = found.unwrap_or(entering(master, current));
            current = leaving(master, state);
            state
        })
        .collect()
}

/// The stretches of a bar that a fermata holds, in any track, staff or voice.
fn held(score: &Score, bar: usize, feel: Feel) -> Vec<(Rational, Rational)> {
    let voices = (score.tracks.iter())
        .flat_map(|track| &track.staves)
        .filter_map(|staff| staff.bars.get(bar))
        .flat_map(|bar| &bar.voices);
    let mut spans = Vec::new();
    for voice in voices {
        let mut offset = Rational::from_integer(0);
        for beat in &voice.beats {
            let from = offset;
            offset += beat.duration.length();
            if beat.effects.fermata {
                spans.push((swing(feel, from), swing(feel, offset)));
            }
        }
    }
    // Two parts pausing together pause once.
    spans.sort();
    let mut apart: Vec<(Rational, Rational)> = Vec::with_capacity(spans.len());
    for (from, to) in spans {
        match apart.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => apart.push((from, to)),
        }
    }
    apart
}

/// A place where the score sets the tempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    pub bar: usize,
    /// Whole notes from the start of the bar.
    pub position: Rational,
    /// Whole notes from the start of the score, as it is written.
    at: Rational,
    pub tempo: u32,
    /// The tempo moves steadily from here to the next point.
    pub gradual: bool,
}

/// Every place the score sets the tempo, in the order they are written: the start, each
/// bar that has a tempo of its own or starts a gradual change, and each change inside a
/// bar.
pub fn tempo_points(score: &Score) -> Vec<TempoPoint> {
    let zero = Rational::from_integer(0);
    let mut points = Vec::new();
    let mut start = zero;
    let mut current = score.tempo;
    for (bar, master) in score.master_bars.iter().enumerate() {
        if bar == 0 || master.tempo.is_some() || master.tempo_gradual {
            current = master.tempo.unwrap_or(current);
            points.push(TempoPoint {
                bar,
                position: zero,
                at: start,
                tempo: current,
                gradual: master.tempo_gradual,
            });
        }
        for change in &master.tempo_changes {
            current = change.tempo;
            points.push(TempoPoint {
                bar,
                position: change.position,
                at: start + change.position,
                tempo: change.tempo,
                gradual: change.gradual,
            });
        }
        start += master.time_signature.bar_length();
    }
    points
}

/// Whether the tempo set at a point speeds up or slows down from there, if it is the
/// start of a gradual change that goes anywhere.
pub fn gradual_change(points: &[TempoPoint], index: usize) -> Option<std::cmp::Ordering> {
    let (from, to) = (points.get(index)?, points.get(index + 1)?);
    (from.gradual && from.tempo != to.tempo).then(|| to.tempo.cmp(&from.tempo))
}

/// How finely a gradual change of tempo is stepped: the tempo is held for this long, in
/// whole notes, at what it would be half way through.
const RAMP_STEP: Rational = Rational::new_raw(1, 16);

/// How each bar of `order` is timed, at `speed` times the tempo written.
pub fn timings(score: &Score, order: &[usize], speed: f64) -> Vec<BarTiming> {
    let in_force = in_force(score);
    let whole = |tempo: f64| 240.0 / (tempo.max(1.0) * speed);
    // Gradual changes run from one place the tempo is set to the next, as the score is
    // written, whatever order its bars are played in.
    let points = tempo_points(score);
    let ramps: Vec<(&TempoPoint, &TempoPoint)> = (points.windows(2))
        .filter(|pair| pair[0].gradual && pair[0].tempo != pair[1].tempo)
        .map(|pair| (&pair[0], &pair[1]))
        .collect();
    let mut starts = Vec::with_capacity(score.master_bars.len());
    let mut start = Rational::from_integer(0);
    for master in &score.master_bars {
        starts.push(start);
        start += master.time_signature.bar_length();
    }
    (order.iter())
        .map(|&bar| {
            let master = &score.master_bars[bar];
            let (tempo, feel) = in_force[bar];
            let mut tempos: std::collections::BTreeMap<Rational, f64> =
                [(Rational::from_integer(0), whole(tempo.into()))]
                    .into_iter()
                    .chain(
                        (master.tempo_changes.iter())
                            .map(|c| (swing(feel, c.position), whole(c.tempo.into()))),
                    )
                    .collect();
            // Where a gradual change crosses the bar, the tempo is stepped along it.
            let (from, to) = (
                starts[bar],
                starts[bar] + master.time_signature.bar_length(),
            );
            for (first, last) in &ramps {
                let (mut at, end) = (first.at.max(from), last.at.min(to));
                while at < end {
                    let next = (at + RAMP_STEP).min(end);
                    let through = float((at + next) / 2 - first.at) / float(last.at - first.at);
                    let (slow, fast) = (f64::from(first.tempo), f64::from(last.tempo));
                    tempos.insert(
                        swing(feel, at - from),
                        whole(slow + (fast - slow) * through),
                    );
                    at = next;
                }
            }
            let tempos: Vec<(Rational, f64)> = tempos.into_iter().collect();
            let held = held(score, bar, feel);
            BarTiming::new(
                bar,
                feel,
                &tempos,
                &held,
                master.time_signature.bar_length(),
            )
        })
        .collect()
}

/// Seconds into the piece at which each bar is first played, following repeats and jumps
/// at the tempo in force, or `None` for a bar that is never reached.
pub fn bar_times(score: &Score) -> Vec<Option<f64>> {
    let mut times = vec![None; score.master_bars.len()];
    let mut time = 0.0;
    let order = play_order(&score.master_bars, false);
    for timing in timings(score, &order, 1.0) {
        times[timing.bar].get_or_insert(time);
        time += timing.length;
    }
    times
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Jump, TempoChange, TimeSignature, demo_score};

    fn masters(count: usize) -> Vec<MasterBar> {
        vec![MasterBar::new(TimeSignature::COMMON); count]
    }

    #[test]
    fn repeats_and_alternate_endings_are_followed() {
        // |: 0 | 1 (first time) :| 2 (second time) | 3 |
        let mut bars = masters(4);
        bars[0].repeat_start = true;
        bars[1].repeat_count = 2;
        bars[1].alternate_endings = 0b01;
        bars[2].alternate_endings = 0b10;
        assert_eq!(play_order(&bars, false), [0, 1, 0, 2, 3]);

        // A repeat with no start goes back to the beginning; three times through.
        let mut bars = masters(3);
        bars[1].repeat_count = 3;
        assert_eq!(play_order(&bars, false), [0, 1, 0, 1, 0, 1, 2]);

        // Two repeats in a row each go back to their own start.
        let mut bars = masters(4);
        bars[1].repeat_count = 2;
        bars[2].repeat_start = true;
        bars[3].repeat_count = 2;
        assert_eq!(play_order(&bars, false), [0, 1, 0, 1, 2, 3, 2, 3]);
        assert_eq!(play_order(&masters(3), false), [0, 1, 2]);
    }

    #[test]
    fn jumps_are_taken_once() {
        // 0 | segno 1 | 2 to coda | 3 D.S. al Coda | coda 4 | 5
        let mut bars = masters(6);
        bars[1].targets = vec![Target::Segno];
        bars[2].jumps = vec![Jump::ToCoda];
        bars[3].jumps = vec![Jump::DalSegnoAlCoda];
        bars[4].targets = vec![Target::Coda];
        assert_eq!(play_order(&bars, false), [0, 1, 2, 3, 1, 2, 4, 5]);

        // 0 | 1 fine | 2 D.C. al Fine
        let mut bars = masters(3);
        bars[1].targets = vec![Target::Fine];
        bars[2].jumps = vec![Jump::DaCapoAlFine];
        assert_eq!(play_order(&bars, false), [0, 1, 2, 0, 1]);

        // A plain D.C. plays the piece twice and then stops.
        let mut bars = masters(2);
        bars[1].jumps = vec![Jump::DaCapo];
        assert_eq!(play_order(&bars, false), [0, 1, 0, 1]);
    }

    #[test]
    fn repeats_after_a_jump_are_passed_through_unless_asked_for() {
        // |: 0 | 1 (first time) :| 2 (second time) | 3 :| x3 | 4 D.C. | 5
        let mut bars = masters(6);
        bars[0].repeat_start = true;
        bars[1].repeat_count = 2;
        bars[1].alternate_endings = 0b01;
        bars[2].alternate_endings = 0b10;
        bars[3].repeat_start = true;
        bars[3].repeat_count = 3;
        bars[4].jumps = vec![Jump::DaCapo];
        let first = [0, 1, 0, 2, 3, 3, 3, 4];
        // After the D.C. each repeat is played once, by way of its last ending.
        assert_eq!(
            play_order(&bars, false),
            [&first[..], &[0, 2, 3, 4, 5]].concat()
        );
        assert_eq!(
            play_order(&bars, true),
            [&first[..], &first[..], &[5]].concat()
        );
    }

    #[test]
    fn a_range_follows_its_repeats_but_not_its_jumps() {
        // 0 |: 1 | 2 :| 3 D.C. | 4
        let mut bars = masters(5);
        bars[1].repeat_start = true;
        bars[2].repeat_count = 2;
        bars[3].jumps = vec![Jump::DaCapo];
        assert_eq!(range_order(&bars, 1, 4), [1, 2, 1, 2, 3]);
        // A repeat whose start is outside the range goes back to where the range starts.
        assert_eq!(range_order(&bars, 2, 4), [2, 2, 3]);
        assert_eq!(range_order(&bars, 3, 9), [3, 4]);
        assert!(range_order(&bars, 7, 9).is_empty());
    }

    #[test]
    fn bar_times_follow_repeats_and_tempo_changes() {
        let mut score = demo_score();
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.0)]);
        // The first bar twice, at 120; then the second.
        score.master_bars[0].repeat_count = 2;
        assert_eq!(bar_times(&score), [Some(0.0), Some(4.0)]);
        // A faster first bar gets to the second sooner.
        score.master_bars[0].tempo = Some(240);
        score.master_bars[1].tempo = Some(120);
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.0)]);
        // A bar that only an unreachable ending holds is never played.
        score.master_bars[1].alternate_endings = 0b100;
        assert_eq!(bar_times(&score)[1], None);
    }

    #[test]
    fn a_change_of_tempo_lasts_for_the_bars_played_after_it() {
        // 0 | 1 to coda | 2 (tempo 60) D.C. al Coda | coda 3
        let mut score = demo_score();
        score.master_bars = masters(4);
        for staff in score.tracks.iter_mut().flat_map(|t| &mut t.staves) {
            staff.bars.resize(4, crate::Bar::empty());
        }
        score.master_bars[1].jumps = vec![Jump::ToCoda];
        score.master_bars[2].tempo = Some(60);
        score.master_bars[2].jumps = vec![Jump::DaCapoAlCoda];
        score.master_bars[3].targets = vec![Target::Coda];
        let order = play_order(&score.master_bars, false);
        assert_eq!(order, [0, 1, 2, 0, 1, 3]);
        let lengths: Vec<f64> = (timings(&score, &order, 1.0).iter())
            .map(|t| t.length)
            .collect();
        // Back at the start the first tempo is back too, and the coda carries on from the
        // bars played before it, not from the slow bar before it on the page.
        assert_eq!(lengths, [2.0, 2.0, 4.0, 2.0, 2.0, 2.0]);
        // Twice the speed, half the time.
        assert_eq!(timings(&score, &order, 2.0)[2].length, 2.0);
    }

    #[test]
    fn a_tempo_change_inside_a_bar_starts_on_its_beat_and_carries_on() {
        let mut score = demo_score();
        let change = |n, d, tempo| TempoChange {
            position: Rational::new(n, d),
            tempo,
            gradual: false,
        };
        // Half of the first bar at 120, a quarter at 60 and a quarter at 240.
        score.master_bars[0].tempo_changes = vec![change(1, 2, 60), change(3, 4, 240)];
        assert!(score.master_bars[0].is_valid());
        let timing = &timings(&score, &[0, 1], 1.0);
        let at = |n, d| timing[0].seconds(Rational::new(n, d));
        assert_eq!(
            [at(1, 4), at(1, 2), at(5, 8), at(3, 4), at(1, 1)],
            [0.5, 1.0, 1.5, 2.0, 2.25]
        );
        assert_eq!(timing[0].whole(), 2.0);
        assert_eq!(timing[0].whole_at(Rational::new(5, 8)), 4.0);
        // The next bar carries on at the last of them, and a timer on it says when.
        assert_eq!(timing[1].length, 1.0);
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.25)]);

        // A fermata under a change is held at the tempo of the change.
        score.tracks[0].staves[0].bars[0].voices[0].beats = vec![
            crate::Beat::rest(crate::Duration::new(crate::NoteValue::Half)),
            crate::Beat::rest(crate::Duration::new(crate::NoteValue::Half)),
        ];
        score.tracks[0].staves[0].bars[0].voices[0].beats[1]
            .effects
            .fermata = true;
        assert_eq!(timings(&score, &[0], 1.0)[0].length, 1.0 + 2.0 + 0.5);

        // Changes have to be inside the bar, in order, and a tempo.
        let mut bar = score.master_bars[0].clone();
        bar.tempo_changes = vec![change(3, 4, 240), change(1, 2, 60)];
        assert!(!bar.is_valid());
        for wrong in [change(0, 1, 60), change(1, 1, 60), change(1, 2, 0)] {
            bar.tempo_changes = vec![wrong];
            assert!(!bar.is_valid());
        }
    }

    #[test]
    fn a_fermata_holds_every_part_and_swing_moves_the_off_beats() {
        use crate::{Beat, Duration, NoteValue, Track};

        let mut score = demo_score();
        let quarter = Duration::new(NoteValue::Quarter);
        let bar = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        *bar = vec![Beat::rest(quarter); 4];
        bar[1].effects.fermata = true;
        // A second part pausing over the same beat.
        let mut other = Track::empty("Other", crate::Instrument::default(), 2);
        let half = Duration::new(NoteValue::Half);
        other.staves[0].bars[0].voices[0].beats = vec![
            Beat::rest(Duration::new(NoteValue::Eighth)),
            Beat::rest(Duration::new(NoteValue::Eighth)),
            Beat::rest(quarter),
            Beat::rest(half),
        ];
        other.staves[0].bars[0].voices[0].beats[2].effects.fermata = true;
        score.tracks.push(other);

        let timing = &timings(&score, &[0, 1], 1.0)[0];
        let at = |n, d| timing.seconds(Rational::new(n, d));
        // The second quarter lasts twice as long, and what follows is that much later.
        assert_eq!(
            [at(0, 1), at(1, 4), at(3, 8), at(1, 2)],
            [0.0, 0.5, 1.0, 1.5]
        );
        assert_eq!((at(1, 1), timing.length), (2.5, 2.5));
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.5)]);

        score.master_bars[0].feel = Some(Feel::SwingEighths);
        let timing = &timings(&score, &[0], 1.0)[0];
        assert!((timing.seconds(Rational::new(1, 8)) - 1.0 / 3.0).abs() < 1e-9);
        // The metronome keeps even time.
        assert_eq!(timing.at(Rational::new(1, 8)), 0.25);
        assert_eq!(timing.whole(), 2.0);
    }

    #[test]
    fn second_signs_and_several_jumps_on_a_bar_are_followed() {
        // A segno, then a second segno in a bar that can also leave for the second coda;
        // a bar with two jumps, back to the second segno and then back to the first "al
        // double coda"; and the second coda last.
        let mut bars = masters(6);
        bars[1].targets = vec![Target::Segno];
        bars[2].targets = vec![Target::SegnoSegno];
        bars[2].jumps = vec![Jump::ToDoubleCoda];
        bars[4].jumps = vec![Jump::DalSegnoSegno, Jump::DalSegnoAlDoubleCoda];
        bars[5].targets = vec![Target::DoubleCoda];
        // Each jump is taken once, in the order written; the way on to the coda is only
        // open after the jump that says so.
        assert_eq!(play_order(&bars, false), [0, 1, 2, 3, 4, 2, 3, 4, 1, 2, 5]);
        // A jump to a sign the score does not have goes back to the start.
        bars[2].targets.clear();
        assert_eq!(play_order(&bars, false)[..7], [0, 1, 2, 3, 4, 0, 1]);
        for jump in [
            Jump::DaCapoAlDoubleCoda,
            Jump::DalSegnoSegnoAlFine,
            Jump::ToDoubleCoda,
        ] {
            assert!(jump.label().len() > 3);
        }
    }

    #[test]
    fn dotted_and_scottish_feels_move_the_second_note_of_each_pair() {
        // At 120 a quarter note is half a second. Where the eighth off the beat falls,
        // and the sixteenth after it, in each feel.
        let eighth = Rational::new(1, 8);
        let sixteenth = Rational::new(1, 16);
        for (feel, off_beat, after) in [
            (Feel::Straight, 0.25, 0.125),
            (Feel::SwingEighths, 1.0 / 3.0, 1.0 / 6.0),
            (Feel::DottedEighths, 0.375, 0.1875),
            (Feel::ScottishEighths, 0.125, 0.0625),
            (Feel::SwingSixteenths, 0.25, 1.0 / 6.0),
            (Feel::DottedSixteenths, 0.25, 0.1875),
            (Feel::ScottishSixteenths, 0.25, 0.0625),
        ] {
            let mut score = demo_score();
            score.master_bars[0].feel = Some(feel);
            let bar = &timings(&score, &[0], 1.0)[0];
            assert!((bar.seconds(eighth) - off_beat).abs() < 1e-9, "{feel:?}");
            assert!((bar.seconds(sixteenth) - after).abs() < 1e-9, "{feel:?}");
            // The beat itself does not move, and the bar is as long as ever.
            assert!((bar.seconds(Rational::new(1, 4)) - 0.5).abs() < 1e-9);
            assert!((bar.length - 2.0).abs() < 1e-9);
            assert!(!feel.label().is_empty());
        }
    }

    #[test]
    fn a_gradual_change_moves_the_tempo_steadily_to_the_next() {
        let mut score = demo_score();
        score.tempo = 60;
        score.master_bars[0].tempo_gradual = true;
        score.master_bars[1].tempo = Some(120);
        let timed = timings(&score, &[0, 1], 1.0);
        // A bar that speeds up evenly from 60 to 120 lasts four seconds times the natural
        // logarithm of two, where it would last four at 60 and two at 120.
        let expected = 4.0 * std::f64::consts::LN_2;
        assert!(
            (timed[0].length - expected).abs() < 0.02,
            "{}",
            timed[0].length
        );
        assert!((timed[1].length - 2.0).abs() < 1e-9);
        // Half way through it is going at about 90, and it starts at about 60.
        let half_way = 240.0 / timed[0].whole_at(Rational::new(1, 2));
        assert!((90.0..94.0).contains(&half_way), "{half_way}");
        assert!((60.0..63.0).contains(&(240.0 / timed[0].whole())));

        // The same change taken over two bars: each is quicker than the last, and they
        // last twice as long together.
        score
            .master_bars
            .push(MasterBar::new(TimeSignature::COMMON));
        score.master_bars[1].tempo = None;
        score.master_bars[2].tempo = Some(120);
        let timed = timings(&score, &[0, 1, 2], 1.0);
        assert!(timed[0].length > timed[1].length && timed[1].length > timed[2].length);
        assert!((timed[0].length + timed[1].length - 2.0 * expected).abs() < 0.04);
        // And inside a bar, from one change to the next, which holds from there.
        let mut score = demo_score();
        score.master_bars[0].tempo_changes = vec![
            TempoChange {
                position: Rational::new(1, 4),
                tempo: 120,
                gradual: true,
            },
            TempoChange {
                position: Rational::new(3, 4),
                tempo: 60,
                gradual: false,
            },
        ];
        let bar = &timings(&score, &[0], 1.0)[0];
        let slowing = bar.at(Rational::new(3, 4)) - bar.at(Rational::new(1, 4));
        assert!(
            (slowing - 2.0 * std::f64::consts::LN_2).abs() < 0.02,
            "{slowing}"
        );
        assert!((bar.at(Rational::new(1, 4)) - 0.5).abs() < 1e-9);
        assert!((bar.length - bar.at(Rational::new(3, 4)) - 1.0).abs() < 1e-9);

        let points = tempo_points(&score);
        let moving: Vec<_> = (0..points.len())
            .map(|i| gradual_change(&points, i))
            .collect();
        assert_eq!(moving, [None, Some(std::cmp::Ordering::Less), None]);
    }
}
