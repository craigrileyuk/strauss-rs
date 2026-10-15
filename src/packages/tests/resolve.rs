use super::*;
use crate::config::{Autoload, Config};

fn enumerate(config: &Config) -> PackageSet {
    enumerate_packages(config).unwrap()
}

fn package<'a>(set: &'a PackageSet, name: &str) -> &'a Package {
    set.get(name)
        .unwrap_or_else(|| panic!("{name} not found in {:?}", package_names(set)))
}

#[test]
fn visits_packages_depth_first_in_the_order_strauss_does() {
    let set = enumerate(&fixture_config());

    // Each root, then what it requires: `acme/lib` pulls in `acme/dep`, which
    // pulls in `acme/fallback`, before the next root `acme/logger`.
    assert_eq!(
        package_names(&set),
        [
            "acme/lib",
            "Acme/Dep",
            "acme/fallback",
            "acme/logger",
            "acme/meta",
            "acme/plugin-pkg",
            "acme/missing-dir"
        ]
    );
}

#[test]
fn leaves_out_installed_packages_nothing_requires() {
    let set = enumerate(&fixture_config());

    assert!(set.get("acme/unused").is_none());
}

#[test]
fn a_dependency_cycle_terminates() {
    let set = enumerate(&fixture_config());

    // acme/lib -> acme/dep -> acme/lib
    assert_eq!(package(&set, "acme/lib").dependencies[0], "Acme/Dep");
    assert_eq!(package(&set, "Acme/Dep").dependencies[0], "acme/lib");
    assert_eq!(
        set.packages
            .iter()
            .filter(|package| package.name.eq_ignore_ascii_case("acme/lib"))
            .count(),
        1
    );
}

#[test]
fn platform_requirements_are_neither_packages_nor_reported() {
    let set = enumerate(&fixture_config());

    for package in &set.packages {
        for requirement in &package.requires {
            assert!(!is_platform_package(requirement), "{requirement}");
        }
    }

    // `php`, `ext-json` and `lib-curl` are required in the fixture.
    assert!(
        set.skipped
            .iter()
            .all(|skipped| !is_platform_package(&skipped.name))
    );
    assert!(set.get("php").is_none());
}

#[test]
fn reports_a_required_package_that_is_not_installed() {
    let set = enumerate(&fixture_config());

    assert_eq!(set.skipped.len(), 1);
    assert_eq!(set.skipped[0].name, "missing/pkg");
    assert_eq!(set.skipped[0].reason, SkipReason::NotInstalled);
}

#[test]
fn virtual_packages_are_not_reported_as_missing() {
    let set = enumerate(&fixture_config());

    let skipped: Vec<&str> = set
        .skipped
        .iter()
        .map(|skipped| skipped.name.as_str())
        .collect();

    // Provided by `acme/logger`.
    assert!(!skipped.contains(&"psr/log-implementation"));
    // Strauss' fixed list of virtual packages.
    assert!(!skipped.contains(&"php-http/client-implementation"));
}

#[test]
fn a_virtual_package_is_met_by_what_provides_it() {
    let set = enumerate(&fixture_config());

    // `acme/lib` requires `psr/log-implementation`; `acme/logger` provides it.
    assert_eq!(
        package(&set, "acme/lib").dependencies,
        ["Acme/Dep", "acme/logger"]
    );
    assert_eq!(
        package(&set, "acme/logger").provides,
        ["psr/log-implementation", "acme/old-logger"]
    );
}

#[test]
fn a_provider_is_only_linked_when_it_is_in_the_enumeration() {
    let mut config = fixture_config();
    config.packages = vec!["acme/lib".to_string()];

    let set = enumerate(&config);

    assert_eq!(
        package_names(&set),
        ["acme/lib", "Acme/Dep", "acme/fallback"]
    );
    assert_eq!(package(&set, "acme/lib").dependencies, ["Acme/Dep"]);
}

#[test]
fn a_metapackage_has_requirements_but_no_files() {
    let set = enumerate(&fixture_config());
    let meta = package(&set, "acme/meta");

    assert_eq!(meta.path, None);
    assert_eq!(meta.requires, ["acme/dep"]);
    assert_eq!(meta.dependencies, ["Acme/Dep"]);
    // No directory is expected, so there is nothing to warn about.
    assert!(
        set.warnings
            .iter()
            .all(|warning| warning.package != "acme/meta")
    );
}

#[test]
fn uses_the_install_path_rather_than_assuming_vendor() {
    let set = enumerate(&fixture_config());

    assert_eq!(
        package(&set, "acme/plugin-pkg").path,
        Some(fixture_dir().join("web/plugins/plugin-pkg"))
    );
    assert_eq!(
        package(&set, "acme/lib").path,
        Some(fixture_dir().join("vendor/acme/lib"))
    );
}

#[test]
fn warns_about_a_package_whose_directory_is_missing() {
    let set = enumerate(&fixture_config());
    let missing = package(&set, "acme/missing-dir");

    assert_eq!(missing.path, None);
    assert_eq!(set.warnings.len(), 1);
    assert_eq!(set.warnings[0].package, "acme/missing-dir");
    assert!(
        set.warnings[0]
            .message
            .ends_with("vendor/acme/missing-dir is not a directory"),
        "{}",
        set.warnings[0].message
    );
}

#[test]
fn falls_back_to_the_packages_own_composer_json() {
    let set = enumerate(&fixture_config());
    let fallback = package(&set, "acme/fallback");

    // Not in installed.json; its composer.json has no name, so the requested
    // one is used.
    assert_eq!(fallback.name, "acme/fallback");
    assert_eq!(fallback.version, None);
    assert_eq!(fallback.autoload.psr4.0[0].0, "Acme\\Fallback\\");
    assert_eq!(
        fallback.path,
        Some(fixture_dir().join("vendor/acme/fallback"))
    );
    // `php` is a platform requirement.
    assert!(fallback.requires.is_empty());
}

#[test]
fn records_the_metadata_later_stages_need() {
    let set = enumerate(&fixture_config());
    let lib = package(&set, "acme/lib");

    assert_eq!(lib.version.as_deref(), Some("1.2.0"));
    assert_eq!(lib.package_type.as_deref(), Some("library"));
    assert_eq!(lib.autoload.psr4.0[0].0, "Acme\\Lib\\");
    assert_eq!(lib.requires, ["acme/dep", "psr/log-implementation"]);
    assert_eq!(lib.suggests, ["acme/unused"]);
    assert_eq!(lib.license, ["MIT"]);
    assert!(!lib.autoload_overridden);

    assert_eq!(package(&set, "Acme/Dep").license, ["BSD-3-Clause"]);
    assert!(package(&set, "acme/logger").license.is_empty());
    assert_eq!(package(&set, "Acme/Dep").autoload.psr0.0[0].0, "Acme_Dep_");
    assert_eq!(
        package(&set, "acme/logger").autoload.files,
        ["functions.php"]
    );
    assert_eq!(package(&set, "acme/plugin-pkg").autoload.classmap, ["inc/"]);
}

#[test]
fn override_autoload_replaces_the_packages_own() {
    let mut config = fixture_config();
    let mut replacement = Autoload::default();
    replacement.classmap = vec!["src/".to_string()];
    // Package names are case-insensitive.
    config
        .override_autoload
        .insert("ACME/LIB".to_string(), replacement);

    let set = enumerate(&config);
    let lib = package(&set, "acme/lib");

    assert!(lib.autoload_overridden);
    assert_eq!(lib.autoload.classmap, ["src/"]);
    assert!(lib.autoload.psr4.0.is_empty());

    assert!(!package(&set, "acme/logger").autoload_overridden);
}

#[test]
fn packages_can_be_named_in_any_case() {
    let mut config = fixture_config();
    config.packages = vec!["ACME/LIB".to_string(), "acme/lib".to_string()];

    let set = enumerate(&config);

    assert_eq!(
        package_names(&set),
        ["acme/lib", "Acme/Dep", "acme/fallback"]
    );
}

#[test]
fn no_packages_means_nothing_to_enumerate() {
    let mut config = fixture_config();
    config.packages = Vec::new();

    let set = enumerate(&config);

    assert!(set.packages.is_empty());
    assert!(set.skipped.is_empty());
}

#[test]
fn explicit_packages_replace_the_composer_require_list() {
    let config = Config::from_json(
        &fixture_dir(),
        Some(r#"{"packages": ["acme/logger"]}"#),
        Some(r#"{"require": {"acme/lib": "*"}}"#),
    )
    .unwrap();

    assert_eq!(package_names(&enumerate(&config)), ["acme/logger"]);
}

#[test]
fn fails_when_dependencies_are_not_installed() {
    let config = Config::from_json(&fixture_dir().join("web"), None, None).unwrap();

    assert!(matches!(
        enumerate_packages(&config),
        Err(PackagesError::NotInstalled { .. })
    ));
}

#[test]
fn reads_installed_json_from_the_configured_vendor_directory() {
    let config = Config::from_json(
        &fixture_dir(),
        None,
        Some(r#"{"config": {"vendor-dir": "elsewhere"}, "require": {"acme/lib": "*"}}"#),
    )
    .unwrap();

    assert!(matches!(
        enumerate_packages(&config),
        Err(PackagesError::NotInstalled { path }) if path.ends_with("elsewhere/composer/installed.json")
    ));
}
