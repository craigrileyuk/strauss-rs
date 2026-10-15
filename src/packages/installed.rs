//! Reading `vendor/composer/installed.json`, which lists every package
//! Composer installed and where.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::config::{Autoload, Ordered, StringOrList};
use crate::packages::error::PackagesError;

/// Composer 2 writes `{"packages": [...]}`; Composer 1 wrote the list itself.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InstalledFile {
    Composer2 { packages: Vec<InstalledPackage> },
    Composer1(Vec<InstalledPackage>),
}

/// The parts of a package's `composer.json` that `installed.json` repeats.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstalledPackage {
    pub name: String,

    pub version: Option<String>,

    #[serde(rename = "type")]
    pub package_type: Option<String>,

    /// Where the package is, relative to `vendor/composer`. Composer 1 didn't
    /// record it; the package was always in `vendor/<name>`.
    #[serde(rename = "install-path")]
    pub install_path: Option<String>,

    #[serde(default)]
    pub autoload: Autoload,

    #[serde(default)]
    pub require: Ordered<String>,

    #[serde(default)]
    pub provide: Ordered<String>,

    #[serde(default)]
    pub replace: Ordered<String>,

    #[serde(default)]
    pub suggest: Ordered<String>,

    pub license: Option<StringOrList>,
}

impl InstalledPackage {
    /// Every name the package can stand in for: what it `provide`s and what
    /// it `replace`s.
    pub fn provided_names(&self) -> impl Iterator<Item = &str> {
        self.provide
            .0
            .iter()
            .chain(self.replace.0.iter())
            .map(|(name, _)| name.as_str())
    }
}

/// The installed packages, looked up by name. Composer package names are
/// case-insensitive.
#[derive(Debug, Default)]
pub struct InstalledIndex {
    packages: Vec<InstalledPackage>,
    by_name: HashMap<String, usize>,
}

impl InstalledIndex {
    pub fn parse(json: &str) -> Result<Self, String> {
        let file: InstalledFile = serde_json::from_str(json).map_err(|error| error.to_string())?;

        let packages = match file {
            InstalledFile::Composer2 { packages } | InstalledFile::Composer1(packages) => packages,
        };

        Ok(Self::new(packages))
    }

    pub fn new(packages: Vec<InstalledPackage>) -> Self {
        let mut by_name = HashMap::new();

        for (index, package) in packages.iter().enumerate() {
            by_name.insert(package.name.to_ascii_lowercase(), index);
        }

        Self { packages, by_name }
    }

    /// Reads `<vendor>/composer/installed.json`.
    pub fn load(vendor_dir: &Path) -> Result<Self, PackagesError> {
        let path = vendor_dir.join("composer").join("installed.json");

        let json = match fs::read_to_string(&path) {
            Ok(json) => json,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(PackagesError::NotInstalled { path });
            }
            Err(source) => return Err(PackagesError::Read { path, source }),
        };

        Self::parse(&json).map_err(|message| PackagesError::Parse { path, message })
    }

    pub fn get(&self, name: &str) -> Option<&InstalledPackage> {
        self.by_name
            .get(&name.to_ascii_lowercase())
            .map(|index| &self.packages[*index])
    }

    /// Whether an installed package provides or replaces `name`, which makes
    /// `name` a virtual package such as `psr/log-implementation`.
    pub fn is_provided(&self, name: &str) -> bool {
        self.packages.iter().any(|package| {
            package
                .provided_names()
                .any(|provided| provided.eq_ignore_ascii_case(name))
        })
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }
}
