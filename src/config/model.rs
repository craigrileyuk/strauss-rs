//! The resolved configuration.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::composer::Autoload;
use crate::php_pattern::{PhpPattern, Replacement};

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
