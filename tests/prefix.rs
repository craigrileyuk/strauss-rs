//! `strauss-rs prefix`: writing the result, on a copy of a fixture.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// A copy of `test/<name>` in the target directory for this test.
fn copy_of(name: &str, test: &str) -> PathBuf {
    let from: PathBuf = [env!("CARGO_MANIFEST_DIR"), "test", name].iter().collect();
    let to = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("prefix")
        .join(test);

    let _ = fs::remove_dir_all(&to);
    copy_directory(&from, &to);

    to
}

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();

    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();

        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

fn prefix(project: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .arg("prefix")
        .arg("-C")
        .arg(project)
        .args(extra)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn writes_the_prefixed_packages_and_the_autoloader() {
    let project = copy_of("edit-fixture", "writes");
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty());

    let summary = stderr(&output);

    assert!(
        summary.contains("Wrote 13 files to vendor-prefixed (11 modified)"),
        "{summary}"
    );
    assert!(summary.contains(
        "Wrote vendor-prefixed/autoload-classmap.php and vendor-prefixed/autoload.php with 12 classes"
    ));

    let client =
        fs::read_to_string(project.join("vendor-prefixed/acme/core/src/Client.php")).unwrap();

    assert!(client.contains("namespace P\\V\\Acme\\Core;"));
    assert!(client.contains("Modified on "));

    let classmap =
        fs::read_to_string(project.join("vendor-prefixed/autoload-classmap.php")).unwrap();

    assert!(classmap.contains("'P\\\\V\\\\Acme\\\\Core\\\\Client' =>"));

    // The vendor directory is as it was.
    let original = fs::read_to_string(project.join("vendor/acme/core/src/Client.php")).unwrap();

    assert!(original.contains("namespace Acme\\Core;"));
}

#[test]
fn a_dry_run_writes_nothing() {
    let project = copy_of("edit-fixture", "dry-run");
    let output = prefix(&project, &["--dry-run"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Would write 13 files to vendor-prefixed (11 modified)"));
    assert!(!project.join("vendor-prefixed").exists());
}

#[test]
fn json_reports_what_was_written() {
    let project = copy_of("edit-fixture", "json");
    let output = prefix(&project, &["--json"]);

    assert!(output.status.success(), "{}", stderr(&output));

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(report["written"], 13);
    assert_eq!(report["modified"], 11);
    assert_eq!(report["classmap_entries"], 12);
    assert_eq!(report["autoloader"][1], "vendor-prefixed/autoload.php");
}

#[test]
fn delete_vendor_packages_removes_them_after_copying() {
    let project = copy_of("edit-fixture", "delete");

    let mut config: Value =
        serde_json::from_str(&fs::read_to_string(project.join("strauss.json")).unwrap()).unwrap();

    config["delete_vendor_packages"] = true.into();
    fs::write(project.join("strauss.json"), config.to_string()).unwrap();

    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Deleted 13 files from vendor, including 4 whole packages"));
    assert!(
        project
            .join("vendor-prefixed/acme/core/src/Client.php")
            .exists()
    );
    assert!(!project.join("vendor/acme").exists());
    assert!(project.join("vendor/composer/installed.json").exists());
}

#[test]
fn fails_clearly_when_the_packages_are_not_installed() {
    let project = copy_of("edit-fixture", "not-installed");

    fs::remove_file(project.join("vendor/composer/installed.json")).unwrap();

    let output = prefix(&project, &[]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("run `composer install` first"),
        "{}",
        stderr(&output)
    );
    assert!(!project.join("vendor-prefixed").exists());
}

#[test]
fn quiet_leaves_out_the_summary() {
    let project = copy_of("edit-fixture", "quiet");
    let output = prefix(&project, &["--quiet"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    assert!(project.join("vendor-prefixed/autoload.php").exists());
}

#[test]
fn nothing_is_drawn_when_stderr_is_not_a_terminal() {
    let project = copy_of("edit-fixture", "not-a-terminal");
    let text = stderr(&prefix(&project, &[]));

    // The summary only: no progress bars or the escape codes that draw them.
    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert!(!text.contains("Analysing"), "{text:?}");
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn many_files_with_the_same_problem_are_one_warning() {
    let project = copy_of("edit-fixture", "grouped-warnings");

    for name in ["A", "B", "C", "D", "E"] {
        fs::write(
            project.join(format!("vendor/acme/core/src/Latin{name}.php")),
            b"<?php\nnamespace Acme\\Core;\n$s = '\xe9';\n",
        )
        .unwrap();
    }

    let text = stderr(&prefix(&project, &[]));
    let warnings: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();

    assert_eq!(warnings.len(), 2, "{text}");
    assert!(
        warnings[0].starts_with("warning: 5 files: the file is not valid UTF-8"),
        "{text}"
    );
    assert!(
        warnings[1].starts_with("warning: 5 files: copied as it is: it is not valid UTF-8"),
        "{text}"
    );
}
