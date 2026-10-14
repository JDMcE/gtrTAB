//! Score data model: Score → Tracks → Staves → Bars → Voices → Beats → Notes.

mod demo;
mod drums;
mod duration;
mod effects;
mod instrument;
mod pedal;
mod playback;
mod recording;
mod score;
mod style;
mod tuning;

pub use demo::demo_score;
pub use drums::{DRUM_KIT, DrumHead, DrumPiece, KitPiece, drum_piece, kit_is_valid, standard_kit};
pub use duration::{Duration, NoteValue, Rational, Tuplet};
pub use effects::{
    Accent, BeatEffects, Bend, BendKind, ChordDiagram, CurvePoint, Dynamic, Fade, Finger, Grace,
    GraceNote, Hairpin, Harmonic, NoteEffects, PickStroke, Slap, Slide, Strum, Trill, Vibrato,
    interval_label,
};
pub use instrument::{Clef, Instrument, Mix};
pub use pedal::{Knob, Pedal, PedalKind, Preset, Sound, presets};
pub use playback::{
    BarTiming, TempoPoint, bar_times, gradual_change, play_order, range_order, tempo_points,
    timings,
};
pub use recording::{AudioTrack, SyncPoint, moved, on_beats, synced};
pub use score::{
    Bar, Beat, Feel, Jump, MasterBar, ModelError, Note, Score, ScoreInfo, Staff, Target,
    TempoChange, TimeSignature, Track, Voice,
};
pub use style::{BarNumbers, Style, TabRhythm};
pub use tuning::Tuning;
