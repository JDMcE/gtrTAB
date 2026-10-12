//! MIDI export, from the events the synth plays: the piece is compiled as for playback and
//! every note, bend and change of the mix is written where it falls. Time is kept in bars
//! and beats as the score has them, with the tempo written out wherever it changes, so
//! that a held beat or a tempo that moves by degrees is in the file's tempo and the bars
//! still line up in whatever opens it.

use gtr_audio::{NoteEvent, Options, Timeline, compile};
use gtr_model::{Fade, Mix, Rational, Score, play_order, timings};

use super::TICKS_PER_QUARTER;
use crate::Skipped;

/// How far the pitch wheel is set to reach, in semitones: as far as the whammy bar goes.
const BEND_RANGE: f32 = 12.0;
/// The velocity of a note of ordinary loudness.
const VELOCITY: f32 = 80.0;
/// Seconds between the steps a curve is written in.
const CURVE_STEP: f64 = 0.01;

/// Something a track does at a moment. The class orders what happens on the same tick:
/// notes end before anything else, and start after it.
struct Event {
    tick: u64,
    class: u8,
    bytes: Vec<u8>,
}

fn event(tick: u64, class: u8, bytes: impl Into<Vec<u8>>) -> Event {
    Event {
        tick,
        class,
        bytes: bytes.into(),
    }
}

fn meta(tick: u64, kind: u8, data: &[u8]) -> Event {
    let mut bytes = vec![0xFF, kind];
    quantity(&mut bytes, data.len() as u64);
    bytes.extend(data);
    event(tick, 1, bytes)
}

/// A number as MIDI writes lengths and times: seven bits to a byte, most significant
/// first, every byte but the last with its top bit set.
fn quantity(out: &mut Vec<u8>, value: u64) {
    let mut bytes = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        bytes.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    out.extend(bytes.into_iter().rev());
}

fn chunk(out: &mut Vec<u8>, mut events: Vec<Event>, end: u64) {
    events.sort_by_key(|e| (e.tick, e.class));
    let mut data = Vec::new();
    let mut last = 0;
    for event in events {
        quantity(&mut data, event.tick - last);
        last = event.tick;
        data.extend(event.bytes);
    }
    quantity(&mut data, end.saturating_sub(last));
    data.extend([0xFF, 0x2F, 0x00]);
    out.extend(b"MTrk");
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
}

/// Where each moment of the piece falls in the file: seconds of playback against ticks.
struct Clock {
    /// Seconds and ticks at each place the two were lined up, in order.
    points: Vec<(f64, f64)>,
}

impl Clock {
    fn tick(&self, seconds: f64) -> u64 {
        let after = self.points.partition_point(|p| p.0 <= seconds);
        let (from, to) = match (after.checked_sub(1), self.points.get(after)) {
            (Some(before), Some(to)) => (self.points[before], *to),
            // Past the end the last tempo goes on.
            _ => {
                let count = self.points.len();
                match count {
                    0 | 1 => return 0,
                    _ => (self.points[count - 2], self.points[count - 1]),
                }
            }
        };
        let share = match to.0 > from.0 {
            true => (seconds - from.0) / (to.0 - from.0),
            false => 0.0,
        };
        (from.1 + share * (to.1 - from.1)).round().max(0.0) as u64
    }
}

fn ticks(length: Rational) -> f64 {
    let whole = f64::from(TICKS_PER_QUARTER) * 4.0;
    *length.numer() as f64 / *length.denom() as f64 * whole
}

pub(super) fn write(score: &Score, skipped: &mut Skipped) -> Vec<u8> {
    let timeline: Timeline = compile(score, 0, &Options::default());
    let masters = &score.master_bars;
    let order = play_order(masters, false);

    // The conductor's track: the bars as they are played, each with its time and key
    // where those change, and the tempo of every stretch of every bar.
    let mut conductor = Vec::new();
    if !score.info.title.is_empty() {
        conductor.push(meta(0, 0x03, score.info.title.as_bytes()));
    }
    if !score.info.copyright.is_empty() {
        conductor.push(meta(0, 0x02, score.info.copyright.as_bytes()));
    }
    let mut clock = Clock { points: Vec::new() };
    let (mut seconds, mut tick) = (0.0f64, 0.0f64);
    let mut written: (Option<gtr_model::TimeSignature>, Option<i8>, u32) = (None, None, 0);
    // Tempo is looked at every sixty-fourth of a whole note.
    let step = Rational::new(1, 64);
    for timing in timings(score, &order, 1.0) {
        let master = &masters[timing.bar];
        let at = tick.round() as u64;
        if written.0 != Some(master.time_signature) {
            let time = master.time_signature;
            let power = time.denominator.trailing_zeros() as u8;
            conductor.push(meta(at, 0x58, &[time.numerator, power, 24, 8]));
            written.0 = Some(time);
        }
        if written.1 != Some(master.key_signature) {
            conductor.push(meta(at, 0x59, &[master.key_signature as u8, 0]));
            written.1 = Some(master.key_signature);
        }
        let length = master.time_signature.bar_length();
        let mut position = Rational::from_integer(0);
        while position < length {
            let next = (position + step).min(length);
            let lasts = timing.at(next) - timing.at(position);
            let span = ticks(next - position);
            clock
                .points
                .push((seconds + timing.at(position), tick + ticks(position)));
            // Microseconds to the quarter note over this stretch.
            let tempo = (lasts / span * f64::from(TICKS_PER_QUARTER) * 1e6).round();
            let tempo = tempo.clamp(1.0, 16_777_215.0) as u32;
            if tempo != written.2 {
                let at = (tick + ticks(position)).round() as u64;
                conductor.push(meta(at, 0x51, &tempo.to_be_bytes()[1..]));
                written.2 = tempo;
            }
            position = next;
        }
        seconds += timing.length;
        tick += ticks(length);
    }
    clock.points.push((seconds, tick));
    let end = tick.round() as u64;

    // Two channels to a track while there are enough, the second for the notes whose
    // pitch moves, so that a bend does not carry the chord under it along. The tenth
    // channel is percussion's.
    let heard: Vec<usize> = (0..score.tracks.len())
        .filter(|index| timeline.channels[*index].audible)
        .collect();
    skipped.add_if(heard.len() < score.tracks.len(), "tracks that are muted");
    let pitched = (heard.iter())
        .filter(|index| !score.tracks[**index].instrument.is_percussion())
        .count();
    let free: Vec<u8> = (0..16).filter(|channel| *channel != 9).collect();
    let pair = pitched * 2 <= free.len();
    skipped.add_if(pitched > free.len(), "a channel of its own for every track");
    let mut next = 0;

    let mut out = Vec::new();
    out.extend(b"MThd");
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend((heard.len() as u16 + 1).to_be_bytes());
    out.extend((TICKS_PER_QUARTER as u16).to_be_bytes());
    chunk(&mut out, conductor, end);

    for index in heard {
        let track = &score.tracks[index];
        let drums = track.instrument.is_percussion();
        let (main, moving) = match drums {
            true => (9, 9),
            false => {
                let main = free[next % free.len()];
                next += 1;
                let moving = match pair {
                    true => free[next % free.len()],
                    false => main,
                };
                next += usize::from(pair);
                (main, moving)
            }
        };
        let mut events = vec![meta(0, 0x03, track.name.as_bytes())];
        let volume = |volume: f64| {
            (volume * 127.0 / f64::from(Mix::MAX_VOLUME))
                .round()
                .clamp(0.0, 127.0) as u8
        };
        let pan = |pan: f64| (64.0 + pan * 63.0).round().clamp(0.0, 127.0) as u8;
        let mut channels = vec![main];
        if moving != main {
            channels.push(moving);
        }
        for channel in &channels {
            if !drums {
                let program = track.instrument.midi_program();
                events.push(event(0, 1, [0xC0 | channel, program]));
                // How far the pitch wheel reaches.
                for (controller, value) in [(101, 0), (100, 0), (6, BEND_RANGE as u8), (38, 0)] {
                    events.push(event(0, 1, [0xB0 | channel, controller, value]));
                }
            }
            events.push(event(
                0,
                1,
                [0xB0 | channel, 7, volume(track.mix.volume.into())],
            ));
            events.push(event(
                0,
                1,
                [0xB0 | channel, 10, pan(f64::from(track.mix.pan) / 100.0)],
            ));
        }
        for change in timeline.automation.iter().filter(|a| a.channel == index) {
            let at = clock.tick(change.time);
            for channel in &channels {
                if let Some(gain) = change.gain {
                    // The mixer's gain is the square of its volume, with 75 as one.
                    let level = f64::from(gain.max(0.0)).sqrt() * f64::from(Mix::UNITY);
                    events.push(event(at, 1, [0xB0 | channel, 7, volume(level)]));
                }
                if let Some(to) = change.pan {
                    events.push(event(at, 1, [0xB0 | channel, 10, pan(to.into())]));
                }
            }
        }
        for note in timeline.notes.iter().filter(|n| n.channel == index) {
            let moves = !note.pitch_curve.is_empty() || note.vibrato > 0.0 || note.fade.is_some();
            let channel = match moves && !drums {
                true => moving,
                false => main,
            };
            note_events(&mut events, note, channel, &clock);
        }
        chunk(&mut out, events, end);
    }
    out
}

/// A note: where it starts and ends, and what its pitch and its loudness do in between.
fn note_events(events: &mut Vec<Event>, note: &NoteEvent, channel: u8, clock: &Clock) {
    let start = clock.tick(note.time);
    let end = clock.tick(note.time + note.duration).max(start + 1);
    let pitch = note.pitch.min(127);
    let velocity = (note.velocity * VELOCITY).round().clamp(1.0, 127.0) as u8;
    events.push(event(start, 2, [0x90 | channel, pitch, velocity]));
    events.push(event(end, 0, [0x80 | channel, pitch, 0]));

    let wheel = |tick: u64, semitones: f32| {
        let value = (8192.0 + semitones / BEND_RANGE * 8192.0)
            .round()
            .clamp(0.0, 16383.0) as u16;
        event(
            tick,
            1,
            [0xE0 | channel, (value & 0x7F) as u8, (value >> 7) as u8],
        )
    };
    if !note.pitch_curve.is_empty() {
        // The curve in small steps, each on the line between the points either side.
        let at = |time: f64| {
            let curve = &note.pitch_curve;
            let after = curve.partition_point(|p| f64::from(p.0) <= time);
            match (after.checked_sub(1).map(|i| curve[i]), curve.get(after)) {
                (Some(from), Some(to)) => {
                    let share = (time as f32 - from.0) / (to.0 - from.0).max(1e-6);
                    from.1 + share * (to.1 - from.1)
                }
                (Some(last), None) => last.1,
                (None, Some(first)) => first.1,
                (None, None) => 0.0,
            }
        };
        let steps = ((note.duration / CURVE_STEP).ceil() as usize).clamp(1, 2000);
        let mut last = None;
        for step in 0..=steps {
            let time = note.duration * step as f64 / steps as f64;
            let moved = wheel(clock.tick(note.time + time).min(end), at(time));
            if last.as_ref() != Some(&moved.bytes) {
                last = Some(moved.bytes.clone());
                events.push(moved);
            }
        }
        // The wheel is let go when the note ends.
        events.push(Event {
            class: 0,
            ..wheel(end, 0.0)
        });
    }
    if note.vibrato > 0.0 {
        // Vibrato is the modulation wheel's, deeper for a wider one.
        let depth = (note.vibrato * 127.0).round().clamp(32.0, 127.0) as u8;
        events.push(event(start, 1, [0xB0 | channel, 1, depth]));
        events.push(event(end, 0, [0xB0 | channel, 1, 0]));
    }
    if let Some(fade) = note.fade {
        // A fade is the expression pedal's, moved over the note in a few steps.
        const STEPS: u64 = 16;
        for step in 0..=STEPS {
            let through = step as f64 / STEPS as f64;
            let level = match fade {
                Fade::In => through,
                Fade::Out => 1.0 - through,
                Fade::Swell => 1.0 - (2.0 * through - 1.0).abs(),
            };
            let tick = start + (end - start) * step / STEPS;
            events.push(event(
                tick,
                1,
                [0xB0 | channel, 11, (level * 127.0).round() as u8],
            ));
        }
        events.push(event(end, 0, [0xB0 | channel, 11, 127]));
    }
}
