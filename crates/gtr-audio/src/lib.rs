//! Playback: compiles a score to timed events and synthesises them.
//!
//! For now the whole passage is rendered to a buffer up front with a plucked-string model.

mod events;
mod synth;

pub use events::{
    Automation, BeatTime, Channel, Click, NoteEvent, Options, Timbre, Timeline, Tone, compile,
};
pub use gtr_model::play_order;
pub use synth::{Stereo, render, render_channel, render_clicks};
