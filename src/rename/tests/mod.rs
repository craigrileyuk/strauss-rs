//! Tests for deciding what gets renamed. They run the whole pipeline on
//! `test/rename-fixture`: seven packages with a case for each rule (autoload
//! kinds, exclusions, patterns, prefixes), changing the configuration where a
//! test needs something the fixture's `strauss.json` doesn't have.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::config::{Config, Parallel};
use crate::packages::{enumerate_files, enumerate_packages};
use crate::php_pattern::PhpPattern;
use crate::symbols::{SymbolKind, scan};

mod autoload;
mod copy;
mod exclusions;
mod names;
mod plan;

pub(super) fn fixture_dir() -> PathBuf {
    fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/rename-fixture")).unwrap()
}

pub(super) struct Run {
    pub plan: RenamePlan,
    pub files: Vec<PackageFile>,
}

/// Runs everything up to and including the plan, after letting the test change
/// the configuration.
pub(super) fn run(change: impl FnOnce(&mut Config)) -> Run {
    let mut config = Config::load(&fixture_dir()).unwrap();
    config.parallel = Parallel::Disabled;

    change(&mut config);

    let packages = enumerate_packages(&config).unwrap();
    let mut files = enumerate_files(&config, &packages).unwrap();
    let table = scan(&config, &files);

    exclude_namespaces_from_copy(&config, &mut files, &table);

    let plan = plan(&config, &packages, &files, &table);

    Run { plan, files }
}

pub(super) fn default_run() -> Run {
    run(|_| {})
}

pub(super) fn symbol<'a>(plan: &'a RenamePlan, kind: SymbolKind, name: &str) -> &'a PlannedSymbol {
    plan.get(kind, name)
        .unwrap_or_else(|| panic!("{kind:?} {name} is not in the plan"))
}

/// What the symbol is renamed to, if it is.
pub(super) fn new_name<'a>(plan: &'a RenamePlan, kind: SymbolKind, name: &str) -> Option<&'a str> {
    symbol(plan, kind, name).replacement.as_deref()
}

pub(super) fn pattern(source: &str) -> PhpPattern {
    PhpPattern::regex(source).unwrap()
}
