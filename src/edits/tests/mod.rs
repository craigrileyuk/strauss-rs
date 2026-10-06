//! Tests for planning the changes to files.
//!
//! Most plan a snippet of PHP against a name map made by hand, so each rule can be
//! tried on its own. `project.rs` runs the whole pipeline on `test/edit-fixture`.

use std::path::PathBuf;

use php_rs_parser::parse;

use super::*;
use crate::analyser::analyse;
use crate::config::{Config, Parallel};
use crate::edits::file::{FileInput, Planned, plan_file};
use crate::edits::names::NameMap;
use crate::packages::{enumerate_files, enumerate_packages};
use crate::rename;
use crate::symbols::SymbolKind;
use crate::symbols::scan;

mod apply;
mod declarations;
mod docblocks;
mod imports;
mod names;
mod plan;
mod project;
mod references;
mod strings;

pub(super) fn fixture_dir(name: &str) -> PathBuf {
    std::fs::canonicalize(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test")
            .join(name),
    )
    .unwrap()
}

/// The names renamed, for the small cases:
///
/// * namespace `Acme\Core` becomes `P\V\Acme\Core`, and so do the classes `Client`
///   and `Sub\Thing` in it; `Acme\Core\Excluded` is not renamed;
/// * the global class `GlobalClass`, function `g_helper` and constant `G_CONST`
///   get prefixes.
pub(super) fn names() -> NameMap {
    NameMap::default()
        .with(
            SymbolKind::Namespace,
            "Acme\\Core",
            Some("P\\V\\Acme\\Core"),
        )
        .with(
            SymbolKind::Namespace,
            "Acme\\Core\\Sub",
            Some("P\\V\\Acme\\Core\\Sub"),
        )
        .with(SymbolKind::Namespace, "Acme\\Core\\Excluded", None)
        .with(
            SymbolKind::Class,
            "Acme\\Core\\Client",
            Some("P\\V\\Acme\\Core\\Client"),
        )
        .with(
            SymbolKind::Class,
            "Acme\\Core\\Sub\\Thing",
            Some("P\\V\\Acme\\Core\\Sub\\Thing"),
        )
        .with(SymbolKind::Class, "Acme\\Core\\Excluded\\Item", None)
        .with(
            SymbolKind::Function,
            "Acme\\Core\\helper",
            Some("P\\V\\Acme\\Core\\helper"),
        )
        .with(
            SymbolKind::Constant,
            "Acme\\Core\\VERSION",
            Some("P\\V\\Acme\\Core\\VERSION"),
        )
        .with(SymbolKind::Class, "GlobalClass", Some("P_GlobalClass"))
        .with(SymbolKind::Function, "g_helper", Some("p_g_helper"))
        .with(SymbolKind::Constant, "G_CONST", Some("P_C_G_CONST"))
}

/// Plans `source` and returns the planned edits.
pub(super) fn plan_source(names: &NameMap, source: &str, keeps_namespaces: bool) -> Planned {
    let result = parse(source);

    assert!(
        result.errors.is_empty(),
        "PHP parse errors: {:?}",
        result.errors
    );

    let analysis = analyse(source, &result.program);

    plan_file(
        names,
        &FileInput {
            file: "pkg/file.php",
            source,
            analysis: &analysis,
            keeps_namespaces,
        },
    )
}

/// `source` with its planned edits made.
pub(super) fn rewrite(names: &NameMap, source: &str) -> String {
    rewrite_with(names, source, false)
}

pub(super) fn rewrite_with(names: &NameMap, source: &str, keeps_namespaces: bool) -> String {
    let planned = plan_source(names, source, keeps_namespaces);

    assert!(planned.warnings.is_empty(), "{:?}", planned.warnings);

    let changed = apply_edits(source, &planned.edits).unwrap();

    // The result is still PHP.
    assert!(
        parse(&changed).errors.is_empty(),
        "the rewritten file does not parse:\n{changed}"
    );

    changed
}

/// A snippet inside `namespace Acme\Consumer;`.
pub(super) fn in_namespace(body: &str) -> String {
    format!("<?php\nnamespace Acme\\Consumer;\n\n{body}\n")
}

/// Everything the planner is given and what it makes of it.
pub(super) struct Run {
    pub edits: EditPlan,
    pub rename: rename::RenamePlan,
    pub files: Vec<crate::packages::PackageFile>,
    pub table: crate::symbols::SymbolTable,
}

/// Runs the pipeline up to the edits on a project in `test/`, after letting
/// the test change the configuration.
pub(super) fn run(project: &str, change: impl FnOnce(&mut Config)) -> Run {
    let mut config = Config::load(&fixture_dir(project)).unwrap();
    config.parallel = Parallel::Disabled;

    change(&mut config);

    let packages = enumerate_packages(&config).unwrap();
    let mut files = enumerate_files(&config, &packages).unwrap();
    let table = scan(&config, &files);

    rename::exclude_namespaces_from_copy(&config, &mut files, &table);

    let rename = rename::plan(&config, &packages, &files, &table);
    let edits = plan(&config, &files, &table, &rename);

    Run {
        edits,
        rename,
        files,
        table,
    }
}

/// The edit-fixture, which has a case for most things.
pub(super) fn edit_fixture() -> Run {
    run("edit-fixture", |_| {})
}

/// A file of a project with its edits made.
pub(super) fn rewritten_file(run: &Run, file: &str) -> String {
    let edits = run
        .edits
        .get(file)
        .unwrap_or_else(|| panic!("{file} has no edits"));

    let source = std::fs::read_to_string(&edits.source_path).unwrap();

    apply_edits(&source, &edits.edits).unwrap()
}

pub(super) fn pattern(source: &str) -> crate::php_pattern::PhpPattern {
    crate::php_pattern::PhpPattern::regex(source).unwrap()
}
