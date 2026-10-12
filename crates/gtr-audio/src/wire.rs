//! A timeline as bytes, for handing to the synth on the audio thread. The format is private
//! to one build of the app: nothing written with it is kept.

use gtr_model::Fade;

use crate::{Automation, BeatTime, Channel, Click, NoteEvent, Timbre, Timeline, Tone};

/// First bytes of an encoded timeline.
const MAGIC: &[u8; 4] = b"GTL1";

const TIMBRES: [Timbre; 11] = [
    Timbre::Steel,
    Timbre::Nylon,
    Timbre::Clean,
    Timbre::Overdrive,
    Timbre::Distortion,
    Timbre::Bass,
    Timbre::Piano,
    Timbre::Drums,
    Timbre::Ukulele,
    Timbre::Banjo,
    Timbre::Mandolin,
];
const TONES: [Tone; 4] = [Tone::Normal, Tone::PalmMute, Tone::Dead, Tone::Harmonic];
const FADES: [Fade; 3] = [Fade::In, Fade::Out, Fade::Swell];

/// The place of `value` in `table`, as the byte that stands for it.
fn code<T: PartialEq>(table: &[T], value: T) -> u8 {
    table.iter().position(|v| *v == value).unwrap_or(0) as u8
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn count(&mut self, value: usize) {
        self.0.extend_from_slice(&(value as u32).to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|[byte]| byte)
    }

    fn count(&mut self) -> Option<usize> {
        self.take().map(|bytes| u32::from_le_bytes(bytes) as usize)
    }

    fn f32(&mut self) -> Option<f32> {
        self.take().map(f32::from_le_bytes)
    }

    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }

    /// Reads a count and then that many items. Fails if the bytes run out, without first
    /// setting aside room for a count that is not to be believed.
    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let count = self.count()?;
        let mut list = Vec::with_capacity(count.min(self.0.len()));
        for _ in 0..count {
            list.push(item(self)?);
        }
        Some(list)
    }
}

impl Timeline {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer(MAGIC.to_vec());
        w.f64(self.duration);
        w.f64(self.count_in);
        w.u8(self.looped.into());
        w.count(self.channels.len());
        for channel in &self.channels {
            w.u8(channel.audible.into());
            w.u8(code(&TIMBRES, channel.timbre));
            w.f32(channel.gain);
            w.f32(channel.pan);
        }
        w.count(self.automation.len());
        for change in &self.automation {
            w.count(change.channel);
            w.f64(change.time);
            w.u8(u8::from(change.gain.is_some()) | u8::from(change.pan.is_some()) << 1);
            w.f32(change.gain.unwrap_or(0.0));
            w.f32(change.pan.unwrap_or(0.0));
        }
        w.count(self.notes.len());
        for note in &self.notes {
            w.count(note.channel);
            w.f64(note.time);
            w.f64(note.duration);
            w.u8(note.pitch);
            w.u8(code(&TONES, note.tone));
            w.u8(note.soft.into());
            w.f32(note.velocity);
            w.f32(note.vibrato);
            w.u8(note.fade.map_or(0, |fade| 1 + code(&FADES, fade)));
            w.count(note.pitch_curve.len());
            for &(time, semitones) in &note.pitch_curve {
                w.f32(time);
                w.f32(semitones);
            }
        }
        w.count(self.clicks.len());
        for click in &self.clicks {
            w.f64(click.time);
            w.u8(click.accent.into());
        }
        w.count(self.beats.len());
        for beat in &self.beats {
            w.count(beat.bar);
            w.count(beat.beat);
            w.f64(beat.time);
        }
        w.0
    }

    /// Reads what [`Timeline::to_bytes`] wrote, or nothing if the bytes are not that.
    pub fn from_bytes(bytes: &[u8]) -> Option<Timeline> {
        let mut r = Reader(bytes);
        if &r.take::<4>()? != MAGIC {
            return None;
        }
        let duration = r.f64()?;
        let count_in = r.f64()?;
        let looped = r.u8()? != 0;
        let channels = r.list(|r| {
            Some(Channel {
                audible: r.u8()? != 0,
                timbre: *TIMBRES.get(usize::from(r.u8()?))?,
                gain: r.f32()?,
                pan: r.f32()?,
            })
        })?;
        let automation = r.list(|r| {
            let (channel, time, set) = (r.count()?, r.f64()?, r.u8()?);
            let (gain, pan) = (r.f32()?, r.f32()?);
            Some(Automation {
                channel,
                time,
                gain: (set & 1 != 0).then_some(gain),
                pan: (set & 2 != 0).then_some(pan),
            })
        })?;
        let notes = r.list(|r| {
            Some(NoteEvent {
                channel: r.count()?,
                time: r.f64()?,
                duration: r.f64()?,
                pitch: r.u8()?,
                tone: *TONES.get(usize::from(r.u8()?))?,
                soft: r.u8()? != 0,
                velocity: r.f32()?,
                vibrato: r.f32()?,
                fade: match r.u8()? {
                    0 => None,
                    kind => Some(*FADES.get(usize::from(kind - 1))?),
                },
                pitch_curve: r.list(|r| Some((r.f32()?, r.f32()?)))?,
            })
        })?;
        let clicks = r.list(|r| {
            Some(Click {
                time: r.f64()?,
                accent: r.u8()? != 0,
            })
        })?;
        let beats = r.list(|r| {
            Some(BeatTime {
                bar: r.count()?,
                beat: r.count()?,
                time: r.f64()?,
            })
        })?;
        r.0.is_empty().then_some(Timeline {
            channels,
            automation,
            notes,
            clicks,
            beats,
            count_in,
            duration,
            looped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeline_survives_the_trip() {
        let mut note = NoteEvent::plain(0.5, 0.25, 64);
        note.channel = 1;
        note.tone = Tone::Harmonic;
        note.soft = true;
        note.velocity = 1.3;
        note.vibrato = 0.5;
        note.pitch_curve = vec![(0.0, 0.0), (0.1, 2.0)];
        let timeline = Timeline {
            channels: TIMBRES
                .iter()
                .map(|&timbre| Channel {
                    audible: timbre != Timbre::Bass,
                    gain: 0.7,
                    pan: -0.25,
                    timbre,
                })
                .collect(),
            automation: vec![
                Automation {
                    channel: 1,
                    time: 0.5,
                    gain: Some(0.0),
                    pan: None,
                },
                Automation {
                    channel: 0,
                    time: 1.0,
                    gain: None,
                    pan: Some(1.0),
                },
            ],
            notes: (TONES.iter())
                .map(|&tone| NoteEvent {
                    tone,
                    ..NoteEvent::plain(0.0, 1.0, 40)
                })
                .chain([note])
                .collect(),
            clicks: vec![Click {
                time: 0.0,
                accent: true,
            }],
            beats: vec![BeatTime {
                bar: 3,
                beat: 2,
                time: 1.5,
            }],
            count_in: 0.5,
            duration: 2.0,
            looped: true,
        };
        let bytes = timeline.to_bytes();
        assert_eq!(Timeline::from_bytes(&bytes), Some(timeline));
        assert_eq!(
            Timeline::from_bytes(&Timeline::default().to_bytes()),
            Some(Timeline::default())
        );
    }

    #[test]
    fn bytes_that_are_not_a_timeline_are_refused() {
        let bytes = Timeline {
            notes: vec![NoteEvent::plain(0.0, 1.0, 40)],
            ..Timeline::default()
        }
        .to_bytes();
        assert_eq!(Timeline::from_bytes(b""), None);
        assert_eq!(Timeline::from_bytes(b"nope"), None);
        // Cut short, with something left over, or claiming more notes than there are.
        assert_eq!(Timeline::from_bytes(&bytes[..bytes.len() - 1]), None);
        assert_eq!(Timeline::from_bytes(&[&bytes[..], &[0]].concat()), None);
        let mut lying = bytes.clone();
        lying[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Timeline::from_bytes(&lying), None);
    }
}
