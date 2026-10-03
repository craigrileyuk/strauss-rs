use super::*;

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
        error(r#"{"update_call_sites": [1]}"#).contains("`update_call_sites[0]` must be a string")
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
