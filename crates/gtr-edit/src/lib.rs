//! Score editing: every change is a [`Command`], which makes it undoable.

mod command;
mod editor;
mod history;

pub use command::{BeatRef, Command, EditError};
pub use editor::{Cursor, Editor, EditorState, MAX_FRET, Move};
pub use history::History;
