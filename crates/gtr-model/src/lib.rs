//! Score data model: Score → Tracks → Staves → Bars → Voices → Beats → Notes.

mod demo;
mod duration;
mod effects;
mod score;
mod tuning;

pub use demo::demo_score;
pub use duration::{Duration, NoteValue, Rational, Tuplet};
pub use effects::{
    Accent, BeatEffects, Bend, BendKind, CurvePoint, Dynamic, Finger, Grace, Hairpin, Harmonic,
    NoteEffects, PickStroke, Slap, Slide, Strum, Trill, Vibrato, interval_label,
};
pub use score::{
    Bar, Beat, MasterBar, ModelError, Note, Score, ScoreInfo, Staff, TimeSignature, Track, Voice,
};
pub use tuning::Tuning;
