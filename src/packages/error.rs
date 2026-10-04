//! Why enumerating packages failed.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum PackagesError {
    /// There is no `vendor/composer/installed.json`: Composer hasn't installed
    /// the project's dependencies.
    NotInstalled { path: PathBuf },
    /// A file couldn't be read.
    Read { path: PathBuf, source: io::Error },
    /// `installed.json` isn't what Composer writes.
    Parse { path: PathBuf, message: String },
    /// A package's directory couldn't be listed.
    Walk { package: String, message: String },
}

impl fmt::Display for PackagesError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PackagesError::NotInstalled { path } => write!(
                formatter,
                "{} does not exist; run `composer install` first",
                path.display()
            ),
            PackagesError::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            PackagesError::Parse { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            PackagesError::Walk { package, message } => {
                write!(
                    formatter,
                    "could not list the files of {package}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for PackagesError {}
