use super::*;
use crate::config::validate::OBJECT_OPTIONS;
use std::path::Path;

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
    assert!(!config.include_autoloader);
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
                "exclude_presets": ["wordpress"],
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
