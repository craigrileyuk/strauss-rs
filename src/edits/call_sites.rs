//! `update_call_sites`: the project's own files, changed where they use symbols
//! that have been renamed.
//!
//! These are the files in the project's `autoload` (or those listed in
//! `update_call_sites`), found by walking their directories. The vendor
//! directory and the target directory are never touched. The project's own
//! namespaces are not renamed; only what it refers to is.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::config::{Autoload, Config, UpdateCallSites};
use crate::edits::file::{FileInput, plan_file};
use crate::edits::model::{EditPlan, EditWarning, FileEdits};
use crate::edits::names::NameMap;
use crate::symbols::analyse::analyse_files;

pub(super) fn plan(config: &Config, names: &NameMap, plan: &mut EditPlan) {
    let (entries, from_autoload) = match &config.update_call_sites {
        UpdateCallSites::Disabled => return,
        UpdateCallSites::FromAutoload => (autoload_paths(config), true),
        UpdateCallSites::Paths(paths) => (paths.clone(), false),
    };

    let mut warnings = Vec::new();
    let files = collect_files(config, &entries, from_autoload, &mut warnings);

    let paths: Vec<&Path> = files.iter().map(|(path, _)| path.as_path()).collect();
    let workers = crate::parallel::worker_count(config.parallel, paths.len());
    let analyses = analyse_files(&paths, workers);

    for ((path, relative), analysis) in files.iter().zip(analyses) {
        let warn = |message: String| EditWarning {
            file: relative.clone(),
            message: format!("{relative}: {message}"),
        };

        if config
            .exclude_files_from_update
            .file_patterns
            .iter()
            .any(|pattern| pattern.is_match(relative))
        {
            continue;
        }

        let analysed = match analysis {
            Ok(analysed) if analysed.lossy => {
                warnings.push(warn(
                    "not updated: it is not valid UTF-8, so it can't be edited safely".into(),
                ));
                continue;
            }
            Ok(analysed) => analysed,
            Err(message) => {
                warnings.push(warn(format!("not updated: {message}")));
                continue;
            }
        };

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                warnings.push(warn(format!(
                    "not updated: could not read the file: {error}"
                )));
                continue;
            }
        };

        let planned = plan_file(
            names,
            &FileInput {
                file: relative,
                source: &source,
                analysis: &analysed.analysis,
                keeps_namespaces: true,
            },
        );

        warnings.extend(planned.warnings.into_iter().map(warn));

        if planned.edits.is_empty() {
            plan.unchanged_files += 1;
        } else {
            plan.call_sites.push(FileEdits {
                package: String::new(),
                file: relative.clone(),
                source_path: path.clone(),
                edits: planned.edits,
            });
        }
    }

    plan.warnings.extend(warnings);
}

/// The files and directories in the project's `autoload`.
fn autoload_paths(config: &Config) -> Vec<String> {
    let Ok(text) = fs::read_to_string(config.project_dir.join("composer.json")) else {
        return Vec::new();
    };

    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|composer| composer.get("autoload").cloned())
        .and_then(|autoload| serde_json::from_value::<Autoload>(autoload).ok())
        .map(|autoload| autoload.paths().map(String::from).collect())
        .unwrap_or_default()
}

/// The `.php` files under `entries`, as (path, path relative to the project),
/// without the vendor and target directories.
fn collect_files(
    config: &Config,
    entries: &[String],
    from_autoload: bool,
    warnings: &mut Vec<EditWarning>,
) -> Vec<(PathBuf, String)> {
    let project = &config.project_dir;
    let vendor = project.join(config.vendor_directory.trim_matches('/'));
    let target = project.join(config.target_directory.trim_matches('/'));

    let mut found = BTreeSet::new();

    for entry in entries {
        let path = project.join(entry.trim_matches('/'));

        let Ok(path) = fs::canonicalize(&path) else {
            // Composer autoload paths often name directories that don't exist yet.
            if !from_autoload {
                warnings.push(EditWarning {
                    file: entry.clone(),
                    message: format!("update_call_sites: {entry} does not exist"),
                });
            }
            continue;
        };

        if !path.starts_with(project) {
            warnings.push(EditWarning {
                file: entry.clone(),
                message: format!("update_call_sites: {entry} is outside the project"),
            });
            continue;
        }

        if path.is_file() {
            found.insert(path);
            continue;
        }

        let (vendor, target) = (vendor.clone(), target.clone());
        let walk = WalkBuilder::new(&path)
            .standard_filters(false)
            .filter_entry(move |entry| {
                let path = entry.path();

                !(path.starts_with(&vendor) || path.starts_with(&target))
            })
            .build();

        for entry in walk.flatten() {
            let file = entry.path();

            if entry.file_type().is_some_and(|kind| kind.is_file())
                && file.extension().is_some_and(|extension| extension == "php")
            {
                found.insert(file.to_path_buf());
            }
        }
    }

    found
        .into_iter()
        .filter(|file| !file.starts_with(&vendor) && !file.starts_with(&target))
        .filter_map(|file| {
            let relative = file
                .strip_prefix(project)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");

            Some((file, relative))
        })
        .collect()
}
