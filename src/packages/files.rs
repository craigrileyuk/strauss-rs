//! The files of each package, and which of them are copied.
//!
//! This follows PHP Strauss' `FileEnumerator` and the part of
//! `FileCopyScanner` that doesn't need to have read the files: every file in
//! a package's directory is listed, not just autoloaded ones, minus those git
//! would leave out of an archive.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::config::Config;
use crate::packages::error::PackagesError;
use crate::packages::gitattributes::GitAttributes;
use crate::packages::model::{CopyExclusion, Package, PackageFile, PackageSet};

/// Lists the files of every package, ordered by path.
pub fn enumerate_files(
    config: &Config,
    set: &PackageSet,
) -> Result<Vec<PackageFile>, PackagesError> {
    let mut files = Vec::new();

    for package in &set.packages {
        let Some(path) = &package.path else {
            continue;
        };

        let relative_paths = list_package(package, path, config.exclude_git_files)?;

        files.extend(
            relative_paths
                .into_iter()
                .map(|relative| package_file(config, package, path, relative)),
        );
    }

    files.sort_by(|a, b| a.vendor_relative_path.cmp(&b.vendor_relative_path));

    Ok(files)
}

/// Every file under `root`, as a path relative to it using `/`.
///
/// With `exclude_git_files` that leaves out `.git`, files matched by a
/// `.gitignore` in the package (at any depth), and files marked
/// `export-ignore` in its `.gitattributes`. Only the package's own git
/// configuration applies: not the project's, or the user's global one.
fn list_package(
    package: &Package,
    root: &Path,
    exclude_git_files: bool,
) -> Result<Vec<String>, PackagesError> {
    let walk_error = |message: String| PackagesError::Walk {
        package: package.name.clone(),
        message,
    };

    let mut builder = WalkBuilder::new(root);

    builder
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .git_ignore(exclude_git_files);

    if exclude_git_files {
        builder.filter_entry(|entry| !(entry.depth() > 0 && entry.file_name() == ".git"));
    }

    let attributes = if exclude_git_files {
        GitAttributes::load(root).map_err(|error| walk_error(error.to_string()))?
    } else {
        GitAttributes::default()
    };

    let mut relative_paths = Vec::new();

    for entry in builder.build() {
        let entry = entry.map_err(|error| walk_error(error.to_string()))?;

        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }

        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };

        let relative = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        if exclude_git_files && attributes.is_export_ignored(&relative) {
            continue;
        }

        relative_paths.push(relative);
    }

    Ok(relative_paths)
}

fn package_file(config: &Config, package: &Package, root: &Path, relative: String) -> PackageFile {
    let vendor_relative_path = format!("{}/{}", package.name, relative);

    let target_path = relative.split('/').fold(
        config
            .project_dir
            .join(&config.target_directory)
            .join(&package.name),
        |path, segment| path.join(segment),
    );

    let not_copied_because = copy_exclusion(config, package, &vendor_relative_path);

    PackageFile {
        package: package.name.clone(),
        source_path: relative
            .split('/')
            .fold(root.to_path_buf(), |path, segment| path.join(segment)),
        package_relative_path: relative,
        vendor_relative_path,
        target_path,
        copy: not_copied_because.is_none(),
        not_copied_because,
    }
}

/// Why a file is not copied, if it isn't. Namespaces in `exclude_from_copy`
/// are not considered here: they depend on what the file declares.
fn copy_exclusion(
    config: &Config,
    package: &Package,
    vendor_relative_path: &str,
) -> Option<CopyExclusion> {
    if is_target_the_vendor_directory(config) {
        return Some(CopyExclusion::TargetIsVendor);
    }

    if config
        .exclude_from_copy
        .packages
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(&package.name))
    {
        return Some(CopyExclusion::PackageExcluded);
    }

    config
        .exclude_from_copy
        .file_patterns
        .iter()
        .find(|pattern| pattern.is_match(vendor_relative_path))
        .map(|pattern| CopyExclusion::PatternMatched(pattern.source().to_string()))
}

pub(super) fn is_target_the_vendor_directory(config: &Config) -> bool {
    let normalise = |path: &str| -> PathBuf {
        path.trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != ".")
            .collect()
    };

    normalise(&config.target_directory) == normalise(&config.vendor_directory)
}
