use gtr_model::{Accent, Beat, BendKind, Harmonic, Note, Rational, Score, Slide, Staff, Vibrato};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub struct Options {
    /// First bar to play.
    pub from_bar: usize,
    /// Bar to stop before; `None` plays to the end.
    pub to_bar: Option<usize>,
    pub metronome: bool,
    /// Play one bar of clicks before the music.
    pub count_in: bool,
    /// Multiplier on the score tempo, e.g. 0.5 for half speed.
    pub speed: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            from_bar: 0,
            to_bar: None,
            metronome: false,
            count_in: false,
            speed: 1.0,
        }
    }
}

/// How a note is sounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Normal,
    PalmMute,
    /// A muted, percussive click with no clear pitch.
    Dead,
    Harmonic,
}

/// A sounding note. Tied notes are merged into the note they extend.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteEvent {
    /// Seconds from the start of playback.
    pub time: f64,
    pub duration: f64,
    /// MIDI pitch before any bend.
    pub pitch: u8,
    /// Loudness relative to a normal note.
    pub velocity: f32,
    pub tone: Tone,
    /// Sounded by the fretting hand alone, as after a hammer-on or legato slide.
    pub soft: bool,
    /// Pitch offset over the note as `(seconds from its start, semitones)` points, joined by
    /// straight lines. Empty for a steady pitch.
    pub pitch_curve: Vec<(f32, f32)>,
    /// Vibrato depth in semitones; zero for none.
    pub vibrato: f32,
}

impl NoteEvent {
    /// A plain note with no articulation.
    pub fn plain(time: f64, duration: f64, pitch: u8) -> Self {
        NoteEvent {
            time,
            duration,
            pitch,
            velocity: 1.0,
            tone: Tone::Normal,
            soft: false,
            pitch_curve: Vec::new(),
            vibrato: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Click {
    pub time: f64,
    /// First click of a bar.
    pub accent: bool,
}

/// When a beat of the followed track sounds, for moving the playback cursor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BeatTime {
    pub bar: usize,
    pub beat: usize,
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Timeline {
    pub notes: Vec<NoteEvent>,
    pub clicks: Vec<Click>,
    /// Beats of the followed track in time order.
    pub beats: Vec<BeatTime>,
    /// Total length in seconds.
    pub duration: f64,
}

/// Longest a let-ring note is allowed to sustain.
const MAX_RING: f64 = 4.0;

/// Compiles the score to timed events. Every track sounds; `follow` picks the track whose
/// beats are reported for the playback cursor.
pub fn compile(score: &Score, follow: usize, options: &Options) -> Timeline {
    let bar_count = score.master_bars.len();
    let to_bar = options.to_bar.unwrap_or(bar_count).min(bar_count);
    let from_bar = options.from_bar.min(to_bar);
    let speed = if options.speed > 0.0 {
        options.speed
    } else {
        1.0
    };
    let whole_note = 240.0 / (f64::from(score.tempo.max(1)) * speed);
    let seconds = |length: Rational| *length.numer() as f64 / *length.denom() as f64 * whole_note;

    let mut timeline = Timeline {
        notes: Vec::new(),
        clicks: Vec::new(),
        beats: Vec::new(),
        duration: 0.0,
    };

    // Bar start times, with one extra entry for the end of the passage.
    let mut time = 0.0;
    if options.count_in && from_bar < to_bar {
        let signature = score.master_bars[from_bar].time_signature;
        add_clicks(&mut timeline.clicks, time, signature, &seconds);
        time += seconds(signature.bar_length());
    }
    let mut starts = Vec::with_capacity(to_bar - from_bar + 1);
    for master in &score.master_bars[from_bar..to_bar] {
        starts.push(time);
        if options.metronome {
            add_clicks(&mut timeline.clicks, time, master.time_signature, &seconds);
        }
        time += seconds(master.time_signature.bar_length());
    }
    starts.push(time);
    timeline.duration = time;

    for (index, track) in score.tracks.iter().enumerate() {
        for staff in &track.staves {
            let followed = index == follow && std::ptr::eq(staff, &track.staves[0]);
            add_staff(&mut timeline, staff, from_bar, &starts, followed, &seconds);
        }
    }
    timeline.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    timeline
}

fn add_clicks(
    clicks: &mut Vec<Click>,
    start: f64,
    signature: gtr_model::TimeSignature,
    seconds: &impl Fn(Rational) -> f64,
) {
    let pulse = Rational::new(1, signature.denominator.into());
    for i in 0..signature.numerator {
        clicks.push(Click {
            time: start + seconds(pulse * i64::from(i)),
            accent: i == 0,
        });
    }
}

/// What is known about the note last played on a string.
#[derive(Clone, Copy, Default)]
struct StringState {
    /// Event still sounding from the previous beat, which a tie extends.
    sounding: Option<usize>,
    /// The previous note leads into the next by hammer-on, pull-off or legato slide.
    legato: bool,
    /// A let-ring note that sustains until the string is played again.
    ringing: Option<usize>,
}

fn add_staff(
    timeline: &mut Timeline,
    staff: &Staff,
    from_bar: usize,
    starts: &[f64],
    followed: bool,
    seconds: &impl Fn(Rational) -> f64,
) {
    let end = starts[starts.len() - 1];
    let bars = &staff.bars[from_bar..from_bar + starts.len() - 1];
    let mut strings = vec![StringState::default(); staff.tuning.strings.len()];
    for (b, bar) in bars.iter().enumerate() {
        for (v, voice) in bar.voices.iter().enumerate() {
            let mut offset = Rational::from_integer(0);
            for (i, beat) in voice.beats.iter().enumerate() {
                let time = starts[b] + seconds(offset);
                let duration = seconds(beat.duration.length());
                offset += beat.duration.length();
                if followed && v == 0 {
                    timeline.beats.push(BeatTime {
                        bar: from_bar + b,
                        beat: i,
                        time,
                    });
                }
                // Tremolo picking re-picks the beat in equal short notes.
                let picks = match beat.effects.tremolo {
                    Some(value) => (duration / seconds(value.length())).round().max(1.0) as usize,
                    None => 1,
                };
                let pick_length = duration / picks as f64;

                let mut next = vec![StringState::default(); strings.len()];
                for note in &beat.notes {
                    let string = usize::from(note.string);
                    let Some(pitch) = staff.tuning.pitch(note.string, note.fret, staff.capo) else {
                        continue;
                    };
                    // Other voices sound, but take no part in ties and legato.
                    let previous = match v {
                        0 => strings[string],
                        _ => StringState::default(),
                    };
                    if let (true, Some(index)) = (note.tied, previous.sounding) {
                        let event = &mut timeline.notes[index];
                        event.duration = time + duration - event.time;
                        next[string] = StringState {
                            sounding: Some(index),
                            legato: joins_next(note),
                            ringing: previous.ringing,
                        };
                        continue;
                    }
                    if let Some(index) = previous.ringing {
                        let event = &mut timeline.notes[index];
                        event.duration = time - event.time;
                    }

                    let following = following_note(bars, b, v, i, note.string);
                    let mut event = note_event(note, staff, pitch, time, pick_length, following);
                    event.soft = previous.legato;
                    for pick in 1..picks {
                        let mut repeat = event.clone();
                        repeat.time = time + pick_length * pick as f64;
                        repeat.soft = false;
                        timeline.notes.push(repeat);
                    }
                    timeline.notes.push(event);
                    let index = timeline.notes.len() - 1;
                    next[string] = StringState {
                        sounding: (picks == 1).then_some(index),
                        legato: joins_next(note),
                        ringing: (note.effects.let_ring && picks == 1).then_some(index),
                    };
                }
                if v == 0 {
                    // Strings left alone keep ringing; everything else starts afresh.
                    for (state, old) in next.iter_mut().zip(&strings) {
                        if state.sounding.is_none() {
                            state.ringing = old.ringing;
                        }
                    }
                    strings = next;
                }
            }
        }
        // A tie or slur only reaches into the next bar from this bar's last beat, which the
        // string state already reflects; an empty bar breaks it.
        if bar.voices.first().is_none_or(|v| v.beats.is_empty()) {
            for state in &mut strings {
                state.sounding = None;
                state.legato = false;
            }
        }
    }
    // Notes still ringing at the end sustain to the end of the passage.
    for index in strings.iter().filter_map(|s| s.ringing) {
        let event = &mut timeline.notes[index];
        event.duration = (end - event.time).clamp(event.duration, MAX_RING);
    }
}

fn joins_next(note: &Note) -> bool {
    note.effects.hammer || note.effects.slide == Some(Slide::Legato)
}

/// The note on `string` in the beat after `beat`, looking into the next bar if need be.
fn following_note(
    bars: &[gtr_model::Bar],
    bar: usize,
    voice: usize,
    beat: usize,
    string: u8,
) -> Option<&Note> {
    let next: Option<&Beat> = match bars[bar].voices[voice].beats.get(beat + 1) {
        Some(next) => Some(next),
        None => bars
            .get(bar + 1)
            .and_then(|b| b.voices.get(voice))
            .and_then(|v| v.beats.first()),
    };
    next?.note_on(string)
}

/// Builds the event for a freshly sounded note from its effects.
fn note_event(
    note: &Note,
    staff: &Staff,
    fretted: u8,
    time: f64,
    length: f64,
    following: Option<&Note>,
) -> NoteEvent {
    let effects = &note.effects;
    let mut event = NoteEvent::plain(time, length, fretted);
    let d = length as f32;

    if let Some(harmonic) = effects.harmonic {
        event.tone = Tone::Harmonic;
        event.pitch = match harmonic {
            // A natural harmonic sounds an overtone of the open string, chosen by where
            // the string is touched.
            Harmonic::Natural => {
                let open = staff
                    .tuning
                    .pitch(note.string, 0, staff.capo)
                    .unwrap_or(fretted);
                let above = match note.fret % 12 {
                    0 => 12,
                    7 => 19,
                    5 => 24,
                    4 | 9 => 28,
                    3 => 31,
                    _ => 12 + note.fret % 12,
                };
                open.saturating_add(above + if note.fret >= 24 { 12 } else { 0 })
            }
            _ => fretted.saturating_add(12),
        }
        .min(127);
    }
    if effects.palm_mute {
        event.tone = Tone::PalmMute;
    }
    if effects.dead {
        event.tone = Tone::Dead;
        event.duration = event.duration.min(0.08);
    }
    if effects.staccato {
        event.duration *= 0.5;
    }
    event.velocity = match effects.accent {
        Some(Accent::Normal) => 1.35,
        Some(Accent::Heavy) => 1.7,
        None => 1.0,
    } * if effects.ghost { 0.45 } else { 1.0 };
    event.vibrato = match effects.vibrato {
        Some(Vibrato::Slight) => 0.2,
        Some(Vibrato::Wide) => 0.5,
        None => 0.0,
    };

    if let Some(bend) = effects.bend {
        let height = f32::from(bend.quarter_tones) / 2.0;
        // The string is pushed up over the first part of the note, and let down again
        // over the second half if the bend is released.
        let rise = (d * 0.3).min(0.25);
        event.pitch_curve = match bend.kind {
            BendKind::Bend => vec![(0.0, 0.0), (rise, height)],
            BendKind::BendRelease => {
                vec![
                    (0.0, 0.0),
                    (rise, height),
                    (d * 0.5, height),
                    (d * 0.5 + rise, 0.0),
                ]
            }
            BendKind::Prebend => vec![(0.0, height)],
            BendKind::PrebendRelease => {
                vec![(0.0, height), (d * 0.4, height), (d * 0.4 + rise, 0.0)]
            }
        };
    } else if let Some(slide) = effects.slide {
        let glide = (d * 0.35).min(0.12);
        event.pitch_curve = match (slide, following) {
            (Slide::Shift | Slide::Legato, Some(target)) => {
                let interval = f32::from(target.fret) - f32::from(note.fret);
                vec![(0.0, 0.0), (d - glide, 0.0), (d, interval)]
            }
            (Slide::Shift | Slide::Legato, None) => Vec::new(),
            (Slide::InFromBelow, _) => vec![(0.0, -2.0), (glide, 0.0)],
            (Slide::InFromAbove, _) => vec![(0.0, 2.0), (glide, 0.0)],
            (Slide::OutDown, _) => vec![(0.0, 0.0), (d - glide, 0.0), (d, -4.0)],
            (Slide::OutUp, _) => vec![(0.0, 0.0), (d - glide, 0.0), (d, 4.0)],
        };
    }
    event
}

#[cfg(test)]
mod tests {
    use gtr_model::{Beat, Bend, Duration, Note, NoteEffects, NoteValue, Score, demo_score};

    use super::*;

    /// A blank score whose first bar holds quarter notes on the low E string.
    fn quarters(notes: Vec<Note>) -> Score {
        let mut score = Score::blank();
        score.tracks[0].staves[0].bars[0].voices[0].beats = notes
            .into_iter()
            .map(|n| Beat::new(Duration::new(NoteValue::Quarter), vec![n]))
            .collect();
        score
    }

    fn with(fret: u8, edit: impl FnOnce(&mut NoteEffects)) -> Note {
        let mut note = Note::new(5, fret);
        edit(&mut note.effects);
        note
    }

    #[test]
    fn demo_timeline() {
        let score = demo_score();
        let t = compile(&score, 0, &Options::default());
        // 120 bpm in 4/4: two-second bars, eighth notes a quarter of a second apart.
        assert_eq!(t.duration, 4.0);
        assert_eq!(t.beats.len(), 13);
        assert_eq!(t.beats[1].time, 0.25);
        assert_eq!(
            (t.beats[8].bar, t.beats[8].beat, t.beats[8].time),
            (1, 0, 2.0)
        );
        assert_eq!(t.notes.len(), 15);
        // Low E, then B on the A string at fret 2.
        assert_eq!(t.notes[0].pitch, 40);
        assert_eq!(t.notes[2].pitch, 47);
        // The final chord is a half note.
        let chord: Vec<_> = t.notes.iter().filter(|n| n.time == 3.0).collect();
        assert_eq!(chord.len(), 3);
        assert!(chord.iter().all(|n| n.duration == 1.0));
        assert!(t.clicks.is_empty());
        assert!(
            t.notes
                .iter()
                .all(|n| *n == NoteEvent::plain(n.time, n.duration, n.pitch))
        );
    }

    #[test]
    fn range_speed_metronome_and_count_in() {
        let score = demo_score();
        let options = Options {
            from_bar: 1,
            to_bar: Some(2),
            metronome: true,
            count_in: true,
            speed: 0.5,
        };
        let t = compile(&score, 0, &options);
        // One count-in bar and one bar of music, each four seconds at half speed.
        assert_eq!(t.duration, 8.0);
        assert_eq!(t.clicks.len(), 8);
        assert!(t.clicks[0].accent && !t.clicks[1].accent);
        assert_eq!(t.clicks[5].time, 5.0);
        assert_eq!(t.beats[0].bar, 1);
        assert_eq!(t.beats[0].time, 4.0);
        assert_eq!(t.notes[0].time, 4.0);
    }

    #[test]
    fn ties_extend_the_previous_note() {
        let mut score = demo_score();
        let quarter = Duration::new(NoteValue::Quarter);
        let tied = Note {
            tied: true,
            ..Note::new(5, 3)
        };
        score.tracks[0].staves[0].bars[0].voices[0].beats = vec![
            Beat::new(quarter, vec![Note::new(5, 3)]),
            Beat::new(quarter, vec![tied]),
            // A tie with nothing sounding on the string starts a new note.
            Beat::rest(quarter),
            Beat::new(quarter, vec![tied]),
        ];
        // And a tie across the bar line.
        score.tracks[0].staves[0].bars[1].voices[0].beats = vec![Beat::new(quarter, vec![tied])];
        let t = compile(&score, 0, &Options::default());
        let notes: Vec<(f64, f64)> = t.notes.iter().map(|n| (n.time, n.duration)).collect();
        assert_eq!(notes, [(0.0, 1.0), (1.5, 1.0)]);
    }

    #[test]
    fn articulations_shape_the_event() {
        let score = quarters(vec![
            with(3, |e| e.palm_mute = true),
            with(3, |e| e.dead = true),
            with(3, |e| e.staccato = true),
            with(3, |e| {
                e.accent = Some(Accent::Heavy);
                e.ghost = true;
                e.vibrato = Some(Vibrato::Wide);
            }),
        ]);
        let t = compile(&score, 0, &Options::default());
        assert_eq!(t.notes[0].tone, Tone::PalmMute);
        assert_eq!(t.notes[1].tone, Tone::Dead);
        assert!(t.notes[1].duration <= 0.08);
        assert_eq!(t.notes[2].duration, 0.25);
        assert!((t.notes[3].velocity - 1.7 * 0.45).abs() < 1e-6);
        assert_eq!(t.notes[3].vibrato, 0.5);
    }

    #[test]
    fn hammer_ons_and_slides_join_notes() {
        let score = quarters(vec![
            with(5, |e| e.hammer = true),
            with(7, |e| e.slide = Some(Slide::Shift)),
            with(10, |e| e.slide = Some(Slide::Legato)),
            with(8, |_| {}),
        ]);
        let t = compile(&score, 0, &Options::default());
        let soft: Vec<bool> = t.notes.iter().map(|n| n.soft).collect();
        // The hammered note and the note reached by a legato slide are not picked.
        assert_eq!(soft, [false, true, false, true]);
        assert_eq!(t.notes[1].pitch_curve.last(), Some(&(0.5, 3.0)));
        assert_eq!(t.notes[2].pitch_curve.last(), Some(&(0.5, -2.0)));
        assert!(t.notes[0].pitch_curve.is_empty());
    }

    #[test]
    fn bends_and_harmonics_change_pitch() {
        let bend = |kind, quarter_tones| {
            Some(Bend {
                kind,
                quarter_tones,
            })
        };
        let score = quarters(vec![
            with(7, |e| e.bend = bend(BendKind::Bend, 4)),
            with(7, |e| e.bend = bend(BendKind::PrebendRelease, 2)),
            with(12, |e| e.harmonic = Some(Harmonic::Natural)),
            with(7, |e| e.harmonic = Some(Harmonic::Natural)),
        ]);
        let t = compile(&score, 0, &Options::default());
        assert_eq!(t.notes[0].pitch_curve, [(0.0, 0.0), (0.15, 2.0)]);
        assert_eq!(t.notes[1].pitch_curve[0], (0.0, 1.0));
        assert_eq!(t.notes[1].pitch_curve.last().unwrap().1, 0.0);
        // Harmonics over the 12th and 7th frets of the low E string: E3 and B3.
        assert_eq!((t.notes[2].pitch, t.notes[2].tone), (52, Tone::Harmonic));
        assert_eq!(t.notes[3].pitch, 59);
    }

    #[test]
    fn let_ring_sustains_until_the_string_is_played_again() {
        let mut score = quarters(vec![
            with(0, |e| e.let_ring = true),
            Note::new(4, 2),
            Note::new(4, 3),
            Note::new(5, 3),
        ]);
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .notes
            .push(Note::new(4, 0));
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .notes
            .sort_by_key(|n| n.string);
        let t = compile(&score, 0, &Options::default());
        let ringing = t.notes.iter().find(|n| n.pitch == 40).unwrap();
        // The open low E rings through two beats on the next string, until fret 3 replaces it.
        assert_eq!(ringing.duration, 1.5);
    }

    #[test]
    fn tremolo_picking_repeats_the_note() {
        let mut score = quarters(vec![Note::new(5, 5)]);
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .effects
            .tremolo = Some(NoteValue::Sixteenth);
        let t = compile(&score, 0, &Options::default());
        let times: Vec<f64> = t.notes.iter().map(|n| n.time).collect();
        assert_eq!(times, [0.0, 0.125, 0.25, 0.375]);
        assert!(t.notes.iter().all(|n| n.duration == 0.125));
    }
}
