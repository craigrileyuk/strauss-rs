//! Why loading the configuration failed.

use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::config::CONFIG_FILE;

#[derive(Debug)]
pub enum ConfigError {
    /// A config file exists but couldn't be read.
    Read { path: PathBuf, source: io::Error },
    /// A config file isn't valid JSON, or has the wrong shape.
    Parse { file: &'static str, message: String },
    /// An option has a value Strauss can't use.
    Invalid { key: String, message: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            ConfigError::Parse { file, message } => write!(formatter, "{file}: {message}"),
            ConfigError::Invalid { key, message } => {
                write!(formatter, "{CONFIG_FILE}: `{key}` {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

pub(super) fn invalid(key: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        key: key.into(),
        message: message.into(),
    }
}
