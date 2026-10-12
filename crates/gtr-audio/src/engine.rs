//! The streaming synth: plays a timeline a block of samples at a time, so that it can run
//! in an audio callback, take a new timeline while it plays, and be mixed as it goes.

use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::rc::Rc;

use gtr_model::Fade;

use crate::soundfont::{Playing, SampleVoice};
use crate::voice::{ClickVoice, Voice, shape};
use crate::{Automation, BeatTime, Channel, SoundFont, Timbre, Timeline};

/// Seconds of silence after the last bar so the last notes can die away.
const TAIL: f64 = 0.3;
/// Seconds a change to the mixer takes, so that it does not click.
const MIX_GLIDE: f32 = 0.015;

/// Two channels of samples in `-1.0..=1.0`, of equal length.
#[derive(Debug, Clone, PartialEq)]
pub struct Stereo {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Gains for the two sides. A centred track plays at full level on both, and one panned
/// all the way sounds as loud from its one side.
fn pan_gains(gain: f32, pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (gain * angle.cos() * SQRT_2, gain * angle.sin() * SQRT_2)
}

/// Moves a gain a step closer to where it is heading, landing on it once the difference
/// is too small to hear.
fn toward(current: f32, target: f32, glide: f32) -> f32 {
    let off = target - current;
    if off.abs() < 1e-4 {
        target
    } else {
        current + glide * off
    }
}

/// One track's place in the mix while it plays.
struct Strip {
    /// What the mixer asks for.
    mixer: Channel,
    /// What the score last changed the volume and pan to. These are measured from the
    /// mixer's settings, so that moving a slider moves the whole passage with it.
    scored_gain: Option<f32>,
    scored_pan: Option<f32>,
    /// Gains for the two sides: where they are heading and where they have got to.
    target: (f32, f32),
    current: (f32, f32),
    /// The amplifier's filter, carried from block to block.
    amp: f32,
    /// The track's samples for the block being made: what its modelled voices make, which
    /// goes through the amplifier, and the two sides of what its sampled voices make.
    bus: Vec<f32>,
    sides: (Vec<f32>, Vec<f32>),
}

impl Strip {
    fn new(mixer: Channel) -> Strip {
        let mut strip = Strip {
            mixer,
            scored_gain: None,
            scored_pan: None,
            target: (0.0, 0.0),
            current: (0.0, 0.0),
            amp: 0.0,
            bus: Vec::new(),
            sides: (Vec::new(), Vec::new()),
        };
        strip.aim();
        strip.current = strip.target;
        strip
    }

    /// Works out the gains the strip should head for.
    fn aim(&mut self) {
        self.target = match self.mixer.audible {
            true => pan_gains(
                self.mixer.gain * self.scored_gain.unwrap_or(1.0),
                self.mixer.pan + self.scored_pan.unwrap_or(0.0),
            ),
            false => (0.0, 0.0),
        };
    }

    /// Takes a change written in the score, which happens on its beat, at once.
    fn score(&mut self, change: &Automation) {
        self.scored_gain = change.gain.or(self.scored_gain);
        self.scored_pan = change.pan.or(self.scored_pan);
        self.aim();
        self.current = self.target;
    }
}

/// A voice of a track, and how many samples it is from starting.
struct Sounding {
    channel: usize,
    wait: usize,
    voice: Voice,
    /// A fade over the voice: its kind, the samples it takes, and how far through it is.
    fade: Option<(Fade, usize, usize)>,
}

/// How loud a fading note is, `through` its fade from 0.0 to 1.0. A fade in stays up
/// once it is there, for as long as the note rings on.
fn fade_gain(fade: Fade, through: f32) -> f32 {
    let through = through.clamp(0.0, 1.0);
    match fade {
        Fade::In => through,
        Fade::Out => 1.0 - through,
        Fade::Swell => 1.0 - (2.0 * through - 1.0).abs(),
    }
}

/// Plays a [`Timeline`] block by block.
pub struct Engine {
    sample_rate: u32,
    timeline: Timeline,
    /// Samples played since the start of the timeline, and the number it has in all.
    position: usize,
    total: usize,
    /// The first note, click and change of the timeline still to come.
    next_note: usize,
    next_click: usize,
    next_change: usize,
    voices: Vec<Sounding>,
    clicks: Vec<(usize, ClickVoice)>,
    click_bus: Vec<f32>,
    /// Where a fading voice is made before it is added to its track.
    fade_bus: [Vec<f32>; 3],
    strips: Vec<Strip>,
    /// The samples notes are played from, where the bank has their instrument.
    font: Option<Rc<SoundFont>>,
}

impl Engine {
    /// An engine with nothing to play.
    pub fn new(sample_rate: u32) -> Engine {
        Engine {
            sample_rate,
            timeline: Timeline::default(),
            position: 0,
            total: 0,
            next_note: 0,
            next_click: 0,
            next_change: 0,
            voices: Vec::new(),
            clicks: Vec::new(),
            click_bus: Vec::new(),
            fade_bus: Default::default(),
            strips: Vec::new(),
            font: None,
        }
    }

    fn sample(&self, seconds: f64) -> usize {
        (seconds * f64::from(self.sample_rate)) as usize
    }

    /// Plays from the samples of `font` from the next note on, or with `None` from the
    /// built-in models again. Notes already sounding ring out as they were.
    pub fn set_soundfont(&mut self, font: Option<SoundFont>) {
        self.font = font.map(Rc::new);
    }

    /// Starts playing `timeline` from its beginning, dropping whatever was sounding.
    pub fn load(&mut self, timeline: Timeline) {
        self.voices.clear();
        self.clicks.clear();
        self.strips.clear();
        self.set_timeline(timeline, 0.0);
    }

    /// Swaps in a newer version of the passage being played and carries on from the same
    /// beat of it, so that an edit is heard without stopping. Notes already sounding ring
    /// out as they were.
    pub fn replace(&mut self, timeline: Timeline) {
        let now = self.seconds();
        let at = if now < self.timeline.count_in {
            // As far through the count-in, which a change of speed makes longer or shorter.
            now * timeline.count_in / self.timeline.count_in
        } else {
            same_place(&self.timeline.beats, &timeline.beats, now)
        };
        self.set_timeline(timeline, at);
    }

    fn set_timeline(&mut self, timeline: Timeline, at: f64) {
        self.timeline = timeline;
        self.total =
            ((self.timeline.duration + TAIL) * f64::from(self.sample_rate)).ceil() as usize;
        self.position = self.sample(at.max(0.0)).min(self.total);

        let rate = f64::from(self.sample_rate);
        let before = |seconds: f64| ((seconds * rate) as usize) < self.position;
        self.next_note = self.timeline.notes.partition_point(|n| before(n.time));
        self.next_click = self.timeline.clicks.partition_point(|c| before(c.time));
        self.next_change = (self.timeline.automation).partition_point(|c| before(c.time));

        // A track keeps its strip, so that a change to its mix glides instead of jumping.
        let count = (self.timeline.notes.iter())
            .map(|n| n.channel + 1)
            .max()
            .unwrap_or(0)
            .max(self.timeline.channels.len());
        self.strips.truncate(count);
        for index in 0..count {
            let mixer = (self.timeline.channels.get(index).copied()).unwrap_or_default();
            match self.strips.get_mut(index) {
                Some(strip) => {
                    strip.mixer = mixer;
                    (strip.scored_gain, strip.scored_pan) = (None, None);
                }
                None => self.strips.push(Strip::new(mixer)),
            }
        }
        // What the score has changed by this point in the passage still holds.
        for change in &self.timeline.automation[..self.next_change] {
            if let Some(strip) = self.strips.get_mut(change.channel) {
                strip.scored_gain = change.gain.or(strip.scored_gain);
                strip.scored_pan = change.pan.or(strip.scored_pan);
            }
        }
        for strip in &mut self.strips {
            strip.aim();
        }
    }

    /// Moves a track in the mix while it plays. Ignored for a track the passage lacks.
    pub fn set_mix(&mut self, channel: usize, audible: bool, gain: f32, pan: f32) {
        if let Some(strip) = self.strips.get_mut(channel) {
            strip.mixer = Channel {
                audible,
                gain,
                pan,
                ..strip.mixer
            };
            strip.aim();
        }
    }

    /// Seconds played since the start of the timeline.
    pub fn seconds(&self) -> f64 {
        self.position as f64 / f64::from(self.sample_rate)
    }

    /// Samples the passage and its dying notes come to in all.
    pub fn length(&self) -> usize {
        self.total
    }

    /// True once the passage and its dying notes have all been played.
    pub fn finished(&self) -> bool {
        self.position >= self.total
    }

    /// Writes the next samples to `left` and `right`, which are filled with silence past
    /// the end of the passage.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let frames = (left.len().min(right.len())).min(self.total - self.position);
        if frames == 0 {
            return;
        }
        let rate = self.sample_rate as f32;
        let end = self.position + frames;
        for strip in &mut self.strips {
            for bus in [&mut strip.bus, &mut strip.sides.0, &mut strip.sides.1] {
                bus.clear();
                bus.resize(frames, 0.0);
            }
        }

        // Notes that begin in this block.
        while let Some(note) = self.timeline.notes.get(self.next_note) {
            let start = self.sample(note.time);
            if start >= end {
                break;
            }
            let seed = self.next_note as u32;
            self.next_note += 1;
            // A note from before the place playback was moved to is not started half-way.
            if start < self.position {
                continue;
            }
            let timbre = (self.timeline.channels.get(note.channel)).map(|c| c.timbre);
            let timbre = timbre.unwrap_or_default();
            let limit = self.total - start;
            // From the SoundFont if it has the instrument and a sample for the note, and
            // from the instrument's model if not.
            let sampled = match (&self.font, timbre.preset()) {
                (Some(font), Some(preset)) => {
                    let playing = Playing {
                        held: timbre != Timbre::Drums,
                        trim: timbre.sample_trim(),
                        rate,
                        limit,
                    };
                    SampleVoice::start(font, preset, note, playing)
                }
                _ => Vec::new(),
            };
            let voices: Vec<Voice> = match sampled.is_empty() {
                true => (Voice::new(note, timbre, rate, limit, seed).into_iter()).collect(),
                false => (sampled.into_iter().map(Box::new).map(Voice::Sampled)).collect(),
            };
            for voice in voices {
                // A closed hi-hat stops the open one, and the like.
                if let Voice::Sampled(new) = &voice
                    && new.class() != 0
                {
                    for sounding in &mut self.voices {
                        if let Voice::Sampled(old) = &mut sounding.voice
                            && sounding.channel == note.channel
                            && sounding.wait == 0
                            && old.class() == new.class()
                        {
                            old.choke();
                        }
                    }
                }
                let length = ((note.duration * f64::from(rate)) as usize).max(1);
                self.voices.push(Sounding {
                    channel: note.channel,
                    wait: start - self.position,
                    voice,
                    fade: note.fade.map(|fade| (fade, length, 0)),
                });
            }
        }
        let strips = &mut self.strips;
        let scratch = &mut self.fade_bus;
        self.voices.retain_mut(|sounding| {
            let from = sounding.wait.min(frames);
            sounding.wait -= from;
            let strip = &mut strips[sounding.channel];
            let (left, right) = (&mut strip.sides.0, &mut strip.sides.1);
            let Some((fade, length, at)) = &mut sounding.fade else {
                return (sounding.voice).render(
                    &mut strip.bus[from..],
                    &mut left[from..],
                    &mut right[from..],
                );
            };
            // A fading voice is made by itself, and added at the level it has got to.
            for side in scratch.iter_mut() {
                side.clear();
                side.resize(frames - from, 0.0);
            }
            let [bus, own_left, own_right] = scratch;
            let alive = sounding.voice.render(bus, own_left, own_right);
            for i in 0..frames - from {
                let gain = fade_gain(*fade, (*at + i) as f32 / *length as f32);
                strip.bus[from + i] += bus[i] * gain;
                left[from + i] += own_left[i] * gain;
                right[from + i] += own_right[i] * gain;
            }
            *at += frames - from;
            // Once it has faded out there is nothing more to hear of it.
            alive && !(matches!(fade, Fade::Out | Fade::Swell) && *at >= *length)
        });

        // Each track through its amplifier and into its place, which the score can move
        // at any beat.
        let changes = &self.timeline.automation[self.next_change..];
        let changes = &changes[..changes.partition_point(|c| self.sample(c.time) < end)];
        let glide = 1.0 - (-1.0 / (MIX_GLIDE * rate)).exp();
        for (index, strip) in self.strips.iter_mut().enumerate() {
            let timbre = self.timeline.channels.get(index).map(|c| c.timbre);
            shape(
                &mut strip.bus,
                timbre.unwrap_or_default(),
                rate,
                &mut strip.amp,
            );
            let mut pending = (changes.iter())
                .filter(|change| change.channel == index)
                .map(|change| ((change.time * f64::from(self.sample_rate)) as usize, change))
                .peekable();
            for i in 0..frames {
                while let Some((_, change)) = pending.next_if(|(at, _)| *at <= self.position + i) {
                    strip.score(change);
                }
                strip.current.0 = toward(strip.current.0, strip.target.0, glide);
                strip.current.1 = toward(strip.current.1, strip.target.1, glide);
                left[i] += strip.current.0 * (strip.bus[i] + strip.sides.0[i]);
                right[i] += strip.current.1 * (strip.bus[i] + strip.sides.1[i]);
            }
        }
        self.next_change += changes.len();

        // The metronome, on top of the mix.
        while let Some(click) = self.timeline.clicks.get(self.next_click) {
            let start = self.sample(click.time);
            if start >= end {
                break;
            }
            self.next_click += 1;
            let voice = ClickVoice::new(click.accent, rate);
            self.clicks.push((start - self.position, voice));
        }
        if !self.clicks.is_empty() {
            self.click_bus.clear();
            self.click_bus.resize(frames, 0.0);
            let bus = &mut self.click_bus;
            self.clicks.retain_mut(|(wait, voice)| {
                let from = (*wait).min(frames);
                *wait -= from;
                voice.render(&mut bus[from..])
            });
            for (i, sample) in self.click_bus.iter().enumerate() {
                left[i] += sample;
                right[i] += sample;
            }
        }

        // A soft limiter, so that loud passages round off instead of clipping.
        for sample in left[..frames].iter_mut().chain(&mut right[..frames]) {
            *sample = sample.tanh();
        }
        self.position = end;
    }
}

/// Where in the `new` version of a passage playback should carry on from, having reached
/// `now` seconds into the `old` one: as far past the same beat. Where the beat sounding is
/// no longer there, the time itself is kept.
fn same_place(old: &[BeatTime], new: &[BeatTime], now: f64) -> f64 {
    let Some(last) = old.partition_point(|b| b.time <= now).checked_sub(1) else {
        return now;
    };
    let beat = old[last];
    let same = |b: &&BeatTime| b.bar == beat.bar && b.beat == beat.beat;
    // A beat inside a repeat is played more than once; stay in the same pass.
    let pass = old[..last].iter().filter(same).count();
    let mut found = new.iter().filter(same);
    match found.clone().nth(pass).or(found.next_back()) {
        Some(there) => there.time + (now - beat.time),
        None => now,
    }
}

/// Renders a whole timeline to stereo: every audible track placed by its volume and pan,
/// with the clicks on top.
pub fn render(timeline: &Timeline, sample_rate: u32) -> Stereo {
    let mut engine = Engine::new(sample_rate);
    engine.load(timeline.clone());
    let mut left = vec![0.0f32; engine.total];
    let mut right = vec![0.0f32; engine.total];
    engine.process(&mut left, &mut right);
    Stereo { left, right }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoteEvent, Timbre, Tone};

    fn timeline_of(notes: Vec<NoteEvent>, channels: Vec<Channel>, duration: f64) -> Timeline {
        Timeline {
            channels,
            notes,
            duration,
            ..Timeline::default()
        }
    }

    fn render_with(note: NoteEvent, channel: Channel) -> Stereo {
        let duration = note.duration;
        render(&timeline_of(vec![note], vec![channel], duration), 44_100)
    }

    fn render_one(note: NoteEvent) -> Vec<f32> {
        let stereo = render_with(note, Channel::default());
        assert_eq!(stereo.left, stereo.right);
        stereo.left
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|s| s * s).sum()
    }

    #[test]
    fn a_fade_brings_a_note_up_takes_it_down_or_both() {
        let note = |fade| NoteEvent {
            fade,
            ..NoteEvent::plain(0.0, 1.0, 52)
        };
        let plain = render_one(note(None));
        let quarters = |samples: &[f32]| {
            let quarter = 44_100 / 4;
            [0, 1, 2, 3].map(|q| energy(&samples[q * quarter..(q + 1) * quarter]))
        };
        let as_written = quarters(&plain);
        // Faded in, the pick is not heard: the first quarter of the note is far under
        // what it was, and the last is as it was.
        let faded_in = quarters(&render_one(note(Some(Fade::In))));
        assert!(faded_in[0] < as_written[0] * 0.1);
        assert!(faded_in[3] > as_written[3] * 0.5);
        // Faded out, it starts as it was and is gone by the end.
        let faded_out = quarters(&render_one(note(Some(Fade::Out))));
        assert!(faded_out[0] > as_written[0] * 0.5);
        assert!(faded_out[3] < as_written[3] * 0.1);
        // A swell is quiet at both ends.
        let swell = quarters(&render_one(note(Some(Fade::Swell))));
        assert!(swell[0] < as_written[0] * 0.2 && swell[3] < as_written[3] * 0.2);
        assert!(swell[1] > swell[0] && swell[1] > swell[3]);

        // It makes no difference how the sound is asked for, in one piece or in blocks.
        let timeline = timeline_of(vec![note(Some(Fade::Swell))], vec![Channel::default()], 1.0);
        let whole = render(&timeline, 44_100);
        let mut engine = Engine::new(44_100);
        engine.load(timeline);
        let blocks = play_out(&mut engine, 333);
        assert_eq!(whole.left[..40_000], blocks.left[..40_000]);
        assert!(
            [Fade::In, Fade::Out, Fade::Swell]
                .iter()
                .all(|&fade| (0.0..=1.0).contains(&fade_gain(fade, 0.3)))
        );
    }

    /// Plays the engine to the end in blocks of `block` samples.
    fn play_out(engine: &mut Engine, block: usize) -> Stereo {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
        while !engine.finished() {
            engine.process(&mut l, &mut r);
            left.extend_from_slice(&l);
            right.extend_from_slice(&r);
        }
        Stereo { left, right }
    }

    /// A passage with something of everything: two tracks, a bend, drums, clicks and a
    /// change of pan.
    fn busy() -> Timeline {
        let mut bent = NoteEvent::plain(0.1, 0.6, 64);
        bent.pitch_curve = vec![(0.0, 0.0), (0.2, 2.0)];
        bent.vibrato = 0.3;
        let mut snare = NoteEvent::plain(0.25, 0.1, 38);
        snare.channel = 1;
        let mut kick = NoteEvent::plain(0.5, 0.1, 36);
        kick.channel = 1;
        Timeline {
            channels: vec![
                Channel {
                    timbre: Timbre::Distortion,
                    ..Channel::default()
                },
                Channel {
                    timbre: Timbre::Drums,
                    pan: 0.3,
                    ..Channel::default()
                },
            ],
            automation: vec![Automation {
                channel: 0,
                time: 0.4,
                gain: Some(0.5),
                pan: Some(-0.5),
            }],
            notes: vec![NoteEvent::plain(0.0, 0.5, 45), bent, snare, kick],
            clicks: vec![
                crate::Click {
                    time: 0.0,
                    accent: true,
                },
                crate::Click {
                    time: 0.5,
                    accent: false,
                },
            ],
            beats: Vec::new(),
            count_in: 0.0,
            duration: 1.0,
        }
    }

    #[test]
    fn playing_in_blocks_gives_the_same_samples_as_rendering_at_once() {
        let timeline = busy();
        let whole = render(&timeline, 44_100);
        assert!(energy(&whole.left) > 1.0 && whole.left != whole.right);
        for block in [128, 1_000] {
            let mut engine = Engine::new(44_100);
            engine.load(timeline.clone());
            let played = play_out(&mut engine, block);
            assert_eq!(played.left[..whole.left.len()], whole.left[..]);
            assert_eq!(played.right[..whole.right.len()], whole.right[..]);
            // Past the end there is only silence.
            assert!(played.left[whole.left.len()..].iter().all(|&s| s == 0.0));
        }
    }

    #[test]
    fn the_mixer_moves_a_track_while_it_plays() {
        let timeline = timeline_of(
            vec![NoteEvent::plain(0.0, 2.0, 45)],
            vec![Channel::default()],
            2.0,
        );
        let mut engine = Engine::new(44_100);
        engine.load(timeline);
        let (mut left, mut right) = (vec![0.0; 22_050], vec![0.0; 22_050]);
        engine.process(&mut left, &mut right);
        assert!(energy(&right) > 1.0);
        // Hard left: the right side glides to nothing, without a jump.
        engine.set_mix(0, true, 1.0, -1.0);
        engine.process(&mut left, &mut right);
        assert!(right[0].abs() > 1e-4);
        assert_eq!(energy(&right[12_000..]), 0.0);
        assert!(energy(&left[12_000..]) > 0.1);
        // Muted: nothing on either side. A track the passage lacks is ignored.
        engine.set_mix(0, false, 1.0, -1.0);
        engine.set_mix(7, true, 1.0, 0.0);
        engine.process(&mut left, &mut right);
        assert_eq!(energy(&left[12_000..]), 0.0);
    }

    #[test]
    fn a_new_timeline_carries_on_from_the_same_beat() {
        let beat = |bar, beat, time| BeatTime { bar, beat, time };
        // Two bars played twice over, then the same at half the tempo.
        let old = [
            beat(0, 0, 0.0),
            beat(1, 0, 1.0),
            beat(0, 0, 2.0),
            beat(1, 0, 3.0),
        ];
        let slow: Vec<_> = (old.iter())
            .map(|b| beat(b.bar, b.beat, b.time * 2.0))
            .collect();
        assert_eq!(same_place(&old, &slow, 1.25), 2.25);
        // The second time through stays the second time through.
        assert_eq!(same_place(&old, &slow, 3.5), 6.5);
        // Without the repeat, the nearest there is; with the beat gone, the same time.
        assert_eq!(same_place(&old, &slow[..2], 3.5), 2.5);
        assert_eq!(same_place(&old, &[beat(4, 0, 0.0)], 3.5), 3.5);
        assert_eq!(same_place(&[beat(0, 0, 0.5)], &slow, 0.25), 0.25);

        // In the engine: a note added ahead of the playback position is heard, and the
        // one already sounding rings on.
        let mut timeline = timeline_of(
            vec![NoteEvent::plain(0.0, 0.4, 45)],
            vec![Channel::default()],
            1.0,
        );
        timeline.beats = vec![beat(0, 0, 0.0), beat(0, 1, 0.5)];
        let mut engine = Engine::new(44_100);
        engine.load(timeline.clone());
        let (mut left, mut right) = (vec![0.0; 11_025], vec![0.0; 11_025]);
        engine.process(&mut left, &mut right);
        timeline.notes.push(NoteEvent::plain(0.5, 0.4, 52));
        engine.replace(timeline);
        assert_eq!(engine.seconds(), 0.25);
        engine.process(&mut left, &mut right);
        assert!(energy(&left[..4_000]) > 0.1);
        engine.process(&mut left, &mut right);
        assert!(energy(&left[..4_000]) > 0.1);
        assert!(!engine.finished());
    }

    #[test]
    fn a_change_of_speed_in_the_count_in_stays_as_far_through_it() {
        let passage = |count_in: f64| Timeline {
            beats: vec![BeatTime {
                bar: 0,
                beat: 0,
                time: count_in,
            }],
            count_in,
            duration: count_in * 2.0,
            ..Timeline::default()
        };
        // Half a second into a count-in of two seconds, and then of four and of one.
        let mut engine = Engine::new(44_100);
        engine.load(passage(2.0));
        let (mut left, mut right) = (vec![0.0; 22_050], vec![0.0; 22_050]);
        engine.process(&mut left, &mut right);
        engine.replace(passage(4.0));
        assert_eq!(engine.seconds(), 1.0);
        engine.replace(passage(1.0));
        assert_eq!(engine.seconds(), 0.25);
    }

    #[test]
    fn a_soundfont_plays_the_instruments_it_has_and_the_models_play_the_rest() {
        use crate::soundfont::testing::{looped, sound_font};

        // A bank with a steel guitar and a drum kit whose two pieces cut each other off.
        let piece = |key: i16| {
            let mut zone = looped();
            zone.splice(0..0, [(43, (key << 8) | key), (57, 1)]);
            zone
        };
        let bytes = sound_font(
            100,
            &[
                (0, 25, vec![looped()]),
                (128, 0, vec![piece(42), piece(46)]),
            ],
        );
        let channel = |timbre| Channel {
            timbre,
            ..Channel::default()
        };
        let on = |channel: usize, time: f64, pitch: u8| NoteEvent {
            channel,
            ..NoteEvent::plain(time, 0.5, pitch)
        };
        let timeline = timeline_of(
            vec![
                on(0, 0.0, 69),
                on(1, 0.0, 69),
                on(2, 0.0, 46),
                on(2, 0.25, 42),
            ],
            vec![
                channel(Timbre::Steel),
                channel(Timbre::Ukulele),
                channel(Timbre::Drums),
            ],
            1.0,
        );
        let alone = |timeline: &Timeline, index: usize, font: bool| {
            let mut timeline = timeline.clone();
            for (i, channel) in timeline.channels.iter_mut().enumerate() {
                channel.audible = i == index;
            }
            let mut engine = Engine::new(44_100);
            engine.set_soundfont(font.then(|| SoundFont::parse(&bytes).unwrap()));
            engine.load(timeline);
            play_out(&mut engine, 512).left
        };
        // The guitar is the bank's sine wave now: the same from one period to the next,
        // which a plucked string is not.
        let sampled = alone(&timeline, 0, true);
        let drift = (sampled[4_000..4_400].iter().zip(&sampled[4_100..4_500]))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(energy(&sampled[..20_000]) > 1.0 && drift < 1e-3);
        assert!(sampled != alone(&timeline, 0, false));
        // The bank has no ukulele, so that is the model's either way.
        assert_eq!(alone(&timeline, 1, true), alone(&timeline, 1, false));
        // The open hi-hat rings until the closed one, a quarter of a second in, stops it:
        // from then on the kit sounds as it would had the open one never been hit.
        let kit = alone(&timeline, 2, true);
        assert!(energy(&kit[9_000..11_000]) > 0.1);
        let mut closed_only = timeline.clone();
        closed_only.notes.remove(2);
        let closed_only = alone(&closed_only, 2, true);
        assert!(energy(&closed_only[13_000..15_000]) > 0.1);
        for i in 13_000..20_000 {
            assert!(
                (kit[i] - closed_only[i]).abs() < 1e-3,
                "the open hi-hat still sounds"
            );
        }
    }

    #[test]
    fn volume_and_pan_place_a_track() {
        let note = NoteEvent::plain(0.0, 0.5, 45);
        let centre = energy(&render_one(note.clone()));

        let hard_left = Channel {
            pan: -1.0,
            ..Channel::default()
        };
        let panned = render_with(note.clone(), hard_left);
        assert!(energy(&panned.right) < centre * 1e-6);
        assert!(energy(&panned.left) > centre);

        let quiet = Channel {
            gain: 0.5,
            ..Channel::default()
        };
        let quiet = energy(&render_with(note.clone(), quiet).left);
        assert!(quiet < centre * 0.3 && quiet > centre * 0.2);

        let silent = Channel {
            gain: 0.0,
            ..Channel::default()
        };
        assert_eq!(energy(&render_with(note.clone(), silent).left), 0.0);
        let muted = Channel {
            audible: false,
            ..Channel::default()
        };
        assert_eq!(energy(&render_with(note, muted).left), 0.0);
    }

    #[test]
    fn automation_moves_a_track_part_way_through() {
        let mut timeline = timeline_of(
            vec![NoteEvent::plain(0.0, 1.0, 45)],
            vec![Channel::default()],
            1.0,
        );
        timeline.automation = vec![Automation {
            channel: 0,
            time: 0.5,
            gain: None,
            pan: Some(-1.0),
        }];
        let mix = render(&timeline, 44_100);
        // Centred for the first half second, then hard left.
        assert!(energy(&mix.right[..20_000]) > 1.0);
        assert_eq!(energy(&mix.right[23_000..]), 0.0);
        assert!(energy(&mix.left[23_000..40_000]) > 0.1);
        // And a change of volume to nothing silences what is left.
        timeline.automation[0].gain = Some(0.0);
        assert_eq!(energy(&render(&timeline, 44_100).left[23_000..]), 0.0);
    }

    #[test]
    fn the_mixer_moves_what_the_score_changes_too() {
        let mut timeline = timeline_of(
            vec![NoteEvent::plain(0.0, 1.0, 45)],
            vec![Channel::default()],
            1.0,
        );
        timeline.automation = vec![Automation {
            channel: 0,
            time: 0.5,
            gain: Some(0.5),
            pan: Some(-0.5),
        }];
        let written = render(&timeline, 44_100);
        // With the slider at half, the whole passage is at half of what it was: before
        // the change and after it.
        timeline.channels[0].gain = 0.5;
        let turned_down = render(&timeline, 44_100);
        for i in [5_000, 30_000] {
            assert!((turned_down.left[i].atanh() - written.left[i].atanh() * 0.5).abs() < 1e-6);
            assert!(written.left[i] != 0.0);
        }
        // Panned right by as much as the score pans left, the track ends up in the middle.
        timeline.channels[0].pan = 0.5;
        let centred = render(&timeline, 44_100);
        assert!(energy(&centred.right[..20_000]) > energy(&centred.left[..20_000]) * 2.0);
        assert_eq!(centred.left[23_000..], centred.right[23_000..]);
    }

    #[test]
    fn drums_sound_like_their_pieces() {
        let hit = |piece: u8| {
            let channel = Channel {
                timbre: Timbre::Drums,
                ..Channel::default()
            };
            // A drum rings on its own time: the note here is far too short to hold it.
            let notes = vec![NoteEvent::plain(0.0, 0.01, piece)];
            render(&timeline_of(notes, vec![channel], 2.0), 44_100).left
        };
        let crossings = |samples: &[f32]| samples.windows(2).filter(|w| w[0] * w[1] < 0.0).count();
        let (kick, hat, crash) = (hit(36), hit(42), hit(49));
        assert!(
            kick.iter()
                .chain(&hat)
                .chain(&crash)
                .all(|s| s.abs() <= 1.0)
        );
        // A bass drum is low and a hi-hat is all top end.
        assert!(crossings(&kick[..4_000]) < 40);
        assert!(crossings(&hat[..2_000]) > 400);
        // The closed hat is over in a moment; the crash rings for a second and more.
        assert!(energy(&kick[..4_000]) > 1.0 && energy(&hat[..2_000]) > 0.01);
        assert_eq!(energy(&hat[4_000..]), 0.0);
        assert!(energy(&crash[30_000..50_000]) > 1e-4);
    }

    #[test]
    fn every_timbre_is_in_tune_and_bounded() {
        for timbre in [
            Timbre::Steel,
            Timbre::Nylon,
            Timbre::Clean,
            Timbre::Overdrive,
            Timbre::Distortion,
            Timbre::Bass,
            Timbre::Piano,
        ] {
            let channel = Channel {
                timbre,
                ..Channel::default()
            };
            let samples = render_with(NoteEvent::plain(0.0, 1.0, 45), channel).left;
            let off = cents(measured_frequency(&samples[2_000..14_000], 110.0), 110.0);
            assert!(off.abs() < 6.0, "{timbre:?}: {off} cents off");
            assert!(samples.iter().all(|s| s.abs() <= 1.0));
            assert!(
                energy(&samples[2_000..14_000]) > 1.0,
                "{timbre:?} is too quiet"
            );
        }
    }

    #[test]
    fn distortion_sustains_longer_than_a_clean_note() {
        let late = |timbre| {
            let channel = Channel {
                timbre,
                ..Channel::default()
            };
            let samples = render_with(NoteEvent::plain(0.0, 2.0, 52), channel).left;
            energy(&samples[70_000..80_000]) / energy(&samples[2_000..12_000])
        };
        assert!(late(Timbre::Distortion) > late(Timbre::Clean) * 2.0);
    }

    /// Estimates the fundamental of `window` from its autocorrelation peak near `expected`.
    fn measured_frequency(window: &[f32], expected: f32) -> f32 {
        let span = window.len() / 2;
        let correlation = |lag: usize| -> f32 {
            (window[..span].iter().zip(&window[lag..]))
                .map(|(a, b)| a * b)
                .sum()
        };
        let guess = 44_100.0 / expected;
        let lags = (guess * 0.9) as usize..=(guess * 1.1) as usize + 1;
        let best = lags
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

    fn cents(actual: f32, expected: f32) -> f32 {
        1200.0 * (actual / expected).log2()
    }

    #[test]
    fn notes_are_in_tune() {
        // Low E, A440 and the 24th fret of the high E string.
        for (pitch, expected) in [(40, 82.41), (69, 440.0), (88, 1318.5)] {
            let samples = render_one(NoteEvent::plain(0.0, 1.0, pitch));
            let off = cents(
                measured_frequency(&samples[2_000..14_000], expected),
                expected,
            );
            assert!(off.abs() < 6.0, "pitch {pitch}: {off} cents off");
        }
    }

    #[test]
    fn a_bend_raises_the_pitch() {
        let mut note = NoteEvent::plain(0.0, 1.0, 64);
        note.pitch_curve = vec![(0.0, 0.0), (0.2, 2.0)];
        let samples = render_one(note);
        // E4 at the start, bent up a whole tone to F#4 once the bend has landed.
        let early = cents(measured_frequency(&samples[..1_200], 329.63), 329.63);
        let late = cents(measured_frequency(&samples[20_000..30_000], 369.99), 369.99);
        assert!(early.abs() < 60.0, "start is {early} cents from E");
        assert!(late.abs() < 8.0, "bent note is {late} cents from F#");
    }

    #[test]
    fn harmonics_are_in_tune_and_muted_notes_die_quickly() {
        let mut harmonic = NoteEvent::plain(0.0, 1.0, 64);
        harmonic.tone = Tone::Harmonic;
        let samples = render_one(harmonic);
        let off = cents(measured_frequency(&samples[2_000..12_000], 329.63), 329.63);
        assert!(off.abs() < 6.0, "harmonic is {off} cents off");

        let energy = |samples: &[f32]| energy(&samples[8_000..12_000]);
        let open = energy(&render_one(NoteEvent::plain(0.0, 1.0, 45)));
        let mut muted = NoteEvent::plain(0.0, 1.0, 45);
        muted.tone = Tone::PalmMute;
        let palm = energy(&render_one(muted.clone()));
        muted.tone = Tone::Dead;
        let dead = energy(&render_one(muted));
        assert!(palm < open * 0.2, "palm mute {palm} vs open {open}");
        assert!(dead < palm * 0.01, "dead {dead} vs palm mute {palm}");
    }

    #[test]
    fn output_is_bounded_and_ends_quietly() {
        let mut loud = NoteEvent::plain(0.0, 1.0, 40);
        loud.velocity = 1.7;
        loud.vibrato = 0.5;
        let samples = render_one(loud);
        assert_eq!(samples.len(), (1.3f64 * 44_100.0).ceil() as usize);
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        assert!(samples.iter().any(|s| s.abs() > 0.02));
        assert!(samples[samples.len() - 4_000..].iter().all(|&s| s == 0.0));
    }
}
