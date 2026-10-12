//! Playback: compiles a score to timed events and synthesises them.
//!
//! The synth is an [`Engine`] that plays a [`Timeline`] a block at a time, from the samples
//! of a [`SoundFont`] if it has been given one and otherwise with built-in models of plucked
//! strings and drums; [`render`] runs it over a whole passage at once, and [`wav`] writes the
//! result as a file.

mod engine;
mod events;
mod soundfont;
mod voice;
mod wav;
mod wire;

pub use soundfont::{SoundFont, SoundFontError};

pub use engine::{Engine, Stereo, render};
pub use events::{
    Automation, BeatTime, Channel, Click, NoteEvent, Options, Pulse, Timbre, Timeline, Tone,
    audition, compile,
};
pub use wav::wav;
