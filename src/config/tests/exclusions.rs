use super::*;

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

#[test]
fn the_wordpress_preset_is_excluded_by_default() {
    assert_eq!(strauss_only("{}").exclude_presets, ["wordpress"]);
}

#[test]
fn presets_can_be_turned_off() {
    assert!(
        strauss_only(r#"{"exclude_presets": []}"#)
            .exclude_presets
            .is_empty()
    );
}

#[test]
fn preset_names_are_normalised_and_deduplicated() {
    assert_eq!(
        strauss_only(r#"{"exclude_presets": [" WordPress ", "wordpress"]}"#).exclude_presets,
        ["wordpress"]
    );
}

#[test]
fn rejects_an_unknown_preset() {
    let message = error(r#"{"exclude_presets": ["wordpress", "drupal"]}"#);

    assert!(message.contains("`exclude_presets[1]`"), "{message}");
    assert!(message.contains("`drupal` is not a preset"), "{message}");
    assert!(message.contains("wordpress, woocommerce"), "{message}");
}

#[test]
fn the_woocommerce_preset_is_opt_in() {
    assert_eq!(strauss_only("{}").exclude_presets, ["wordpress"]);
    assert_eq!(
        strauss_only(r#"{"exclude_presets": ["wordpress", "woocommerce"]}"#).exclude_presets,
        ["wordpress", "woocommerce"]
    );
    assert_eq!(
        strauss_only(r#"{"exclude_presets": ["WooCommerce"]}"#).exclude_presets,
        ["woocommerce"]
    );
}
