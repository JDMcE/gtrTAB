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
    fn a_pedal_is_stored_briefly() {
        let pedal = Pedal::with(PedalKind::Overdrive, &[70]);
        let json = serde_json::to_string(&pedal).unwrap();
        assert_eq!(json, r#"{"kind":"overdrive","knobs":[70,50,50]}"#);
        let off: Pedal = serde_json::from_str(r#"{"kind":"wah","on":false}"#).unwrap();
        assert!(!off.on && off.knobs.is_empty() && off.knob(0) == 0.5);
        assert_eq!(serde_json::from_str::<Pedal>(&json).unwrap(), pedal);
    }
}
