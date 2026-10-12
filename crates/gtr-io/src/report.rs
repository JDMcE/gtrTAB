//! What an import or an export gives back, and how either can fail.

use std::collections::BTreeSet;

use gtr_model::{ModelError, Score};

/// A score read from another program's file.
#[derive(Debug)]
pub struct Imported {
    pub score: Score,
    /// Kinds of content the file contained that were not imported, in alphabetical order.
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("not a file of a kind that can be imported")]
    UnknownFormat,
    #[error("{0} files are not supported yet")]
    UnsupportedVersion(String),
    #[error("the file is damaged: {0}")]
    Corrupt(String),
    #[error("the imported score is inconsistent: {0}")]
    Invalid(#[from] ModelError),
}

/// A score written as another program's file.
#[derive(Debug)]
pub struct Exported {
    pub bytes: Vec<u8>,
    /// Kinds of content the score holds that the format has no place for, or that were
    /// written only roughly, in alphabetical order.
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("the file could not be put together: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Names of the kinds of content that were left out.
#[derive(Default)]
pub(crate) struct Skipped(pub(crate) BTreeSet<&'static str>);

impl Skipped {
    pub(crate) fn add(&mut self, what: &'static str) {
        self.0.insert(what);
    }

    pub(crate) fn add_if(&mut self, condition: bool, what: &'static str) {
        if condition {
            self.add(what);
        }
    }

    pub(crate) fn list(self) -> Vec<String> {
        self.0.into_iter().map(String::from).collect()
    }
}
