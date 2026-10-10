//! Playback: compiles a score to timed events and synthesises them.
//!
//! For now the whole passage is rendered to a buffer up front with a plucked-string model.

mod events;
mod synth;

pub use events::{BeatTime, Click, NoteEvent, Options, Timeline, Tone, compile};
pub use synth::render;
