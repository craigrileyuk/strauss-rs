//! Tests for package enumeration. Most read `test/packages-fixture`, a small
//! project laid out like a Composer install, with the awkward cases in it: a
//! dependency cycle, a virtual package with a provider, a metapackage, a
//! plugin installed outside `vendor/`, a package that is missing, and one that
//! `installed.json` doesn't list.
//!
//! The cases that need git files (`.gitignore`, `.git`) are in `tests/packages.rs`,
//! because a fixture can't contain them.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::config::Config;

mod files;
mod gitattributes;
mod installed;
mod platform;
mod resolve;

pub(super) fn fixture_dir() -> PathBuf {
    fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/packages-fixture"))
        .unwrap()
}

pub(super) fn fixture_config() -> Config {
    Config::load(&fixture_dir()).unwrap()
}

pub(super) fn package_names(set: &PackageSet) -> Vec<&str> {
    set.packages
        .iter()
        .map(|package| package.name.as_str())
        .collect()
}
