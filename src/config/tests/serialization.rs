use serde_json::Value;

use super::*;

/// Serializes the resolved configuration, loads that as a `strauss.json`, and
/// checks the same configuration comes back.
fn assert_round_trips(strauss: &str, composer: Option<&str>) {
    let first = Config::from_json(Path::new("/project"), Some(strauss), composer).unwrap();
    let json = serde_json::to_string_pretty(&first).unwrap();

    let second = Config::from_json(Path::new("/project"), Some(&json), composer)
        .unwrap_or_else(|error| panic!("{error}\n{json}"));

    assert_eq!(serde_json::to_string_pretty(&second).unwrap(), json);
}

#[test]
fn defaults_round_trip() {
    assert_round_trips("{}", None);
}

#[test]
fn inferred_values_round_trip() {
    assert_round_trips(
        "{}",
        Some(
            r#"{
                "name": "acme/my-plugin",
                "autoload": {"psr-4": {"Acme\\Plugin\\": "src/"}},
                "require": {"php": ">=8.0", "league/flysystem": "^3"}
            }"#,
        ),
    );
}

#[test]
fn every_option_round_trips() {
    assert_round_trips(
        r#"{
            "target_directory": "lib/prefixed",
            "namespace_prefix": "Acme\\Vendor",
            "classmap_prefix": "Acme_",
            "constant_prefix": "ACME_",
            "functions_prefix": "acme_fn_",
            "packages": ["a/b", "c/d"],
            "update_call_sites": ["includes", "templates"],
            "include_root_autoload": true,
            "optimize_autoloader": false,
            "parallel": 3,
            "override_autoload": {
                "a/b": {"psr-4": {"Zed\\": ["z/", "y/"], "Aye\\": "a/"}, "classmap": ["c/"], "files": ["f.php"]}
            },
            "exclude_from_copy": {"packages": ["x/y"], "namespaces": ["X\\Y"], "file_patterns": ["~/tests/~", "docs"]},
            "exclude_from_prefix": {"packages": ["p/q"]},
            "exclude_files_from_update": {"namespaces": ["Q\\R"]},
            "exclude_constants": {"packages": ["k/l"], "constants": ["ABSPATH", "WP_PLUGIN_DIR"]},
            "exclude_from_string_rename": {"functions": ["value"]},
            "namespace_replacement_patterns": {
                "Zebra": "First",
                "~^Apple\\\\(.*)~": "Second\\\\$1"
            },
            "delete_vendor_packages": true,
            "delete_vendor_files": true,
            "exclude_git_files": false,
            "include_modified_date": false,
            "include_author": false,
            "classmap_output": false
        }"#,
        None,
    );
}

#[test]
fn disabled_function_prefixing_round_trips() {
    let strauss = r#"{"classmap_prefix": "Acme_", "functions_prefix": false}"#;
    let config = strauss_only(strauss);
    let json: Value = serde_json::to_value(&config).unwrap();

    // Leaving the option out would turn the default back on.
    assert_eq!(json["functions_prefix"], false);

    assert_round_trips(strauss, None);
}

#[test]
fn update_call_sites_and_parallel_serialize_as_they_are_written() {
    let json = |strauss: &str| serde_json::to_value(strauss_only(strauss)).unwrap();

    assert_eq!(json("{}")["update_call_sites"], false);
    assert_eq!(
        json(r#"{"update_call_sites": true}"#)["update_call_sites"],
        true
    );
    assert_eq!(
        json(r#"{"update_call_sites": ["a"]}"#)["update_call_sites"],
        serde_json::json!(["a"])
    );
    assert_eq!(json("{}")["parallel"], true);
    assert_eq!(json(r#"{"parallel": false}"#)["parallel"], false);
    assert_eq!(json(r#"{"parallel": 8}"#)["parallel"], 8);
}

#[test]
fn project_paths_are_not_part_of_the_output() {
    let json = serde_json::to_value(strauss_only("{}")).unwrap();

    assert!(json.get("project_dir").is_none());
    assert!(json.get("vendor_directory").is_none());
}

#[test]
fn serializes_namespace_replacement_patterns_in_priority_order() {
    let config = strauss_only(
        r#"{"namespace_replacement_patterns": {"Zebra": "First", "Apple": "Second", "Middle": "Third"}}"#,
    );

    // `to_string` keeps the order; a `Value` would sort the keys.
    let json = serde_json::to_string(&config).unwrap();
    let zebra = json.find("\"Zebra\"").unwrap();
    let apple = json.find("\"Apple\"").unwrap();
    let middle = json.find("\"Middle\"").unwrap();

    assert!(zebra < apple && apple < middle, "{json}");
}
