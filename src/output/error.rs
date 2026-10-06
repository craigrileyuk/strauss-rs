//! Why writing the output failed.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum OutputError {
    /// A file or directory couldn't be read, written or deleted.
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },

    /// The planned edits don't fit the file, which has changed since it was read.
    Edit { file: String, message: String },
}

impl OutputError {
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        OutputError::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OutputError::Io {
                action,
                path,
                source,
            } => write!(formatter, "could not {action} {}: {source}", path.display()),
            OutputError::Edit { file, message } => write!(formatter, "{file}: {message}"),
        }
    }
}

impl std::error::Error for OutputError {}
