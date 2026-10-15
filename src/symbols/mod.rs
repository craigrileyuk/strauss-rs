//! The symbols each package declares.
//!
//! This follows PHP Strauss' `FileSymbolScanner`: every `.php` file of every
//! package is analysed, including those of packages that won't be renamed,
//! because what they declare decides what other code can refer to. Symbols PHP
//! provides itself are left out (see [`BuiltIns`]).

#![allow(dead_code)]

pub(crate) mod analyse;
mod builtins;
mod model;
pub(crate) mod presets;

// The module's API; the CLI uses some of it so far.
#[allow(unused_imports)]
pub use builtins::BuiltIns;
#[allow(unused_imports)]
pub use model::*;
#[allow(unused_imports)]
pub use presets::PresetSymbols;

use crate::analyser::{self, FileAnalysis};
use crate::config::Config;
use crate::packages::PackageFile;

use model::key;

/// Analyses the `.php` files and collects the symbols they declare.
pub fn scan(config: &Config, files: &[PackageFile]) -> SymbolTable {
    // The same test PHP Strauss uses: a `.php` suffix.
    let php_files: Vec<&PackageFile> = files
        .iter()
        .filter(|file| file.vendor_relative_path.ends_with(".php"))
        .collect();

    let paths: Vec<&std::path::Path> = php_files
        .iter()
        .map(|file| file.source_path.as_path())
        .collect();
    let workers = crate::parallel::worker_count(config.parallel, paths.len());

    let analyses = analyse::analyse_files(&paths, workers);

    let mut table = SymbolTable::default();

    let presets: Vec<(&str, &PresetSymbols)> = config
        .exclude_presets
        .iter()
        .filter_map(|name| Some((name.as_str(), PresetSymbols::get(name)?)))
        .collect();

    for (file, analysis) in php_files.into_iter().zip(analyses) {
        let (analysis, lossy) = match analysis {
            Ok(analysed) => (Some(analysed.analysis), analysed.lossy),
            Err(message) => {
                table.warnings.push(ScanWarning {
                    package: file.package.clone(),
                    file: file.vendor_relative_path.clone(),
                    message,
                });

                (None, false)
            }
        };

        if lossy {
            table.warnings.push(ScanWarning {
                package: file.package.clone(),
                file: file.vendor_relative_path.clone(),
                message: "the file is not valid UTF-8; its symbols are recorded, but it can't be edited safely"
                    .to_string(),
            });
        }

        if let Some(analysis) = &analysis {
            record(&mut table, file, analysis, &presets);
        }

        table.files.push(ScannedFile {
            package: file.package.clone(),
            vendor_relative_path: file.vendor_relative_path.clone(),
            source_path: file.source_path.clone(),
            analysis,
            lossy,
        });
    }

    table
}

/// Adds the symbols a file declares to the table.
fn record(
    table: &mut SymbolTable,
    file: &PackageFile,
    analysis: &FileAnalysis,
    presets: &[(&str, &PresetSymbols)],
) {
    let built_ins = BuiltIns::get();

    for namespace in &analysis.namespaces {
        // The global namespace isn't a symbol: there is nothing to rename.
        if let Some(name) = &namespace.namespace {
            add(table, file, SymbolKind::Namespace, name, name.clone(), None);
        }

        for declared in &namespace.symbols {
            let kind = match declared.kind {
                analyser::SymbolKind::Class => SymbolKind::Class,
                analyser::SymbolKind::Interface => SymbolKind::Interface,
                analyser::SymbolKind::Trait => SymbolKind::Trait,
                analyser::SymbolKind::Enum => SymbolKind::Enum,
                analyser::SymbolKind::Function => SymbolKind::Function,
                analyser::SymbolKind::Constant => SymbolKind::Constant,
            };

            let is_php_built_in = match kind {
                kind if kind.is_class_like() => built_ins.is_class_like(&declared.name),
                SymbolKind::Function => built_ins.is_function(&declared.name),
                _ => false,
            };

            let provided_by = if is_php_built_in {
                Some("php")
            } else {
                presets
                    .iter()
                    .find(|(_, preset)| match kind {
                        kind if kind.is_class_like() => preset.is_class_like(&declared.name),
                        SymbolKind::Function => preset.is_function(&declared.name),
                        SymbolKind::Constant => preset.is_constant(&declared.name),
                        _ => false,
                    })
                    .map(|(name, _)| *name)
            };

            if let Some(provided_by) = provided_by {
                table.built_ins.push(BuiltInSymbol {
                    kind,
                    name: declared.name.clone(),
                    package: file.package.clone(),
                    file: file.vendor_relative_path.clone(),
                    provided_by: provided_by.to_string(),
                });

                continue;
            }

            let namespace_of_symbol = declared
                .name
                .rsplit_once('\\')
                .map(|(namespace, _)| namespace.to_string())
                .unwrap_or_default();

            add(
                table,
                file,
                kind,
                &declared.name,
                namespace_of_symbol,
                Some(declared),
            );
        }
    }
}

fn add(
    table: &mut SymbolTable,
    file: &PackageFile,
    kind: SymbolKind,
    name: &str,
    namespace: String,
    declared: Option<&analyser::DeclaredSymbol>,
) {
    let source = SymbolSource {
        package: file.package.clone(),
        file: file.vendor_relative_path.clone(),
    };

    let index = match table.index.get(&key(kind, name)) {
        Some(index) => *index,
        None => {
            table.symbols.push(DiscoveredSymbol {
                kind,
                name: name.to_string(),
                namespace,
                sources: Vec::new(),
                is_abstract: declared.is_some_and(|declared| declared.is_abstract),
                extends: declared
                    .map(|declared| declared.extends.clone())
                    .unwrap_or_default(),
                implements: declared
                    .map(|declared| declared.implements.clone())
                    .unwrap_or_default(),
                backing_type: declared.and_then(|declared| declared.backing_type.clone()),
            });

            let index = table.symbols.len() - 1;

            table.index.insert(key(kind, name), index);

            index
        }
    };

    let sources = &mut table.symbols[index].sources;

    // A file can declare a symbol more than once (conditionally, say).
    if !sources.contains(&source) {
        sources.push(source);
    }
}

#[cfg(test)]
mod tests;
