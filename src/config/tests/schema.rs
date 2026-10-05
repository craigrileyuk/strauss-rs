use super::*;
use crate::config::CONFIG_FILE;
use serde_json::Value;
use std::fs;
use std::path::Path;

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
    assert_eq!(default("parallel"), Value::from("auto"));
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
