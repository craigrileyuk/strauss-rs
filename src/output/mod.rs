//! Writing the result: the prefixed packages in the target directory and the
//! autoloader that loads them.
//!
//! Everything before this only reads. The [rename plan](crate::rename) and
//! the [edit plan](crate::edits) say what the files should become; this makes
//! it so, following PHP Strauss' `Copier`, `Prefixer` and cleanup.

#![allow(dead_code)]

mod autoload;
mod clean;
mod copy;
mod error;
mod header;

// The module's API; the CLI uses some of it so far.
#[allow(unused_imports)]
pub use error::OutputError;
#[allow(unused_imports)]
pub use header::{Header, add_header, format_date, today};

use std::collections::HashSet;
use std::fs;

use serde::Serialize;

use crate::config::Config;
use crate::edits::EditPlan;
use crate::packages::{PackageFile, PackageSet, is_target_the_vendor_directory};
use crate::rename::RenamePlan;

/// Everything that has been worked out.
pub struct Input<'a> {
    pub config: &'a Config,
    pub packages: &'a PackageSet,
    pub files: &'a [PackageFile],
    pub rename: &'a RenamePlan,
    pub edits: &'a EditPlan,
}

pub struct Options {
    /// Work out what would be written, and write nothing.
    pub dry_run: bool,

    /// The date for the note in changed files, as `05-October-2026`.
    pub date: String,

    /// From the configuration; here so they can be overridden.
    pub include_modified_date: bool,
    pub include_author: bool,
}

impl Options {
    pub fn new(config: &Config, dry_run: bool, date: String) -> Self {
        Self {
            dry_run,
            date,
            include_modified_date: config.include_modified_date,
            include_author: config.include_author,
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    /// Files written to the target directory (or changed in place).
    pub written: usize,

    /// How many of them have changes made to them.
    pub modified: usize,

    /// The autoloader files written, relative to the project.
    pub autoloader: Vec<String>,

    /// Classes, interfaces, traits and enums in the autoloader.
    pub classmap_entries: usize,

    /// Files deleted from the vendor directory.
    pub deleted_files: usize,

    /// Packages deleted from the vendor directory.
    pub deleted_packages: Vec<String>,

    pub warnings: Vec<String>,
}

/// Writes the output. Nothing is deleted unless everything was written.
pub fn write(input: &Input, options: &Options) -> Result<Report, OutputError> {
    let symlinked = symlinked_packages(input);
    let mut report = Report::default();

    let copied = copy::run(input, options, &symlinked)?;

    report.written = copied.written.len();
    report.modified = copied.modified;
    report.warnings.extend(copied.warnings.iter().cloned());

    if input.config.classmap_output && !is_target_the_vendor_directory(input.config) {
        let autoloader = autoload::plan(input, &copied.written, &mut report.warnings);
        let target = input
            .config
            .project_dir
            .join(&input.config.target_directory);

        report.classmap_entries = autoloader.classmap.len();
        report.autoloader = autoload::write(&autoloader, &target, options.dry_run)?
            .into_iter()
            .map(|name| format!("{}/{name}", input.config.target_directory))
            .collect();
    }

    let deleted = clean::run(input, options, &copied.written, &symlinked)?;

    report.deleted_files = deleted.files;
    report.deleted_packages = deleted.packages;
    report.warnings.extend(deleted.warnings);

    Ok(report)
}

/// The packages (lower-cased names) whose directory in `vendor` is, or is in, a
/// link to somewhere else.
fn symlinked_packages(input: &Input) -> HashSet<String> {
    let vendor = input
        .config
        .project_dir
        .join(&input.config.vendor_directory);

    input
        .packages
        .packages
        .iter()
        .filter(|package| {
            let lexical = vendor.join(&package.name);

            fs::canonicalize(&lexical).is_ok_and(|real| real != lexical)
        })
        .map(|package| package.name.to_ascii_lowercase())
        .collect()
}

fn composer_json(config: &Config) -> Option<serde_json::Value> {
    let text = fs::read_to_string(config.project_dir.join("composer.json")).ok()?;

    serde_json::from_str(&text).ok()
}

/// The first author in the project's composer.json.
pub(crate) fn project_author(config: &Config) -> Option<String> {
    composer_json(config)?
        .get("authors")?
        .as_array()?
        .first()?
        .get("name")?
        .as_str()
        .map(String::from)
}

/// The `files` in the project's own `autoload`.
pub(crate) fn project_autoload_files(config: &Config, warnings: &mut Vec<String>) -> Vec<String> {
    let Some(composer) = composer_json(config) else {
        return Vec::new();
    };

    let autoload = composer.get("autoload");

    if autoload.is_some_and(|autoload| {
        ["psr-4", "psr-0", "classmap"]
            .iter()
            .any(|key| autoload.get(key).is_some())
    }) {
        warnings.push(
            "include_root_autoload: only the `files` of the project's autoload are included"
                .to_string(),
        );
    }

    autoload
        .and_then(|autoload| autoload.get("files"))
        .and_then(|files| files.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
