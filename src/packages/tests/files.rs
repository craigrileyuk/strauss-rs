use super::*;
use crate::config::Config;
use crate::php_pattern::PhpPattern;

fn files_of(config: &Config) -> Vec<PackageFile> {
    let set = enumerate_packages(config).unwrap();

    enumerate_files(config, &set).unwrap()
}

fn paths(files: &[PackageFile]) -> Vec<&str> {
    files
        .iter()
        .map(|file| file.vendor_relative_path.as_str())
        .collect()
}

fn file<'a>(files: &'a [PackageFile], vendor_relative_path: &str) -> &'a PackageFile {
    files
        .iter()
        .find(|file| file.vendor_relative_path == vendor_relative_path)
        .unwrap_or_else(|| panic!("{vendor_relative_path} not found in {:?}", paths(files)))
}

#[test]
fn lists_every_file_of_every_package_ordered_by_path() {
    let files = files_of(&fixture_config());

    // Not just the autoloaded files: the README, tests and composer.json too.
    assert_eq!(
        paths(&files),
        [
            "Acme/Dep/lib/Dep.php",
            "acme/fallback/composer.json",
            "acme/fallback/src/F.php",
            "acme/lib/README.md",
            "acme/lib/src/Lib.php",
            "acme/lib/src/Sub/Helper.php",
            "acme/lib/tests/LibTest.php",
            "acme/logger/functions.php",
            "acme/logger/src/Logger.php",
            "acme/plugin-pkg/inc/a.php",
            "acme/plugin-pkg/plugin.php",
        ]
    );
}

#[test]
fn records_where_each_file_is_and_where_it_goes() {
    let files = files_of(&fixture_config());
    let project = fixture_dir();

    let helper = file(&files, "acme/lib/src/Sub/Helper.php");

    assert_eq!(helper.package, "acme/lib");
    assert_eq!(helper.package_relative_path, "src/Sub/Helper.php");
    assert_eq!(
        helper.source_path,
        project.join("vendor/acme/lib/src/Sub/Helper.php")
    );
    assert_eq!(
        helper.target_path,
        project.join("vendor-prefixed/acme/lib/src/Sub/Helper.php")
    );
    assert!(helper.copy);
    assert!(helper.not_copied_because.is_none());
    assert!(helper.source_path.is_file());
}

#[test]
fn a_package_outside_vendor_is_copied_to_a_path_by_package_name() {
    let files = files_of(&fixture_config());
    let project = fixture_dir();

    let plugin = file(&files, "acme/plugin-pkg/plugin.php");

    assert_eq!(
        plugin.source_path,
        project.join("web/plugins/plugin-pkg/plugin.php")
    );
    assert_eq!(
        plugin.target_path,
        project.join("vendor-prefixed/acme/plugin-pkg/plugin.php")
    );
}

#[test]
fn the_target_directory_is_configurable() {
    let config = Config::from_json(
        &fixture_dir(),
        Some(r#"{"target_directory": "lib/prefixed"}"#),
        Some(r#"{"require": {"acme/logger": "*"}}"#),
    )
    .unwrap();

    let files = files_of(&config);

    assert_eq!(
        file(&files, "acme/logger/functions.php").target_path,
        fixture_dir().join("lib/prefixed/acme/logger/functions.php")
    );
}

#[test]
fn packages_without_files_contribute_none() {
    let files = files_of(&fixture_config());

    assert!(
        !paths(&files)
            .iter()
            .any(|path| path.starts_with("acme/meta/"))
    );
    assert!(
        !paths(&files)
            .iter()
            .any(|path| path.starts_with("acme/missing-dir/"))
    );
    assert!(
        !paths(&files)
            .iter()
            .any(|path| path.starts_with("acme/unused/"))
    );
}

#[test]
fn copies_everything_by_default() {
    assert!(files_of(&fixture_config()).iter().all(|file| file.copy));
}

#[test]
fn a_package_can_be_excluded_from_copy() {
    let mut config = fixture_config();
    // Package names are case-insensitive.
    config.exclude_from_copy.packages = vec!["ACME/Lib".to_string()];

    let files = files_of(&config);

    for file in files.iter().filter(|file| file.package == "acme/lib") {
        assert!(!file.copy, "{}", file.vendor_relative_path);
        assert_eq!(
            file.not_copied_because,
            Some(CopyExclusion::PackageExcluded)
        );
    }

    assert!(file(&files, "acme/logger/src/Logger.php").copy);
}

#[test]
fn files_can_be_excluded_by_pattern_against_the_vendor_relative_path() {
    let mut config = fixture_config();
    config.exclude_from_copy.file_patterns = vec![
        PhpPattern::regex("~^acme/lib/tests/~").unwrap(),
        PhpPattern::regex("\\.md$").unwrap(),
    ];

    let files = files_of(&config);

    assert_eq!(
        file(&files, "acme/lib/tests/LibTest.php").not_copied_because,
        Some(CopyExclusion::PatternMatched(
            "~^acme/lib/tests/~".to_string()
        ))
    );
    assert_eq!(
        file(&files, "acme/lib/README.md").not_copied_because,
        Some(CopyExclusion::PatternMatched("\\.md$".to_string()))
    );
    assert!(file(&files, "acme/lib/src/Lib.php").copy);
    assert_eq!(files.iter().filter(|file| !file.copy).count(), 2);
}

#[test]
fn nothing_is_copied_when_the_target_is_the_vendor_directory() {
    for target in ["vendor", "/vendor/"] {
        let config = Config::from_json(
            &fixture_dir(),
            Some(&format!(r#"{{"target_directory": "{target}"}}"#)),
            Some(r#"{"require": {"acme/lib": "*"}}"#),
        )
        .unwrap();

        let files = files_of(&config);

        assert!(!files.is_empty());
        assert!(
            files
                .iter()
                .all(|file| !file.copy
                    && file.not_copied_because == Some(CopyExclusion::TargetIsVendor)),
            "{target}"
        );
    }
}

#[test]
fn the_vendor_directory_follows_composers_config() {
    let config = Config::from_json(
        &fixture_dir(),
        Some(r#"{"target_directory": "vendor"}"#),
        Some(r#"{"config": {"vendor-dir": "elsewhere"}, "require": {"acme/lib": "*"}}"#),
    )
    .unwrap();

    // The vendor directory is `elsewhere`, so `vendor` is just another target.
    assert!(!crate::packages::files::is_target_the_vendor_directory(
        &config
    ));
}

#[test]
fn the_first_reason_a_file_is_not_copied_is_the_one_given() {
    let mut config = fixture_config();
    config.exclude_from_copy.packages = vec!["acme/lib".to_string()];
    config.exclude_from_copy.file_patterns = vec![PhpPattern::regex("~acme/lib~").unwrap()];

    let files = files_of(&config);

    assert_eq!(
        file(&files, "acme/lib/src/Lib.php").not_copied_because,
        Some(CopyExclusion::PackageExcluded)
    );
}

#[test]
fn describes_why_a_file_is_not_copied() {
    assert_eq!(
        CopyExclusion::PackageExcluded.to_string(),
        "the package is in exclude_from_copy.packages"
    );
    assert_eq!(
        CopyExclusion::PatternMatched("~x~".to_string()).to_string(),
        "it matches exclude_from_copy.file_patterns `~x~`"
    );
    assert_eq!(
        CopyExclusion::TargetIsVendor.to_string(),
        "the target directory is the vendor directory"
    );
}
