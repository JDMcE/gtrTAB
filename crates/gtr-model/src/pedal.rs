//! Effects: a track's sound goes through a chain of pedals on its way to the mix, and the
//! whole mix through a chain of its own. Each pedal is a kind and the settings of its
//! knobs, every knob from 0 to 100, so that a chain is stored and edited the same way
//! whatever is in it.

use serde::{Deserialize, Serialize};

/// What a pedal does to the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum PedalKind {
    /// Evens out the level: loud notes are held back and quiet ones brought up.
    Compressor,
    /// A peak swept up and down the range of the voice, by the pedal or by how hard the
    /// strings are played.
    Wah,
    /// A soft clip with a hump in the middle, that a clean amplifier is pushed with.
    Overdrive,
    /// A hard clip at high gain.
    Distortion,
    /// A valve preamplifier with its tone controls, and the power stage after it.
    Amp,
    /// The speaker cabinet and the microphone in front of it.
    Cabinet,
    /// Bass, middle and treble, the middle's frequency chosen.
    Eq,
    Chorus,
    Delay,
    Reverb,
}

/// One knob of a kind of pedal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Knob {
    pub name: &'static str,
    pub default: u8,
}

/// A knob, written so that a list of them can be borrowed for good.
macro_rules! knob {
    ($name:literal, $default:literal) => {
        Knob {
            name: $name,
            default: $default,
        }
    };
}

impl PedalKind {
    pub const ALL: [PedalKind; 10] = [
        PedalKind::Compressor,
        PedalKind::Wah,
        PedalKind::Overdrive,
        PedalKind::Distortion,
        PedalKind::Amp,
        PedalKind::Cabinet,
        PedalKind::Eq,
        PedalKind::Chorus,
        PedalKind::Delay,
        PedalKind::Reverb,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PedalKind::Compressor => "Compressor",
            PedalKind::Wah => "Wah",
            PedalKind::Overdrive => "Overdrive",
            PedalKind::Distortion => "Distortion",
            PedalKind::Amp => "Amp",
            PedalKind::Cabinet => "Cabinet",
            PedalKind::Eq => "EQ",
            PedalKind::Chorus => "Chorus",
            PedalKind::Delay => "Delay",
            PedalKind::Reverb => "Reverb",
        }
    }

    /// The pedal's knobs in order, each with the setting a new pedal has. A level knob at
    /// 50 leaves the level as it is.
    pub fn knobs(self) -> &'static [Knob] {
        match self {
            PedalKind::Compressor => &[
                knob!("Threshold", 50),
                knob!("Ratio", 40),
                knob!("Attack", 20),
                knob!("Release", 40),
                knob!("Level", 50),
            ],
            PedalKind::Wah => &[
                knob!("Pedal", 50),
                knob!("Resonance", 60),
                knob!("Auto", 0),
                knob!("Mix", 100),
            ],
            PedalKind::Overdrive => &[knob!("Drive", 40), knob!("Tone", 50), knob!("Level", 50)],
            PedalKind::Distortion => &[knob!("Gain", 60), knob!("Tone", 50), knob!("Level", 50)],
            PedalKind::Amp => &[
                knob!("Gain", 50),
                knob!("Bass", 50),
                knob!("Middle", 50),
                knob!("Treble", 50),
                knob!("Presence", 50),
                knob!("Master", 50),
            ],
            PedalKind::Cabinet => &[knob!("Size", 50), knob!("Mic", 50), knob!("Level", 50)],
            PedalKind::Eq => &[
                knob!("Bass", 50),
                knob!("Middle", 50),
                knob!("Treble", 50),
                knob!("Frequency", 50),
                knob!("Level", 50),
            ],
            PedalKind::Chorus => &[knob!("Rate", 30), knob!("Depth", 50), knob!("Mix", 50)],
            PedalKind::Delay => &[
                knob!("Time", 35),
                knob!("Feedback", 35),
                knob!("Tone", 60),
                knob!("Mix", 30),
            ],
            PedalKind::Reverb => &[knob!("Size", 50), knob!("Damping", 50), knob!("Mix", 25)],
        }
    }
}

/// One pedal of a chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Pedal {
    pub kind: PedalKind,
    /// False for a pedal that is in the chain but switched off, which the sound goes
    /// straight past.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    #[ts(optional, as = "Option<bool>")]
    pub on: bool,
    /// The knobs' settings in the order [`PedalKind::knobs`] gives them, from 0 to
    /// [`Pedal::MAX_KNOB`]. Knobs past the end of the list are at their defaults.
    #[serde(default)]
    pub knobs: Vec<u8>,
}

fn on() -> bool {
    true
}

fn is_on(on: &bool) -> bool {
    *on
}

impl Pedal {
    pub const MAX_KNOB: u8 = 100;
    /// The most pedals a chain can have.
    pub const MAX_CHAIN: usize = 16;

    /// A pedal of the kind, switched on, with every knob at its default.
    pub fn new(kind: PedalKind) -> Pedal {
        Pedal {
            kind,
            on: true,
            knobs: kind.knobs().iter().map(|k| k.default).collect(),
        }
    }

    /// A pedal of the kind with the knobs given in order, the rest at their defaults.
    pub fn with(kind: PedalKind, knobs: &[u8]) -> Pedal {
        let mut pedal = Pedal::new(kind);
        for (setting, &value) in pedal.knobs.iter_mut().zip(knobs) {
            *setting = value;
        }
        pedal
    }

    /// A pedal of the kind with the knobs named set, the rest at their defaults.
    ///
    /// # Panics
    /// If the kind has no knob of one of the names.
    pub fn named(kind: PedalKind, knobs: &[(&str, u8)]) -> Pedal {
        let mut pedal = Pedal::new(kind);
        for &(name, value) in knobs {
            let index = (kind.knobs().iter())
                .position(|k| k.name == name)
                .unwrap_or_else(|| panic!("{kind:?} has no {name} knob"));
            pedal.knobs[index] = value;
        }
        pedal
    }

    /// The setting of knob `index`, from 0.0 to 1.0.
    pub fn knob(&self, index: usize) -> f32 {
        let default = self.kind.knobs().get(index).map_or(0, |k| k.default);
        f32::from(self.knobs.get(index).copied().unwrap_or(default)) / f32::from(Pedal::MAX_KNOB)
    }

    pub fn is_valid(&self) -> bool {
        self.knobs.len() <= self.kind.knobs().len()
            && self.knobs.iter().all(|&k| k <= Pedal::MAX_KNOB)
    }

    /// True for a chain that can be stored: not too long, and every pedal in it valid.
    pub fn chain_is_valid(chain: &[Pedal]) -> bool {
        chain.len() <= Pedal::MAX_CHAIN && chain.iter().all(Pedal::is_valid)
    }
}

/// Another sound a track can switch to part-way through, from a beat on: the sound of an
/// instrument, played through a chain of pedals of its own. What is written stays as it
/// is; only how it sounds changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Sound {
    pub name: String,
    pub instrument: crate::Instrument,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Pedal>>")]
    pub effects: Vec<Pedal>,
}

impl Sound {
    /// The most sounds a track can have besides its own.
    pub const MAX_SOUNDS: usize = 8;
    /// Longest name of a sound, in characters.
    pub const MAX_NAME: usize = 40;

    /// True for a sound a track playing `instrument` can have: one with a name and a valid
    /// chain, and a drum kit only for drums and drums for nothing else, since a drum track
    /// and a pitched one write their notes in different ways.
    pub fn is_valid_for(&self, instrument: crate::Instrument) -> bool {
        let name = self.name.trim().chars().count();
        (1..=Sound::MAX_NAME).contains(&name)
            && Pedal::chain_is_valid(&self.effects)
            && (self.instrument == crate::Instrument::Drums)
                == (instrument == crate::Instrument::Drums)
    }

    /// True for a list of sounds a track playing `instrument` can have.
    pub fn list_is_valid(sounds: &[Sound], instrument: crate::Instrument) -> bool {
        sounds.len() <= Sound::MAX_SOUNDS && sounds.iter().all(|s| s.is_valid_for(instrument))
    }
}

/// A chain ready to be put on a track or the mix, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    /// For the whole mix rather than a track.
    pub master: bool,
    pub pedals: Vec<Pedal>,
}

/// The chains the app comes with: for a track, from clean to high gain and for bass and
/// acoustic instruments, and for the mix, a room or a hall.
pub fn presets() -> Vec<Preset> {
    use PedalKind::*;
    let track = |name, pedals| Preset {
        name,
        master: false,
        pedals,
    };
    let cabinet = |size| Pedal::named(Cabinet, &[("Size", size)]);
    vec![
        track(
            "Clean",
            vec![
                Pedal::named(Compressor, &[("Threshold", 55), ("Ratio", 30)]),
                Pedal::named(Amp, &[("Gain", 20), ("Treble", 60)]),
                cabinet(40),
                Pedal::named(Reverb, &[("Size", 45), ("Mix", 20)]),
            ],
        ),
        track(
            "Chorus clean",
            vec![
                Pedal::named(Amp, &[("Gain", 20), ("Treble", 60)]),
                cabinet(40),
                Pedal::named(Chorus, &[("Rate", 25), ("Depth", 60), ("Mix", 60)]),
                Pedal::named(Reverb, &[("Size", 55), ("Mix", 25)]),
            ],
        ),
        track(
            "Crunch",
            vec![
                Pedal::named(Amp, &[("Gain", 62), ("Middle", 60), ("Presence", 55)]),
                cabinet(60),
                Pedal::named(Reverb, &[("Size", 35), ("Mix", 15)]),
            ],
        ),
        track(
            "Blues lead",
            vec![
                Pedal::named(Overdrive, &[("Drive", 35), ("Tone", 55), ("Level", 65)]),
                Pedal::named(Amp, &[("Gain", 55), ("Middle", 60)]),
                cabinet(50),
                Pedal::named(Delay, &[("Time", 30), ("Feedback", 25), ("Mix", 20)]),
                Pedal::named(Reverb, &[("Size", 50), ("Mix", 20)]),
            ],
        ),
        track(
            "Rock rhythm",
            vec![
                Pedal::named(
                    Amp,
                    &[("Gain", 75), ("Bass", 55), ("Middle", 55), ("Treble", 60)],
                ),
                cabinet(85),
                Pedal::named(Reverb, &[("Size", 30), ("Mix", 12)]),
            ],
        ),
        track(
            "Metal",
            vec![
                // A drive pedal with little gain and a lot of level tightens the low end
                // of a high-gain amp.
                Pedal::named(Overdrive, &[("Drive", 10), ("Tone", 60), ("Level", 80)]),
                Pedal::named(
                    Amp,
                    &[("Gain", 88), ("Bass", 60), ("Middle", 35), ("Treble", 62)],
                ),
                cabinet(100),
                Pedal::named(Eq, &[("Bass", 45), ("Middle", 55), ("Frequency", 60)]),
            ],
        ),
        track(
            "Solo",
            vec![
                Pedal::named(Distortion, &[("Gain", 55), ("Tone", 55), ("Level", 55)]),
                Pedal::named(Amp, &[("Gain", 60), ("Middle", 65)]),
                cabinet(80),
                Pedal::named(Delay, &[("Time", 40), ("Feedback", 35), ("Mix", 30)]),
                Pedal::named(Reverb, &[("Size", 60), ("Mix", 25)]),
            ],
        ),
        track(
            "Funk",
            vec![
                Pedal::named(Compressor, &[("Threshold", 40), ("Ratio", 60)]),
                Pedal::named(Wah, &[("Pedal", 20), ("Auto", 55), ("Resonance", 70)]),
                Pedal::named(Amp, &[("Gain", 25), ("Treble", 65)]),
                cabinet(40),
            ],
        ),
        track(
            "Ambient",
            vec![
                Pedal::named(Compressor, &[("Threshold", 50)]),
                Pedal::named(Amp, &[("Gain", 25)]),
                cabinet(50),
                Pedal::named(Chorus, &[("Rate", 15), ("Depth", 40), ("Mix", 40)]),
                Pedal::named(
                    Delay,
                    &[("Time", 55), ("Feedback", 55), ("Tone", 45), ("Mix", 40)],
                ),
                Pedal::named(Reverb, &[("Size", 90), ("Damping", 40), ("Mix", 50)]),
            ],
        ),
        track(
            "Acoustic",
            vec![
                Pedal::named(Compressor, &[("Threshold", 60), ("Ratio", 25)]),
                Pedal::named(Eq, &[("Bass", 45), ("Treble", 58)]),
                Pedal::named(Reverb, &[("Size", 45), ("Mix", 22)]),
            ],
        ),
        track(
            "Bass",
            vec![
                Pedal::named(Compressor, &[("Threshold", 45), ("Ratio", 50)]),
                Pedal::named(Eq, &[("Bass", 58), ("Middle", 45), ("Frequency", 30)]),
            ],
        ),
        Preset {
            name: "Room",
            master: true,
            pedals: vec![Pedal::named(
                Reverb,
                &[("Size", 30), ("Damping", 60), ("Mix", 15)],
            )],
        },
        Preset {
            name: "Hall",
            master: true,
            pedals: vec![Pedal::named(
                Reverb,
                &[("Size", 80), ("Damping", 40), ("Mix", 22)],
            )],
        },
        Preset {
            name: "Glue",
            master: true,
            pedals: vec![
                Pedal::named(
                    Compressor,
                    &[("Threshold", 60), ("Ratio", 20), ("Attack", 40)],
                ),
                Pedal::named(Reverb, &[("Size", 40), ("Mix", 12)]),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knobs_read_with_their_defaults() {
        for kind in PedalKind::ALL {
            let pedal = Pedal::new(kind);
            assert!(pedal.is_valid());
            assert_eq!(pedal.knobs.len(), kind.knobs().len());
            // A pedal stored without its knobs has them at their defaults.
            let bare = Pedal {
                kind,
                on: true,
                knobs: Vec::new(),
            };
            for i in 0..kind.knobs().len() {
                assert_eq!(bare.knob(i), pedal.knob(i));
            }
        }
        let delay = Pedal::with(PedalKind::Delay, &[100]);
        assert_eq!(delay.knob(0), 1.0);
        assert_eq!(delay.knob(1), 0.35);

        assert!(!Pedal::with(PedalKind::Reverb, &[101]).is_valid());
        let mut extra = Pedal::new(PedalKind::Chorus);
        extra.knobs.push(0);
        assert!(!extra.is_valid());
        assert!(!Pedal::chain_is_valid(&vec![
            Pedal::new(PedalKind::Eq);
            Pedal::MAX_CHAIN + 1
        ]));
    }

    #[test]
    fn presets_are_valid_chains_with_names_of_their_own() {
        let presets = presets();
        assert!(presets.iter().all(|p| Pedal::chain_is_valid(&p.pedals)));
        let mut names: Vec<_> = presets.iter().map(|p| p.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), presets.len());
        assert!(presets.iter().any(|p| p.master) && presets.iter().any(|p| !p.master));
    }

    #[test]
    #[should_panic(expected = "Reverb has no Drive knob")]
    fn a_knob_named_wrongly_is_caught() {
        Pedal::named(PedalKind::Reverb, &[("Drive", 1)]);
    }

    #[test]
    fn a_pedal_is_stored_briefly() {
        let pedal = Pedal::with(PedalKind::Overdrive, &[70]);
        let json = serde_json::to_string(&pedal).unwrap();
        assert_eq!(json, r#"{"kind":"overdrive","knobs":[70,50,50]}"#);
        let off: Pedal = serde_json::from_str(r#"{"kind":"wah","on":false}"#).unwrap();
        assert!(!off.on && off.knobs.is_empty() && off.knob(0) == 0.5);
        assert_eq!(serde_json::from_str::<Pedal>(&json).unwrap(), pedal);
    }
}
