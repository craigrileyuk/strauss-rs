//! `strauss-rs symbols`: the symbols each package declares, as the user sees them.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

fn symbols(project: &str, extra: &[&str]) -> Output {
    let project: PathBuf = [env!("CARGO_MANIFEST_DIR"), project].iter().collect();

    Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .arg("symbols")
        .arg("-C")
        .arg(project)
        .args(extra)
        .output()
        .unwrap()
}

fn report(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn lists_each_packages_symbols() {
    let report = report(&symbols("test/symbols-fixture", &[]));
    let lib = &report["packages"]["acme/app-lib"];

    assert_eq!(lib["namespaces"][0], "Acme\\App");
    assert_eq!(
        lib["interfaces"],
        serde_json::json!(["Acme\\App\\Contracts\\Marker"])
    );
    assert_eq!(
        lib["traits"],
        serde_json::json!(["Acme\\App\\Concerns\\Greets"])
    );
    assert_eq!(lib["enums"], serde_json::json!(["Acme\\App\\Status"]));
    assert_eq!(
        lib["constants"],
        serde_json::json!(["Acme\\App\\VERSION", "ACME_APP_DEFINED"])
    );
    assert_eq!(lib["php_files"], 9);
    assert_eq!(lib["unanalysed_files"], 1);
    assert_eq!(lib["lossy_files"], 1);

    // Shared symbols appear under each package that declares them.
    assert_eq!(
        report["packages"]["acme/dup"]["classes"],
        serde_json::json!(["Acme\\App\\Base"])
    );
}

#[test]
fn lists_what_php_provides_separately() {
    let report = report(&symbols("test/symbols-fixture", &[]));

    let built_ins: Vec<(&str, &str)> = report["built_ins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|symbol| {
            (
                symbol["kind"].as_str().unwrap(),
                symbol["name"].as_str().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        built_ins,
        [
            ("interface", "Stringable"),
            ("function", "str_contains"),
            ("function", "STR_STARTS_WITH"),
            ("class", "ArrayObject")
        ]
    );
}

#[test]
fn reports_unanalysable_files_in_the_json_and_on_stderr() {
    let output = symbols("test/symbols-fixture", &[]);
    let report = report(&output);

    let files: Vec<&str> = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warning| warning["file"].as_str().unwrap())
        .collect();

    assert_eq!(
        files,
        ["acme/app-lib/src/broken.php", "acme/app-lib/src/latin1.php"]
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("warning: acme/app-lib/src/broken.php: parse error: "),
        "{stderr}"
    );
    assert!(
        stderr.contains("warning: acme/app-lib/src/latin1.php: the file is not valid UTF-8"),
        "{stderr}"
    );
}

#[test]
fn the_report_lists_packages_before_the_smaller_sections() {
    let output = symbols("test/symbols-fixture", &[]);
    let text = String::from_utf8_lossy(&output.stdout);

    let packages = text.find("\"packages\"").unwrap();
    let built_ins = text.find("\"built_ins\"").unwrap();
    let warnings = text.find("\"warnings\"").unwrap();

    assert!(packages < built_ins && built_ins < warnings);
}

#[test]
fn does_not_depend_on_the_number_of_workers() {
    let sequential = symbols("test/symbols-fixture", &["--parallel=false"]);

    // Compare real output, not two failures that are both empty.
    assert!(
        sequential.status.success(),
        "{}",
        String::from_utf8_lossy(&sequential.stderr)
    );
    assert!(sequential.stdout.len() > 1000);

    for workers in ["--parallel=2", "--parallel=8", "--parallel"] {
        let parallel = symbols("test/symbols-fixture", &[workers]);

        assert!(
            parallel.status.success(),
            "{workers}: {}",
            String::from_utf8_lossy(&parallel.stderr)
        );
        assert_eq!(parallel.stdout, sequential.stdout, "{workers}");
    }
}

#[test]
fn rejects_an_invalid_number_of_workers() {
    let output = symbols("test/symbols-fixture", &["--parallel=lots"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("expected `auto`, `true`, `false`, or a whole number")
    );
}

#[test]
fn explains_how_to_fix_a_project_without_installed_dependencies() {
    let output = symbols("test/symbols-fixture/vendor/acme", &[]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .trim()
            .ends_with("does not exist; run `composer install` first")
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn works_on_the_other_fixture_too() {
    let report = report(&symbols("test/packages-fixture", &[]));

    assert_eq!(
        report["packages"]["acme/logger"]["functions"],
        serde_json::json!(["acme_log"])
    );
    assert_eq!(
        report["packages"]["acme/plugin-pkg"]["classes"],
        serde_json::json!(["PluginA"])
    );
}

#[test]
fn lists_what_wordpress_provides_separately() {
    let report = report(&symbols("test/presets-fixture", &[]));

    let mut built_ins: Vec<(&str, &str, &str)> = report["built_ins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|symbol| {
            (
                symbol["kind"].as_str().unwrap(),
                symbol["name"].as_str().unwrap(),
                symbol["provided_by"].as_str().unwrap(),
            )
        })
        .collect();
    built_ins.sort();

    assert_eq!(
        built_ins,
        [
            ("class", "WP_Error", "wordpress"),
            ("constant", "ABSPATH", "wordpress"),
            ("constant", "WP_DEBUG", "wordpress"),
            ("function", "WP_JSON_ENCODE", "wordpress"),
        ]
    );
}
