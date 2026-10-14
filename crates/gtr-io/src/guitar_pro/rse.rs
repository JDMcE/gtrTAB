//! The pedals and amplifiers of Guitar Pro 6 and later, which its Realistic Sound Engine
//! keeps for each of a track's sounds and for the whole mix: each an id such as
//! `E03_OverdriveScreamer`, whose letter says what sort of unit it is (A for an amplifier,
//! E for a pedal, M for a unit of the mix) and whose name says what it does, and its
//! settings, each from 0 to 1.
//!
//! What the settings stand for is nowhere written down. They are matched to the knobs here
//! by what they hold in files whose sound is known: a pedal's drive, tone and level, an
//! amplifier's gain, bass, middle, treble and master, an equaliser's bands from low to
//! high. A unit is matched to a pedal here by what its name says it does, so that one of a
//! known sort is read even when its model is not known; one of a sort with no pedal here
//! is left out, and said to be.

use gtr_model::{Pedal, PedalKind};

/// A unit as a file holds it: its id, its settings, and whether it is switched off.
pub(crate) struct Unit<'a> {
    pub id: &'a str,
    pub settings: Vec<f64>,
    pub bypassed: bool,
}

/// What a unit of Guitar Pro's is as a pedal here, if it is a sort there is one of.
pub(crate) fn pedal(unit: &Unit) -> Option<Pedal> {
    let kind = kind(unit.id)?;
    let at = |i: usize| unit.settings.get(i).copied();
    let knob = |value: Option<f64>| value.map(|v| (v.clamp(0.0, 1.0) * 100.0).round() as u8);
    let mean = |from: usize, to: usize| {
        let values: Vec<f64> = (from..to).filter_map(at).collect();
        (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
    };
    let mut pedal = Pedal::new(kind);
    pedal.on = !unit.bypassed;
    let set: Vec<Option<u8>> = match kind {
        PedalKind::Overdrive | PedalKind::Distortion => vec![knob(at(0)), knob(at(1)), knob(at(2))],
        PedalKind::Amp => vec![
            knob(at(0)),
            knob(at(1)),
            knob(at(2)),
            knob(at(3)),
            None,
            knob(at(4)),
        ],
        // A graphic equaliser's bands, low to high, gathered into bass, middle and treble:
        // seven of a pedal's and ten of the mix's, after two switches and a level.
        PedalKind::Eq if unit.id.contains("10Band") => {
            vec![knob(mean(3, 6)), knob(mean(6, 9)), knob(mean(9, 13))]
        }
        PedalKind::Eq => vec![knob(mean(0, 2)), knob(mean(2, 5)), knob(mean(5, 7))],
        // A studio reverb: switched on, its size, its damping, and what of it is heard last.
        PedalKind::Reverb => vec![
            knob(at(1)),
            knob(at(2)),
            knob(unit.settings.last().copied()),
        ],
        PedalKind::Compressor => vec![knob(at(0)), knob(at(1))],
        PedalKind::Wah | PedalKind::Chorus | PedalKind::Delay | PedalKind::Cabinet => {
            vec![knob(at(0)), knob(at(1))]
        }
    };
    for (slot, value) in pedal.knobs.iter_mut().zip(set) {
        if let Some(value) = value {
            *slot = value;
        }
    }
    Some(pedal)
}

/// The sort of pedal a unit is, by what its id says.
fn kind(id: &str) -> Option<PedalKind> {
    let name = id
        .split_once('_')
        .map_or(id, |(_, name)| name)
        .to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    Some(if id.starts_with('A') {
        PedalKind::Amp
    } else if has(&["volumeandpan"]) {
        return None;
    } else if has(&["reverb"]) {
        PedalKind::Reverb
    } else if has(&["delay", "echo"]) {
        PedalKind::Delay
    } else if has(&["chorus"]) {
        PedalKind::Chorus
    } else if has(&["wah"]) {
        PedalKind::Wah
    } else if has(&["overdrive", "screamer", "drive"]) {
        PedalKind::Overdrive
    } else if has(&["distortion", "fuzz", "metal"]) {
        PedalKind::Distortion
    } else if has(&["compressor", "dynamic", "comp"]) {
        PedalKind::Compressor
    } else if has(&["eq"]) {
        PedalKind::Eq
    } else if has(&["cab"]) {
        PedalKind::Cabinet
    } else {
        return None;
    })
}

/// The units of a chain that a file holds, from the `<Effect>` elements under `node`: empty
/// slots and the mix's volume and pan, which are not pedals, are passed over. Says whether
/// any unit had no pedal to be.
pub(crate) fn chain(node: roxmltree::Node) -> (Vec<Pedal>, bool) {
    let mut pedals = Vec::new();
    let mut unmatched = false;
    for effect in node.descendants().filter(|n| n.has_tag_name("Effect")) {
        let Some(id) = effect.attribute("id") else {
            continue;
        };
        let settings = (effect.children())
            .find(|n| n.has_tag_name("Parameters"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        let bypassed = effect
            .children()
            .any(|n| n.has_tag_name("ByPass") || n.has_tag_name("Bypass"));
        match pedal(&Unit {
            id,
            settings,
            bypassed,
        }) {
            Some(pedal) if pedals.len() < Pedal::MAX_CHAIN => pedals.push(pedal),
            Some(_) => unmatched = true,
            None => unmatched |= !id.contains("VolumeAndPan"),
        }
    }
    (pedals, unmatched)
}

/// A pedal as a unit of Guitar Pro's: the id of one of its units of the same sort, and its
/// settings, or `None` for a sort it has no unit of. `master` asks for one of the units of
/// the mix, which has only the studio's. Only units seen in Guitar Pro's own files are
/// written.
pub(crate) fn unit(pedal: &Pedal, master: bool) -> Option<(&'static str, Vec<f64>)> {
    let knob = |i: usize| f64::from(pedal.knob(i));
    Some(match (pedal.kind, master) {
        (PedalKind::Overdrive, false) => ("E03_OverdriveScreamer", vec![knob(0), knob(1), knob(2)]),
        (PedalKind::Amp, false) => (
            "A05_StackBritishVintage",
            vec![knob(0), knob(1), knob(2), knob(3), knob(5)],
        ),
        (PedalKind::Eq, false) => {
            let (bass, middle, treble) = (knob(0), knob(1), knob(2));
            (
                "E30_EqGEq",
                vec![bass, bass, middle, middle, middle, treble, treble, 0.685714],
            )
        }
        (PedalKind::Eq, true) => {
            let (bass, middle, treble) = (knob(0), knob(1), knob(2));
            let mut settings = vec![0.0, 0.0, 0.5];
            settings.extend([bass; 3]);
            settings.extend([middle; 3]);
            settings.extend([treble; 4]);
            ("M08_GraphicEQ10Band", settings)
        }
        // Guitar Pro 6 puts the studio's units in a track's chain as well as the mix's.
        (PedalKind::Reverb, _) => (
            "M04_StudioReverbRoomAmbience",
            vec![1.0, knob(0), knob(1), 0.5, knob(2)],
        ),
        (PedalKind::Compressor, _) => ("M07_DynamicClassicDynamic", vec![knob(0), knob(1), 1.0]),
        _ => return None,
    })
}

/// A chain written as Guitar Pro's `<Effect>` elements, and whether any pedal had no unit
/// to be written as.
pub(crate) fn effects(chain: &[Pedal], master: bool) -> (String, bool) {
    let mut out = String::new();
    let mut left_out = false;
    for pedal in chain {
        let Some((id, settings)) = unit(pedal, master) else {
            left_out = true;
            continue;
        };
        let settings: Vec<String> = settings.iter().map(|v| format!("{v:.6}")).collect();
        out.push_str(&format!(
            "<Effect id=\"{id}\">{}<Parameters>{}</Parameters></Effect>\n",
            if pedal.on { "" } else { "<ByPass />" },
            settings.join(" ")
        ));
    }
    (out, left_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(id: &str, settings: &[f64]) -> Option<Pedal> {
        pedal(&Unit {
            id,
            settings: settings.to_vec(),
            bypassed: false,
        })
    }

    #[test]
    fn units_are_read_as_the_pedals_their_names_say() {
        let screamer = read("E03_OverdriveScreamer", &[0.84, 0.5, 0.84]).unwrap();
        assert_eq!(
            (screamer.kind, screamer.knobs.as_slice()),
            (PedalKind::Overdrive, &[84, 50, 84][..])
        );
        let stack = read("A05_StackBritishVintage", &[0.85, 0.67, 0.36, 0.66, 0.52]).unwrap();
        assert_eq!(stack.kind, PedalKind::Amp);
        assert_eq!(stack.knobs, [85, 67, 36, 66, 50, 52]);
        // A model not known, of a sort that is.
        assert_eq!(
            read("A99_ComboSomething", &[]).unwrap().kind,
            PedalKind::Amp
        );
        assert_eq!(
            read("E77_DelayTape", &[0.3, 0.4]).unwrap().kind,
            PedalKind::Delay
        );
        let eq = read(
            "E30_EqGEq",
            &[0.78, 0.76, 0.49, 0.53, 0.29, 0.82, 0.9, 0.31],
        )
        .unwrap();
        assert_eq!(eq.knobs[..3], [77, 44, 86]);
        let room = read("M04_StudioReverbRoomAmbience", &[1.0, 0.43, 0.4, 0.5, 0.2]).unwrap();
        assert_eq!(room.knobs, [43, 40, 20]);
        assert_eq!(
            read("M07_DynamicClassicDynamic", &[0.5, 0.5, 1.0])
                .unwrap()
                .kind,
            PedalKind::Compressor
        );
        // The mix's volume and pan, and a sort with no pedal here, are not pedals.
        assert!(read("I01_VolumeAndPan", &[0.76, 0.5]).is_none());
        assert!(read("E50_PitchOctaver", &[0.5]).is_none());
    }

    #[test]
    fn pedals_written_as_units_read_back_as_they_were() {
        for (pedal, master) in [
            (
                Pedal {
                    kind: PedalKind::Overdrive,
                    on: true,
                    knobs: vec![70, 30, 60],
                },
                false,
            ),
            (
                Pedal {
                    kind: PedalKind::Amp,
                    on: false,
                    knobs: vec![80, 40, 60, 55, 50, 45],
                },
                false,
            ),
            (
                Pedal {
                    kind: PedalKind::Eq,
                    on: true,
                    knobs: vec![60, 40, 70, 50, 50],
                },
                false,
            ),
            (
                Pedal {
                    kind: PedalKind::Eq,
                    on: true,
                    knobs: vec![60, 40, 70, 50, 50],
                },
                true,
            ),
            (
                Pedal {
                    kind: PedalKind::Reverb,
                    on: true,
                    knobs: vec![40, 30, 25],
                },
                true,
            ),
            (
                Pedal {
                    kind: PedalKind::Compressor,
                    on: true,
                    knobs: vec![35, 60, 20, 40, 50],
                },
                true,
            ),
            (
                Pedal {
                    kind: PedalKind::Reverb,
                    on: true,
                    knobs: vec![60, 20, 35],
                },
                false,
            ),
        ] {
            let (written, left_out) = effects(std::slice::from_ref(&pedal), master);
            assert!(!left_out);
            let xml = format!("<Chain>{written}</Chain>");
            let document = roxmltree::Document::parse(&xml).unwrap();
            let (read, unmatched) = chain(document.root_element());
            assert!(!unmatched);
            // Knobs that Guitar Pro has no setting for come back as they start.
            let mut expected = pedal.clone();
            for (i, knob) in pedal.kind.knobs().iter().enumerate() {
                let kept = match pedal.kind {
                    PedalKind::Amp => i != 4,
                    PedalKind::Eq | PedalKind::Overdrive | PedalKind::Reverb => i < 3,
                    _ => i < 2,
                };
                if !kept {
                    expected.knobs[i] = knob.default;
                }
            }
            assert_eq!(read, [expected], "{written}");
        }
        // A pedal with no unit to be is left out, and said to be.
        let (written, left_out) = effects(&[Pedal::new(PedalKind::Wah)], false);
        assert!(written.is_empty() && left_out);
        let (written, left_out) = effects(&[Pedal::new(PedalKind::Overdrive)], true);
        assert!(written.is_empty() && left_out);
    }
}
