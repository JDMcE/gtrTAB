use gtr_model::{Rational, Score, Staff};
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

/// A sounding note. Tied notes are merged into the note they extend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NoteEvent {
    /// Seconds from the start of playback.
    pub time: f64,
    pub duration: f64,
    /// MIDI pitch.
    pub pitch: u8,
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

fn add_staff(
    timeline: &mut Timeline,
    staff: &Staff,
    from_bar: usize,
    starts: &[f64],
    followed: bool,
    seconds: &impl Fn(Rational) -> f64,
) {
    let bars = &staff.bars[from_bar..from_bar + starts.len() - 1];
    // Index of the note still sounding on each string, so that ties can extend it.
    let mut sounding: Vec<Option<usize>> = vec![None; staff.tuning.strings.len()];
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
                if v != 0 {
                    // Other voices sound, but do not take part in tie tracking.
                    for note in &beat.notes {
                        if let Some(pitch) = staff.tuning.pitch(note.string, note.fret, staff.capo)
                        {
                            timeline.notes.push(NoteEvent {
                                time,
                                duration,
                                pitch,
                            });
                        }
                    }
                    continue;
                }
                let mut next = vec![None; sounding.len()];
                for note in &beat.notes {
                    let string = usize::from(note.string);
                    let Some(pitch) = staff.tuning.pitch(note.string, note.fret, staff.capo) else {
                        continue;
                    };
                    next[string] = match sounding[string] {
                        Some(previous) if note.tied => {
                            let event = &mut timeline.notes[previous];
                            event.duration = time + duration - event.time;
                            Some(previous)
                        }
                        _ => {
                            timeline.notes.push(NoteEvent {
                                time,
                                duration,
                                pitch,
                            });
                            Some(timeline.notes.len() - 1)
                        }
                    };
                }
                sounding = next;
            }
        }
        // A tie only reaches into the next bar from the bar's last beat, which `sounding`
        // already reflects; an empty bar breaks it.
        if bar.voices.first().is_none_or(|v| v.beats.is_empty()) {
            sounding.fill(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use gtr_model::{Beat, Duration, Note, NoteValue, demo_score};

    use super::*;

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
            string: 5,
            fret: 3,
            tied: true,
        };
        score.tracks[0].staves[0].bars[0].voices[0].beats = vec![
            Beat {
                duration: quarter,
                notes: vec![Note::new(5, 3)],
            },
            Beat {
                duration: quarter,
                notes: vec![tied],
            },
            // A tie with nothing sounding on the string starts a new note.
            Beat::rest(quarter),
            Beat {
                duration: quarter,
                notes: vec![tied],
            },
        ];
        // And a tie across the bar line.
        score.tracks[0].staves[0].bars[1].voices[0].beats = vec![Beat {
            duration: quarter,
            notes: vec![tied],
        }];
        let t = compile(&score, 0, &Options::default());
        let notes: Vec<(f64, f64)> = t.notes.iter().map(|n| (n.time, n.duration)).collect();
        assert_eq!(notes, [(0.0, 1.0), (1.5, 1.0)]);
    }
}
