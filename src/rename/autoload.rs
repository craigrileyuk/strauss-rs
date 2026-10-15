//! Which symbols are autoloaded.
//!
//! A symbol that nothing autoloads is only reached by the package's own code
//! requiring it, so doesn't need renaming. A symbol the package's autoloader
//! can find has to be, because other code finds it by name. This follows PHP
//! Strauss' `AutoloadedEnumerator::scanSetIsAutoloaded()`.
//!
//! A namespace is autoloaded when it is within a `psr-0` or `psr-4` key; a file
//! when it is in an autoloaded directory, a `classmap` path or the `files` list,
//! or declares an autoloaded symbol. Autoloaded files make their symbols
//! autoloaded; a symbol declared in several files is autoloaded as soon as any
//! of them is.

use std::collections::HashMap;

use crate::config::{Autoload, StringOrList};
use crate::packages::PackageSet;
use crate::rename::is_within_namespace;
use crate::rename::model::PlannedSymbol;
use crate::symbols::{SymbolKind, SymbolTable};

pub(super) fn mark(entries: &mut [PlannedSymbol], table: &SymbolTable, packages: &PackageSet) {
    // The symbols each file declares.
    let mut declared_in: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, entry) in entries.iter().enumerate() {
        for source in &entry.sources {
            declared_in
                .entry(source.file.clone())
                .or_default()
                .push(index);
        }
    }

    let mut file_autoloaded: HashMap<&str, bool> = table
        .files
        .iter()
        .map(|file| (file.vendor_relative_path.as_str(), false))
        .collect();

    // A namespace within an autoload key is autoloaded, and so are the files
    // that declare it.
    for index in 0..entries.len() {
        if entries[index].kind != SymbolKind::Namespace
            || !in_psr_namespace(&entries[index], packages)
        {
            continue;
        }

        entries[index].autoloaded = true;

        for source in &entries[index].sources {
            if let Some(autoloaded) = file_autoloaded.get_mut(source.file.as_str()) {
                *autoloaded = true;
            }
        }
    }

    for file in &table.files {
        let path = file.vendor_relative_path.as_str();
        let relative = package_relative_path(&file.package, path);

        let Some(package) = packages.get(&file.package) else {
            continue;
        };

        let autoload = &package.autoload;
        let symbols = declared_in.get(path).map(Vec::as_slice).unwrap_or(&[]);

        let mut autoloaded = file_autoloaded[path];

        if !autoloaded && in_classmap(autoload, relative) {
            autoloaded = true;
        }

        if !autoloaded && in_files(autoload, relative) {
            autoloaded = true;
        }

        // It declares a symbol that is autoloaded another way.
        if !autoloaded
            && symbols.iter().any(|index| {
                entries[*index].kind != SymbolKind::Namespace && entries[*index].autoloaded
            })
        {
            autoloaded = true;
        }

        if !autoloaded && in_psr_directory(autoload.psr4.0.iter(), relative, "/\\") {
            autoloaded = true;
        }

        // Everything a PSR-0 file declares is found by its name, except what is
        // in the global namespace.
        if !autoloaded && in_psr_directory(autoload.psr0.0.iter(), relative, "\\") {
            autoloaded = true;

            for index in symbols {
                if !entries[*index].is_global() || entries[*index].kind == SymbolKind::Namespace {
                    entries[*index].autoloaded = true;
                }
            }
        }

        file_autoloaded.insert(path, autoloaded);

        if autoloaded {
            for index in symbols {
                entries[*index].autoloaded = true;
            }
        }
    }
}

/// `src/Foo.php` for `acme/lib/src/Foo.php` in package `acme/lib`.
pub(super) fn package_relative_path<'a>(package: &str, vendor_relative_path: &'a str) -> &'a str {
    vendor_relative_path
        .get(package.len()..)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(vendor_relative_path)
}

/// Whether the namespace is within a `psr-0` or `psr-4` key of a package that
/// declares it.
fn in_psr_namespace(entry: &PlannedSymbol, packages: &PackageSet) -> bool {
    entry
        .packages
        .iter()
        .filter_map(|name| packages.get(name))
        .any(|package| {
            package
                .autoload
                .psr4
                .0
                .iter()
                .chain(package.autoload.psr0.0.iter())
                .any(|(key, _)| {
                    let key = key.trim_matches('\\');

                    key.is_empty() || is_within_namespace(&entry.name, key)
                })
        })
}

/// A file in a directory a `psr-0` or `psr-4` rule maps. `trim` is what PHP
/// Strauss strips from the front of both paths before comparing: PSR-4 allows
/// a leading slash, PSR-0 a leading backslash.
pub(super) fn in_psr_directory<'a>(
    rules: impl Iterator<Item = &'a (String, StringOrList)>,
    relative: &str,
    characters_to_trim: &str,
) -> bool {
    let trim = |text: &str| {
        text.trim_start_matches(|c| characters_to_trim.contains(c))
            .to_string()
    };
    let relative = trim(relative);

    rules
        .flat_map(|(_, directories)| directories.iter())
        .any(|directory| relative.starts_with(&trim(directory)))
}

pub(super) fn in_files(autoload: &Autoload, relative: &str) -> bool {
    autoload.files.iter().any(|path| path == relative)
}

/// In a `classmap` path and not in `exclude-from-classmap`.
pub(super) fn in_classmap(autoload: &Autoload, relative: &str) -> bool {
    autoload.classmap.iter().any(|path| {
        let excluded = autoload
            .exclude_from_classmap
            .iter()
            .any(|excluded| relative.starts_with(excluded.as_str()));

        !excluded && (path == "." || relative.starts_with(path.as_str()))
    })
}
