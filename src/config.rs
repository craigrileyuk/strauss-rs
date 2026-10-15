#![allow(dead_code)]
//! Strauss configuration, read from `strauss.json` in the project root.
//!
//! PHP Strauss reads the same options from `extra.strauss` in `composer.json`.
//! The option names, shapes, defaults and inference rules here follow
//! `StraussConfig.php` so that an existing `extra.strauss` object can be moved
//! into `strauss.json` unchanged. `composer.json` is still read, but only to
//! infer values that were not set (prefixes and the package list), as PHP
//! Strauss does.
//!
//! Where this deliberately differs from PHP Strauss:
//!
//! * `strauss.json` is the options object itself, with no `extra.strauss`
//!   wrapper.
//! * Unknown keys are an error, so a typo can't silently do nothing.
//! * Values are validated when loaded (prefixes must be usable PHP names,
//!   regular expressions must compile, `target_directory` must stay inside the
//!   project) rather than failing, or misbehaving, later.
//! * Mozart's legacy keys (`dep_directory`, `dep_namespace`,
//!   `exclude_packages`, …) and the `extra.mozart` fallback are not supported.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;

use crate::php_pattern::{PhpPattern, Replacement};

pub const CONFIG_FILE: &str = "strauss.json";
pub const COMPOSER_FILE: &str = "composer.json";

const DEFAULT_TARGET_DIRECTORY: &str = "vendor-prefixed";
const DEFAULT_VENDOR_DIRECTORY: &str = "vendor";

/// The resolved configuration: every option set, either by the user or by
/// the same defaults and inference as PHP Strauss.
#[derive(Debug)]
pub struct Config {
    /// The directory containing `strauss.json` and `composer.json`.
    pub project_dir: PathBuf,

    /// Composer's vendor directory, relative to the project. Read from
    /// `config.vendor-dir` in `composer.json`. Default `vendor`.
    pub vendor_directory: String,

    /// `target_directory`: where the prefixed files are written, relative to
    /// the project, without leading or trailing slashes. Default
    /// `vendor-prefixed`.
    pub target_directory: String,

    /// `namespace_prefix`: prepended to every namespace, without leading or
    /// trailing backslashes (`Acme\Plugin`). Inferred from `composer.json`.
    pub namespace_prefix: Option<String>,

    /// `classmap_prefix`: prepended to classes in the global namespace
    /// (`Acme_Plugin_`). Inferred from `composer.json`.
    pub classmap_prefix: Option<String>,

    /// `constant_prefix`: prepended to constants declared with `define()`.
    /// `None` leaves constants alone.
    pub constant_prefix: Option<String>,

    /// `functions_prefix`: prepended to global functions. Defaults to the
    /// lower-cased `classmap_prefix`; `false` or `""` disables it.
    pub functions_prefix: Option<String>,

    /// `packages`: the packages to process. Defaults to the `require` keys of
    /// `composer.json`, which includes platform requirements such as `php` and
    /// `ext-json`; those won't match an installed package.
    pub packages: Vec<String>,

    /// `update_call_sites`.
    pub update_call_sites: UpdateCallSites,

    /// `include_root_autoload`: include the project's own `autoload` section
    /// in the generated autoloader. Default `false`.
    pub include_root_autoload: bool,

    /// `optimize_autoloader`: force an optimised, classmap-authoritative
    /// autoloader. Default `true`.
    pub optimize_autoloader: bool,

    /// `parallel`. Default: as many workers as there are spare CPUs.
    pub parallel: Parallel,

    /// `override_autoload`: autoload settings that replace a package's own,
    /// keyed by package name.
    pub override_autoload: BTreeMap<String, Autoload>,

    /// `exclude_from_copy`: packages, namespaces and files that are not copied
    /// into `target_directory` at all.
    pub exclude_from_copy: Exclusions,

    /// `exclude_from_prefix`: symbols defined here are not renamed (though
    /// where they are used is still updated).
    pub exclude_from_prefix: Exclusions,

    /// `exclude_files_from_update`: files, packages and namespaces in which
    /// no changes are made.
    pub exclude_files_from_update: Exclusions,

    /// `exclude_constants`: like `exclude_from_prefix`, for constants only.
    pub exclude_constants: ConstantExclusions,

    /// `exclude_from_string_rename`: symbols that are renamed in code but left
    /// alone inside strings.
    pub exclude_from_string_rename: StringRenameExclusions,

    /// `namespace_replacement_patterns`, in priority order: the first pattern
    /// that changes a namespace wins.
    pub namespace_replacement_patterns: Vec<NamespaceReplacement>,

    /// `delete_vendor_packages`: delete the packages' vendor directories after
    /// processing. Default `false`.
    pub delete_vendor_packages: bool,

    /// `delete_vendor_files`: delete the copied files from the packages'
    /// vendor directories after processing. Default `false`.
    pub delete_vendor_files: bool,

    /// `exclude_git_files`: skip files that wouldn't be in the package's
    /// distributed archive (`.git`, `.gitignore`d and `export-ignore` files).
    /// Default `true`.
    pub exclude_git_files: bool,

    /// `include_modified_date`: put a date in the header written to modified
    /// files. Default `true`.
    pub include_modified_date: bool,

    /// `include_author`: put the author in the header written to modified
    /// files. Default `true`.
    pub include_author: bool,

    /// `classmap_output`: write `autoload-classmap.php` and `autoload.php`.
    /// Inferred: `false` when `target_directory` is in the project's
    /// `autoload`, `true` otherwise.
    pub classmap_output: bool,
}

/// `update_call_sites`: which project files to update where they use the
/// prefixed symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCallSites {
    /// `false` (the default): don't update the project's files.
    Disabled,
    /// `true`: the files and directories in the project's `autoload`.
    FromAutoload,
    /// An explicit list of files and directories, relative to the project.
    Paths(Vec<String>),
}

/// `parallel`: how many worker processes analyse files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parallel {
    /// `true` or `0`: as many workers as there are spare CPUs.
    Auto,
    /// `false` or `1`: do everything in one process.
    Disabled,
    /// `N`: at most `N` workers.
    Workers(usize),
}

/// The `packages`, `namespaces` and `file_patterns` lists shared by the
/// `exclude_*` options.
#[derive(Debug, Default)]
pub struct Exclusions {
    /// Package names, e.g. `league/flysystem`.
    pub packages: Vec<String>,
    /// Namespaces, matched exactly, without leading or trailing backslashes.
    pub namespaces: Vec<String>,
    /// Regular expressions matched against the path relative to the vendor
    /// directory.
    pub file_patterns: Vec<PhpPattern>,
}

#[derive(Debug, Default)]
pub struct ConstantExclusions {
    pub exclusions: Exclusions,
    /// Constant names that are never prefixed, e.g. `WP_PLUGIN_DIR`.
    pub constants: Vec<String>,
}

#[derive(Debug, Default)]
pub struct StringRenameExclusions {
    /// Global function names whose name is left unchanged inside strings,
    /// such as `$array['value']`.
    pub functions: Vec<String>,
}

#[derive(Debug)]
pub struct NamespaceReplacement {
    pub pattern: PhpPattern,
    pub replacement: Replacement,
}

/// An `autoload` object, as in `composer.json`.
#[derive(Debug, Default, Deserialize)]
pub struct Autoload {
    #[serde(rename = "psr-4", default)]
    pub psr4: Ordered<StringOrList>,
    #[serde(rename = "psr-0", default)]
    pub psr0: Ordered<StringOrList>,
    #[serde(default)]
    pub classmap: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

impl Autoload {
    /// Every path the autoload section refers to.
    fn paths(&self) -> impl Iterator<Item = &str> {
        let mapped = self
            .psr4
            .0
            .iter()
            .chain(self.psr0.0.iter())
            .flat_map(|(_, paths)| paths.iter());

        mapped
            .chain(self.classmap.iter().map(String::as_str))
            .chain(self.files.iter().map(String::as_str))
    }
}

/// A JSON string or an array of strings, as Composer allows for autoload
/// paths.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let (one, many) = match self {
            StringOrList::One(path) => (Some(path.as_str()), &[][..]),
            StringOrList::Many(paths) => (None, paths.as_slice()),
        };

        one.into_iter().chain(many.iter().map(String::as_str))
    }
}

/// A JSON object whose key order is kept. Order matters for
/// `namespace_replacement_patterns` (priority) and for Composer's `psr-4`
/// (Strauss infers the prefix from the first key).
#[derive(Debug)]
pub struct Ordered<V>(pub Vec<(String, V)>);

impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Ordered(Vec::new())
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Ordered<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor<V>(PhantomData<V>);

        impl<'de, V: Deserialize<'de>> Visitor<'de> for OrderedVisitor<V> {
            type Value = Ordered<V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();

                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }

                Ok(Ordered(entries))
            }
        }

        deserializer.deserialize_map(OrderedVisitor(PhantomData))
    }
}

#[derive(Debug)]
pub enum ConfigError {
    /// A config file exists but couldn't be read.
    Read { path: PathBuf, source: io::Error },
    /// A config file isn't valid JSON, or has the wrong shape.
    Parse { file: &'static str, message: String },
    /// An option has a value Strauss can't use.
    Invalid { key: String, message: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            ConfigError::Parse { file, message } => write!(formatter, "{file}: {message}"),
            ConfigError::Invalid { key, message } => {
                write!(formatter, "{CONFIG_FILE}: `{key}` {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn invalid(key: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        key: key.into(),
        message: message.into(),
    }
}

/* *********************************************************
 * The files as written
 ********************************************************* */

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    /// Allows `"$schema"` to be set for editor support.
    #[serde(rename = "$schema")]
    schema: Option<String>,

    target_directory: Option<String>,
    namespace_prefix: Option<String>,
    classmap_prefix: Option<String>,
    constant_prefix: Option<String>,
    #[serde(alias = "function_prefix")]
    functions_prefix: Option<Value>,
    packages: Option<Vec<String>>,
    update_call_sites: Option<Value>,
    include_root_autoload: Option<bool>,
    optimize_autoloader: Option<bool>,
    parallel: Option<Value>,
    override_autoload: Option<BTreeMap<String, Autoload>>,
    exclude_from_copy: Option<RawExclusions>,
    exclude_from_prefix: Option<RawExclusions>,
    exclude_files_from_update: Option<RawExclusions>,
    exclude_constants: Option<RawConstantExclusions>,
    exclude_from_string_rename: Option<RawStringRenameExclusions>,
    namespace_replacement_patterns: Option<Ordered<String>>,
    delete_vendor_packages: Option<bool>,
    delete_vendor_files: Option<bool>,
    exclude_git_files: Option<bool>,
    include_modified_date: Option<bool>,
    include_author: Option<bool>,
    classmap_output: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExclusions {
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    namespaces: Vec<String>,
    #[serde(default)]
    file_patterns: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConstantExclusions {
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    namespaces: Vec<String>,
    #[serde(default)]
    file_patterns: Vec<String>,
    #[serde(default)]
    constants: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStringRenameExclusions {
    #[serde(default)]
    functions: Vec<String>,
}

/// The parts of `composer.json` used to infer settings.
#[derive(Debug, Default, Deserialize)]
struct ComposerJson {
    name: Option<String>,
    #[serde(default)]
    autoload: Autoload,
    #[serde(default)]
    require: Ordered<Value>,
    #[serde(default)]
    config: ComposerConfig,
}

#[derive(Debug, Default, Deserialize)]
struct ComposerConfig {
    #[serde(rename = "vendor-dir")]
    vendor_dir: Option<String>,
}

/* *********************************************************
 * Loading
 ********************************************************* */

impl Config {
    /// Reads `strauss.json` and `composer.json` from `project_dir`. Neither
    /// has to exist: Strauss needs no configuration to run.
    pub fn load(project_dir: &Path) -> Result<Config, ConfigError> {
        let strauss = read_optional(&project_dir.join(CONFIG_FILE))?;
        let composer = read_optional(&project_dir.join(COMPOSER_FILE))?;

        Config::from_json(project_dir, strauss.as_deref(), composer.as_deref())
    }

    /// Builds the configuration from the contents of the two files.
    pub fn from_json(
        project_dir: &Path,
        strauss_json: Option<&str>,
        composer_json: Option<&str>,
    ) -> Result<Config, ConfigError> {
        if let Some(json) = strauss_json {
            check_shape(json)?;
        }

        let raw: RawConfig = match strauss_json {
            Some(json) => serde_json::from_str(json).map_err(|error| ConfigError::Parse {
                file: CONFIG_FILE,
                message: error.to_string(),
            })?,
            None => RawConfig::default(),
        };

        let composer: ComposerJson = match composer_json {
            Some(json) => serde_json::from_str(json).map_err(|error| ConfigError::Parse {
                file: COMPOSER_FILE,
                message: error.to_string(),
            })?,
            None => ComposerJson::default(),
        };

        let target_directory = match &raw.target_directory {
            Some(directory) => normalise_target_directory(directory)?,
            None => DEFAULT_TARGET_DIRECTORY.to_string(),
        };

        let explicit_namespace_prefix = raw
            .namespace_prefix
            .as_deref()
            .map(|prefix| validate_namespace_prefix("namespace_prefix", prefix))
            .transpose()?;

        let explicit_classmap_prefix = raw
            .classmap_prefix
            .as_deref()
            .map(|prefix| validate_name_prefix("classmap_prefix", prefix))
            .transpose()?;

        // As in PHP Strauss, the namespace prefix is inferred first (and may
        // use an explicit classmap prefix), then the classmap prefix (which
        // may use the namespace prefix).
        let namespace_prefix = explicit_namespace_prefix
            .or_else(|| infer_namespace_prefix(&composer, explicit_classmap_prefix.as_deref()));

        let classmap_prefix = explicit_classmap_prefix
            .or_else(|| infer_classmap_prefix(&composer, namespace_prefix.as_deref()));

        let constant_prefix = match raw.constant_prefix.as_deref() {
            None | Some("") => None,
            Some(prefix) => Some(validate_name_prefix("constant_prefix", prefix)?),
        };

        let functions_prefix = resolve_functions_prefix(raw.functions_prefix, &classmap_prefix)?;

        let packages = match raw.packages {
            Some(packages) if !packages.is_empty() => {
                for (index, package) in packages.iter().enumerate() {
                    require_non_empty(&format!("packages[{index}]"), package)?;
                }

                packages
            }
            _ => composer
                .require
                .0
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
        };

        let classmap_output = raw
            .classmap_output
            .unwrap_or_else(|| !autoloads_directory(&composer.autoload, &target_directory));

        let mut namespace_replacement_patterns = Vec::new();

        for (index, (pattern, replacement)) in raw
            .namespace_replacement_patterns
            .map(|patterns| patterns.0)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            require_non_empty(
                &format!("namespace_replacement_patterns[{index}]"),
                &pattern,
            )?;

            namespace_replacement_patterns.push(NamespaceReplacement {
                pattern: PhpPattern::literal_or_regex(&pattern)
                    .map_err(|message| invalid("namespace_replacement_patterns", message))?,
                replacement: Replacement::parse(&replacement),
            });
        }

        let exclude_constants = raw.exclude_constants.unwrap_or_default();

        for (index, constant) in exclude_constants.constants.iter().enumerate() {
            if !is_identifier(constant) {
                return Err(invalid(
                    format!("exclude_constants.constants[{index}]"),
                    format!("`{constant}` is not a valid constant name"),
                ));
            }
        }

        let exclude_from_string_rename = raw.exclude_from_string_rename.unwrap_or_default();

        for (index, function) in exclude_from_string_rename.functions.iter().enumerate() {
            if !is_identifier(function) {
                return Err(invalid(
                    format!("exclude_from_string_rename.functions[{index}]"),
                    format!("`{function}` is not a valid function name"),
                ));
            }
        }

        Ok(Config {
            project_dir: project_dir.to_path_buf(),
            vendor_directory: composer
                .config
                .vendor_dir
                .as_deref()
                .map(|directory| directory.trim_matches('/').to_string())
                .filter(|directory| !directory.is_empty())
                .unwrap_or_else(|| DEFAULT_VENDOR_DIRECTORY.to_string()),
            target_directory,
            namespace_prefix,
            classmap_prefix,
            constant_prefix,
            functions_prefix,
            packages,
            update_call_sites: resolve_update_call_sites(raw.update_call_sites)?,
            include_root_autoload: raw.include_root_autoload.unwrap_or(false),
            optimize_autoloader: raw.optimize_autoloader.unwrap_or(true),
            parallel: resolve_parallel(raw.parallel)?,
            override_autoload: raw.override_autoload.unwrap_or_default(),
            exclude_from_copy: resolve_exclusions(
                "exclude_from_copy",
                raw.exclude_from_copy.unwrap_or_default(),
            )?,
            exclude_from_prefix: resolve_exclusions(
                "exclude_from_prefix",
                raw.exclude_from_prefix.unwrap_or_default(),
            )?,
            exclude_files_from_update: resolve_exclusions(
                "exclude_files_from_update",
                raw.exclude_files_from_update.unwrap_or_default(),
            )?,
            exclude_constants: ConstantExclusions {
                exclusions: resolve_exclusions(
                    "exclude_constants",
                    RawExclusions {
                        packages: exclude_constants.packages,
                        namespaces: exclude_constants.namespaces,
                        file_patterns: exclude_constants.file_patterns,
                    },
                )?,
                constants: exclude_constants.constants,
            },
            exclude_from_string_rename: StringRenameExclusions {
                functions: exclude_from_string_rename.functions,
            },
            namespace_replacement_patterns,
            delete_vendor_packages: raw.delete_vendor_packages.unwrap_or(false),
            delete_vendor_files: raw.delete_vendor_files.unwrap_or(false),
            exclude_git_files: raw.exclude_git_files.unwrap_or(true),
            include_modified_date: raw.include_modified_date.unwrap_or(true),
            include_author: raw.include_author.unwrap_or(true),
            classmap_output,
        })
    }
}

/// Options whose value is a JSON object.
const OBJECT_OPTIONS: &[&str] = &[
    "exclude_from_copy",
    "exclude_from_prefix",
    "exclude_files_from_update",
    "exclude_constants",
    "exclude_from_string_rename",
    "namespace_replacement_patterns",
    "override_autoload",
];

/// Serde reads a JSON array as a struct's fields in order, so
/// `"exclude_from_copy": ["a/b"]` would be taken as `packages: "a/b"` and fail
/// with a confusing message (or, worse, `[["a/b"]]` would be accepted). Check
/// that objects are objects first. Syntax errors are left for the real parse,
/// which reports their line and column.
fn check_shape(json: &str) -> Result<(), ConfigError> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Ok(());
    };

    let Value::Object(options) = &value else {
        return Err(ConfigError::Parse {
            file: CONFIG_FILE,
            message: "must contain a JSON object".to_string(),
        });
    };

    for key in OBJECT_OPTIONS {
        match options.get(*key) {
            None | Some(Value::Null) | Some(Value::Object(_)) => {}
            Some(_) => return Err(invalid(*key, "must be an object")),
        }
    }

    if let Some(Value::Object(packages)) = options.get("override_autoload") {
        for (package, autoload) in packages {
            if !autoload.is_object() {
                return Err(invalid(
                    format!("override_autoload.{package}"),
                    "must be an object",
                ));
            }
        }
    }

    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<String>, ConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/* *********************************************************
 * Validation
 ********************************************************* */

fn require_non_empty(key: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(invalid(key, "must not be empty"));
    }

    Ok(())
}

/// A PHP identifier: letters, digits and underscores (and bytes above 0x7f),
/// not starting with a digit.
fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();

    match characters.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() || !first.is_ascii() => {}
        _ => return false,
    }

    characters.all(|character| {
        character == '_' || character.is_ascii_alphanumeric() || !character.is_ascii()
    })
}

/// `target_directory` has no leading or trailing slash and must stay inside
/// the project: it is written to, and with `delete_vendor_*` set, deleted
/// around. Parent (`..`) segments and drive letters are rejected.
fn normalise_target_directory(directory: &str) -> Result<String, ConfigError> {
    let trimmed = directory.trim_matches(|character| character == '/' || character == '\\');

    if trimmed.is_empty() {
        return Err(invalid("target_directory", "must not be empty"));
    }

    // A leading slash is allowed: as in Strauss, the directory is joined onto
    // the project path, so `/lib/prefixed` means `<project>/lib/prefixed`.
    let escapes_project = directory.as_bytes().get(1) == Some(&b':')
        || trimmed
            .split(['/', '\\'])
            .any(|segment| segment == ".." || segment == ".");

    if escapes_project {
        return Err(invalid(
            "target_directory",
            format!("`{directory}` must be a path inside the project"),
        ));
    }

    Ok(trimmed.replace('\\', "/"))
}

/// Backslashes are trimmed from both ends (Strauss does the same when it
/// reads the prefix), and each segment must be a valid PHP name.
fn validate_namespace_prefix(key: &str, prefix: &str) -> Result<String, ConfigError> {
    let trimmed = prefix.trim_matches('\\');

    if trimmed.is_empty() {
        return Err(invalid(key, "must not be empty"));
    }

    if let Some(segment) = trimmed.split('\\').find(|segment| !is_identifier(segment)) {
        return Err(invalid(
            key,
            format!("`{prefix}` is not a valid namespace: `{segment}` is not a valid name"),
        ));
    }

    Ok(trimmed.to_string())
}

/// Used for `classmap_prefix` and `constant_prefix`, which are glued onto the
/// front of a name (`Acme_Plugin_` + `Client`).
fn validate_name_prefix(key: &str, prefix: &str) -> Result<String, ConfigError> {
    if prefix.is_empty() {
        return Err(invalid(key, "must not be empty"));
    }

    if !is_identifier(prefix) {
        return Err(invalid(
            key,
            format!(
                "`{prefix}` can only contain letters, numbers and underscores, and can't start with a number"
            ),
        ));
    }

    Ok(prefix.to_string())
}

fn resolve_exclusions(key: &str, raw: RawExclusions) -> Result<Exclusions, ConfigError> {
    for (index, package) in raw.packages.iter().enumerate() {
        require_non_empty(&format!("{key}.packages[{index}]"), package)?;
    }

    let mut namespaces = Vec::new();

    for (index, namespace) in raw.namespaces.iter().enumerate() {
        // `Acme\Lib\` is how a PSR-4 key is written; namespaces are matched
        // without the surrounding backslashes.
        let trimmed = namespace.trim_matches('\\');

        require_non_empty(&format!("{key}.namespaces[{index}]"), trimmed)?;
        namespaces.push(trimmed.to_string());
    }

    let mut file_patterns = Vec::new();

    for (index, pattern) in raw.file_patterns.iter().enumerate() {
        require_non_empty(&format!("{key}.file_patterns[{index}]"), pattern)?;

        file_patterns.push(
            PhpPattern::regex(pattern)
                .map_err(|message| invalid(format!("{key}.file_patterns[{index}]"), message))?,
        );
    }

    Ok(Exclusions {
        packages: raw.packages,
        namespaces,
        file_patterns,
    })
}

fn resolve_update_call_sites(value: Option<Value>) -> Result<UpdateCallSites, ConfigError> {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => Ok(UpdateCallSites::Disabled),
        Some(Value::Bool(true)) => Ok(UpdateCallSites::FromAutoload),
        Some(Value::Array(items)) => {
            let mut paths = Vec::new();

            for (index, item) in items.into_iter().enumerate() {
                let key = format!("update_call_sites[{index}]");

                match item {
                    Value::String(path) => {
                        require_non_empty(&key, &path)?;
                        paths.push(path);
                    }
                    _ => return Err(invalid(key, "must be a string")),
                }
            }

            // An empty list disables updating, as in Strauss.
            if paths.is_empty() {
                Ok(UpdateCallSites::Disabled)
            } else {
                Ok(UpdateCallSites::Paths(paths))
            }
        }
        Some(_) => Err(invalid(
            "update_call_sites",
            "must be true, false, or an array of files and directories",
        )),
    }
}

fn resolve_parallel(value: Option<Value>) -> Result<Parallel, ConfigError> {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(true)) => Ok(Parallel::Auto),
        Some(Value::Bool(false)) => Ok(Parallel::Disabled),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(0) => Ok(Parallel::Auto),
            Some(1) => Ok(Parallel::Disabled),
            Some(workers) => Ok(Parallel::Workers(workers as usize)),
            None => Err(invalid(
                "parallel",
                "must be true, false, or a whole number",
            )),
        },
        Some(_) => Err(invalid(
            "parallel",
            "must be true, false, or a whole number",
        )),
    }
}

/// Unset (or `null`) defaults to the lower-cased classmap prefix. `false` or
/// `""` turns function prefixing off.
fn resolve_functions_prefix(
    value: Option<Value>,
    classmap_prefix: &Option<String>,
) -> Result<Option<String>, ConfigError> {
    match value {
        None | Some(Value::Null) => {
            Ok(classmap_prefix.as_ref().map(|prefix| prefix.to_lowercase()))
        }
        Some(Value::Bool(false)) => Ok(None),
        Some(Value::String(prefix)) if prefix.is_empty() => Ok(None),
        Some(Value::String(prefix)) => validate_name_prefix("functions_prefix", &prefix).map(Some),
        Some(_) => Err(invalid(
            "functions_prefix",
            "must be a string, or false to disable function prefixing",
        )),
    }
}

/* *********************************************************
 * Inference from composer.json
 ********************************************************* */

/// The prefix PHP Strauss uses when `namespace_prefix` isn't set: the
/// project's first `psr-4` namespace, else its first `psr-0` namespace, else
/// its package name (`acme/my-plugin` -> `Acme\My_Plugin`), else the
/// classmap prefix without its trailing underscore.
fn infer_namespace_prefix(
    composer: &ComposerJson,
    classmap_prefix: Option<&str>,
) -> Option<String> {
    if let Some(key) = first_autoload_namespace(composer) {
        return Some(key.trim_matches('\\').to_string());
    }

    if let Some(name) = package_name(composer) {
        let prefix = capitalise_words(&words_with_separators(name).replace('/', "\\"));

        return Some(prefix.trim_matches('\\').to_string());
    }

    classmap_prefix
        .map(|prefix| prefix.trim_end_matches('_').to_string())
        .filter(|prefix| !prefix.is_empty())
}

/// The prefix PHP Strauss uses when `classmap_prefix` isn't set: the
/// project's first `psr-4` or `psr-0` namespace with underscores for
/// backslashes (`Acme\Plugin\` -> `Acme_Plugin_`), else its package name
/// (`acme/my-plugin` -> `Acme_My_Plugin`), else the namespace prefix.
fn infer_classmap_prefix(
    composer: &ComposerJson,
    namespace_prefix: Option<&str>,
) -> Option<String> {
    if let Some(key) = first_autoload_namespace(composer) {
        return Some(key.replace('\\', "_"));
    }

    if let Some(name) = package_name(composer) {
        return Some(capitalise_words(
            &words_with_separators(name).replace('/', "_"),
        ));
    }

    namespace_prefix
        .map(|prefix| format!("{}_", words_with_separators(prefix).trim_end_matches('_')))
        .filter(|prefix| prefix != "_")
}

fn first_autoload_namespace(composer: &ComposerJson) -> Option<&str> {
    composer
        .autoload
        .psr4
        .0
        .iter()
        .chain(composer.autoload.psr0.0.iter())
        .map(|(namespace, _)| namespace.as_str())
        .find(|namespace| !namespace.trim_matches('\\').is_empty())
}

/// The root package's name. A `composer.json` without a name is
/// `__root__` to Composer, which Strauss ignores.
fn package_name(composer: &ComposerJson) -> Option<&str> {
    composer
        .name
        .as_deref()
        .filter(|name| !name.is_empty() && *name != "__root__")
}

/// `preg_replace('/[^\w\/]+/', '_', $name)`: every run of characters that
/// aren't word characters or `/` becomes a single underscore.
fn words_with_separators(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut in_separator_run = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '/' {
            output.push(character);
            in_separator_run = false;
        } else if !in_separator_run {
            output.push('_');
            in_separator_run = true;
        }
    }

    output
}

/// Uppercases a lowercase letter at the start, or after `_` or `\`
/// (`preg_replace_callback('/(?<=^|_|\\\\)[a-z]/', 'strtoupper', …)`).
fn capitalise_words(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut previous: Option<char> = None;

    for character in name.chars() {
        let at_word_start = matches!(previous, None | Some('_') | Some('\\'));

        if at_word_start && character.is_ascii_lowercase() {
            output.push(character.to_ascii_uppercase());
        } else {
            output.push(character);
        }

        previous = Some(character);
    }

    output
}

/// Whether any of the project's autoload paths is `directory`.
fn autoloads_directory(autoload: &Autoload, directory: &str) -> bool {
    autoload.paths().any(|path| {
        path.trim_matches(|character| character == '/' || character == '\\') == directory
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(strauss: &str, composer: &str) -> Config {
        Config::from_json(Path::new("/project"), Some(strauss), Some(composer))
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn strauss_only(strauss: &str) -> Config {
        Config::from_json(Path::new("/project"), Some(strauss), None)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn error(strauss: &str) -> String {
        Config::from_json(Path::new("/project"), Some(strauss), None)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn needs_no_files() {
        let config = Config::from_json(Path::new("/project"), None, None).unwrap();

        assert_eq!(config.target_directory, "vendor-prefixed");
        assert_eq!(config.vendor_directory, "vendor");
        assert_eq!(config.namespace_prefix, None);
        assert_eq!(config.classmap_prefix, None);
        assert_eq!(config.constant_prefix, None);
        assert_eq!(config.functions_prefix, None);
        assert!(config.packages.is_empty());
        assert_eq!(config.update_call_sites, UpdateCallSites::Disabled);
        assert!(!config.include_root_autoload);
        assert!(config.optimize_autoloader);
        assert_eq!(config.parallel, Parallel::Auto);
        assert!(config.override_autoload.is_empty());
        assert!(config.exclude_from_copy.packages.is_empty());
        assert!(config.namespace_replacement_patterns.is_empty());
        assert!(!config.delete_vendor_packages);
        assert!(!config.delete_vendor_files);
        assert!(config.exclude_git_files);
        assert!(config.include_modified_date);
        assert!(config.include_author);
        assert!(config.classmap_output);
    }

    #[test]
    fn empty_object_is_the_same_as_no_file() {
        let config = strauss_only("{}");

        assert_eq!(config.target_directory, "vendor-prefixed");
        assert!(config.optimize_autoloader);
    }

    #[test]
    fn reads_the_readme_example() {
        let config = strauss_only(
            r#"{
                "target_directory": "vendor-prefixed",
                "namespace_prefix": "BrianHenryIE\\My_Project\\",
                "classmap_prefix": "BrianHenryIE_My_Project_",
                "constant_prefix": "BHMP_",
                "packages": ["league/flysystem"],
                "update_call_sites": false,
                "include_root_autoload": false,
                "optimize_autoloader": true,
                "parallel": true,
                "override_autoload": {},
                "exclude_from_copy": {"packages": [], "namespaces": [], "file_patterns": []},
                "exclude_from_prefix": {"packages": [], "namespaces": [], "file_patterns": []},
                "exclude_constants": {"packages": [], "namespaces": [], "file_patterns": [], "constants": []},
                "exclude_from_string_rename": {"functions": []},
                "namespace_replacement_patterns": {},
                "delete_vendor_packages": false,
                "delete_vendor_files": false,
                "exclude_git_files": true
            }"#,
        );

        assert_eq!(
            config.namespace_prefix.as_deref(),
            Some("BrianHenryIE\\My_Project")
        );
        assert_eq!(
            config.classmap_prefix.as_deref(),
            Some("BrianHenryIE_My_Project_")
        );
        assert_eq!(config.constant_prefix.as_deref(), Some("BHMP_"));
        assert_eq!(config.packages, ["league/flysystem"]);
    }

    #[test]
    fn accepts_schema_key() {
        strauss_only(r#"{"$schema": "https://example.com/strauss.schema.json"}"#);
    }

    #[test]
    fn rejects_unknown_keys() {
        let message = error(r#"{"target_dir": "x"}"#);

        assert!(message.starts_with("strauss.json:"), "{message}");
        assert!(message.contains("unknown field `target_dir`"), "{message}");
    }

    #[test]
    fn rejects_unknown_keys_in_nested_objects() {
        let message = error(r#"{"exclude_from_prefix": {"package": ["a/b"]}}"#);

        assert!(message.contains("unknown field `package`"), "{message}");
    }

    #[test]
    fn rejects_mozart_keys() {
        assert!(error(r#"{"dep_directory": "x"}"#).contains("unknown field `dep_directory`"));
        assert!(error(r#"{"exclude_packages": []}"#).contains("unknown field `exclude_packages`"));
    }

    #[test]
    fn reports_json_syntax_errors_with_the_file_name() {
        let message = error("{");

        assert!(message.starts_with("strauss.json:"), "{message}");
    }

    #[test]
    fn reports_composer_json_errors_with_the_file_name() {
        let message = Config::from_json(Path::new("/p"), None, Some("{"))
            .unwrap_err()
            .to_string();

        assert!(message.starts_with("composer.json:"), "{message}");
    }

    /* ---- target_directory ---- */

    #[test]
    fn trims_slashes_from_target_directory() {
        assert_eq!(
            strauss_only(r#"{"target_directory": "/lib/prefixed/"}"#).target_directory,
            "lib/prefixed"
        );
    }

    #[test]
    fn rejects_target_directory_outside_the_project() {
        for directory in ["", "/", "../out", "a/../../out", "C:\\out", "."] {
            let json = format!(
                r#"{{"target_directory": {}}}"#,
                serde_json::to_string(directory).unwrap()
            );
            let message = error(&json);

            assert!(
                message.contains("`target_directory`"),
                "{directory:?}: {message}"
            );
        }
    }

    /* ---- prefixes ---- */

    #[test]
    fn rejects_invalid_prefixes() {
        assert!(error(r#"{"namespace_prefix": ""}"#).contains("must not be empty"));
        assert!(error(r#"{"namespace_prefix": "\\"}"#).contains("must not be empty"));
        assert!(
            error(r#"{"namespace_prefix": "My-Plugin\\Lib"}"#)
                .contains("`My-Plugin` is not a valid name")
        );
        assert!(error(r#"{"namespace_prefix": "A\\\\B"}"#).contains("is not a valid name"));
        assert!(error(r#"{"classmap_prefix": ""}"#).contains("must not be empty"));
        assert!(error(r#"{"classmap_prefix": "My Plugin_"}"#).contains("can only contain letters"));
        assert!(error(r#"{"classmap_prefix": "1st_"}"#).contains("can't start with a number"));
        assert!(error(r#"{"constant_prefix": "A-B_"}"#).contains("can only contain letters"));
    }

    #[test]
    fn empty_constant_prefix_leaves_constants_alone() {
        assert_eq!(
            strauss_only(r#"{"constant_prefix": ""}"#).constant_prefix,
            None
        );
    }

    #[test]
    fn trims_backslashes_from_namespace_prefix() {
        assert_eq!(
            strauss_only(r#"{"namespace_prefix": "\\Acme\\Plugin\\"}"#)
                .namespace_prefix
                .as_deref(),
            Some("Acme\\Plugin")
        );
    }

    /* ---- inference ---- */

    #[test]
    fn infers_prefixes_from_the_first_psr4_namespace() {
        let config = config(
            "{}",
            r#"{
                "name": "acme/my-plugin",
                "autoload": {"psr-4": {"Acme\\Plugin\\": "src/", "Acme\\Other\\": "other/"}}
            }"#,
        );

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme\\Plugin"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_Plugin_"));
    }

    #[test]
    fn psr4_order_is_the_order_in_the_file_not_alphabetical() {
        let config = config(
            "{}",
            r#"{"autoload": {"psr-4": {"Zebra\\": "z/", "Apple\\": "a/"}}}"#,
        );

        assert_eq!(config.namespace_prefix.as_deref(), Some("Zebra"));
    }

    #[test]
    fn infers_prefixes_from_psr0_when_there_is_no_psr4() {
        let config = config("{}", r#"{"autoload": {"psr-0": {"Acme_Plugin_": "src/"}}}"#);

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme_Plugin_"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_Plugin_"));
    }

    #[test]
    fn skips_an_empty_psr4_namespace() {
        let config = config(
            "{}",
            r#"{"autoload": {"psr-4": {"": "src/", "Acme\\": "lib/"}}}"#,
        );

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme"));
    }

    #[test]
    fn infers_prefixes_from_the_package_name() {
        let config = config("{}", r#"{"name": "acme/my-plugin"}"#);

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme\\My_Plugin"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_My_Plugin"));
    }

    #[test]
    fn ignores_the_root_package_name() {
        let config = config("{}", r#"{"name": "__root__"}"#);

        assert_eq!(config.namespace_prefix, None);
        assert_eq!(config.classmap_prefix, None);
    }

    #[test]
    fn infers_namespace_prefix_from_an_explicit_classmap_prefix() {
        let config = strauss_only(r#"{"classmap_prefix": "Acme_Plugin_"}"#);

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme_Plugin"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_Plugin_"));
    }

    #[test]
    fn infers_classmap_prefix_from_an_explicit_namespace_prefix() {
        let config = strauss_only(r#"{"namespace_prefix": "Acme\\Plugin"}"#);

        assert_eq!(config.namespace_prefix.as_deref(), Some("Acme\\Plugin"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_Plugin_"));
    }

    #[test]
    fn explicit_prefixes_win_over_inference() {
        let config = config(
            r#"{"namespace_prefix": "Mine", "classmap_prefix": "Mine_"}"#,
            r#"{"autoload": {"psr-4": {"Acme\\": "src/"}}}"#,
        );

        assert_eq!(config.namespace_prefix.as_deref(), Some("Mine"));
        assert_eq!(config.classmap_prefix.as_deref(), Some("Mine_"));
    }

    #[test]
    fn packages_default_to_the_composer_require_keys_in_order() {
        let config = config(
            "{}",
            r#"{"require": {"php": ">=8.0", "zebra/z": "^1", "apple/a": "^2"}}"#,
        );

        assert_eq!(config.packages, ["php", "zebra/z", "apple/a"]);
    }

    #[test]
    fn explicit_packages_win_over_require() {
        let config = config(
            r#"{"packages": ["league/flysystem"]}"#,
            r#"{"require": {"other/pkg": "^1"}}"#,
        );

        assert_eq!(config.packages, ["league/flysystem"]);
    }

    #[test]
    fn empty_packages_fall_back_to_require_as_in_strauss() {
        let config = config(r#"{"packages": []}"#, r#"{"require": {"other/pkg": "^1"}}"#);

        assert_eq!(config.packages, ["other/pkg"]);
    }

    #[test]
    fn reads_vendor_dir_from_composer_config() {
        let config = config("{}", r#"{"config": {"vendor-dir": "lib/vendor/"}}"#);

        assert_eq!(config.vendor_directory, "lib/vendor");
    }

    #[test]
    fn classmap_output_is_off_when_the_target_directory_is_autoloaded() {
        let classmap = config("{}", r#"{"autoload": {"classmap": ["vendor-prefixed/"]}}"#);
        let psr4 = config(
            r#"{"target_directory": "lib/prefixed"}"#,
            r#"{"autoload": {"psr-4": {"Acme\\": ["src/", "lib/prefixed"]}}}"#,
        );
        let unrelated = config("{}", r#"{"autoload": {"classmap": ["src/"]}}"#);

        assert!(!classmap.classmap_output);
        assert!(!psr4.classmap_output);
        assert!(unrelated.classmap_output);
    }

    #[test]
    fn explicit_classmap_output_wins() {
        let config = config(
            r#"{"classmap_output": true}"#,
            r#"{"autoload": {"classmap": ["vendor-prefixed/"]}}"#,
        );

        assert!(config.classmap_output);
    }

    /* ---- functions_prefix ---- */

    #[test]
    fn functions_prefix_defaults_to_the_lowercase_classmap_prefix() {
        let config = strauss_only(r#"{"classmap_prefix": "Acme_Plugin_"}"#);

        assert_eq!(config.functions_prefix.as_deref(), Some("acme_plugin_"));
    }

    #[test]
    fn functions_prefix_can_be_set_or_disabled() {
        let set = strauss_only(r#"{"classmap_prefix": "Acme_", "functions_prefix": "acme_fn_"}"#);
        let alias = strauss_only(r#"{"function_prefix": "acme_fn_"}"#);
        let disabled_false =
            strauss_only(r#"{"classmap_prefix": "Acme_", "functions_prefix": false}"#);
        let disabled_empty =
            strauss_only(r#"{"classmap_prefix": "Acme_", "functions_prefix": ""}"#);
        let null = strauss_only(r#"{"classmap_prefix": "Acme_", "functions_prefix": null}"#);

        assert_eq!(set.functions_prefix.as_deref(), Some("acme_fn_"));
        assert_eq!(alias.functions_prefix.as_deref(), Some("acme_fn_"));
        assert_eq!(disabled_false.functions_prefix, None);
        assert_eq!(disabled_empty.functions_prefix, None);
        // As in Strauss, `null` is the same as leaving it out.
        assert_eq!(null.functions_prefix.as_deref(), Some("acme_"));
    }

    #[test]
    fn rejects_invalid_functions_prefix() {
        assert!(error(r#"{"functions_prefix": true}"#).contains("or false to disable"));
        assert!(error(r#"{"functions_prefix": 3}"#).contains("or false to disable"));
        assert!(error(r#"{"functions_prefix": "a-b"}"#).contains("can only contain letters"));
    }

    /* ---- update_call_sites / parallel ---- */

    #[test]
    fn reads_update_call_sites() {
        assert_eq!(
            strauss_only(r#"{"update_call_sites": false}"#).update_call_sites,
            UpdateCallSites::Disabled
        );
        assert_eq!(
            strauss_only(r#"{"update_call_sites": true}"#).update_call_sites,
            UpdateCallSites::FromAutoload
        );
        assert_eq!(
            strauss_only(r#"{"update_call_sites": []}"#).update_call_sites,
            UpdateCallSites::Disabled
        );
        assert_eq!(
            strauss_only(r#"{"update_call_sites": ["includes", "templates"]}"#).update_call_sites,
            UpdateCallSites::Paths(vec!["includes".into(), "templates".into()])
        );
    }

    #[test]
    fn rejects_invalid_update_call_sites() {
        assert!(error(r#"{"update_call_sites": "src"}"#).contains("true, false, or an array"));
        assert!(
            error(r#"{"update_call_sites": [1]}"#)
                .contains("`update_call_sites[0]` must be a string")
        );
        assert!(error(r#"{"update_call_sites": [""]}"#).contains("must not be empty"));
    }

    #[test]
    fn reads_parallel() {
        assert_eq!(
            strauss_only(r#"{"parallel": true}"#).parallel,
            Parallel::Auto
        );
        assert_eq!(strauss_only(r#"{"parallel": 0}"#).parallel, Parallel::Auto);
        assert_eq!(
            strauss_only(r#"{"parallel": false}"#).parallel,
            Parallel::Disabled
        );
        assert_eq!(
            strauss_only(r#"{"parallel": 1}"#).parallel,
            Parallel::Disabled
        );
        assert_eq!(
            strauss_only(r#"{"parallel": 4}"#).parallel,
            Parallel::Workers(4)
        );
    }

    #[test]
    fn rejects_invalid_parallel() {
        assert!(error(r#"{"parallel": -1}"#).contains("true, false, or a whole number"));
        assert!(error(r#"{"parallel": 1.5}"#).contains("true, false, or a whole number"));
        assert!(error(r#"{"parallel": "4"}"#).contains("true, false, or a whole number"));
    }

    /* ---- exclusions ---- */

    #[test]
    fn reads_exclusions() {
        let config = strauss_only(
            r#"{
                "exclude_from_copy": {"packages": ["a/b"], "namespaces": ["Acme\\Lib"], "file_patterns": ["~/tests/~"]},
                "exclude_from_prefix": {"packages": ["c/d"]},
                "exclude_files_from_update": {"namespaces": ["Acme\\Other"]},
                "exclude_constants": {"packages": ["e/f"], "constants": ["WP_PLUGIN_DIR", "ABSPATH"]},
                "exclude_from_string_rename": {"functions": ["value", "when"]}
            }"#,
        );

        assert_eq!(config.exclude_from_copy.packages, ["a/b"]);
        assert_eq!(config.exclude_from_copy.namespaces, ["Acme\\Lib"]);
        assert!(config.exclude_from_copy.file_patterns[0].is_match("vendor/a/b/tests/Foo.php"));
        assert_eq!(config.exclude_from_prefix.packages, ["c/d"]);
        assert_eq!(config.exclude_files_from_update.namespaces, ["Acme\\Other"]);
        assert_eq!(config.exclude_constants.exclusions.packages, ["e/f"]);
        assert_eq!(
            config.exclude_constants.constants,
            ["WP_PLUGIN_DIR", "ABSPATH"]
        );
        assert_eq!(
            config.exclude_from_string_rename.functions,
            ["value", "when"]
        );
    }

    #[test]
    fn trims_backslashes_from_excluded_namespaces() {
        let config = strauss_only(
            r#"{"exclude_from_prefix": {"namespaces": ["\\Acme\\Lib\\", "Acme\\Other"]}}"#,
        );

        assert_eq!(
            config.exclude_from_prefix.namespaces,
            ["Acme\\Lib", "Acme\\Other"]
        );
    }

    #[test]
    fn rejects_invalid_exclusions() {
        assert!(
            error(r#"{"exclude_from_copy": {"packages": [""]}}"#)
                .contains("`exclude_from_copy.packages[0]` must not be empty")
        );
        assert!(
            error(r#"{"exclude_from_copy": {"namespaces": ["\\"]}}"#)
                .contains("`exclude_from_copy.namespaces[0]` must not be empty")
        );
        assert!(
            error(r#"{"exclude_from_prefix": {"file_patterns": ["~(~"]}}"#)
                .contains("`exclude_from_prefix.file_patterns[0]`")
        );
        assert!(
            error(r#"{"exclude_constants": {"constants": ["NOT-VALID"]}}"#)
                .contains("`NOT-VALID` is not a valid constant name")
        );
        assert!(
            error(r#"{"exclude_from_string_rename": {"functions": ["a b"]}}"#)
                .contains("`a b` is not a valid function name")
        );
        assert!(
            error(r#"{"exclude_from_copy": ["a/b"]}"#)
                .contains("`exclude_from_copy` must be an object")
        );
    }

    /* ---- namespace_replacement_patterns ---- */

    #[test]
    fn keeps_namespace_replacement_patterns_in_file_order() {
        let config = strauss_only(
            r#"{"namespace_replacement_patterns": {
                "Zebra": "First",
                "~^Apple\\\\(.*)~": "Second\\\\$1",
                "Middle": "Third"
            }}"#,
        );

        let patterns: Vec<&str> = config
            .namespace_replacement_patterns
            .iter()
            .map(|entry| entry.pattern.source())
            .collect();

        assert_eq!(patterns, ["Zebra", "~^Apple\\\\(.*)~", "Middle"]);
    }

    #[test]
    fn namespace_replacement_patterns_can_be_applied() {
        let config = strauss_only(
            r#"{"namespace_replacement_patterns": {
                "~^BrianHenryIE\\\\(.*)~": "BrianHenryIE\\\\WC_Cash_App_Gateway\\\\$1"
            }}"#,
        );

        let entry = &config.namespace_replacement_patterns[0];

        assert_eq!(
            entry
                .pattern
                .replace_all("BrianHenryIE\\Lib", &entry.replacement)
                .unwrap(),
            "BrianHenryIE\\WC_Cash_App_Gateway\\Lib"
        );
    }

    #[test]
    fn rejects_invalid_namespace_replacement_patterns() {
        assert!(
            error(r#"{"namespace_replacement_patterns": {"~(~": "x"}}"#)
                .contains("`namespace_replacement_patterns`")
        );
        assert!(
            error(r#"{"namespace_replacement_patterns": {"": "x"}}"#).contains("must not be empty")
        );
        assert!(error(r#"{"namespace_replacement_patterns": {"A": 1}}"#).contains("invalid type"));
    }

    /* ---- override_autoload ---- */

    #[test]
    fn reads_override_autoload() {
        let config = strauss_only(
            r#"{"override_autoload": {
                "acme/lib": {"psr-4": {"Acme\\Lib\\": ["src/", "lib/"]}, "classmap": ["extra/"], "files": ["f.php"]}
            }}"#,
        );

        let autoload = &config.override_autoload["acme/lib"];

        assert_eq!(autoload.psr4.0[0].0, "Acme\\Lib\\");
        assert_eq!(
            autoload.psr4.0[0].1.iter().collect::<Vec<_>>(),
            ["src/", "lib/"]
        );
        assert_eq!(autoload.classmap, ["extra/"]);
        assert_eq!(autoload.files, ["f.php"]);
    }

    #[test]
    fn rejects_arrays_where_objects_are_expected() {
        for key in OBJECT_OPTIONS {
            for value in ["[]", r#"["a/b"]"#, r#"[["a/b"]]"#, r#""text""#, "1", "true"] {
                let message = error(&format!(r#"{{"{key}": {value}}}"#));

                assert!(
                    message.contains(&format!("`{key}` must be an object")),
                    "{key}: {value}: {message}"
                );
            }
        }

        assert!(
            error(r#"{"override_autoload": {"acme/lib": []}}"#)
                .contains("`override_autoload.acme/lib` must be an object")
        );
    }

    #[test]
    fn null_object_options_are_the_same_as_absent() {
        let config = strauss_only(r#"{"exclude_from_copy": null, "override_autoload": null}"#);

        assert!(config.exclude_from_copy.packages.is_empty());
        assert!(config.override_autoload.is_empty());
    }

    #[test]
    fn rejects_a_top_level_array() {
        assert_eq!(error("[]"), "strauss.json: must contain a JSON object");
        assert_eq!(
            error(r#""text""#),
            "strauss.json: must contain a JSON object"
        );
    }

    /* ---- booleans ---- */

    #[test]
    fn reads_boolean_options() {
        let config = strauss_only(
            r#"{
                "include_root_autoload": true,
                "optimize_autoloader": false,
                "delete_vendor_packages": true,
                "delete_vendor_files": true,
                "exclude_git_files": false,
                "include_modified_date": false,
                "include_author": false
            }"#,
        );

        assert!(config.include_root_autoload);
        assert!(!config.optimize_autoloader);
        assert!(config.delete_vendor_packages);
        assert!(config.delete_vendor_files);
        assert!(!config.exclude_git_files);
        assert!(!config.include_modified_date);
        assert!(!config.include_author);
    }

    #[test]
    fn rejects_non_boolean_booleans() {
        assert!(error(r#"{"delete_vendor_packages": "yes"}"#).contains("invalid type: string"));
        assert!(error(r#"{"optimize_autoloader": 1}"#).contains("invalid type"));
    }

    /* ---- files ---- */

    #[test]
    fn loads_a_project_directory() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/config-fixture");
        let config = Config::load(&directory).unwrap();

        assert_eq!(config.project_dir, directory);
        assert_eq!(config.target_directory, "lib/prefixed");
        assert_eq!(
            config.namespace_prefix.as_deref(),
            Some("Acme\\Plugin\\Vendor")
        );
        assert_eq!(config.classmap_prefix.as_deref(), Some("Acme_Plugin_"));
        assert_eq!(config.constant_prefix.as_deref(), Some("ACME_"));
        assert_eq!(config.packages, ["league/flysystem", "psr/log"]);
        assert_eq!(config.exclude_from_prefix.namespaces, ["Psr\\Log"]);
    }

    #[test]
    fn a_directory_without_either_file_uses_defaults() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/no-such-project");
        let config = Config::load(&directory).unwrap();

        assert_eq!(config.target_directory, "vendor-prefixed");
        assert_eq!(config.namespace_prefix, None);
    }

    /* ---- strauss.schema.json ---- */

    fn schema() -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("strauss.schema.json");
        let json = fs::read_to_string(path).unwrap();

        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn schema_lists_exactly_the_keys_the_loader_accepts() {
        // serde names every accepted key in its unknown-field error, so the
        // list can't drift from `RawConfig`.
        let message = error(r#"{"not_an_option": 1}"#);
        let (_, listed) = message.split_once("expected one of ").unwrap();
        let listed = listed.split(" at line").next().unwrap();

        let mut accepted: Vec<&str> = listed
            .split(", ")
            .map(|key| key.trim_matches('`'))
            .collect();
        accepted.sort_unstable();

        let schema = schema();
        let mut documented: Vec<&str> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        documented.sort_unstable();

        assert_eq!(documented, accepted);
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
    }

    #[test]
    fn schema_defaults_match_the_loader() {
        let schema = schema();
        let config = Config::from_json(Path::new("/project"), None, None).unwrap();
        let default = |key: &str| schema["properties"][key]["default"].clone();

        assert_eq!(
            default("target_directory"),
            Value::from(config.target_directory)
        );
        assert_eq!(
            default("include_root_autoload"),
            Value::from(config.include_root_autoload)
        );
        assert_eq!(
            default("optimize_autoloader"),
            Value::from(config.optimize_autoloader)
        );
        assert_eq!(
            default("delete_vendor_packages"),
            Value::from(config.delete_vendor_packages)
        );
        assert_eq!(
            default("delete_vendor_files"),
            Value::from(config.delete_vendor_files)
        );
        assert_eq!(
            default("exclude_git_files"),
            Value::from(config.exclude_git_files)
        );
        assert_eq!(
            default("include_modified_date"),
            Value::from(config.include_modified_date)
        );
        assert_eq!(
            default("include_author"),
            Value::from(config.include_author)
        );
        assert_eq!(default("update_call_sites"), Value::Bool(false));
        assert_eq!(default("parallel"), Value::Bool(true));
    }

    #[test]
    fn the_fixture_points_at_the_schema() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/config-fixture");
        let json: Value =
            serde_json::from_str(&fs::read_to_string(fixture.join(CONFIG_FILE)).unwrap()).unwrap();
        let reference = json["$schema"].as_str().unwrap();

        assert!(
            fixture.join(reference).is_file(),
            "{reference} doesn't exist"
        );
    }
}
