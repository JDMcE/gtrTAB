//! SoundFont 2 files: reading one, and playing a note from its samples.
//!
//! This reads what a plucked or struck instrument needs: which sample to play for a key and
//! how hard it is hit, its tuning, loop, level, pan, volume envelope and filter. A SoundFont
//! can say a good deal more (modulators, a second envelope, vibrato of its own, reverb and
//! chorus), and none of that is followed, so a bank that leans on it sounds plainer here
//! than in a full SoundFont synth.

use std::f32::consts::{FRAC_PI_4, TAU};
use std::fmt;
use std::rc::Rc;

use crate::{NoteEvent, Tone};

/// Level of a sample played at full velocity with no attenuation, set so that a sampled
/// instrument is about as loud as a modelled one.
const SAMPLE_GAIN: f32 = 1.0;
/// Samples between updates of a bending note's pitch.
const CONTROL_PERIOD: usize = 32;
const VIBRATO_HZ: f32 = 5.5;
/// Seconds into a note before vibrato starts.
const VIBRATO_DELAY: f32 = 0.08;
/// Seconds of a sample skipped for a note that is not picked, to leave out the pick.
const SOFT_START: f32 = 0.012;
/// Seconds a voice takes to stop when another of its class cuts it off, as a closed hi-hat
/// does an open one.
const CHOKE: f32 = 0.01;
/// The level below which a dying voice is over.
const SILENCE: f32 = 1e-4;
/// The bank that holds the drum kits.
pub(crate) const DRUM_BANK: u16 = 128;

// The generators this reader follows, by their numbers in the SoundFont specification.
const START_OFFSET: u16 = 0;
const END_OFFSET: u16 = 1;
const LOOP_START_OFFSET: u16 = 2;
const LOOP_END_OFFSET: u16 = 3;
const START_COARSE: u16 = 4;
const FILTER_CUTOFF: u16 = 8;
const FILTER_Q: u16 = 9;
const END_COARSE: u16 = 12;
const PAN: u16 = 17;
const DELAY: u16 = 33;
const ATTACK: u16 = 34;
const HOLD: u16 = 35;
const DECAY: u16 = 36;
const SUSTAIN: u16 = 37;
const RELEASE: u16 = 38;
const INSTRUMENT: u16 = 41;
const KEY_RANGE: u16 = 43;
const VELOCITY_RANGE: u16 = 44;
const LOOP_START_COARSE: u16 = 45;
const FIXED_KEY: u16 = 46;
const FIXED_VELOCITY: u16 = 47;
const ATTENUATION: u16 = 48;
const LOOP_END_COARSE: u16 = 50;
const COARSE_TUNE: u16 = 51;
const FINE_TUNE: u16 = 52;
const SAMPLE: u16 = 53;
const SAMPLE_MODES: u16 = 54;
const SCALE_TUNING: u16 = 56;
const EXCLUSIVE_CLASS: u16 = 57;
const ROOT_KEY: u16 = 58;

/// Why a file could not be used as a SoundFont.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFontError {
    NotASoundFont,
    /// A SoundFont 3 file, whose samples are compressed.
    Compressed,
    Malformed(&'static str),
}

impl fmt::Display for SoundFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundFontError::NotASoundFont => write!(f, "not a SoundFont file"),
            SoundFontError::Compressed => {
                write!(f, "compressed SoundFonts (.sf3) are not supported")
            }
            SoundFontError::Malformed(what) => write!(f, "the SoundFont is damaged: {what}"),
        }
    }
}

impl std::error::Error for SoundFontError {}

/// Whether a sample goes round its loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Looping {
    Off,
    On,
    /// Round the loop while the note is held, then on to the end of the sample.
    UntilRelease,
}

/// One sample of a preset and how it is played: the keys and velocities it answers to, and
/// everything the preset and its instrument say about it, already added up.
#[derive(Debug, Clone, PartialEq)]
struct Region {
    keys: (u8, u8),
    velocities: (u8, u8),
    /// Where the sample starts and ends among all the samples, and its loop.
    start: usize,
    end: usize,
    loop_start: usize,
    loop_end: usize,
    looping: Looping,
    sample_rate: f32,
    /// The key at which the sample plays at its own pitch.
    root_key: f32,
    /// Cents the sample is tuned by, and cents between one key and the next.
    tune: f32,
    scale_tuning: f32,
    /// Linear gain from the region's attenuation.
    gain: f32,
    /// From -1.0 (left) to 1.0 (right).
    pan: f32,
    /// The volume envelope, in seconds; `sustain` is the linear level it settles at.
    delay: f32,
    attack: f32,
    hold: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    /// Low-pass filter cutoff in Hz, and its resonance.
    cutoff: f32,
    resonance: f32,
    /// Voices of the same class, other than zero, cut each other off.
    class: u16,
    fixed_key: Option<u8>,
    fixed_velocity: Option<u8>,
}

struct Preset {
    bank: u16,
    program: u16,
    regions: Vec<Region>,
}

/// A SoundFont 2 bank, ready to be played from.
pub struct SoundFont {
    name: String,
    samples: Vec<i16>,
    presets: Vec<Preset>,
}

fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

fn u32_at(data: &[u8], at: usize) -> usize {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
}

/// The chunks of a RIFF list: their four-letter names and what they hold.
fn chunks(mut data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    std::iter::from_fn(move || {
        let head = data.get(..8)?;
        let size = u32_at(head, 4);
        let body = data.get(8..8 + size)?;
        // Chunks start on even bytes.
        data = data.get(8 + size + (size & 1)..).unwrap_or(&[]);
        Some((&head[..4], body))
    })
}

/// Seconds for a time in the SoundFont's unit, of which 1200 double it.
fn seconds(timecents: i32) -> f32 {
    2f32.powf(timecents.clamp(-12_000, 8_000) as f32 / 1200.0)
}

/// The zones of presets or of instruments: for each, a run of generators.
struct Zones<'a> {
    /// Index of each zone's first generator; one more entry than there are zones.
    bags: Vec<usize>,
    /// Four bytes each: the generator's number and its amount.
    generators: &'a [u8],
}

/// The generators of one zone.
#[derive(Clone, Copy)]
struct Zone<'a>(&'a [u8]);

impl Zone<'_> {
    fn raw(&self, generator: u16) -> Option<u16> {
        (self.0.as_chunks::<4>().0.iter().map(|c| &c[..]))
            .find(|g| u16_at(g, 0) == generator)
            .map(|g| u16_at(g, 2))
    }

    fn get(&self, generator: u16) -> Option<i32> {
        self.raw(generator).map(|raw| i32::from(raw as i16))
    }

    /// A range of keys or velocities, lowest and highest.
    fn range(&self, generator: u16) -> Option<(u8, u8)> {
        self.raw(generator).map(|raw| (raw as u8, (raw >> 8) as u8))
    }

    /// The generator a zone ends with says what it is a zone of.
    fn ends_with(&self, generator: u16) -> bool {
        (self.0.as_chunks::<4>().0.last()).is_some_and(|g| u16_at(g, 0) == generator)
    }
}

impl<'a> Zones<'a> {
    fn new(bags: &[u8], generators: &'a [u8]) -> Zones<'a> {
        Zones {
            bags: bags
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| &c[..])
                .map(|b| usize::from(u16_at(b, 0)))
                .collect(),
            generators,
        }
    }

    /// The zones numbered `from..to`: the one that applies to all the others, if there is
    /// one, and the others, which each end with the generator `ends_with`.
    fn of(&self, from: usize, to: usize, ends_with: u16) -> (Zone<'a>, Vec<Zone<'a>>) {
        let mut global = Zone(&[]);
        let mut zones = Vec::new();
        for bag in from..to {
            let (Some(&start), Some(&end)) = (self.bags.get(bag), self.bags.get(bag + 1)) else {
                break;
            };
            let Some(generators) = self.generators.get(start * 4..end * 4) else {
                break;
            };
            let zone = Zone(generators);
            if zone.ends_with(ends_with) {
                zones.push(zone);
            } else if bag == from {
                global = zone;
            }
        }
        (global, zones)
    }
}

impl SoundFont {
    /// Reads a SoundFont 2 file.
    pub fn parse(bytes: &[u8]) -> Result<SoundFont, SoundFontError> {
        if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"sfbk" {
            return Err(SoundFontError::NotASoundFont);
        }
        let mut name = String::new();
        let mut samples: Option<&[u8]> = None;
        let mut parts: [&[u8]; 7] = [&[]; 7];
        const PARTS: [&[u8; 4]; 7] = [
            b"phdr", b"pbag", b"pgen", b"inst", b"ibag", b"igen", b"shdr",
        ];
        for (id, list) in chunks(&bytes[12..]) {
            if id != b"LIST" || list.len() < 4 {
                continue;
            }
            for (id, body) in chunks(&list[4..]) {
                match id {
                    b"ifil" if body.len() >= 2 && u16_at(body, 0) > 2 => {
                        return Err(SoundFontError::Compressed);
                    }
                    b"INAM" => {
                        let text = body.split(|&b| b == 0).next().unwrap_or(&[]);
                        name = String::from_utf8_lossy(text).trim().to_owned();
                    }
                    b"smpl" => samples = Some(body),
                    _ => {
                        if let Some(part) = PARTS.iter().position(|p| *p == id) {
                            parts[part] = body;
                        }
                    }
                }
            }
        }
        let samples = samples.ok_or(SoundFontError::Malformed("it has no samples"))?;
        let [phdr, pbag, pgen, inst, ibag, igen, shdr] = parts;
        if phdr.len() < 76 || inst.len() < 44 || shdr.len() < 92 {
            return Err(SoundFontError::Malformed("it has no presets"));
        }
        let samples: Vec<i16> = (samples.as_chunks::<2>().0.iter())
            .map(|s| i16::from_le_bytes(*s))
            .collect();

        let preset_zones = Zones::new(pbag, pgen);
        let instrument_zones = Zones::new(ibag, igen);
        let instruments: Vec<&[u8]> = inst.as_chunks::<22>().0.iter().map(|c| &c[..]).collect();
        let headers: Vec<&[u8]> = shdr.as_chunks::<46>().0.iter().map(|c| &c[..]).collect();
        let records: Vec<&[u8]> = phdr.as_chunks::<38>().0.iter().map(|c| &c[..]).collect();
        let mut presets = Vec::with_capacity(records.len());
        // The last record of each list only marks where the one before it ends.
        for pair in records.windows(2) {
            let (record, next) = (pair[0], pair[1]);
            let bags = (
                usize::from(u16_at(record, 24)),
                usize::from(u16_at(next, 24)),
            );
            let (preset_global, zones) = preset_zones.of(bags.0, bags.1, INSTRUMENT);
            let mut regions = Vec::new();
            for preset_zone in zones {
                let index = preset_zone.raw(INSTRUMENT).map_or(usize::MAX, usize::from);
                let (Some(instrument), Some(next)) =
                    (instruments.get(index), instruments.get(index + 1))
                else {
                    continue;
                };
                let bags = (
                    usize::from(u16_at(instrument, 20)),
                    usize::from(u16_at(next, 20)),
                );
                let (global, zones) = instrument_zones.of(bags.0, bags.1, SAMPLE);
                regions.extend(zones.into_iter().filter_map(|zone| {
                    let header = headers.get(usize::from(zone.raw(SAMPLE)?))?;
                    let layers = [zone, global, preset_zone, preset_global];
                    region(layers, header, samples.len())
                }));
            }
            presets.push(Preset {
                program: u16_at(record, 20),
                bank: u16_at(record, 22),
                regions,
            });
        }
        Ok(SoundFont {
            name,
            samples,
            presets,
        })
    }

    /// The name the bank gives itself.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How many presets the bank has.
    pub fn preset_count(&self) -> usize {
        self.presets.len()
    }

    fn preset(&self, bank: u16, program: u16) -> Option<&Preset> {
        (self.presets.iter()).find(|p| p.bank == bank && p.program == program)
    }

    /// Whether the bank has an instrument at `bank` and `program`.
    pub fn has(&self, bank: u16, program: u16) -> bool {
        self.preset(bank, program)
            .is_some_and(|p| !p.regions.is_empty())
    }

    /// One sample, from -1.0 to 1.0; silence outside the samples there are.
    fn sample(&self, index: usize) -> f32 {
        self.samples
            .get(index)
            .map_or(0.0, |&s| f32::from(s) / 32_768.0)
    }
}

/// Works out one region from its zone, the instrument's global zone, the preset's zone and
/// the preset's global zone, in that order, and the header of its sample. Returns nothing
/// for a sample that cannot be played.
fn region(layers: [Zone; 4], header: &[u8], sample_count: usize) -> Option<Region> {
    let [zone, global, preset_zone, preset_global] = layers;
    // What the instrument says, with the specification's value where it says nothing.
    let own = |generator: u16, default: i32| {
        (zone.get(generator).or(global.get(generator))).unwrap_or(default)
    };
    // The preset's values are added to the instrument's.
    let total = |generator: u16, default: i32| {
        let added = preset_zone.get(generator).or(preset_global.get(generator));
        own(generator, default) + added.unwrap_or(0)
    };
    let range = |generator: u16| {
        let own = zone.range(generator).or(global.range(generator));
        let outer = (preset_zone.range(generator)).or(preset_global.range(generator));
        let (own, outer) = (own.unwrap_or((0, 127)), outer.unwrap_or((0, 127)));
        (own.0.max(outer.0), own.1.min(outer.1))
    };
    let keys = range(KEY_RANGE);
    let velocities = range(VELOCITY_RANGE);
    // A sample kept in the sound card's ROM is not in the file.
    let in_rom = u16_at(header, 44) & 0x8000 != 0;
    if keys.0 > keys.1 || velocities.0 > velocities.1 || in_rom {
        return None;
    }

    let place = |at: usize, fine: u16, coarse: u16| {
        let moved = at as i64 + i64::from(own(fine, 0)) + 32_768 * i64::from(own(coarse, 0));
        usize::try_from(moved).ok()
    };
    let start = place(u32_at(header, 20), START_OFFSET, START_COARSE)?;
    let end = place(u32_at(header, 24), END_OFFSET, END_COARSE)?;
    if start >= end || end > sample_count {
        return None;
    }
    let loop_start = place(u32_at(header, 28), LOOP_START_OFFSET, LOOP_START_COARSE);
    let loop_end = place(u32_at(header, 32), LOOP_END_OFFSET, LOOP_END_COARSE);
    let (loop_start, loop_end) = (loop_start.unwrap_or(0), loop_end.unwrap_or(0));
    let loop_fits = start <= loop_start && loop_start < loop_end && loop_end <= end;
    let looping = match own(SAMPLE_MODES, 0) & 3 {
        1 if loop_fits => Looping::On,
        3 if loop_fits => Looping::UntilRelease,
        _ => Looping::Off,
    };

    let root_key = match (own(ROOT_KEY, -1), header[40]) {
        (key @ 0..=127, _) => key as f32,
        (_, key @ 0..=127) => f32::from(key),
        _ => 60.0,
    };
    let correction = i32::from(header[41] as i8);
    let fixed = |generator: u16| u8::try_from(own(generator, -1)).ok().filter(|v| *v <= 127);
    // Following other SoundFont synths, the attenuation counts for less than it says.
    let attenuation = total(ATTENUATION, 0).clamp(0, 1440) as f32 * 0.4;
    let cutoff = total(FILTER_CUTOFF, 13_500).clamp(1_500, 13_500);
    Some(Region {
        keys,
        velocities,
        start,
        end,
        loop_start,
        loop_end,
        looping,
        sample_rate: u32_at(header, 36).max(1) as f32,
        root_key,
        tune: (total(COARSE_TUNE, 0) * 100 + total(FINE_TUNE, 0) + correction) as f32,
        scale_tuning: total(SCALE_TUNING, 100) as f32,
        gain: 10f32.powf(-attenuation / 200.0),
        pan: total(PAN, 0).clamp(-500, 500) as f32 / 500.0,
        delay: seconds(total(DELAY, -12_000)),
        attack: seconds(total(ATTACK, -12_000)),
        hold: seconds(total(HOLD, -12_000)),
        decay: seconds(total(DECAY, -12_000)),
        sustain: 10f32.powf(-(total(SUSTAIN, 0).clamp(0, 1440) as f32) / 200.0),
        release: seconds(total(RELEASE, -12_000)),
        cutoff: 8.176 * 2f32.powf(cutoff as f32 / 1200.0),
        resonance: 10f32.powf(total(FILTER_Q, 0).clamp(0, 960) as f32 / 200.0),
        class: own(EXCLUSIVE_CLASS, 0).max(0) as u16,
        fixed_key: fixed(FIXED_KEY),
        fixed_velocity: fixed(FIXED_VELOCITY),
    })
}

/// A two-pole low-pass filter.
struct Lowpass {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Lowpass {
    fn new(cutoff: f32, resonance: f32, rate: f32) -> Lowpass {
        let w = TAU * cutoff / rate;
        let alpha = w.sin() / (2.0 * resonance * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let edge = (1.0 - w.cos()) / a0;
        Lowpass {
            b: [edge / 2.0, edge, edge / 2.0],
            a: [-2.0 * w.cos() / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, input: f32) -> f32 {
        let output = self.b[0] * input + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [output, self.y[0]];
        output
    }
}

/// Where a voice is in its volume envelope.
enum Stage {
    /// Silent, for this many more samples.
    Delay(usize),
    Attack,
    /// At full level, for this many more samples.
    Hold(usize),
    /// Falling to the sustain level and staying there.
    Decay,
    Release,
}

/// One sample of a SoundFont, sounding a note.
pub(crate) struct SampleVoice {
    font: Rc<SoundFont>,
    region: Region,
    /// Where in the samples the voice has got to, and how far it moves per sample of output
    /// at the note's written pitch.
    position: f64,
    step: f64,
    bend: f64,
    rate: f32,
    gains: (f32, f32),
    filter: Option<Lowpass>,
    stage: Stage,
    level: f32,
    /// What the level is multiplied by for each sample while it decays and while it is
    /// released.
    decay: f32,
    release: f32,
    /// A muted string dies away by itself: its level, and what that is multiplied by.
    muted: f32,
    damping: f32,
    pitch_curve: Vec<(f32, f32)>,
    vibrato: f32,
    at: usize,
    /// The sample at which the note is let go, and the most the voice may sound for.
    release_at: usize,
    limit: usize,
}

/// How a note is to be played from samples, apart from the note itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Playing {
    /// Whether the note stops when it is let go; a drum does not.
    pub held: bool,
    /// Scales the level the bank gives the note.
    pub trim: f32,
    /// Sample rate of the output.
    pub rate: f32,
    /// The most samples the voice may sound for.
    pub limit: usize,
}

/// What a level is multiplied by per sample to fall by 100 dB in `seconds`.
fn fall(seconds: f32, rate: f32) -> f32 {
    10f32.powf(-5.0 / (seconds * rate).max(1.0))
}

impl SampleVoice {
    /// The voices that sound `note` from the instrument at `bank` and `program`: one for
    /// each sample the bank plays for that key and velocity, and so two for a stereo
    /// instrument.
    pub(crate) fn start(
        font: &Rc<SoundFont>,
        (bank, program): (u16, u16),
        note: &NoteEvent,
        playing: Playing,
    ) -> Vec<SampleVoice> {
        let loudness = note.velocity * if note.soft { 0.8 } else { 1.0 };
        let velocity = (loudness * 100.0).round().clamp(1.0, 127.0) as u8;
        let Some(preset) = font.preset(bank, program) else {
            return Vec::new();
        };
        let matches = |r: &&Region| {
            (r.keys.0..=r.keys.1).contains(&note.pitch)
                && (r.velocities.0..=r.velocities.1).contains(&velocity)
        };
        (preset.regions.iter().filter(matches))
            .map(|region| SampleVoice::new(font, region, note, velocity, playing))
            .collect()
    }

    fn new(
        font: &Rc<SoundFont>,
        region: &Region,
        note: &NoteEvent,
        velocity: u8,
        Playing {
            held,
            trim,
            rate,
            limit,
        }: Playing,
    ) -> SampleVoice {
        let key = f32::from(region.fixed_key.unwrap_or(note.pitch));
        let velocity = f32::from(region.fixed_velocity.unwrap_or(velocity)) / 127.0;
        let cents = (key - region.root_key) * region.scale_tuning + region.tune;
        let step = f64::from(2f32.powf(cents / 1200.0) * region.sample_rate / rate);

        // A muted string is dull and soon over, whatever the sample does.
        let (ring, dull) = match note.tone {
            Tone::Dead => (Some(0.02), 900.0),
            Tone::PalmMute => (Some(0.1), 1_400.0),
            Tone::Normal | Tone::Harmonic => (None, f32::MAX),
        };
        let cutoff = region.cutoff.min(dull);
        let filter = (cutoff < rate * 0.4).then(|| Lowpass::new(cutoff, region.resonance, rate));

        // A note that is not picked starts a little way in, past the sound of the pick.
        let skip = match note.soft {
            true => (SOFT_START * region.sample_rate) as usize,
            false => 0,
        };
        let start = match region.start + skip < region.end {
            true => region.start + skip,
            false => region.start,
        };
        let angle = (region.pan + 1.0) * FRAC_PI_4;
        let gain = SAMPLE_GAIN * trim * region.gain * velocity * velocity;
        SampleVoice {
            font: Rc::clone(font),
            position: start as f64,
            step,
            bend: 1.0,
            rate,
            gains: (gain * angle.cos(), gain * angle.sin()),
            filter,
            stage: Stage::Delay((region.delay * rate) as usize),
            level: 0.0,
            decay: fall(region.decay, rate),
            release: fall(region.release, rate),
            muted: 1.0,
            damping: ring.map_or(1.0, |seconds: f32| (-1.0 / (seconds * rate)).exp()),
            pitch_curve: note.pitch_curve.clone(),
            vibrato: note.vibrato,
            at: 0,
            release_at: match held {
                true => (note.duration * f64::from(rate)) as usize,
                false => usize::MAX,
            },
            limit,
            region: region.clone(),
        }
    }

    /// The class of voices this one cuts off and is cut off by; zero for none.
    pub(crate) fn class(&self) -> u16 {
        self.region.class
    }

    /// Stops the voice quickly, as when another of its class starts.
    pub(crate) fn choke(&mut self) {
        self.release = fall(CHOKE * 1.25, self.rate);
        self.stage = Stage::Release;
    }

    /// Moves the envelope on by one sample. Returns false once the voice has died away.
    fn envelope(&mut self) -> bool {
        let rate = self.rate;
        match &mut self.stage {
            Stage::Delay(0) => self.stage = Stage::Attack,
            Stage::Delay(left) => *left -= 1,
            Stage::Attack => {
                self.level += 1.0 / (self.region.attack * rate).max(1.0);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Hold((self.region.hold * rate) as usize);
                }
            }
            Stage::Hold(0) => self.stage = Stage::Decay,
            Stage::Hold(left) => *left -= 1,
            Stage::Decay => {
                self.level = (self.level * self.decay).max(self.region.sustain);
                if self.level < SILENCE {
                    return false;
                }
            }
            Stage::Release => {
                self.level *= self.release;
                if self.level < SILENCE {
                    return false;
                }
            }
        }
        self.muted *= self.damping;
        self.muted >= SILENCE
    }

    /// Adds the voice's next samples to `left` and `right`. Returns false once it has
    /// nothing left.
    pub(crate) fn render(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let steady = self.pitch_curve.is_empty() && self.vibrato == 0.0;
        let loop_length = (self.region.loop_end - self.region.loop_start) as f64;
        for (left, right) in left.iter_mut().zip(right) {
            if self.at >= self.limit {
                return false;
            }
            if self.at == self.release_at {
                self.stage = Stage::Release;
            }
            if !steady && self.at.is_multiple_of(CONTROL_PERIOD) {
                let t = self.at as f32 / self.rate;
                let mut semitones = crate::voice::curve_at(&self.pitch_curve, t);
                if self.vibrato > 0.0 && t > VIBRATO_DELAY {
                    let swell = ((t - VIBRATO_DELAY) * 6.0).min(1.0);
                    semitones += self.vibrato * swell * (TAU * VIBRATO_HZ * t).sin();
                }
                self.bend = f64::from(2f32.powf(semitones / 12.0));
            }

            let index = self.position as usize;
            let fraction = (self.position - index as f64) as f32;
            let (here, next) = (self.font.sample(index), self.font.sample(index + 1));
            let mut sample = here + (next - here) * fraction;
            if let Some(filter) = &mut self.filter {
                sample = filter.run(sample);
            }
            let sample = sample * self.level * self.muted;
            *left += self.gains.0 * sample;
            *right += self.gains.1 * sample;

            self.at += 1;
            self.position += self.step * self.bend;
            let looping = match self.region.looping {
                Looping::Off => false,
                Looping::On => true,
                Looping::UntilRelease => !matches!(self.stage, Stage::Release),
            };
            if looping {
                while self.position >= self.region.loop_end as f64 {
                    self.position -= loop_length;
                }
            } else if self.position >= self.region.end as f64 {
                return false;
            }
            if !self.envelope() {
                return false;
            }
        }
        true
    }
}

/// A SoundFont made up for tests.
#[cfg(test)]
pub(crate) mod testing {
    /// A zone of an instrument: the generators it sets, ending with its sample.
    pub(crate) type Zone = Vec<(u16, i16)>;

    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    }

    fn list(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
        chunk(b"LIST", &[&kind[..], &chunks.concat()].concat())
    }

    fn name(text: &str) -> [u8; 20] {
        let mut out = [0; 20];
        out[..text.len()].copy_from_slice(text.as_bytes());
        out
    }

    /// The bytes of a SoundFont with one sample: a sine wave of `period` samples at
    /// 44.1 kHz, one second long and looped, with the root key 69. `presets` gives each
    /// preset's bank, program and the zones of its instrument.
    pub(crate) fn sound_font(period: usize, presets: &[(u16, u16, Vec<Zone>)]) -> Vec<u8> {
        let length = 44_100 / period * period;
        let wave: Vec<u8> = (0..length + 46)
            .map(|i| match i < length {
                true => (i as f32 / period as f32 * std::f32::consts::TAU).sin() * 16_000.0,
                false => 0.0,
            })
            .flat_map(|s| (s as i16).to_le_bytes())
            .collect();

        let (mut phdr, mut pbag, mut pgen) = (Vec::new(), Vec::new(), Vec::new());
        let (mut inst, mut ibag, mut igen) = (Vec::new(), Vec::new(), Vec::new());
        for (index, (bank, program, zones)) in presets.iter().enumerate() {
            phdr.extend(name("preset"));
            phdr.extend(
                [*program, *bank, (pbag.len() / 4) as u16]
                    .map(u16::to_le_bytes)
                    .concat(),
            );
            phdr.extend([0; 12]);
            pbag.extend([(pgen.len() / 4) as u16, 0].map(u16::to_le_bytes).concat());
            pgen.extend(
                [super::INSTRUMENT, index as u16]
                    .map(u16::to_le_bytes)
                    .concat(),
            );
            inst.extend(name("instrument"));
            inst.extend(((ibag.len() / 4) as u16).to_le_bytes());
            for zone in zones {
                ibag.extend([(igen.len() / 4) as u16, 0].map(u16::to_le_bytes).concat());
                for &(generator, amount) in zone {
                    igen.extend(generator.to_le_bytes());
                    igen.extend(amount.to_le_bytes());
                }
            }
        }
        // Each list ends with a record that only says where the last real one stops.
        phdr.extend(name("EOP"));
        phdr.extend(
            [0, 0, (pbag.len() / 4) as u16]
                .map(u16::to_le_bytes)
                .concat(),
        );
        phdr.extend([0; 12]);
        pbag.extend([(pgen.len() / 4) as u16, 0].map(u16::to_le_bytes).concat());
        pgen.extend([0; 4]);
        inst.extend(name("EOI"));
        inst.extend(((ibag.len() / 4) as u16).to_le_bytes());
        ibag.extend([(igen.len() / 4) as u16, 0].map(u16::to_le_bytes).concat());
        igen.extend([0; 4]);

        let mut shdr = name("sine").to_vec();
        let ends = [0, length as u32, 0, length as u32, 44_100];
        shdr.extend(ends.map(u32::to_le_bytes).concat());
        shdr.extend([69, 0]);
        shdr.extend([0u16, 1].map(u16::to_le_bytes).concat());
        shdr.extend([0; 46]);

        let body = [
            &b"sfbk"[..],
            &list(
                b"INFO",
                &[
                    chunk(b"ifil", &[2, 0, 1, 0]),
                    chunk(b"INAM", b"Test bank\0"),
                ],
            ),
            &list(b"sdta", &[chunk(b"smpl", &wave)]),
            &list(
                b"pdta",
                &[
                    chunk(b"phdr", &phdr),
                    chunk(b"pbag", &pbag),
                    chunk(b"pmod", &[0; 10]),
                    chunk(b"pgen", &pgen),
                    chunk(b"inst", &inst),
                    chunk(b"ibag", &ibag),
                    chunk(b"imod", &[0; 10]),
                    chunk(b"igen", &igen),
                    chunk(b"shdr", &shdr),
                ],
            ),
        ]
        .concat();
        [&b"RIFF"[..], &(body.len() as u32).to_le_bytes(), &body].concat()
    }

    /// A zone that plays the sample, looped, and lets it go quickly.
    pub(crate) fn looped() -> Zone {
        vec![
            (super::RELEASE, -3_600),
            (super::SAMPLE_MODES, 1),
            (super::SAMPLE, 0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{looped, sound_font};
    use super::*;

    fn playing(held: bool, limit: usize) -> Playing {
        Playing {
            held,
            trim: 1.0,
            rate: 44_100.0,
            limit,
        }
    }

    fn play(font: &Rc<SoundFont>, preset: (u16, u16), note: &NoteEvent, held: bool) -> Vec<f32> {
        let mut voices = SampleVoice::start(font, preset, note, playing(held, 88_200));
        let (mut left, mut right) = (vec![0.0; 88_200], vec![0.0; 88_200]);
        for voice in &mut voices {
            voice.render(&mut left, &mut right);
        }
        left
    }

    fn crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count()
    }

    #[test]
    fn a_bank_is_read_with_its_presets() {
        let bytes = sound_font(100, &[(0, 25, vec![looped()]), (128, 0, vec![looped()])]);
        let font = SoundFont::parse(&bytes).unwrap();
        assert_eq!((font.name(), font.preset_count()), ("Test bank", 2));
        assert!(font.has(0, 25) && font.has(128, 0) && !font.has(0, 24));
        assert_eq!(font.samples.len(), 44_100 + 46);
        let region = &font.presets[0].regions[0];
        assert_eq!(
            (region.start, region.end, region.looping),
            (0, 44_100, Looping::On)
        );
        assert_eq!(
            (region.keys, region.root_key, region.gain),
            ((0, 127), 69.0, 1.0)
        );
    }

    #[test]
    fn what_is_not_a_soundfont_is_refused() {
        let good = sound_font(100, &[(0, 0, vec![looped()])]);
        assert!(matches!(
            SoundFont::parse(b"RIFF"),
            Err(SoundFontError::NotASoundFont)
        ));
        assert!(matches!(
            SoundFont::parse(b"RIFF\x04\0\0\0WAVEfmt "),
            Err(SoundFontError::NotASoundFont)
        ));
        // Cut short, there is nothing to play; and version 3 is compressed.
        assert!(matches!(
            SoundFont::parse(&good[..200]),
            Err(SoundFontError::Malformed(_))
        ));
        let mut newer = good.clone();
        let version = newer.windows(4).position(|w| w == b"ifil").unwrap() + 8;
        newer[version] = 3;
        assert_eq!(
            SoundFont::parse(&newer).err(),
            Some(SoundFontError::Compressed)
        );
        // A file whose lists point nowhere is read without anything to play.
        let mut broken = good;
        let bags = broken.windows(4).position(|w| w == b"ibag").unwrap() + 8;
        broken[bags] = 200;
        assert!(!SoundFont::parse(&broken).unwrap().has(0, 0));
    }

    #[test]
    fn a_note_is_played_at_its_pitch_for_its_length() {
        // A sample of 441 Hz at key 69.
        let font = Rc::new(SoundFont::parse(&sound_font(100, &[(0, 0, vec![looped()])])).unwrap());
        let a = play(&font, (0, 0), &NoteEvent::plain(0.0, 0.5, 69), true);
        assert_eq!(crossings(&a[..22_050]), 220);
        // An octave up is twice as fast, round the loop to the end of the note, after which
        // it is let go and dies away.
        let high = play(&font, (0, 0), &NoteEvent::plain(0.0, 1.5, 81), true);
        assert!((crossings(&high[..44_100]) as i32 - 882).abs() <= 1);
        assert!(high[60_000..66_000].iter().any(|s| s.abs() > 0.1));
        assert!(high[72_000..].iter().all(|&s| s == 0.0));
        // A drum is not held: it plays on after the note, to the end of its sample.
        let mut drum = looped();
        drum.insert(0, (SAMPLE_MODES, 0));
        let font = Rc::new(SoundFont::parse(&sound_font(100, &[(128, 0, vec![drum])])).unwrap());
        let hit = play(&font, (128, 0), &NoteEvent::plain(0.0, 0.01, 69), false);
        assert!(hit[40_000..44_000].iter().any(|s| s.abs() > 0.1));
        assert!(hit[44_200..].iter().all(|&s| s == 0.0));
        // Nothing sounds for a preset the bank does not have.
        assert!(
            play(&font, (0, 7), &NoteEvent::plain(0.0, 0.5, 69), true)
                .iter()
                .all(|&s| s == 0.0)
        );
    }

    #[test]
    fn zones_choose_the_sample_and_shape_it() {
        // Low keys an octave down and quieter; high keys panned hard right and fixed in pitch.
        let low = vec![
            (KEY_RANGE, 59 << 8),
            (COARSE_TUNE, -12),
            (ATTENUATION, 500),
            (SAMPLE_MODES, 1),
            (SAMPLE, 0),
        ];
        let high = vec![
            (KEY_RANGE, (127 << 8) | 60),
            (PAN, 500),
            (SCALE_TUNING, 0),
            (SAMPLE_MODES, 1),
            (SAMPLE, 0),
        ];
        let bytes = sound_font(100, &[(0, 0, vec![low, high])]);
        let font = Rc::new(SoundFont::parse(&bytes).unwrap());

        let low = play(&font, (0, 0), &NoteEvent::plain(0.0, 0.5, 57), true);
        // Key 57 is an octave below the root, and the zone tunes it down another.
        assert!((crossings(&low[..22_050]) as i32 - 55).abs() <= 1);
        let peak = |samples: &[f32]| samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let quiet = peak(&low[2_000..20_000]);

        let mut voices = SampleVoice::start(
            &font,
            (0, 0),
            &NoteEvent::plain(0.0, 0.5, 72),
            playing(true, 9_000),
        );
        assert_eq!(voices.len(), 1);
        let (mut left, mut right) = (vec![0.0; 8_820], vec![0.0; 8_820]);
        assert!(voices[0].render(&mut left, &mut right));
        assert!(peak(&left) < 1e-6 && peak(&right) > quiet * 2.0);
        // Its pitch does not follow the key.
        assert_eq!(crossings(&right), 88);
        // And it may only sound for as long as it is allowed.
        assert!(!voices[0].render(&mut left, &mut right));
    }

    #[test]
    fn harder_notes_are_louder_and_muted_ones_are_short_and_dull() {
        let font = Rc::new(SoundFont::parse(&sound_font(20, &[(0, 0, vec![looped()])])).unwrap());
        let energy = |samples: &[f32]| samples.iter().map(|s| s * s).sum::<f32>();
        let mut note = NoteEvent::plain(0.0, 1.0, 69);
        let plain = play(&font, (0, 0), &note, true);
        note.velocity = 0.5;
        let soft = play(&font, (0, 0), &note, true);
        assert!(energy(&soft[..8_000]) < energy(&plain[..8_000]) * 0.2);

        note.velocity = 1.0;
        note.tone = Tone::PalmMute;
        let muted = play(&font, (0, 0), &note, true);
        // 2205 Hz is well above where a muted string is cut off.
        assert!(energy(&muted[..2_000]) < energy(&plain[..2_000]) * 0.5);
        assert!(energy(&muted[20_000..40_000]) < energy(&plain[20_000..40_000]) * 1e-4);

        // A bend raises the pitch as the note goes on.
        let mut bent = NoteEvent::plain(0.0, 1.0, 69);
        bent.pitch_curve = vec![(0.0, 0.0), (0.1, 12.0)];
        let bent = play(&font, (0, 0), &bent, true);
        assert!(crossings(&bent[22_050..44_100]) > crossings(&plain[22_050..44_100]) * 19 / 10);
    }
}
