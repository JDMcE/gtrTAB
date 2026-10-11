use std::f32::consts::{FRAC_PI_4, SQRT_2, TAU};

use crate::{Channel, NoteEvent, Timbre, Timeline, Tone};

const NOTE_GAIN: f32 = 0.22;
const CLICK_GAIN: f32 = 0.35;
/// Seconds a string keeps ringing after its note ends.
const RELEASE: f32 = 0.06;
/// Seconds of silence appended so the last notes can die away.
const TAIL: f64 = 0.3;
/// Samples between updates of a bending note's pitch.
const CONTROL_PERIOD: usize = 32;
const VIBRATO_HZ: f32 = 5.5;
/// Seconds into a note before vibrato starts.
const VIBRATO_DELAY: f32 = 0.08;

/// Two channels of samples in `-1.0..=1.0`, of equal length.
#[derive(Debug, Clone, PartialEq)]
pub struct Stereo {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Number of samples a rendering of the timeline has.
fn length(timeline: &Timeline, sample_rate: u32) -> usize {
    ((timeline.duration + TAIL) * f64::from(sample_rate)).ceil() as usize
}

/// Renders one track on its own: synthesised and given its instrument's character, but not
/// yet placed in the mix, so its volume and pan can still change while it plays. Returns
/// nothing for a track without notes.
pub fn render_channel(timeline: &Timeline, index: usize, sample_rate: u32) -> Vec<f32> {
    let rate = sample_rate as f32;
    let total = length(timeline, sample_rate);
    let timbre = (timeline.channels.get(index)).map_or(Channel::default().timbre, |c| c.timbre);
    let mut out = Vec::new();
    for (seed, note) in timeline.notes.iter().enumerate() {
        if note.channel != index {
            continue;
        }
        let start = (note.time * f64::from(sample_rate)) as usize;
        // A drum rings for as long as the drum does, however long the note is written.
        let seconds = match timbre {
            Timbre::Drums => drum_ring(note.pitch),
            _ => note.duration as f32 + RELEASE,
        };
        let end = (start + (seconds * rate) as usize).min(total);
        if start < end {
            if out.is_empty() {
                out = vec![0.0f32; total];
            }
            match timbre {
                Timbre::Drums => drum(&mut out[start..end], note, rate, seed as u32),
                _ => pluck(&mut out[start..end], note, timbre, rate, seed as u32),
            }
        }
    }
    shape(&mut out, timbre, rate);
    out
}

/// Renders the metronome and count-in clicks, or nothing if there are none.
pub fn render_clicks(timeline: &Timeline, sample_rate: u32) -> Vec<f32> {
    let rate = sample_rate as f32;
    let total = length(timeline, sample_rate);
    let mut out = match timeline.clicks.is_empty() {
        true => Vec::new(),
        false => vec![0.0f32; total],
    };
    for click in &timeline.clicks {
        let start = (click.time * f64::from(sample_rate)) as usize;
        let frequency = if click.accent { 1760.0 } else { 1320.0 };
        for i in 0..((0.04 * rate) as usize).min(total.saturating_sub(start)) {
            let t = i as f32 / rate;
            out[start + i] += CLICK_GAIN * (TAU * frequency * t).sin() * (-t * 120.0).exp();
        }
    }
    out
}

/// Renders a timeline to stereo: every audible track placed by its volume and pan, with
/// the clicks on top.
pub fn render(timeline: &Timeline, sample_rate: u32) -> Stereo {
    let total = length(timeline, sample_rate);
    let mut left = vec![0.0f32; total];
    let mut right = vec![0.0f32; total];
    let channel_count = (timeline.notes.iter())
        .map(|n| n.channel + 1)
        .max()
        .unwrap_or(0);
    for index in 0..channel_count {
        let channel = (timeline.channels.get(index).copied()).unwrap_or_default();
        if !channel.audible {
            continue;
        }
        // The track's place in the mix can change at any beat.
        let mut changes = (timeline.automation.iter())
            .filter(|change| change.channel == index)
            .map(|change| ((change.time * f64::from(sample_rate)) as usize, change))
            .peekable();
        let mut placed = channel;
        let (mut gain_left, mut gain_right) = pan_gains(&placed);
        for (i, sample) in render_channel(timeline, index, sample_rate)
            .iter()
            .enumerate()
        {
            while let Some((_, change)) = changes.next_if(|(at, _)| *at <= i) {
                placed.gain = change.gain.unwrap_or(placed.gain);
                placed.pan = change.pan.unwrap_or(placed.pan);
                (gain_left, gain_right) = pan_gains(&placed);
            }
            left[i] += gain_left * sample;
            right[i] += gain_right * sample;
        }
    }
    for (i, sample) in render_clicks(timeline, sample_rate).iter().enumerate() {
        left[i] += sample;
        right[i] += sample;
    }
    for sample in left.iter_mut().chain(&mut right) {
        *sample = sample.tanh();
    }
    Stereo { left, right }
}

/// Gains for the two sides. A centred track plays at full level on both, and one panned
/// all the way sounds as loud from its one side.
fn pan_gains(channel: &Channel) -> (f32, f32) {
    let angle = (channel.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (
        channel.gain * angle.cos() * SQRT_2,
        channel.gain * angle.sin() * SQRT_2,
    )
}

/// Runs a whole track through its amplifier: overdriven sounds are clipped, which is what
/// makes chords growl, and then filtered the way a speaker cabinet would.
fn shape(bus: &mut [f32], timbre: Timbre, rate: f32) {
    let (drive, level) = match timbre {
        Timbre::Overdrive => (5.0, 0.42),
        Timbre::Distortion => (16.0, 0.3),
        _ => return,
    };
    let smoothing = 1.0 - (-TAU * 3_800.0 / rate).exp();
    let mut filtered = 0.0;
    for sample in bus {
        filtered += smoothing * ((*sample * drive).tanh() * level - filtered);
        *sample = filtered;
    }
}

/// Pitch offset in semitones at `t` seconds into a note, from its bend or slide curve.
fn curve_at(curve: &[(f32, f32)], t: f32) -> f32 {
    let Some(&(first_time, first_value)) = curve.first() else {
        return 0.0;
    };
    if t <= first_time {
        return first_value;
    }
    for pair in curve.windows(2) {
        let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
        if t < t1 {
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0).max(1e-6);
        }
    }
    curve[curve.len() - 1].1
}

/// How a piece of the drum kit is made up: a pitched body and a burst of noise.
struct DrumVoice {
    /// Seconds until the piece has died away.
    ring: f32,
    /// Pitch of the body in Hz where it ends up, and how many times higher it starts.
    tone: f32,
    sweep: f32,
    /// Loudness of the body and of the noise.
    body: f32,
    noise: f32,
    /// How much of the noise's low end is kept: 1.0 for a full hiss, 0.0 for only the
    /// sizzle of a cymbal.
    dark: f32,
}

/// The make-up of a piece of the kit, by its General MIDI percussion number.
fn drum_voice(piece: u8) -> DrumVoice {
    let voice = |ring, tone, sweep, body, noise, dark| DrumVoice {
        ring,
        tone,
        sweep,
        body,
        noise,
        dark,
    };
    match piece {
        35 | 36 => voice(0.3, 48.0, 3.0, 1.6, 0.1, 1.0),
        37 => voice(0.06, 800.0, 1.0, 0.5, 0.5, 0.3),
        38 | 40 => voice(0.22, 190.0, 1.4, 0.7, 1.0, 0.5),
        39 => voice(0.12, 1_000.0, 1.0, 0.0, 1.0, 0.3),
        41 => voice(0.4, 80.0, 1.6, 1.3, 0.1, 1.0),
        43 => voice(0.4, 95.0, 1.6, 1.3, 0.1, 1.0),
        45 => voice(0.35, 115.0, 1.6, 1.3, 0.1, 1.0),
        47 => voice(0.35, 140.0, 1.6, 1.2, 0.1, 1.0),
        48 => voice(0.3, 165.0, 1.6, 1.2, 0.1, 1.0),
        50 => voice(0.3, 195.0, 1.6, 1.2, 0.1, 1.0),
        42 => voice(0.06, 0.0, 1.0, 0.0, 0.7, 0.0),
        44 => voice(0.04, 0.0, 1.0, 0.0, 0.5, 0.0),
        46 => voice(0.45, 0.0, 1.0, 0.0, 0.7, 0.0),
        49 | 57 | 52 => voice(1.5, 0.0, 1.0, 0.0, 0.8, 0.05),
        55 => voice(0.7, 0.0, 1.0, 0.0, 0.7, 0.0),
        51 | 59 => voice(0.9, 3_400.0, 1.0, 0.12, 0.35, 0.0),
        53 => voice(0.7, 2_600.0, 1.0, 0.5, 0.1, 0.0),
        54 => voice(0.15, 0.0, 1.0, 0.0, 0.6, 0.0),
        56 => voice(0.2, 560.0, 1.0, 0.8, 0.0, 0.0),
        _ => voice(0.08, 400.0, 1.0, 0.4, 0.4, 0.3),
    }
}

/// Seconds a piece of the kit sounds for.
fn drum_ring(piece: u8) -> f32 {
    drum_voice(piece).ring
}

/// Adds one drum hit to `out`: a sine that drops in pitch for the body of a drum, and
/// filtered noise for snares and cymbals, both dying away over the piece's ring time.
fn drum(out: &mut [f32], note: &NoteEvent, rate: f32, seed: u32) {
    let voice = drum_voice(note.pitch);
    let gain = NOTE_GAIN * 1.6 * note.velocity;
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1;
    let mut phase = 0.0f32;
    let (mut previous, mut low) = (0.0f32, 0.0f32);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / rate;
        let fade = (-5.0 * t / voice.ring).exp();
        // The body starts sharp and settles within a few hundredths of a second.
        let frequency = voice.tone * (1.0 + (voice.sweep - 1.0) * (-t * 40.0).exp());
        phase += TAU * frequency / rate;
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let white = (state >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
        // The difference between successive samples keeps only the top of the noise.
        let bright = white - previous;
        previous = white;
        low += 0.25 * (white - low);
        let hiss = bright * (1.0 - voice.dark) * 0.6 + low * voice.dark;
        *sample += gain * fade * (voice.body * phase.sin() + voice.noise * hiss);
    }
}

/// Adds one plucked note to `out` using the Karplus-Strong string model: a burst of noise
/// circulating in a delay line one period long, losing its high frequencies on each pass.
/// Changing the length of the delay while the note sounds bends its pitch.
fn pluck(out: &mut [f32], note: &NoteEvent, timbre: Timbre, rate: f32, seed: u32) {
    let frequency = 440.0 * 2f32.powf((f32::from(note.pitch) - 69.0) / 12.0);
    let base_period = rate / frequency;
    let steady = note.pitch_curve.is_empty() && note.vibrato == 0.0;

    // Room for the longest period the note can reach when bent or slid downwards.
    let lowest = (note.pitch_curve.iter())
        .map(|&(_, semitones)| semitones)
        .fold(0.0f32, f32::min)
        - note.vibrato;
    let size = (base_period * 2f32.powf(-lowest / 12.0)).ceil() as usize + 4;

    // The excitation: bright noise for a picked note, duller for a hammered or muted one,
    // and a single sine cycle for the bell-like tone of a harmonic.
    let smoothing = match (note.tone, note.soft) {
        (Tone::PalmMute, _) => 4,
        (_, true) => 3,
        _ => 1,
    } + match timbre {
        Timbre::Steel | Timbre::Drums => 0,
        Timbre::Clean | Timbre::Overdrive | Timbre::Distortion => 1,
        Timbre::Nylon | Timbre::Bass | Timbre::Piano => 2,
    };
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1;
    let mut line: Vec<f32> = (0..size)
        .map(|i| {
            if note.tone == Tone::Harmonic {
                return (TAU * i as f32 / base_period).sin();
            }
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 23) as f32 - 1.0
        })
        .collect();
    for _ in 0..smoothing {
        let mut previous = 0.0;
        for sample in &mut line {
            previous = 0.5 * (*sample + previous);
            *sample = previous;
        }
    }

    // How much of the string's energy survives each trip round the loop. Lower strings
    // ring longer; muting damps the string hard.
    let decay = match note.tone {
        Tone::Dead => 0.80,
        Tone::PalmMute => 0.955 - 0.01 * (frequency / 400.0).min(1.0),
        Tone::Harmonic => 0.9992,
        Tone::Normal => match timbre {
            Timbre::Nylon => 0.997 - 0.006 * (frequency / 1500.0).min(1.0),
            // An amplifier's compression holds a driven note up.
            Timbre::Overdrive | Timbre::Distortion | Timbre::Bass | Timbre::Piano => {
                0.9992 - 0.002 * (frequency / 1500.0).min(1.0)
            }
            Timbre::Steel | Timbre::Clean | Timbre::Drums => {
                0.9985 - 0.004 * (frequency / 1500.0).min(1.0)
            }
        },
    };
    let gain = NOTE_GAIN * note.velocity * if note.soft { 0.8 } else { 1.0 };

    let release_start = out.len().saturating_sub((RELEASE * rate) as usize);
    let release_length = (out.len() - release_start).max(1) as f32;
    // The averaging filter itself delays by half a sample.
    let mut delay = base_period - 0.5;
    let mut filtered = 0.0;
    let mut write = 0;
    for (i, sample) in out.iter_mut().enumerate() {
        if !steady && i % CONTROL_PERIOD == 0 {
            let t = i as f32 / rate;
            let mut semitones = curve_at(&note.pitch_curve, t);
            if note.vibrato > 0.0 && t > VIBRATO_DELAY {
                let swell = ((t - VIBRATO_DELAY) * 6.0).min(1.0);
                semitones += note.vibrato * swell * (TAU * VIBRATO_HZ * t).sin();
            }
            delay = base_period * 2f32.powf(-semitones / 12.0) - 0.5;
        }

        // Read the line `delay` samples back, between two stored samples.
        let position = (write as f32 - delay).rem_euclid(size as f32);
        let index = position as usize % size;
        let fraction = position - position.floor();
        let current = line[index] + (line[(index + 1) % size] - line[index]) * fraction;

        let envelope = match i.checked_sub(release_start) {
            Some(into) => 1.0 - into as f32 / release_length,
            None => 1.0,
        };
        *sample += gain * envelope * current;

        line[write] = decay * 0.5 * (current + filtered);
        filtered = current;
        write = (write + 1) % size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_with(note: NoteEvent, channel: Channel) -> Stereo {
        let timeline = Timeline {
            channels: vec![channel],
            automation: Vec::new(),
            duration: note.duration,
            notes: vec![note],
            clicks: Vec::new(),
            beats: Vec::new(),
        };
        render(&timeline, 44_100)
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
    fn a_track_rendered_alone_is_the_mix_without_its_placing() {
        let timeline = Timeline {
            channels: vec![
                Channel::default(),
                Channel {
                    audible: false,
                    ..Channel::default()
                },
            ],
            automation: Vec::new(),
            duration: 0.5,
            notes: vec![NoteEvent::plain(0.0, 0.5, 45), {
                let mut other = NoteEvent::plain(0.0, 0.5, 52);
                other.channel = 1;
                other
            }],
            clicks: vec![crate::Click {
                time: 0.0,
                accent: true,
            }],
            beats: Vec::new(),
        };
        let first = render_channel(&timeline, 0, 44_100);
        let second = render_channel(&timeline, 1, 44_100);
        let clicks = render_clicks(&timeline, 44_100);
        // A muted track is still rendered, so that it can be unmuted while playing.
        assert!(energy(&second) > 1.0);
        assert!(render_channel(&timeline, 2, 44_100).is_empty());
        // The stereo mix is the audible track plus the clicks, through the limiter.
        let mix = render(&timeline, 44_100);
        assert_eq!(mix.left.len(), first.len());
        for i in (0..first.len()).step_by(97) {
            assert!((mix.left[i] - (first[i] + clicks[i]).tanh()).abs() < 1e-6);
        }
    }

    #[test]
    fn automation_moves_a_track_part_way_through() {
        let mut timeline = Timeline {
            channels: vec![Channel::default()],
            automation: vec![crate::Automation {
                channel: 0,
                time: 0.5,
                gain: None,
                pan: Some(-1.0),
            }],
            duration: 1.0,
            notes: vec![NoteEvent::plain(0.0, 1.0, 45)],
            clicks: Vec::new(),
            beats: Vec::new(),
        };
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
    fn drums_sound_like_their_pieces() {
        let hit = |piece: u8| {
            let channel = Channel {
                timbre: Timbre::Drums,
                ..Channel::default()
            };
            // A drum rings on its own time: the note here is far too short to hold it.
            let mut timeline = Timeline {
                channels: vec![channel],
                automation: Vec::new(),
                duration: 2.0,
                notes: vec![NoteEvent::plain(0.0, 0.01, piece)],
                clicks: Vec::new(),
                beats: Vec::new(),
            };
            timeline.notes[0].channel = 0;
            render(&timeline, 44_100).left
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
