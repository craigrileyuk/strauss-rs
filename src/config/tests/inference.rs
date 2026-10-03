use super::*;

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
