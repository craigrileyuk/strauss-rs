//! `strauss-rs plan`: what would be renamed, and why not the rest.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

fn plan(project: &str, extra: &[&str]) -> Output {
    let project: PathBuf = [env!("CARGO_MANIFEST_DIR"), project].iter().collect();

    Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .arg("plan")
        .arg("-C")
        .arg(project)
        .args(extra)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).unwrap()
}

fn symbol<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|symbol| symbol["name"] == name)
        .unwrap_or_else(|| panic!("{name} not in the plan"))
}

#[test]
fn shows_what_is_renamed_and_why_not_the_rest() {
    let output = plan("test/rename-fixture", &[]);
    let report = json(&output);

    let client = symbol(&report, "Acme\\Psr4\\Client");

    assert_eq!(client["rename"], true);
    assert_eq!(
        client["replacement"],
        "Prefixed\\Vendor\\Acme\\Psr4\\Client"
    );
    assert_eq!(client["reason"], "it is autoloaded");

    let kept = symbol(&report, "Acme\\Keep\\Kept");

    assert_eq!(kept["rename"], false);
    assert!(kept.get("replacement").is_none());
    assert_eq!(
        kept["reason"],
        "package acme/keep is in exclude_from_prefix.packages"
    );

    assert_eq!(
        symbol(&report, "files_helper")["replacement"],
        "pfx_files_helper"
    );
    assert_eq!(
        symbol(&report, "FILES_CONST")["replacement"],
        "PFX_FILES_CONST"
    );
    assert_eq!(symbol(&report, "Mapped")["replacement"], "Prefixed_Mapped");
    assert_eq!(
        symbol(&report, "Acme\\Pattern")["replacement"],
        "Custom\\Pattern"
    );
}

#[test]
fn counts_the_plan() {
    let report = json(&plan("test/rename-fixture", &[]));

    assert_eq!(report["summary"]["symbols"], 45);
    assert_eq!(report["summary"]["renamed"], 29);
    assert_eq!(report["summary"]["by_kind"]["class"]["renamed"], 8);
    assert_eq!(report["summary"]["reasons"]["autoloaded"], 25);
    assert_eq!(report["symbols"].as_array().unwrap().len(), 45);
}

#[test]
fn lists_the_packages_and_files_that_are_left_alone() {
    let report = json(&plan("test/rename-fixture", &[]));

    let packages: Vec<&str> = report["packages_to_prefix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| package.as_str().unwrap())
        .collect();

    assert_eq!(packages.len(), 6);
    assert!(!packages.contains(&"acme/keep"));

    assert_eq!(
        report["files_excluded_from_prefix"][0]["file"],
        "acme/psr4-lib/src/Legacy/Old.php"
    );
    assert_eq!(
        report["files_excluded_from_prefix"][0]["pattern"],
        "~/Legacy/~"
    );
}

#[test]
fn the_report_is_ordered_for_reading() {
    let output = plan("test/rename-fixture", &[]);
    let text = String::from_utf8_lossy(&output.stdout);

    let positions: Vec<usize> = [
        "\"packages_to_prefix\"",
        "\"summary\"",
        "\"files_excluded_from_prefix\"",
        "\"symbols\": [",
    ]
    .iter()
    .map(|key| text.find(key).unwrap())
    .collect();

    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{positions:?}"
    );
}

#[test]
fn can_show_just_some_packages() {
    let report = json(&plan(
        "test/rename-fixture",
        &["--package", "acme/classmap-lib"],
    ));

    let names: Vec<&str> = report["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|symbol| symbol["name"].as_str().unwrap())
        .collect();

    assert_eq!(
        names,
        [
            "Mapped",
            "Mapped_Interface",
            "MappedTrait",
            "Status",
            "SkippedByClassmap",
            "Loose"
        ]
    );

    // The counts are for the whole plan.
    assert_eq!(report["summary"]["symbols"], 45);
}

#[test]
fn package_names_can_be_repeated_and_ignore_case() {
    let report = json(&plan(
        "test/rename-fixture",
        &["--package=ACME/KEEP", "--package", "acme/not-copied"],
    ));

    let names: Vec<&str> = report["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|symbol| symbol["name"].as_str().unwrap())
        .collect();

    assert_eq!(names.len(), 4);
    assert!(names.contains(&"Acme\\Keep\\Kept"));
    assert!(names.contains(&"Acme\\NotCopied\\N"));
}

#[test]
fn a_package_with_no_symbols_shows_none() {
    let report = json(&plan("test/rename-fixture", &["--package", "acme/nothing"]));

    assert!(report["symbols"].as_array().unwrap().is_empty());
}

#[test]
fn has_nothing_to_warn_about_for_the_fixture() {
    let output = plan("test/rename-fixture", &[]);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn does_not_depend_on_the_number_of_workers() {
    let sequential = plan("test/rename-fixture", &["--parallel=false"]);

    // Compare real output, not two failures that are both empty.
    assert!(
        sequential.status.success(),
        "{}",
        String::from_utf8_lossy(&sequential.stderr)
    );
    assert!(sequential.stdout.len() > 5000);

    for workers in ["--parallel=2", "--parallel=8", "--parallel=auto"] {
        let parallel = plan("test/rename-fixture", &[workers]);

        assert!(parallel.status.success(), "{workers}");
        assert_eq!(parallel.stdout, sequential.stdout, "{workers}");
    }
}

#[test]
fn explains_how_to_fix_a_project_without_installed_dependencies() {
    let output = plan("test/rename-fixture/vendor/acme", &[]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .trim()
            .ends_with("does not exist; run `composer install` first")
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn reports_unanalysable_files_like_the_symbols_command_does() {
    let output = plan("test/symbols-fixture", &[]);
    let report = json(&output);

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("warning: acme/app-lib/src/broken.php: parse error: "));
    assert!(report["summary"]["symbols"].as_u64().unwrap() > 10);
}
