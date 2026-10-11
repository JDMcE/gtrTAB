//! Score data model: Score → Tracks → Staves → Bars → Voices → Beats → Notes.

mod demo;
mod drums;
mod duration;
mod effects;
mod instrument;
mod playback;
mod score;
mod tuning;

pub use demo::demo_score;
pub use drums::{DRUM_KIT, DrumHead, DrumPiece, drum_piece};
pub use duration::{Duration, NoteValue, Rational, Tuplet};
pub use effects::{
    Accent, BeatEffects, Bend, BendKind, ChordDiagram, CurvePoint, Dynamic, Finger, Grace, Hairpin,
    Harmonic, NoteEffects, PickStroke, Slap, Slide, Strum, Trill, Vibrato, interval_label,
};
pub use instrument::{Clef, Instrument, Mix};
pub use playback::{bar_times, play_order};
pub use score::{
    Bar, Beat, Feel, Jump, MasterBar, ModelError, Note, Score, ScoreInfo, Staff, Target,
    TimeSignature, Track, Voice,
};
pub use tuning::Tuning;
