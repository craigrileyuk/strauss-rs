//! `delete_vendor_files` and `delete_vendor_packages`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::output::error::OutputError;
use crate::output::{Input, Options};
use crate::packages::is_target_the_vendor_directory;

#[derive(Debug, Default)]
pub(super) struct Deleted {
    pub files: usize,
    pub packages: Vec<String>,
    pub composer_updated: bool,
    pub warnings: Vec<String>,
}

/// Deletes from the vendor directory what was copied. Nothing that is a link
/// to somewhere else is touched, since that is someone's source and not an
/// installed copy.
pub(super) fn run(
    input: &Input,
    options: &Options,
    written: &HashSet<String>,
    symlinked: &HashSet<String>,
) -> Result<Deleted, OutputError> {
    let config = input.config;
    let mut deleted = Deleted::default();

    if !(config.delete_vendor_files || config.delete_vendor_packages) {
        return Ok(deleted);
    }

    if is_target_the_vendor_directory(config) {
        deleted.warnings.push(
            "delete_vendor_files and delete_vendor_packages are ignored: the target directory is the vendor directory"
                .to_string(),
        );

        return Ok(deleted);
    }

    let vendor = config.project_dir.join(&config.vendor_directory);
    let mut affected: Vec<String> = Vec::new();

    for package in &input.packages.packages {
        let Some(path) = &package.path else {
            continue;
        };

        let copied: Vec<_> = input
            .files
            .iter()
            .filter(|file| {
                file.package.eq_ignore_ascii_case(&package.name)
                    && written.contains(&file.vendor_relative_path)
            })
            .collect();

        if copied.is_empty() {
            continue;
        }

        if symlinked.contains(&package.name.to_ascii_lowercase()) {
            deleted.warnings.push(format!(
                "{} is not deleted: it is a link to another directory",
                package.name
            ));
            continue;
        }

        // Never reach outside the vendor directory.
        if !path.starts_with(fs::canonicalize(&vendor).unwrap_or(vendor.clone())) {
            deleted.warnings.push(format!(
                "{} is not deleted: it is not inside {}",
                package.name,
                vendor.display()
            ));
            continue;
        }

        let everything_copied = input
            .files
            .iter()
            .filter(|file| file.package.eq_ignore_ascii_case(&package.name))
            .all(|file| written.contains(&file.vendor_relative_path));

        if config.delete_vendor_packages && everything_copied {
            if !options.dry_run {
                fs::remove_dir_all(path).map_err(|error| OutputError::io("delete", path, error))?;
                remove_empty_parents(path, &vendor);
            }

            deleted.files += copied.len();
            deleted.packages.push(package.name.clone());
            affected.push(package.name.clone());
            continue;
        }

        if config.delete_vendor_packages {
            deleted.warnings.push(format!(
                "{} is not deleted: some of its files are not copied; only the copied files are deleted",
                package.name
            ));
        }

        affected.push(package.name.clone());

        for file in copied {
            if !options.dry_run {
                fs::remove_file(&file.source_path)
                    .map_err(|error| OutputError::io("delete", &file.source_path, error))?;
            }

            deleted.files += 1;
        }

        if !options.dry_run {
            remove_empty_directories(path);
            remove_empty_parents(path, &vendor);
        }
    }

    if !options.dry_run && !affected.is_empty() {
        super::composer::forget_packages(
            config,
            &affected,
            options.composer.as_deref(),
            &mut deleted.warnings,
        );

        deleted.composer_updated = true;
    }

    Ok(deleted)
}

/// Removes `directory` and the directories inside it that are empty.
fn remove_empty_directories(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            remove_empty_directories(&entry.path());
        }
    }

    // Fails, harmlessly, when something is left in it.
    let _ = fs::remove_dir(directory);
}

/// Removes the directories above `path`, up to but not including `stop`, that
/// are empty (`vendor/acme` once `vendor/acme/lib` is gone).
fn remove_empty_parents(path: &Path, stop: &Path) {
    let stop = fs::canonicalize(stop).unwrap_or_else(|_| stop.to_path_buf());
    let mut current = path.parent();

    while let Some(directory) = current {
        if directory == stop || !directory.starts_with(&stop) {
            break;
        }

        if fs::remove_dir(directory).is_err() {
            break;
        }

        current = directory.parent();
    }
}
