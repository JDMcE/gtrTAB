use crate::{
    Bar, Beat, Duration, MasterBar, Note, NoteValue, Score, ScoreInfo, Staff, TimeSignature, Track,
    Tuning, Voice,
};

/// A two-bar riff in E minor, used as the starting document until real scores can be written.
pub fn demo_score() -> Score {
    let eighth = Duration::new(NoteValue::Eighth);
    let note = |string, fret| Beat {
        duration: eighth,
        notes: vec![Note::new(string, fret)],
    };

    let bar1 = [
        (5, 0),
        (5, 0),
        (4, 2),
        (5, 0),
        (4, 3),
        (5, 0),
        (4, 2),
        (5, 0),
    ];
    let mut bar2: Vec<Beat> = [(5, 0), (5, 0), (4, 2), (5, 0)]
        .into_iter()
        .map(|(s, f)| note(s, f))
        .collect();
    bar2.push(Beat {
        duration: Duration::new(NoteValue::Half),
        notes: vec![Note::new(3, 2), Note::new(4, 2), Note::new(5, 0)],
    });

    Score {
        info: ScoreInfo {
            title: "Demo riff".into(),
            ..ScoreInfo::default()
        },
        tempo: 120,
        master_bars: vec![
            MasterBar {
                time_signature: TimeSignature::COMMON,
            };
            2
        ],
        tracks: vec![Track {
            name: "Guitar".into(),
            staves: vec![Staff {
                tuning: Tuning::guitar_standard(),
                capo: 0,
                bars: vec![
                    Bar {
                        voices: vec![Voice {
                            beats: bar1.into_iter().map(|(s, f)| note(s, f)).collect(),
                        }],
                    },
                    Bar {
                        voices: vec![Voice { beats: bar2 }],
                    },
                ],
            }],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_is_valid_and_bars_are_full() {
        let score = demo_score();
        score.validate().unwrap();
        for (master, bar) in score
            .master_bars
            .iter()
            .zip(&score.tracks[0].staves[0].bars)
        {
            assert_eq!(bar.voices[0].length(), master.time_signature.bar_length());
        }
    }
}
