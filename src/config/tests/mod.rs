//! Tests for loading the configuration, grouped by what they exercise.

use std::path::Path;

use super::*;

mod exclusions;
mod inference;
mod loading;
mod options;
mod prefixes;
mod schema;
mod serialization;

pub(super) fn config(strauss: &str, composer: &str) -> Config {
    Config::from_json(Path::new("/project"), Some(strauss), Some(composer))
        .unwrap_or_else(|error| panic!("{error}"))
}

pub(super) fn strauss_only(strauss: &str) -> Config {
    Config::from_json(Path::new("/project"), Some(strauss), None)
        .unwrap_or_else(|error| panic!("{error}"))
}

pub(super) fn error(strauss: &str) -> String {
    Config::from_json(Path::new("/project"), Some(strauss), None)
        .unwrap_err()
        .to_string()
}
