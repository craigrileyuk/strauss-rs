//! `strauss-rs diff`: the changes that would be made, shown without making them.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

fn project(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "test", name].iter().collect()
}

fn diff(project_name: &str, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .arg("diff")
        .arg("-C")
        .arg(project(project_name))
        .args(extra)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn shows_what_would_change_as_a_diff() {
    let output = diff("edit-fixture", &[]);
    let text = stdout(&output);

    assert!(text.contains(
        "--- a/acme/consumer/src/UsesGlobals.php\n+++ b/acme/consumer/src/UsesGlobals.php\n"
    ));
    assert!(text.contains("-namespace Acme\\Consumer;\n+namespace P\\V\\Acme\\Consumer;\n"));
    assert!(text.contains("-use GlobalClass;\n-use function g_helper;\n"));
    assert!(
        text.contains(
            "+use P_GlobalClass as GlobalClass;\n+use function p_g_helper as g_helper;\n"
        )
    );
    assert!(text.contains("-    public function item(): Excluded\\Item\n+    public function item(): \\Acme\\Core\\Excluded\\Item\n"));
    assert!(text.contains("+use P\\V\\Acme\\Core\\Client;\n+use Acme\\Core\\Excluded\\Item;\n"));
}

#[test]
fn only_files_with_changes_are_in_the_diff() {
    let text = stdout(&diff("edit-fixture", &[]));

    let files: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("--- a/"))
        .collect();

    assert_eq!(files.len(), 11);
    assert!(!files.contains(&"acme/core/src/Excluded/Item.php"));
    // In path order.
    let mut sorted = files.clone();
    sorted.sort();
    assert_eq!(files, sorted);
}

#[test]
fn the_summary_and_nothing_else_goes_to_stderr() {
    let output = diff("edit-fixture", &[]);

    assert_eq!(
        stderr(&output).trim(),
        "11 files to change with 53 edits; 2 unchanged; 0 skipped"
    );
}

#[test]
fn never_changes_a_file() {
    let read_all = || -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        let mut directories = vec![project("edit-fixture")];

        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();

                if path.is_dir() {
                    directories.push(path);
                } else {
                    files.push((path.clone(), fs::read_to_string(&path).unwrap()));
                }
            }
        }

        files.sort();
        files
    };

    let before = read_all();

    for extra in [&[][..], &["--json"][..], &["--package", "acme/core"][..]] {
        stdout(&diff("edit-fixture", extra));
    }

    let after = read_all();

    assert!(before.len() > 10);
    assert_eq!(before, after);
}

#[test]
fn can_show_just_some_packages() {
    let output = diff("edit-fixture", &["--package", "ACME/keep"]);
    let text = stdout(&output);

    assert_eq!(
        text.matches("\n--- a/").count() + usize::from(text.starts_with("--- a/")),
        1
    );
    assert!(text.contains("--- a/acme/keep/src/Kept.php"));
    assert!(!text.contains("acme/core"));
    assert_eq!(
        stderr(&output).trim(),
        "1 files to change with 2 edits; 2 unchanged; 0 skipped"
    );
}

#[test]
fn json_has_the_edits_and_where_they_are() {
    let report: Value = serde_json::from_str(&stdout(&diff("edit-fixture", &["--json"]))).unwrap();

    assert_eq!(report["files"].as_array().unwrap().len(), 11);
    assert_eq!(report["unchanged_files"], 2);
    assert!(report["skipped"].as_array().unwrap().is_empty());
    assert!(report["warnings"].as_array().unwrap().is_empty());

    let kept = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "acme/keep/src/Kept.php")
        .unwrap();

    assert_eq!(kept["package"], "acme/keep");
    assert_eq!(kept["edits"].as_array().unwrap().len(), 2);
    assert_eq!(kept["edits"][0]["kind"], "import");
    assert_eq!(kept["edits"][0]["replacement"], "P\\V\\Acme\\Core\\Client");
    assert_eq!(kept["edits"][1]["kind"], "reference");

    // Positions in the file, to be applied to its original text.
    let source = fs::read_to_string(kept["source_path"].as_str().unwrap()).unwrap();
    let (start, end) = (
        kept["edits"][0]["start"].as_u64().unwrap() as usize,
        kept["edits"][0]["end"].as_u64().unwrap() as usize,
    );

    assert_eq!(&source[start..end], "Acme\\Core\\Client");
}

#[test]
fn does_not_depend_on_the_number_of_workers() {
    let sequential = diff("edit-fixture", &["--parallel=false"]);

    // Compare real output, not two failures that are both empty.
    assert!(stdout(&sequential).len() > 3000);

    for workers in ["--parallel=2", "--parallel=8", "--parallel=auto"] {
        let parallel = diff("edit-fixture", &[workers]);

        assert_eq!(parallel.stdout, sequential.stdout, "{workers}");
    }
}

#[test]
fn says_what_it_skipped() {
    let output = diff("symbols-fixture", &["--json"]);
    let report: Value = serde_json::from_str(&stdout(&output)).unwrap();

    let skipped: Vec<(&str, &str)> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skipped| {
            (
                skipped["file"].as_str().unwrap(),
                skipped["reason"].as_str().unwrap(),
            )
        })
        .collect();

    assert!(skipped.contains(&("acme/app-lib/src/broken.php", "it could not be analysed")));
    assert!(skipped.contains(&("acme/app-lib/src/latin1.php", "it is not valid UTF-8")));
}

#[test]
fn explains_how_to_fix_a_project_without_installed_dependencies() {
    let output = diff("edit-fixture/vendor/acme", &[]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output)
            .trim()
            .ends_with("does not exist; run `composer install` first")
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn rejects_an_invalid_number_of_workers() {
    let output = diff("edit-fixture", &["--parallel=lots"]);

    assert_eq!(output.status.code(), Some(2));
}
