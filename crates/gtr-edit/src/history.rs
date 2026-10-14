use gtr_model::Score;

use crate::{Command, EditError};

/// Undo and redo stacks of inverse commands. Each entry carries a tag `T`, such as the cursor
/// position, describing the editor state to restore alongside the score.
#[derive(Debug)]
pub struct History<T = ()> {
    undo: Vec<(Command, T)>,
    redo: Vec<(Command, T)>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<T> History<T> {
    /// Applies a new command and discards any redo history. `before` is the tag to restore
    /// when the command is undone.
    pub fn execute(
        &mut self,
        score: &mut Score,
        command: Command,
        before: T,
    ) -> Result<(), EditError> {
        self.undo.push((command.apply(score)?, before));
        self.redo.clear();
        Ok(())
    }

    /// Undoes one step and returns the tag saved with it, or `None` if there was nothing to
    /// undo. `current` is saved for the matching redo.
    pub fn undo(&mut self, score: &mut Score, current: T) -> Result<Option<T>, EditError> {
        Self::step(&mut self.undo, &mut self.redo, score, current)
    }

    /// Redoes one step; see [`Self::undo`].
    pub fn redo(&mut self, score: &mut Score, current: T) -> Result<Option<T>, EditError> {
        Self::step(&mut self.redo, &mut self.undo, score, current)
    }

    /// Applies a command as part of the last step, so that it is undone and redone with
    /// it, as a consequence of that step rather than a step of its own. With no step to
    /// undo, it is only applied.
    pub fn fold(&mut self, score: &mut Score, command: Command) -> Result<(), EditError> {
        let Some((last, tag)) = self.undo.pop() else {
            return command.apply(score).map(drop);
        };
        match command.apply(score) {
            // Undone first, then the step it followed.
            Ok(inverse) => self.undo.push((Command::Batch(vec![inverse, last]), tag)),
            Err(e) => {
                self.undo.push((last, tag));
                return Err(e);
            }
        }
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of steps that can be undone.
    pub fn depth(&self) -> usize {
        self.undo.len()
    }

    fn step(
        from: &mut Vec<(Command, T)>,
        to: &mut Vec<(Command, T)>,
        score: &mut Score,
        current: T,
    ) -> Result<Option<T>, EditError> {
        let Some((command, tag)) = from.pop() else {
            return Ok(None);
        };
        to.push((command.apply(score)?, current));
        Ok(Some(tag))
    }
}
