//! The symbols found in a project's packages.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::Serialize;

use crate::analyser::FileAnalysis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Namespace,
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Constant,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Namespace => "namespace",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Trait => "trait",
            SymbolKind::Enum => "enum",
            SymbolKind::Function => "function",
            SymbolKind::Constant => "constant",
        }
    }

    /// Classes, interfaces, traits and enums share one set of names in PHP.
    pub fn is_class_like(self) -> bool {
        matches!(
            self,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum
        )
    }
}

/// A file a symbol is declared in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolSource {
    pub package: String,

    /// The file's path as symbols and patterns see it:
    /// `league/flysystem/src/Client.php`.
    pub file: String,
}

/// A namespace, class, interface, trait, enum, function or constant that a
/// package declares.
///
/// The same symbol can be declared in several files, or packages (a fork, a
/// polyfill, a version of a library bundled by two packages); each
/// declaration is a source. The details are those of the first.
#[derive(Debug, Serialize)]
pub struct DiscoveredSymbol {
    pub kind: SymbolKind,

    /// The full name, without a leading backslash.
    pub name: String,

    /// The namespace it is in; empty for the global namespace. For a namespace
    /// it is the namespace itself.
    pub namespace: String,

    pub sources: Vec<SymbolSource>,

    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_abstract: bool,

    /// What a class extends, or an interface extends.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<String>,

    /// What a class or enum implements.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub implements: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub backing_type: Option<String>,
}

impl DiscoveredSymbol {
    /// The name without its namespace.
    pub fn local_name(&self) -> &str {
        match self.kind {
            SymbolKind::Namespace => &self.name,
            _ => self.name.rsplit('\\').next().unwrap_or(&self.name),
        }
    }

    pub fn is_global(&self) -> bool {
        self.namespace.is_empty()
    }

    /// The packages that declare it.
    pub fn packages(&self) -> Vec<&str> {
        let mut packages: Vec<&str> = Vec::new();

        for source in &self.sources {
            if !packages.contains(&source.package.as_str()) {
                packages.push(&source.package);
            }
        }

        packages
    }
}

/// A PHP file that was analysed.
#[derive(Debug)]
pub struct ScannedFile {
    pub package: String,

    /// `league/flysystem/src/Client.php`.
    pub vendor_relative_path: String,

    pub source_path: PathBuf,

    /// What the analyser found; `None` if the file couldn't be analysed, in
    /// which case there is a warning.
    pub analysis: Option<FileAnalysis>,

    /// The file isn't valid UTF-8. It was analysed with the invalid bytes
    /// replaced, so its symbols are known but its spans can't be used to edit
    /// it. There is a warning.
    pub lossy: bool,
}

/// A symbol left out because PHP itself, or a preset in `exclude_presets`,
/// provides it.
#[derive(Debug, Serialize)]
pub struct BuiltInSymbol {
    pub kind: SymbolKind,
    pub name: String,
    pub package: String,
    pub file: String,
    /// `php`, or the name of the preset: `wordpress`.
    pub provided_by: String,
}

/// A file that couldn't be analysed. Symbols in it are missing from the
/// table, so the package is only partly known.
#[derive(Debug, Serialize)]
pub struct ScanWarning {
    pub package: String,
    pub file: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    /// In the order they were found: files ordered by path.
    pub symbols: Vec<DiscoveredSymbol>,

    pub files: Vec<ScannedFile>,

    /// PHP's own symbols, and those of the `exclude_presets`, that packages
    /// declare too, which are left alone.
    pub built_ins: Vec<BuiltInSymbol>,

    pub warnings: Vec<ScanWarning>,

    /// Symbols by kind and lower-cased name.
    pub(super) index: HashMap<(SymbolKind, String), usize>,
}

impl SymbolTable {
    pub fn get(&self, kind: SymbolKind, name: &str) -> Option<&DiscoveredSymbol> {
        self.index
            .get(&key(kind, name))
            .map(|index| &self.symbols[*index])
    }

    /// The symbols a package declares.
    pub fn symbols_of<'a>(
        &'a self,
        package: &'a str,
    ) -> impl Iterator<Item = &'a DiscoveredSymbol> {
        self.symbols.iter().filter(move |symbol| {
            symbol
                .sources
                .iter()
                .any(|source| source.package == package)
        })
    }

    pub fn report(&self) -> SymbolReport<'_> {
        SymbolReport {
            packages: self.by_package(),
            built_ins: &self.built_ins,
            warnings: &self.warnings,
        }
    }

    /// The symbols, grouped by the package that declares them. A symbol
    /// several packages declare is in each.
    pub fn by_package(&self) -> BTreeMap<String, PackageSymbols> {
        let mut packages: BTreeMap<String, PackageSymbols> = BTreeMap::new();

        for file in &self.files {
            let package = packages.entry(file.package.clone()).or_default();

            package.php_files += 1;

            if file.analysis.is_none() {
                package.unanalysed_files += 1;
            }

            if file.lossy {
                package.lossy_files += 1;
            }
        }

        for symbol in &self.symbols {
            for package in symbol.packages() {
                let package = packages.entry(package.to_string()).or_default();

                let names = match symbol.kind {
                    SymbolKind::Namespace => &mut package.namespaces,
                    SymbolKind::Class => &mut package.classes,
                    SymbolKind::Interface => &mut package.interfaces,
                    SymbolKind::Trait => &mut package.traits,
                    SymbolKind::Enum => &mut package.enums,
                    SymbolKind::Function => &mut package.functions,
                    SymbolKind::Constant => &mut package.constants,
                };

                names.push(symbol.name.clone());
            }
        }

        packages
    }
}

/// What `strauss-rs symbols` prints.
#[derive(Debug, Serialize)]
pub struct SymbolReport<'a> {
    pub packages: BTreeMap<String, PackageSymbols>,

    /// PHP's own symbols, and those of the `exclude_presets`, that packages
    /// declare too, which are left alone.
    pub built_ins: &'a [BuiltInSymbol],

    pub warnings: &'a [ScanWarning],
}

/// The names of what one package declares.
#[derive(Debug, Default, Serialize)]
pub struct PackageSymbols {
    /// The `.php` files that were analysed, or tried.
    pub php_files: usize,

    /// Of those, the ones that couldn't be analysed.
    pub unanalysed_files: usize,

    /// Of those, the ones that aren't valid UTF-8: analysed, but not safe to
    /// edit.
    pub lossy_files: usize,

    pub namespaces: Vec<String>,
    pub classes: Vec<String>,
    pub interfaces: Vec<String>,
    pub traits: Vec<String>,
    pub enums: Vec<String>,
    pub functions: Vec<String>,
    pub constants: Vec<String>,
}

/// Names are looked up case-insensitively, as PHP resolves them, except
/// constants.
pub(super) fn key(kind: SymbolKind, name: &str) -> (SymbolKind, String) {
    let name = name.trim_start_matches('\\');

    match kind {
        SymbolKind::Constant => (kind, name.to_string()),
        _ => (kind, name.to_ascii_lowercase()),
    }
}
