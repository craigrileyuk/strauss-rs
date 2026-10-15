use super::*;
use crate::packages::installed::InstalledIndex;

fn index(json: &str) -> InstalledIndex {
    InstalledIndex::parse(json).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn reads_composer_2_format() {
    let index = index(
        r#"{"packages": [
            {"name": "acme/lib", "version": "1.0.0", "type": "library", "install-path": "../acme/lib",
             "autoload": {"psr-4": {"Acme\\": "src/"}}, "require": {"php": ">=8", "acme/dep": "^1"},
             "suggest": {"acme/extra": "why"}, "license": ["MIT"]}
        ], "dev": true, "dev-package-names": []}"#,
    );

    let package = index.get("acme/lib").unwrap();

    assert_eq!(package.version.as_deref(), Some("1.0.0"));
    assert_eq!(package.package_type.as_deref(), Some("library"));
    assert_eq!(package.install_path.as_deref(), Some("../acme/lib"));
    assert_eq!(package.autoload.psr4.0[0].0, "Acme\\");
    assert_eq!(
        package
            .require
            .0
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["php", "acme/dep"]
    );
    assert_eq!(package.suggest.0[0].0, "acme/extra");
    assert_eq!(index.len(), 1);
}

#[test]
fn reads_composer_1_format() {
    let index = index(r#"[{"name": "acme/lib", "version": "1.0.0"}, {"name": "acme/dep"}]"#);

    assert_eq!(index.len(), 2);
    assert!(index.get("acme/lib").unwrap().install_path.is_none());
    assert!(index.get("acme/dep").is_some());
}

#[test]
fn an_empty_install_has_no_packages() {
    assert_eq!(index(r#"{"packages": []}"#).len(), 0);
    assert_eq!(index("[]").len(), 0);
}

#[test]
fn looks_up_names_case_insensitively() {
    let index = index(r#"{"packages": [{"name": "Acme/Lib"}]}"#);

    assert!(index.get("acme/lib").is_some());
    assert!(index.get("ACME/LIB").is_some());
    assert!(index.get("acme/other").is_none());
}

#[test]
fn accepts_the_empty_arrays_php_writes_for_empty_maps() {
    let index = index(
        r#"{"packages": [{"name": "acme/lib", "require": [], "provide": [], "suggest": [], "autoload": {"psr-4": []}}]}"#,
    );

    let package = index.get("acme/lib").unwrap();

    assert!(package.require.0.is_empty());
    assert!(package.autoload.psr4.0.is_empty());
}

#[test]
fn a_license_is_a_string_or_a_list() {
    let index = index(
        r#"{"packages": [{"name": "a/a", "license": "MIT"}, {"name": "b/b", "license": ["MIT", "GPL-2.0"]}, {"name": "c/c"}]}"#,
    );

    let licenses = |name: &str| -> Vec<String> {
        index
            .get(name)
            .unwrap()
            .license
            .as_ref()
            .map(|license| license.iter().map(String::from).collect())
            .unwrap_or_default()
    };

    assert_eq!(licenses("a/a"), ["MIT"]);
    assert_eq!(licenses("b/b"), ["MIT", "GPL-2.0"]);
    assert!(licenses("c/c").is_empty());
}

#[test]
fn provided_names_include_provide_and_replace() {
    let index = index(
        r#"{"packages": [{"name": "acme/logger", "provide": {"psr/log-implementation": "1.0"}, "replace": {"acme/old": "*"}}]}"#,
    );

    let names: Vec<&str> = index.get("acme/logger").unwrap().provided_names().collect();

    assert_eq!(names, ["psr/log-implementation", "acme/old"]);
    assert!(index.is_provided("psr/log-implementation"));
    assert!(index.is_provided("PSR/log-implementation"));
    assert!(index.is_provided("acme/old"));
    assert!(!index.is_provided("acme/logger"));
}

#[test]
fn rejects_what_composer_does_not_write() {
    assert!(InstalledIndex::parse("{").is_err());
    assert!(InstalledIndex::parse(r#"{"not-packages": []}"#).is_err());
    assert!(InstalledIndex::parse(r#"{"packages": [{"version": "1.0"}]}"#).is_err());
    assert!(InstalledIndex::parse(r#""text""#).is_err());
}

#[test]
fn reports_a_project_without_installed_dependencies() {
    let error = InstalledIndex::load(&fixture_dir().join("web")).unwrap_err();

    assert!(matches!(error, PackagesError::NotInstalled { .. }));
    assert!(
        error
            .to_string()
            .ends_with("does not exist; run `composer install` first"),
        "{error}"
    );
}

#[test]
fn loads_the_fixtures_installed_json() {
    let index = InstalledIndex::load(&fixture_dir().join("vendor")).unwrap();

    assert_eq!(index.len(), 7);
}
