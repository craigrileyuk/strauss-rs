//! Tests for the symbols a project's packages declare. They read
//! `test/symbols-fixture`: three packages with a polyfill, class hierarchies,
//! both namespace styles, a symbol declared twice, and files that aren't
//! analysed as expected (non-`.php`, unparseable, not UTF-8).

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::config::{Config, Parallel};
use crate::packages::{enumerate_files, enumerate_packages};

mod builtins;
mod parallel;
mod presets;
mod table;

pub(super) fn fixture_dir() -> PathBuf {
    fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/symbols-fixture"))
        .unwrap()
}

pub(super) fn scan_fixture(parallel: Parallel) -> SymbolTable {
    let mut config = Config::load(&fixture_dir()).unwrap();
    config.parallel = parallel;

    let set = enumerate_packages(&config).unwrap();
    let files = enumerate_files(&config, &set).unwrap();

    scan(&config, &files)
}

pub(super) fn names(symbols: &[&DiscoveredSymbol]) -> Vec<String> {
    symbols.iter().map(|symbol| symbol.name.clone()).collect()
}
