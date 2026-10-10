use gtr_model::{
    Accent, BeatEffects, Hairpin, Harmonic, NoteEffects, NoteValue, PickStroke, Slap, Slide, Strum,
    Vibrato,
};
use serde::Deserialize;

/// A playing technique that can be switched on and off from the keyboard or toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Technique {
    Hammer,
    PalmMute,
    LetRing,
    Dead,
    Ghost,
    Staccato,
    Tapped,
    Accent,
    HeavyAccent,
    Vibrato,
    WideVibrato,
    SlideShift,
    SlideLegato,
    SlideInBelow,
    SlideInAbove,
    SlideOutDown,
    SlideOutUp,
    HarmonicNatural,
    HarmonicArtificial,
    HarmonicPinch,
    HarmonicTapped,
    HarmonicSemi,
    StrokeDown,
    StrokeUp,
    Tremolo,
    Slap,
    Pop,
    BrushDown,
    BrushUp,
    ArpeggioDown,
    ArpeggioUp,
    Rasgueado,
    Crescendo,
    Decrescendo,
}

/// What a technique attaches to.
pub(crate) enum Target {
    /// Just the note it is applied to.
    Note,
    /// Every note of the beat, as when palm muting a chord.
    Chord,
    /// The beat itself.
    Beat,
}

impl Technique {
    pub const ALL: [Technique; 34] = [
        Technique::Hammer,
        Technique::PalmMute,
        Technique::LetRing,
        Technique::Dead,
        Technique::Ghost,
        Technique::Staccato,
        Technique::Tapped,
        Technique::Accent,
        Technique::HeavyAccent,
        Technique::Vibrato,
        Technique::WideVibrato,
        Technique::SlideShift,
        Technique::SlideLegato,
        Technique::SlideInBelow,
        Technique::SlideInAbove,
        Technique::SlideOutDown,
        Technique::SlideOutUp,
        Technique::HarmonicNatural,
        Technique::HarmonicArtificial,
        Technique::HarmonicPinch,
        Technique::HarmonicTapped,
        Technique::HarmonicSemi,
        Technique::StrokeDown,
        Technique::StrokeUp,
        Technique::Tremolo,
        Technique::Slap,
        Technique::Pop,
        Technique::BrushDown,
        Technique::BrushUp,
        Technique::ArpeggioDown,
        Technique::ArpeggioUp,
        Technique::Rasgueado,
        Technique::Crescendo,
        Technique::Decrescendo,
    ];

    pub(crate) fn target(self) -> Target {
        match self {
            Technique::PalmMute
            | Technique::LetRing
            | Technique::Staccato
            | Technique::Accent
            | Technique::HeavyAccent => Target::Chord,
            _ if self.slide().is_some() || self.harmonic().is_some() => Target::Note,
            Technique::Hammer
            | Technique::Dead
            | Technique::Ghost
            | Technique::Tapped
            | Technique::Vibrato
            | Technique::WideVibrato => Target::Note,
            _ => Target::Beat,
        }
    }

    fn strum(self) -> Option<Strum> {
        Some(match self {
            Technique::BrushDown => Strum::BrushDown,
            Technique::BrushUp => Strum::BrushUp,
            Technique::ArpeggioDown => Strum::ArpeggioDown,
            Technique::ArpeggioUp => Strum::ArpeggioUp,
            Technique::Rasgueado => Strum::Rasgueado,
            _ => return None,
        })
    }

    fn slide(self) -> Option<Slide> {
        Some(match self {
            Technique::SlideShift => Slide::Shift,
            Technique::SlideLegato => Slide::Legato,
            Technique::SlideInBelow => Slide::InFromBelow,
            Technique::SlideInAbove => Slide::InFromAbove,
            Technique::SlideOutDown => Slide::OutDown,
            Technique::SlideOutUp => Slide::OutUp,
            _ => return None,
        })
    }

    fn harmonic(self) -> Option<Harmonic> {
        Some(match self {
            Technique::HarmonicNatural => Harmonic::Natural,
            Technique::HarmonicArtificial => Harmonic::Artificial,
            Technique::HarmonicPinch => Harmonic::Pinch,
            Technique::HarmonicTapped => Harmonic::Tapped,
            Technique::HarmonicSemi => Harmonic::Semi,
            _ => return None,
        })
    }

    /// Whether a note already has this technique.
    pub(crate) fn on_note(self, effects: &NoteEffects) -> bool {
        match self {
            Technique::Hammer => effects.hammer,
            Technique::PalmMute => effects.palm_mute,
            Technique::LetRing => effects.let_ring,
            Technique::Dead => effects.dead,
            Technique::Ghost => effects.ghost,
            Technique::Staccato => effects.staccato,
            Technique::Tapped => effects.tapped,
            Technique::Accent => effects.accent == Some(Accent::Normal),
            Technique::HeavyAccent => effects.accent == Some(Accent::Heavy),
            Technique::Vibrato => effects.vibrato == Some(Vibrato::Slight),
            Technique::WideVibrato => effects.vibrato == Some(Vibrato::Wide),
            _ if self.slide().is_some() => effects.slide == self.slide(),
            _ if self.harmonic().is_some() => effects.harmonic == self.harmonic(),
            _ => false,
        }
    }

    /// Switches the technique on or off for a note. Techniques that exclude each other,
    /// such as the kinds of slide, replace one another.
    pub(crate) fn set_on_note(self, effects: &mut NoteEffects, on: bool) {
        match self {
            Technique::Hammer => effects.hammer = on,
            Technique::PalmMute => effects.palm_mute = on,
            Technique::LetRing => effects.let_ring = on,
            Technique::Dead => effects.dead = on,
            Technique::Ghost => effects.ghost = on,
            Technique::Staccato => effects.staccato = on,
            Technique::Tapped => effects.tapped = on,
            Technique::Accent => effects.accent = on.then_some(Accent::Normal),
            Technique::HeavyAccent => effects.accent = on.then_some(Accent::Heavy),
            Technique::Vibrato => effects.vibrato = on.then_some(Vibrato::Slight),
            Technique::WideVibrato => effects.vibrato = on.then_some(Vibrato::Wide),
            _ if self.slide().is_some() => effects.slide = self.slide().filter(|_| on),
            _ if self.harmonic().is_some() => effects.harmonic = self.harmonic().filter(|_| on),
            _ => {}
        }
    }

    pub(crate) fn on_beat(self, effects: &BeatEffects) -> bool {
        match self {
            Technique::StrokeDown => effects.stroke == Some(PickStroke::Down),
            Technique::StrokeUp => effects.stroke == Some(PickStroke::Up),
            Technique::Tremolo => effects.tremolo.is_some(),
            Technique::Slap => effects.slap == Some(Slap::Slap),
            Technique::Pop => effects.slap == Some(Slap::Pop),
            Technique::Crescendo => effects.hairpin == Some(Hairpin::Crescendo),
            Technique::Decrescendo => effects.hairpin == Some(Hairpin::Decrescendo),
            _ if self.strum().is_some() => effects.strum == self.strum(),
            _ => false,
        }
    }

    pub(crate) fn set_on_beat(self, effects: &mut BeatEffects, on: bool) {
        match self {
            Technique::StrokeDown => effects.stroke = on.then_some(PickStroke::Down),
            Technique::StrokeUp => effects.stroke = on.then_some(PickStroke::Up),
            Technique::Tremolo => effects.tremolo = on.then_some(NoteValue::ThirtySecond),
            Technique::Slap => effects.slap = on.then_some(Slap::Slap),
            Technique::Pop => effects.slap = on.then_some(Slap::Pop),
            Technique::Crescendo => effects.hairpin = on.then_some(Hairpin::Crescendo),
            Technique::Decrescendo => effects.hairpin = on.then_some(Hairpin::Decrescendo),
            _ if self.strum().is_some() => effects.strum = self.strum().filter(|_| on),
            _ => {}
        }
    }
}
