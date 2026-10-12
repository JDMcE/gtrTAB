use gtr_model::{
    Accent, BarTiming, Beat, Bend, Dynamic, Fade, Hairpin, Harmonic, Instrument, Note, Rational,
    Score, Slap, Slide, Staff, Strum, Vibrato, play_order, range_order, timings,
};
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
    /// The staff and voice of the followed track whose beats move the playback cursor.
    pub staff: usize,
    pub voice: usize,
    /// Play the repeats again after a D.C. or D.S., instead of passing through them.
    pub replay_repeats: bool,
    /// Go back to the first bar after the last, until stopped.
    pub looped: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            from_bar: 0,
            to_bar: None,
            metronome: false,
            count_in: false,
            speed: 1.0,
            staff: 0,
            voice: 0,
            replay_repeats: false,
            looped: false,
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

/// The character of a track's sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Timbre {
    /// Bright, long-ringing steel strings.
    #[default]
    Steel,
    /// Softer attack and a shorter ring.
    Nylon,
    Clean,
    Overdrive,
    Distortion,
    Bass,
    /// A struck string: a soft attack and a long ring.
    Piano,
    /// Each note is a piece of the drum kit, chosen by its pitch.
    Drums,
    /// Nylon strings on a small body: soft, and soon over.
    Ukulele,
    /// A skin for a soundboard: a hard, bright attack that dies at once.
    Banjo,
    /// Pairs of short steel strings: bright, with little sustain.
    Mandolin,
}

impl Timbre {
    pub fn of(instrument: Instrument) -> Timbre {
        match instrument {
            Instrument::SteelGuitar => Timbre::Steel,
            Instrument::NylonGuitar => Timbre::Nylon,
            Instrument::Ukulele => Timbre::Ukulele,
            Instrument::Banjo => Timbre::Banjo,
            Instrument::Mandolin => Timbre::Mandolin,
            Instrument::CleanGuitar => Timbre::Clean,
            Instrument::OverdriveGuitar => Timbre::Overdrive,
            Instrument::DistortionGuitar => Timbre::Distortion,
            Instrument::Bass => Timbre::Bass,
            Instrument::Piano => Timbre::Piano,
            Instrument::Drums => Timbre::Drums,
        }
    }

    /// How much louder than a bank has it this sound is played from samples. A driven
    /// guitar is recorded no louder than a clean one, but in a band it is the loud one, as
    /// its model is; and a sampled bass is brought down towards its model, which it
    /// would otherwise be three times as loud as.
    pub fn sample_trim(self) -> f32 {
        match self {
            Timbre::Distortion => 2.0,
            Timbre::Overdrive => 1.3,
            Timbre::Bass => 0.8,
            _ => 1.0,
        }
    }

    /// Where a General MIDI SoundFont keeps this sound, as a bank and a program, if it has
    /// it at all: General MIDI has no ukulele and no mandolin, which are always modelled.
    pub fn preset(self) -> Option<(u16, u16)> {
        Some(match self {
            Timbre::Steel => (0, 25),
            Timbre::Nylon => (0, 24),
            Timbre::Clean => (0, 27),
            Timbre::Overdrive => (0, 29),
            Timbre::Distortion => (0, 30),
            Timbre::Bass => (0, 33),
            Timbre::Piano => (0, 0),
            Timbre::Banjo => (0, 105),
            Timbre::Drums => (crate::soundfont::DRUM_BANK, 0),
            Timbre::Ukulele | Timbre::Mandolin => return None,
        })
    }
}

/// How one track is mixed. The notes of a track that is not audible are in the timeline all
/// the same, so that it can be brought back in while the music plays.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Channel {
    /// False for a track that is muted, or left out by another track's solo.
    pub audible: bool,
    pub gain: f32,
    /// From -1.0 (left) to 1.0 (right).
    pub pan: f32,
    pub timbre: Timbre,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            audible: true,
            gain: 1.0,
            pan: 0.0,
            timbre: Timbre::default(),
        }
    }
}

/// A sounding note. Tied notes are merged into the note they extend.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteEvent {
    /// Index into [`Timeline::channels`].
    pub channel: usize,
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
    /// The note's volume is brought up from nothing, taken down to it, or both, over
    /// its length.
    pub fade: Option<Fade>,
}

impl NoteEvent {
    /// A plain note with no articulation.
    pub fn plain(time: f64, duration: f64, pitch: u8) -> Self {
        NoteEvent {
            channel: 0,
            time,
            duration,
            pitch,
            velocity: 1.0,
            tone: Tone::Normal,
            soft: false,
            pitch_curve: Vec::new(),
            vibrato: 0.0,
            fade: None,
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct BeatTime {
    pub bar: usize,
    pub beat: usize,
    pub time: f64,
}

/// A pulse of the bar, as a metronome counts it, for showing the count while it plays.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Pulse {
    pub time: f64,
    /// Which pulse of the bar it is, from 0.
    pub index: u8,
    /// Pulses in the bar.
    pub count: u8,
    /// One of the count-in's.
    pub count_in: bool,
}

/// A change to a track's place in the mix part-way through, written on a beat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Automation {
    /// Index into [`Timeline::channels`].
    pub channel: usize,
    /// Seconds from the start of playback.
    pub time: f64,
    /// The track's gain from here on, if it changes.
    pub gain: Option<f32>,
    /// The track's pan from here on, from -1.0 (left) to 1.0 (right), if it changes.
    pub pan: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Timeline {
    /// One per track of the score.
    pub channels: Vec<Channel>,
    /// Changes to the channels' gain and pan, in time order.
    pub automation: Vec<Automation>,
    pub notes: Vec<NoteEvent>,
    pub clicks: Vec<Click>,
    /// Beats of the followed track in time order.
    pub beats: Vec<BeatTime>,
    /// Every pulse of every bar in time order, the count-in's among them, whether the
    /// metronome is heard or not. The synth has no use for them and they are not sent to it.
    pub pulses: Vec<Pulse>,
    /// Seconds the count-in lasts, before the first bar starts; zero without one.
    pub count_in: f64,
    /// Total length in seconds.
    pub duration: f64,
    /// Played again from the end of the count-in on reaching `duration`, until stopped.
    pub looped: bool,
}

/// Longest a let-ring note is allowed to sustain.
const MAX_RING: f64 = 4.0;
/// Longest a grace note lasts, in seconds.
const GRACE_LENGTH: f64 = 0.07;
/// Seconds between strings in a quick strum.
const BRUSH_STEP: f64 = 0.012;

/// Seconds between the strokes of a rasgueado, at most.
const RASGUEADO_STEP: f64 = 0.11;

/// One bar of the passage being played.
struct Played {
    /// Seconds from the start of playback.
    start: f64,
    timing: BarTiming,
}

/// A few notes to hear on their own, as when a chord or a scale is tried out: `pitches`
/// played on `instrument` one after another, `gap` seconds apart (0.0 for a chord, or a few
/// hundredths for a strum), each lasting `length` seconds.
pub fn audition(instrument: Instrument, pitches: &[u8], gap: f64, length: f64) -> Timeline {
    let notes: Vec<NoteEvent> = (pitches.iter().enumerate())
        .map(|(i, &pitch)| NoteEvent::plain(i as f64 * gap, length, pitch.min(127)))
        .collect();
    Timeline {
        channels: vec![Channel {
            timbre: Timbre::of(instrument),
            ..Channel::default()
        }],
        duration: (notes.iter()).fold(0.0, |end, n| f64::max(end, n.time + n.duration)),
        notes,
        ..Timeline::default()
    }
}

/// Compiles the score to timed events. Every track's notes are included, and its channel
/// says whether it is audible: not muted, and not left out by another track's solo. `follow`
/// picks the track whose beats are reported for the playback cursor.
///
/// Playing to the end follows repeats and jumps; playing a range of bars (`to_bar` set)
/// follows the repeats inside it.
pub fn compile(score: &Score, follow: usize, options: &Options) -> Timeline {
    let masters = &score.master_bars;
    let bar_count = masters.len();
    let to_bar = options.to_bar.unwrap_or(bar_count).min(bar_count);
    let from_bar = options.from_bar.min(to_bar);
    let speed = if options.speed > 0.0 {
        options.speed
    } else {
        1.0
    };
    let order: Vec<usize> = match options.to_bar {
        Some(_) => range_order(masters, from_bar, to_bar),
        None => {
            let full = play_order(masters, options.replay_repeats);
            match full.iter().position(|&bar| bar == from_bar) {
                Some(at) => full[at..].to_vec(),
                None => (from_bar..to_bar).collect(),
            }
        }
    };

    let any_solo = score.tracks.iter().any(|t| t.mix.solo);
    let mut timeline = Timeline {
        channels: (score.tracks.iter())
            .map(|track| Channel {
                audible: !track.mix.mute && (track.mix.solo || !any_solo),
                gain: track.mix.gain(),
                pan: f32::from(track.mix.pan) / f32::from(gtr_model::Mix::MAX_PAN),
                timbre: Timbre::of(track.instrument),
            })
            .collect(),
        automation: Vec::new(),
        notes: Vec::new(),
        clicks: Vec::new(),
        beats: Vec::new(),
        pulses: Vec::new(),
        count_in: 0.0,
        duration: 0.0,
        looped: options.looped,
    };

    let mut time = 0.0;
    let mut played = Vec::with_capacity(order.len());
    for (position, timing) in timings(score, &order, speed).into_iter().enumerate() {
        let signature = masters[timing.bar].time_signature;
        let pulse = Rational::new(1, signature.denominator.into());
        let pulses = (0..signature.numerator).map(|i| (pulse * i64::from(i), i == 0));
        let count = signature.numerator;
        if position == 0 && options.count_in {
            // The count-in keeps strict time, whatever the bar it leads into holds.
            let whole = timing.whole();
            timeline.pulses.extend((0..count).map(|index| Pulse {
                time: seconds(pulse * i64::from(index), whole),
                index,
                count,
                count_in: true,
            }));
            timeline
                .clicks
                .extend(pulses.clone().map(|(at, accent)| Click {
                    time: seconds(at, whole),
                    accent,
                }));
            time += seconds(signature.bar_length(), whole);
            timeline.count_in = time;
        }
        timeline.pulses.extend((0..count).map(|index| Pulse {
            time: time + timing.at(pulse * i64::from(index)),
            index,
            count,
            count_in: false,
        }));
        if options.metronome {
            timeline.clicks.extend(pulses.map(|(at, accent)| Click {
                time: time + timing.at(at),
                accent,
            }));
        }
        let start = time;
        time += timing.length;
        played.push(Played { start, timing });
    }
    timeline.duration = time;

    for (index, track) in score.tracks.iter().enumerate() {
        let followed_staff = options.staff.min(track.staves.len().saturating_sub(1));
        for (s, staff) in track.staves.iter().enumerate() {
            let followed = (index == follow && s == followed_staff).then_some(options.voice);
            let (first, first_change) = (timeline.notes.len(), timeline.automation.len());
            add_staff(&mut timeline, staff, &played, followed);
            for note in &mut timeline.notes[first..] {
                note.channel = index;
            }
            for change in &mut timeline.automation[first_change..] {
                change.channel = index;
            }
        }
    }
    timeline.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    timeline
        .automation
        .sort_by(|a, b| a.time.total_cmp(&b.time));
    timeline
}

/// Seconds that `length` whole notes last when one lasts `whole` seconds.
fn seconds(length: Rational, whole: f64) -> f64 {
    *length.numer() as f64 / *length.denom() as f64 * whole
}

/// Loudness of each beat of the staff's first voice, per bar, from its dynamic marks and
/// hairpins. A dynamic lasts until the next one; a hairpin swells or fades across the run of
/// beats that carry it and leaves the level where it ends.
fn beat_gains(staff: &Staff) -> Vec<Vec<f32>> {
    let beats: Vec<&Beat> = (staff.bars.iter())
        .flat_map(|bar| bar.voices.first().map_or(&[][..], |v| &v.beats))
        .collect();
    let mut gains = Vec::with_capacity(beats.len());
    let mut level = Dynamic::F.gain();
    let mut i = 0;
    while i < beats.len() {
        if let Some(dynamic) = beats[i].effects.dynamic {
            level = dynamic.gain();
        }
        let Some(hairpin) = beats[i].effects.hairpin else {
            gains.push(level);
            i += 1;
            continue;
        };
        let run = (beats[i..].iter())
            .take_while(|b| b.effects.hairpin == Some(hairpin))
            .count();
        let target = match hairpin {
            Hairpin::Crescendo => level * 1.6,
            Hairpin::Decrescendo => level * 0.55,
        };
        for step in 0..run {
            gains.push(level + (target - level) * step as f32 / run as f32);
        }
        level = target;
        i += run;
    }
    let mut gains = gains.into_iter();
    (staff.bars.iter())
        .map(|bar| {
            let count = bar.voices.first().map_or(0, |v| v.beats.len());
            gains.by_ref().take(count).collect()
        })
        .collect()
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

fn add_staff(timeline: &mut Timeline, staff: &Staff, played: &[Played], followed: Option<usize>) {
    let end = timeline.duration;
    let gains = beat_gains(staff);
    // Each voice keeps track of its own ties, slurs and ringing strings.
    let voice_count = staff.bars.iter().map(|b| b.voices.len()).max().unwrap_or(1);
    let mut states = vec![vec![StringState::default(); staff.places()]; voice_count];
    for (b, this) in played.iter().enumerate() {
        let bar = &staff.bars[this.timing.bar];
        for (v, voice) in bar.voices.iter().enumerate() {
            let strings = &mut states[v];
            let mut offset = Rational::from_integer(0);
            for (i, beat) in voice.beats.iter().enumerate() {
                let from = this.timing.seconds(offset);
                let whole = this.timing.whole_at(offset);
                offset += beat.duration.length();
                let time = this.start + from;
                let duration = this.timing.seconds(offset) - from;
                if followed == Some(v) {
                    timeline.beats.push(BeatTime {
                        bar: this.timing.bar,
                        beat: i,
                        time,
                    });
                }
                if beat.effects.volume.is_some() || beat.effects.pan.is_some() {
                    timeline.automation.push(Automation {
                        channel: 0,
                        time,
                        gain: beat.effects.volume.map(gtr_model::Mix::gain_of),
                        pan: (beat.effects.pan)
                            .map(|pan| f32::from(pan) / f32::from(gtr_model::Mix::MAX_PAN)),
                    });
                }
                // When the beat's notes are struck, from its start; for how long; and how
                // hard. Tremolo picking re-picks the beat in equal short notes, and a
                // rasgueado strums it with one finger after another, the last the hardest.
                let hits: Vec<(f64, f64, f32)> = match (beat.effects.tremolo, beat.effects.strum) {
                    (Some(value), _) => {
                        let pick = seconds(value.length(), whole);
                        let picks = (duration / pick).round().max(1.0) as usize;
                        let length = duration / picks as f64;
                        (0..picks)
                            .map(|i| (length * i as f64, length, 1.0))
                            .collect()
                    }
                    (None, Some(Strum::Rasgueado)) => {
                        let step = (duration / 4.0).min(RASGUEADO_STEP);
                        (0..4)
                            .map(|i| {
                                let length = if i == 3 { duration - step * 3.0 } else { step };
                                (
                                    step * f64::from(i),
                                    length,
                                    [0.7, 0.75, 0.85, 1.0][i as usize],
                                )
                            })
                            .collect()
                    }
                    _ => vec![(0.0, duration, 1.0)],
                };
                let pick_length = hits[0].1;
                let tremolo = beat.effects.tremolo.is_some();

                let gain = match v {
                    0 => gains[this.timing.bar][i],
                    _ => 1.0,
                } * match beat.effects.slap {
                    Some(Slap::Slap) => 1.3,
                    Some(Slap::Pop) => 1.4,
                    None => 1.0,
                };
                // A strum or arpeggio starts the strings one after another: downwards
                // means from the lowest string to the highest.
                let step = match beat.effects.strum {
                    Some(Strum::BrushDown | Strum::BrushUp | Strum::Rasgueado) => BRUSH_STEP,
                    Some(Strum::ArpeggioDown | Strum::ArpeggioUp) => {
                        (duration / (beat.notes.len() + 1) as f64).min(0.06)
                    }
                    None => 0.0,
                };
                let upwards =
                    matches!(beat.effects.strum, Some(Strum::BrushUp | Strum::ArpeggioUp));

                let mut next = vec![StringState::default(); strings.len()];
                for (order, note) in beat.notes.iter().enumerate() {
                    let delay = step
                        * match upwards {
                            true => order,
                            false => beat.notes.len() - 1 - order,
                        } as f64;
                    let string = usize::from(note.string);
                    let Some(pitch) = staff.pitch(note.string, note.fret) else {
                        continue;
                    };
                    let previous = strings[string];
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
                        // The string rings until it is played again, but not for ever: no
                        // longer than its limit, or than the note is written if that is more.
                        let event = &mut timeline.notes[index];
                        let limit = MAX_RING.max(event.duration);
                        event.duration = (time - event.time).min(limit);
                    }

                    let next_bar = played.get(b + 1).map(|next| &staff.bars[next.timing.bar]);
                    let following = following_note(bar, next_bar, v, i, note.string);
                    // Share of this note taken by one note of a trill, if it has one.
                    let trill_step = note.effects.trill.map_or(1.0, |t| {
                        let ratio = t.speed.length() / beat.duration.length();
                        *ratio.numer() as f64 / *ratio.denom() as f64
                    });
                    let mut event =
                        note_event(note, staff, pitch, time, pick_length, following, trill_step);
                    // A tapped note is hammered on by the picking hand, not picked.
                    event.soft = previous.legato || note.effects.tapped;
                    event.fade = beat.effects.fade;
                    event.velocity *= gain * hits[0].2;
                    if event.pitch_curve.is_empty()
                        && let Some(whammy) = &beat.effects.whammy
                    {
                        event.pitch_curve = curve_points(whammy, pick_length);
                    }
                    event.time += delay;
                    event.duration = (event.duration - delay).max(event.duration * 0.25);
                    if let Some(grace) = &note.effects.grace {
                        // Grace notes before the beat borrow their time from what came
                        // before; on the beat they push the main note back.
                        let count = grace.notes.len() as f64;
                        let length = GRACE_LENGTH.min(pick_length / (count + 2.0));
                        let before = !grace.on_beat && event.time >= length * count;
                        let first = match before {
                            true => event.time - length * count,
                            false => event.time,
                        };
                        let mut last_string = None;
                        for (g, grace_note) in grace.notes.iter().enumerate() {
                            let grace_pitch = staff.pitch(grace_note.string, grace_note.fret);
                            let mut ornament = NoteEvent::plain(
                                first + length * g as f64,
                                length,
                                grace_pitch.unwrap_or(pitch),
                            );
                            ornament.velocity = event.velocity;
                            // Only a note on the same string can be hammered into.
                            ornament.soft = grace.hammer && last_string == Some(grace_note.string);
                            last_string = Some(grace_note.string);
                            timeline.notes.push(ornament);
                        }
                        if !before {
                            event.time += length * count;
                            event.duration -= length * count;
                        }
                        event.soft |= grace.hammer && last_string == Some(note.string);
                    }
                    let again: Vec<NoteEvent> = (hits[1..].iter())
                        .map(|&(at, length, loudness)| NoteEvent {
                            time: time + at + delay,
                            duration: (length - delay).max(length * 0.25),
                            velocity: event.velocity / hits[0].2 * loudness,
                            soft: false,
                            ..event.clone()
                        })
                        .collect();
                    timeline.notes.push(event);
                    timeline.notes.extend(again);
                    // What a tie or the ringing string carries on is the last stroke; the
                    // picks of a tremolo are too short to carry anything on.
                    let index = timeline.notes.len() - 1;
                    next[string] = StringState {
                        sounding: (!tremolo).then_some(index),
                        legato: joins_next(note),
                        ringing: (note.effects.let_ring && !tremolo).then_some(index),
                    };
                }
                // Strings left alone keep ringing; everything else starts afresh.
                for (state, old) in next.iter_mut().zip(strings.iter()) {
                    if state.sounding.is_none() {
                        state.ringing = old.ringing;
                    }
                }
                *strings = next;
            }
        }
        // A tie or slur only reaches into the next bar from this bar's last beat, which the
        // string state already reflects; a voice with nothing in the bar breaks it.
        for (v, strings) in states.iter_mut().enumerate() {
            if bar.voices.get(v).is_none_or(|voice| voice.beats.is_empty()) {
                for state in strings {
                    state.sounding = None;
                    state.legato = false;
                }
            }
        }
    }
    // Notes still ringing at the end sustain to the end of the passage.
    for index in states.iter().flatten().filter_map(|s| s.ringing) {
        let event = &mut timeline.notes[index];
        // To the end, within the limit; a note already written longer keeps its length.
        event.duration = (end - event.time).min(MAX_RING).max(event.duration);
    }
}

fn joins_next(note: &Note) -> bool {
    note.effects.hammer || note.effects.slide == Some(Slide::Legato)
}

/// The note on `string` in the beat after `beat`, looking into the bar played next if need
/// be.
fn following_note<'a>(
    bar: &'a gtr_model::Bar,
    next_bar: Option<&'a gtr_model::Bar>,
    voice: usize,
    beat: usize,
    string: u8,
) -> Option<&'a Note> {
    let next: Option<&Beat> = match bar.voices[voice].beats.get(beat + 1) {
        Some(next) => Some(next),
        None => next_bar
            .and_then(|b| b.voices.get(voice))
            .and_then(|v| v.beats.first()),
    };
    next?.note_on(string)
}

/// A bend or whammy curve as `(seconds, semitones)` points over a note `length` seconds long.
fn curve_points(curve: &Bend, length: f64) -> Vec<(f32, f32)> {
    (curve.points.iter())
        .map(|p| {
            let time = f64::from(p.position) / f64::from(Bend::LENGTH) * length;
            (time as f32, f32::from(p.value) / 2.0)
        })
        .collect()
}

/// Builds the event for a freshly sounded note from its effects.
fn note_event(
    note: &Note,
    staff: &Staff,
    fretted: u8,
    time: f64,
    length: f64,
    following: Option<&Note>,
    trill_step: f64,
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
                let open = staff.pitch(note.string, 0).unwrap_or(fretted);
                open.saturating_add(Harmonic::overtone(note.fret))
            }
            // The others sound an overtone of the stopped note, chosen by where above it
            // the string is touched.
            kind => fretted.saturating_add(kind.interval(effects.harmonic_touch)),
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

    if let Some(bend) = &effects.bend {
        event.pitch_curve = curve_points(bend, length);
    } else if let Some(trill) = effects.trill {
        // Flip between the two frets, each held for the trill's note value.
        let interval = f32::from(trill.fret) - f32::from(note.fret);
        let step = (trill_step * length) as f32;
        let flips = ((d / step.max(0.01)) as usize).min(256);
        event.pitch_curve = (1..=flips)
            .flat_map(|flip| {
                let (from, to) = match flip % 2 {
                    1 => (0.0, interval),
                    _ => (interval, 0.0),
                };
                let at = step * flip as f32;
                [(at - 0.004, from), (at, to)]
            })
            .collect();
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
    use gtr_model::{
        Beat, BendKind, Duration, Grace, Note, NoteEffects, NoteValue, Score, Trill, demo_score,
    };

    use gtr_model::{Feel, MasterBar};

    use super::*;

    #[test]
    fn an_audition_plays_its_notes_in_turn_on_one_instrument() {
        let strum = audition(Instrument::Piano, &[48, 52, 55, 200], 0.02, 1.0);
        assert_eq!(strum.channels.len(), 1);
        assert_eq!(strum.channels[0].timbre, Timbre::of(Instrument::Piano));
        let times: Vec<f64> = strum.notes.iter().map(|n| n.time).collect();
        assert_eq!(times, [0.0, 0.02, 0.04, 0.06]);
        assert_eq!(strum.notes[3].pitch, 127);
        assert_eq!(strum.duration, 1.06);
        assert!(audition(Instrument::Bass, &[], 0.3, 0.3).notes.is_empty());
    }

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
    fn mute_solo_and_mix_settings() {
        use gtr_model::{Mix, Track};

        let mut score = quarters(vec![Note::new(5, 0)]);
        let mut bass = Track::empty("Bass", Instrument::Bass, 1);
        bass.staves[0].bars = score.tracks[0].staves[0].bars.clone();
        bass.staves[0].bars[0].voices[0].beats[0].notes = vec![Note::new(3, 0)];
        bass.mix = Mix {
            volume: 0,
            pan: -100,
            ..Mix::default()
        };
        score.tracks.push(bass);

        let t = compile(&score, 0, &Options::default());
        let heard: Vec<(usize, u8)> = t.notes.iter().map(|n| (n.channel, n.pitch)).collect();
        assert_eq!(heard, [(0, 40), (1, 28)]);
        assert_eq!(t.channels[0], Channel::default());
        assert_eq!(
            t.channels[1],
            Channel {
                audible: true,
                gain: 0.0,
                pan: -1.0,
                timbre: Timbre::Bass
            }
        );
        let audible = |score: &Score| -> Vec<bool> {
            let t = compile(score, 0, &Options::default());
            // A silent track keeps its notes, ready to be heard again, and can be followed.
            assert_eq!((t.notes.len(), t.beats.len()), (2, 1));
            t.channels.iter().map(|c| c.audible).collect()
        };

        score.tracks[0].mix.mute = true;
        assert_eq!(audible(&score), [false, true]);
        // Solo silences every other track; a track both soloed and muted stays silent.
        score.tracks[0].mix.mute = false;
        score.tracks[0].mix.solo = true;
        assert_eq!(audible(&score), [true, false]);
        score.tracks[0].mix.mute = true;
        assert_eq!(audible(&score), [false, false]);
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
            looped: true,
            ..Options::default()
        };
        let t = compile(&score, 0, &options);
        // One count-in bar and one bar of music, each four seconds at half speed.
        assert_eq!(t.duration, 8.0);
        assert_eq!(t.count_in, 4.0);
        assert!(t.looped);
        // Every pulse is counted, the count-in's first, whether clicked or not.
        assert_eq!(t.pulses.len(), 8);
        assert!(t.pulses[..4].iter().all(|p| p.count_in && p.count == 4));
        assert_eq!((t.pulses[5].time, t.pulses[5].index), (5.0, 1));
        assert!(!t.pulses[5].count_in);
        let quiet = compile(
            &score,
            0,
            &Options {
                metronome: false,
                ..options
            },
        );
        assert_eq!(quiet.pulses, t.pulses);
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
        let tied = || Note {
            tied: true,
            ..Note::new(5, 3)
        };
        score.tracks[0].staves[0].bars[0].voices[0].beats = vec![
            Beat::new(quarter, vec![Note::new(5, 3)]),
            Beat::new(quarter, vec![tied()]),
            // A tie with nothing sounding on the string starts a new note.
            Beat::rest(quarter),
            Beat::new(quarter, vec![tied()]),
        ];
        // And a tie across the bar line.
        score.tracks[0].staves[0].bars[1].voices[0].beats = vec![Beat::new(quarter, vec![tied()])];
        let t = compile(&score, 0, &Options::default());
        let notes: Vec<(f64, f64)> = t.notes.iter().map(|n| (n.time, n.duration)).collect();
        assert_eq!(notes, [(0.0, 1.0), (1.5, 1.0)]);
    }

    #[test]
    fn every_voice_has_its_own_ties_and_can_be_followed() {
        use gtr_model::Voice;

        let mut score = quarters(vec![Note::new(5, 0); 4]);
        let half = Duration::new(NoteValue::Half);
        score.tracks[0].staves[0].bars[0].voices.push(Voice {
            beats: vec![
                Beat::new(half, vec![Note::new(0, 3)]),
                Beat::new(
                    half,
                    vec![Note {
                        tied: true,
                        ..Note::new(0, 3)
                    }],
                ),
            ],
        });
        let t = compile(&score, 0, &Options::default());
        // Four quarter notes, and one whole note made of two tied halves.
        let lengths: Vec<f64> = t.notes.iter().map(|n| n.duration).collect();
        assert_eq!(lengths.iter().filter(|d| **d == 0.5).count(), 4);
        assert_eq!(lengths.iter().filter(|d| **d == 2.0).count(), 1);
        assert_eq!(t.beats.len(), 4);

        let second = Options {
            voice: 1,
            ..Options::default()
        };
        let times: Vec<f64> = (compile(&score, 0, &second).beats.iter())
            .map(|b| b.time)
            .collect();
        assert_eq!(times, [0.0, 1.0]);
    }

    #[test]
    fn playback_unrolls_repeats_from_the_start_the_cursor_or_in_a_range() {
        let mut score = demo_score();
        score.master_bars[0].repeat_count = 2;
        let t = compile(&score, 0, &Options::default());
        // The first bar twice, then the second: three two-second bars.
        assert_eq!(t.duration, 6.0);
        let bars: Vec<usize> = t.beats.iter().map(|b| b.bar).collect();
        assert_eq!(bars[7..10], [0, 0, 0]);
        assert_eq!((t.beats[8].bar, t.beats[8].time), (0, 2.0));
        assert_eq!(t.beats.last().map(|b| b.bar), Some(1));

        // Starting from the second bar skips what came before it.
        let from_second = Options {
            from_bar: 1,
            ..Options::default()
        };
        assert_eq!(compile(&score, 0, &from_second).duration, 2.0);
        // A selected range follows the repeats inside it.
        let range = Options {
            to_bar: Some(2),
            ..Options::default()
        };
        assert_eq!(compile(&score, 0, &range).duration, 6.0);
        let range = Options {
            from_bar: 1,
            to_bar: Some(2),
            ..Options::default()
        };
        assert_eq!(compile(&score, 0, &range).duration, 2.0);
    }

    #[test]
    fn tempo_changes_and_swing_shape_the_timing() {
        let mut score = demo_score();
        score.master_bars[1].tempo = Some(240);
        let t = compile(&score, 0, &Options::default());
        // Two seconds at 120, then one second at 240.
        assert_eq!(t.duration, 3.0);
        assert_eq!(t.beats[9].time, 2.125);

        // Swung eighths: the off-beat note comes two thirds of the way through the beat.
        score.master_bars[1].tempo = None;
        score.master_bars[0].feel = Some(Feel::SwingEighths);
        let t = compile(&score, 0, &Options::default());
        let times: Vec<f64> = t.beats[..4].iter().map(|b| b.time).collect();
        assert_eq!(times[0], 0.0);
        assert!((times[1] - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(times[2], 0.5);
        assert!((t.notes[0].duration - 1.0 / 3.0).abs() < 1e-9);
        assert!((t.notes[1].duration - 1.0 / 6.0).abs() < 1e-9);
        // The feel lasts into the next bar, and the bar is as long as ever.
        assert!((t.beats[9].time - (2.0 + 1.0 / 3.0)).abs() < 1e-9);
        assert_eq!(t.duration, 4.0);
    }

    #[test]
    fn drums_piano_and_mix_changes_reach_the_timeline() {
        use gtr_model::Track;

        let mut score = Score::blank();
        let quarter = Duration::new(NoteValue::Quarter);
        let mut drums = Track::empty("Drums", Instrument::Drums, 1);
        // Bass drum then snare: the fret is the piece's number, whatever line it is on.
        drums.staves[0].bars[0].voices[0].beats = vec![
            Beat::new(quarter, vec![Note::new(4, 36)]),
            Beat::new(quarter, vec![Note::new(2, 38)]),
        ];
        drums.staves[0].bars[0].voices[0].beats[1].effects.volume = Some(0);
        drums.staves[0].bars[0].voices[0].beats[1].effects.pan = Some(50);
        let mut piano = Track::empty("Piano", Instrument::Piano, 1);
        assert_eq!(piano.staves.len(), 2);
        piano.staves[1].bars[0].voices[0].beats = vec![Beat::new(quarter, vec![Note::new(28, 0)])];
        score.tracks.push(drums);
        score.tracks.push(piano);

        let follow_left_hand = Options {
            staff: 1,
            ..Options::default()
        };
        let t = compile(&score, 2, &follow_left_hand);
        let heard: Vec<(usize, u8)> = t.notes.iter().map(|n| (n.channel, n.pitch)).collect();
        assert_eq!(heard, [(1, 36), (2, 28), (1, 38)]);
        assert_eq!(t.channels[1].timbre, Timbre::Drums);
        assert_eq!(t.channels[2].timbre, Timbre::Piano);
        // The cursor follows the staff asked for.
        assert_eq!(t.beats.len(), 1);
        assert_eq!(
            t.automation,
            [Automation {
                channel: 1,
                time: 0.5,
                gain: Some(0.0),
                pan: Some(0.5),
            }]
        );
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
        let bend = |kind, quarter_tones| Some(Bend::preset(kind, quarter_tones));
        let score = quarters(vec![
            with(7, |e| e.bend = bend(BendKind::Bend, 4)),
            with(7, |e| e.bend = bend(BendKind::PrebendRelease, 2)),
            with(12, |e| e.harmonic = Some(Harmonic::Natural)),
            with(7, |e| e.harmonic = Some(Harmonic::Natural)),
        ]);
        let t = compile(&score, 0, &Options::default());
        // A full bend reaches a whole tone a quarter of the way through the half-second note.
        assert_eq!(
            t.notes[0].pitch_curve,
            [(0.0, 0.0), (0.125, 2.0), (0.5, 2.0)]
        );
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
    fn a_ringing_note_is_limited_however_long_the_string_is_left_alone() {
        // Let ring on the low string, then six bars before that string is played again.
        let mut score = quarters(vec![with(0, |e| e.let_ring = true)]);
        let bar = score.tracks[0].staves[0].bars[0].clone();
        for _ in 0..6 {
            score
                .master_bars
                .push(MasterBar::new(gtr_model::TimeSignature::COMMON));
            score.tracks[0].staves[0].bars.push(gtr_model::Bar::empty());
        }
        score.tracks[0].staves[0].bars[6] = bar.clone();
        let t = compile(&score, 0, &Options::default());
        let lengths: Vec<f64> = t.notes.iter().map(|n| n.duration).collect();
        // The first stops at the limit; the second rings to the end of its bar.
        assert_eq!(lengths, [MAX_RING, 2.0]);

        // A ringing note written longer than the limit, by ties, keeps its written length
        // when the passage ends. This used to stop playback altogether.
        let whole = Duration::new(NoteValue::Whole);
        let tied = Note {
            tied: true,
            ..with(0, |e| e.let_ring = true)
        };
        for bar in &mut score.tracks[0].staves[0].bars {
            *bar = gtr_model::Bar::empty();
        }
        let bars = &mut score.tracks[0].staves[0].bars;
        bars[0].voices[0].beats = vec![Beat::new(whole, vec![with(0, |e| e.let_ring = true)])];
        for bar in &mut bars[1..4] {
            bar.voices[0].beats = vec![Beat::new(whole, vec![tied.clone()])];
        }
        let t = compile(&score, 0, &Options::default());
        assert_eq!(t.notes.len(), 1);
        assert_eq!(t.notes[0].duration, 8.0);
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

    fn first_beat(score: &mut Score) -> &mut Beat {
        &mut score.tracks[0].staves[0].bars[0].voices[0].beats[0]
    }

    #[test]
    fn dynamics_and_hairpins_set_the_loudness() {
        let mut score = quarters(vec![Note::new(5, 1); 4]);
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].effects.dynamic = Some(Dynamic::P);
        beats[1].effects.hairpin = Some(Hairpin::Crescendo);
        beats[2].effects.hairpin = Some(Hairpin::Crescendo);
        let t = compile(&score, 0, &Options::default());
        let loudness: Vec<f32> = t.notes.iter().map(|n| n.velocity).collect();
        let piano = Dynamic::P.gain();
        // Piano, a swell over two beats, then the level the swell reached.
        assert_eq!(loudness[0], piano);
        assert_eq!(loudness[1], piano);
        assert!(loudness[2] > loudness[1] && loudness[3] > loudness[2]);
        assert!((loudness[3] - piano * 1.6).abs() < 1e-6);
    }

    #[test]
    fn strums_spread_a_chord_and_the_whammy_bar_bends_it() {
        let mut score = quarters(vec![Note::new(5, 3)]);
        let beat = first_beat(&mut score);
        beat.notes = vec![Note::new(3, 5), Note::new(4, 5), Note::new(5, 3)];
        beat.effects.strum = Some(Strum::BrushDown);
        beat.effects.whammy = Some(Bend::dive(4));
        let t = compile(&score, 0, &Options::default());
        // A downstroke sounds the lowest string first.
        let pitches: Vec<u8> = t.notes.iter().map(|n| n.pitch).collect();
        assert_eq!(pitches, [43, 50, 55]);
        assert_eq!(t.notes[1].time, BRUSH_STEP);
        assert!(
            t.notes
                .iter()
                .all(|n| n.pitch_curve == [(0.0, 0.0), (0.5, -2.0)])
        );

        first_beat(&mut score).effects.strum = Some(Strum::ArpeggioUp);
        let t = compile(&score, 0, &Options::default());
        assert_eq!(t.notes[0].pitch, 55);
        assert_eq!(t.notes[2].time, 0.12);
    }

    #[test]
    fn a_fermata_holds_the_beat_in_every_track() {
        use gtr_model::Track;

        let mut score = quarters(vec![Note::new(5, 0); 4]);
        score.tracks[0].staves[0].bars[0].voices[0].beats[1]
            .effects
            .fermata = true;
        let mut bass = Track::empty("Bass", Instrument::Bass, 1);
        bass.staves[0].bars[0].voices[0].beats = vec![
            Beat::new(Duration::new(NoteValue::Half), vec![Note::new(3, 0)]),
            Beat::new(Duration::new(NoteValue::Half), vec![Note::new(3, 2)]),
        ];
        score.tracks.push(bass);
        let options = Options {
            metronome: true,
            ..Options::default()
        };
        let t = compile(&score, 0, &options);
        // The second beat lasts twice as long, and the bar half a second longer.
        let times: Vec<f64> = t.beats.iter().map(|b| b.time).collect();
        assert_eq!(times, [0.0, 0.5, 1.5, 2.0]);
        assert_eq!(t.duration, 2.5);
        // The bass, which has no fermata of its own, waits with the guitar.
        let bass: Vec<(f64, f64)> = (t.notes.iter().filter(|n| n.channel == 1))
            .map(|n| (n.time, n.duration))
            .collect();
        assert_eq!(bass, [(0.0, 1.5), (1.5, 1.0)]);
        let clicks: Vec<f64> = t.clicks.iter().map(|c| c.time).collect();
        assert_eq!(clicks, [0.0, 0.5, 1.5, 2.0]);
    }

    #[test]
    fn a_rasgueado_is_four_strokes_and_a_tapped_note_is_not_picked() {
        let mut score = quarters(vec![Note::new(5, 3), with(7, |e| e.tapped = true)]);
        let beat = first_beat(&mut score);
        beat.notes = vec![Note::new(4, 5), Note::new(5, 3)];
        beat.effects.strum = Some(Strum::Rasgueado);
        let t = compile(&score, 0, &Options::default());
        let strokes: Vec<&NoteEvent> = t.notes.iter().filter(|n| n.pitch == 43).collect();
        // One finger after another across the strings, lowest string first, and the last
        // stroke, the hardest, rings for what is left of the beat.
        let times: Vec<f64> = strokes.iter().map(|n| n.time).collect();
        assert_eq!(times, [0.0, 0.11, 0.22, 0.33]);
        assert!(strokes[0].velocity < strokes[3].velocity);
        assert_eq!(strokes[3].velocity, 1.0);
        assert!((strokes[3].duration - 0.17).abs() < 1e-9);
        let upper: Vec<f64> = (t.notes.iter().filter(|n| n.pitch == 50))
            .map(|n| n.time)
            .collect();
        assert_eq!(upper[0], BRUSH_STEP);
        assert_eq!(upper.len(), 4);

        let tapped = t.notes.last().unwrap();
        assert!(tapped.soft && tapped.time == 0.5);
        assert!(t.notes[..8].iter().all(|n| !n.soft));
    }

    #[test]
    fn a_staff_without_strings_plays_its_notes_as_the_pitches_they_are() {
        use gtr_model::Track;

        let mut score = Score::blank();
        let mut piano = Track::empty("Piano", Instrument::Piano, 1);
        let half = Duration::new(NoteValue::Half);
        // A chord of more notes than a hand has strings to make up, then the top note held.
        let chord: Vec<Note> = [48, 52, 55, 60, 64, 67, 72].map(|p| Note::new(p, 0)).into();
        piano.staves[0].bars[0].voices[0].beats = vec![
            Beat::new(half, chord),
            Beat::new(
                half,
                vec![Note {
                    tied: true,
                    ..Note::new(72, 0)
                }],
            ),
        ];
        score.tracks.push(piano);
        score.validate().unwrap();
        let t = compile(&score, 1, &Options::default());
        let notes: Vec<(u8, f64)> = t.notes.iter().map(|n| (n.pitch, n.duration)).collect();
        assert_eq!(
            notes,
            [
                (48, 1.0),
                (52, 1.0),
                (55, 1.0),
                (60, 1.0),
                (64, 1.0),
                (67, 1.0),
                (72, 2.0)
            ]
        );
        assert!(t.notes.iter().all(|n| n.channel == 1));
    }

    #[test]
    fn grace_notes_and_trills() {
        let grace = |on_beat| Some(Grace::single(5, 3, on_beat, true));
        let score = quarters(vec![
            Note::new(5, 0),
            with(5, |e| e.grace = grace(false)),
            with(5, |e| e.grace = grace(true)),
            with(5, |e| {
                e.trill = Some(Trill {
                    fret: 7,
                    speed: NoteValue::Sixteenth,
                })
            }),
        ]);
        let t = compile(&score, 0, &Options::default());
        let events: Vec<(f64, u8)> = t.notes.iter().map(|n| (n.time, n.pitch)).collect();
        // Before the beat: the grace note ends as the main note starts on time.
        assert_eq!(events[1], (0.5 - GRACE_LENGTH, 43));
        assert_eq!(events[2], (0.5, 45));
        // On the beat: the grace note takes the main note's place and pushes it back.
        assert_eq!(events[3], (1.0, 43));
        assert_eq!(events[4], (1.0 + GRACE_LENGTH, 45));
        assert!(t.notes[2].soft && t.notes[4].soft);
        // A sixteenth-note trill flips pitch three times within a quarter note.
        let trill = &t.notes[5].pitch_curve;
        assert_eq!(trill.len(), 8);
        assert_eq!(trill[1], (0.125, 2.0));
        assert_eq!(trill[3], (0.25, 0.0));
    }

    #[test]
    fn several_grace_notes_play_in_turn_on_their_own_strings() {
        use gtr_model::GraceNote;

        let score = quarters(vec![
            Note::new(5, 0),
            with(5, |e| {
                e.grace = Some(Grace {
                    notes: vec![
                        GraceNote { string: 4, fret: 2 },
                        GraceNote { string: 5, fret: 3 },
                        GraceNote { string: 5, fret: 4 },
                    ],
                    on_beat: false,
                    hammer: true,
                })
            }),
        ]);
        let t = compile(&score, 0, &Options::default());
        let events: Vec<(f64, u8, bool)> = (t.notes[1..].iter())
            .map(|n| (n.time, n.pitch, n.soft))
            .collect();
        // Three notes share the time one would take, ending as the main note starts. The
        // first is on the next string up; a note is hammered only from one on its string.
        let step = GRACE_LENGTH;
        assert_eq!(events[0], (0.5 - step * 3.0, 47, false));
        assert_eq!(events[1], (0.5 - step * 2.0, 43, false));
        assert_eq!(events[2], (0.5 - step, 44, true));
        assert_eq!(events[3], (0.5, 45, true));
        assert!(t.notes[1..4].iter().all(|n| n.duration == step));
    }

    #[test]
    fn harmonics_on_a_stopped_string_sound_the_overtone_touched() {
        let harmonic = |kind, touch| {
            with(5, move |e| {
                e.harmonic = Some(kind);
                e.harmonic_touch = touch;
            })
        };
        let score = quarters(vec![
            harmonic(Harmonic::Artificial, None),
            harmonic(Harmonic::Artificial, Some(7)),
            harmonic(Harmonic::Tapped, Some(5)),
            harmonic(Harmonic::Pinch, None),
        ]);
        let t = compile(&score, 0, &Options::default());
        let pitches: Vec<u8> = t.notes.iter().map(|n| n.pitch).collect();
        // A on the low E string: an octave up, an octave and a fifth, and two octaves,
        // which is also where a pinch harmonic sounds unless it says otherwise.
        assert_eq!(pitches, [57, 64, 69, 69]);
        assert!(t.notes.iter().all(|n| n.tone == Tone::Harmonic));
    }
}
