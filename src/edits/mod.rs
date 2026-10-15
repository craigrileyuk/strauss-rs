//! The changes to make to the files, so that the renamed symbols still work.
//!
//! The [rename plan](crate::rename) says what each symbol becomes. This finds
//! everywhere that has to change to match: namespace declarations, `use`
//! statements, global declarations, names in code and the strings that name a
//! function or constant.

#![allow(dead_code)]

mod apply;
mod context;
mod file;
mod model;
mod names;
mod text;

// The module's API; the CLI uses some of it so far.
#[allow(unused_imports)]
pub use apply::{ApplyError, apply_edits};
#[allow(unused_imports)]
pub use model::*;

use std::collections::{HashMap, HashSet};
use std::fs;

use rayon::prelude::*;

use crate::config::Config;
use crate::packages::{PackageFile, is_target_the_vendor_directory};
use crate::parallel;
use crate::rename::{RenamePlan, is_within_namespace};
use crate::symbols::{ScannedFile, SymbolTable};

use file::{FileInput, plan_file};
use names::NameMap;

/// Plans the changes to every PHP file that is copied.
pub fn plan(
    config: &Config,
    files: &[PackageFile],
    table: &SymbolTable,
    rename: &RenamePlan,
) -> EditPlan {
    let names = NameMap::from_plan(rename);

    let package_files: HashMap<&str, &PackageFile> = files
        .iter()
        .map(|file| (file.vendor_relative_path.as_str(), file))
        .collect();

    let prefixed: HashSet<String> = rename
        .packages_to_prefix
        .iter()
        .map(|package| package.to_ascii_lowercase())
        .collect();

    let left_alone: HashSet<&str> = rename
        .files_excluded_from_prefix
        .iter()
        .map(|file| file.file.as_str())
        .collect();

    let target_is_vendor = is_target_the_vendor_directory(config);

    let mut plan = EditPlan::default();
    let mut to_plan: Vec<(&ScannedFile, bool)> = Vec::new();

    for scanned in &table.files {
        let path = scanned.vendor_relative_path.as_str();

        let Some(package_file) = package_files.get(path) else {
            continue;
        };

        let skip = if scanned.lossy {
            Some(SkipReason::NotUtf8)
        } else if scanned.analysis.is_none() {
            Some(SkipReason::NotAnalysed)
        } else if !(package_file.copy || target_is_vendor) {
            Some(SkipReason::NotCopied)
        } else {
            excluded_from_update(config, scanned).map(SkipReason::ExcludedFromUpdate)
        };

        if let Some(reason) = skip {
            plan.skipped.push(SkippedFile {
                file: path.to_string(),
                reason,
            });

            continue;
        }

        let keeps_namespaces =
            !prefixed.contains(&scanned.package.to_ascii_lowercase()) || left_alone.contains(path);

        to_plan.push((scanned, keeps_namespaces));
    }

    let workers = parallel::worker_count(config.parallel, to_plan.len());

    let stage = crate::progress::stage("Planning", to_plan.len());

    let planned: Vec<(&ScannedFile, Result<file::Planned, String>)> = parallel::pool(workers)
        .install(|| {
            to_plan
                .par_iter()
                .map(|(scanned, keeps_namespaces)| {
                    let planned = plan_one(&names, scanned, *keeps_namespaces);

                    stage.tick();

                    (*scanned, planned)
                })
                .collect()
        });

    for (scanned, result) in planned {
        match result {
            Err(message) => plan.warnings.push(EditWarning {
                file: scanned.vendor_relative_path.clone(),
                message,
            }),

            Ok(planned) => {
                plan.warnings
                    .extend(planned.warnings.into_iter().map(|message| EditWarning {
                        file: scanned.vendor_relative_path.clone(),
                        message,
                    }));

                if planned.edits.is_empty() {
                    plan.unchanged_files += 1;
                } else {
                    plan.files.push(FileEdits {
                        package: scanned.package.clone(),
                        file: scanned.vendor_relative_path.clone(),
                        source_path: scanned.source_path.clone(),
                        edits: planned.edits,
                    });
                }
            }
        }
    }

    plan
}

fn plan_one(
    names: &NameMap,
    scanned: &ScannedFile,
    keeps_namespaces: bool,
) -> Result<file::Planned, String> {
    let analysis = scanned
        .analysis
        .as_ref()
        .expect("files without an analysis are skipped");

    let source = fs::read_to_string(&scanned.source_path)
        .map_err(|error| format!("could not read the file: {error}"))?;

    Ok(plan_file(
        names,
        &FileInput {
            file: &scanned.vendor_relative_path,
            source: &source,
            analysis,
            keeps_namespaces,
        },
    ))
}

/// What in `exclude_files_from_update` covers the file, if anything.
fn excluded_from_update(config: &Config, scanned: &ScannedFile) -> Option<String> {
    let exclusions = &config.exclude_files_from_update;

    if let Some(package) = exclusions
        .packages
        .iter()
        .find(|package| package.eq_ignore_ascii_case(&scanned.package))
    {
        return Some(format!("package `{package}`"));
    }

    if let Some(pattern) = exclusions
        .file_patterns
        .iter()
        .find(|pattern| pattern.is_match(&scanned.vendor_relative_path))
    {
        return Some(format!("file pattern `{}`", pattern.source()));
    }

    let analysis = scanned.analysis.as_ref()?;

    // A namespace listed covers the namespaces inside it too.
    analysis
        .namespaces
        .iter()
        .filter_map(|namespace| namespace.namespace.as_deref())
        .find_map(|declared| {
            exclusions
                .namespaces
                .iter()
                .find(|excluded| is_within_namespace(declared, excluded))
        })
        .map(|namespace| format!("namespace `{namespace}`"))
}

#[cfg(test)]
mod tests;
