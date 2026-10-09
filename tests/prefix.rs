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

/// The edit fixture with a plugin of its own that uses `Acme\Core\Client`, and
/// `update_call_sites` set to `setting`.
fn project_with_call_sites(test: &str, setting: &str) -> PathBuf {
    let project = copy_of("edit-fixture", test);

    let composer = fs::read_to_string(project.join("composer.json")).unwrap();
    fs::write(
        project.join("composer.json"),
        composer.replacen(
            "\"require\"",
            "\"autoload\": {\"psr-4\": {\"Me\\\\Plugin\\\\\": \"src/\"}},\n    \"require\"",
            1,
        ),
    )
    .unwrap();

    let config = fs::read_to_string(project.join("strauss.json")).unwrap();
    fs::write(
        project.join("strauss.json"),
        config.replacen(
            '{',
            &format!("{{\n    \"update_call_sites\": {setting},"),
            1,
        ),
    )
    .unwrap();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/Plugin.php"),
        "<?php\nnamespace Me\\Plugin;\n\nuse Acme\\Core\\Client;\n\nclass Plugin\n{\n    public function run()\n    {\n        return new Client();\n    }\n}\n",
    )
    .unwrap();

    project
}

#[test]
fn updates_the_projects_own_files_when_update_call_sites_is_on() {
    let project = project_with_call_sites("call-sites", "true");
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Updated 1 of your own files"));

    let plugin = fs::read_to_string(project.join("src/Plugin.php")).unwrap();

    assert!(plugin.contains("use P\\V\\Acme\\Core\\Client;"), "{plugin}");
    assert!(plugin.contains("namespace Me\\Plugin;"), "{plugin}");
}

#[test]
fn leaves_the_projects_own_files_alone_by_default() {
    let project = project_with_call_sites("call-sites-off", "false");
    let before = fs::read_to_string(project.join("src/Plugin.php")).unwrap();
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(project.join("src/Plugin.php")).unwrap(),
        before
    );
}

#[test]
fn the_command_line_overrides_update_call_sites() {
    let project = project_with_call_sites("call-sites-flag", "false");
    let output = prefix(&project, &["--updateCallSites"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        fs::read_to_string(project.join("src/Plugin.php"))
            .unwrap()
            .contains("P\\V\\Acme\\Core\\Client")
    );
}

#[test]
fn a_dry_run_does_not_change_the_projects_own_files() {
    let project = project_with_call_sites("call-sites-dry", "true");
    let before = fs::read_to_string(project.join("src/Plugin.php")).unwrap();
    let output = prefix(&project, &["--dry-run"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Would update 1 of your own files"));
    assert_eq!(
        fs::read_to_string(project.join("src/Plugin.php")).unwrap(),
        before
    );
}

#[test]
fn only_the_listed_paths_are_updated_when_update_call_sites_is_a_list() {
    let project = project_with_call_sites("call-sites-list", "[\"elsewhere\"]");
    let before = fs::read_to_string(project.join("src/Plugin.php")).unwrap();
    let output = prefix(&project, &[]);

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(project.join("src/Plugin.php")).unwrap(),
        before
    );
    assert!(stderr(&output).contains("elsewhere does not exist"));
}

const COMPOSER_AUTOLOAD: &str = "<?php\n\nrequire_once __DIR__ . '/composer/autoload_real.php';\n\nreturn ComposerAutoloaderInitabc::getLoader();\n";

fn with_include_autoloader(test: &str, setting: bool) -> PathBuf {
    let project = copy_of("edit-fixture", test);

    fs::write(project.join("vendor/autoload.php"), COMPOSER_AUTOLOAD).unwrap();

    let config = fs::read_to_string(project.join("strauss.json")).unwrap();
    fs::write(
        project.join("strauss.json"),
        config.replacen(
            '{',
            &format!("{{\n    \"include_autoloader\": {setting},"),
            1,
        ),
    )
    .unwrap();

    project
}

#[test]
fn includes_the_autoloader_when_include_autoloader_is_on() {
    let project = with_include_autoloader("include-on", true);
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Updated vendor/autoload.php"));
    assert!(
        fs::read_to_string(project.join("vendor/autoload.php"))
            .unwrap()
            .contains("require_once __DIR__ . '/../vendor-prefixed/autoload.php';")
    );

    // A second run leaves it as it is.
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!stderr(&output).contains("Updated vendor/autoload.php"));
    assert_eq!(
        fs::read_to_string(project.join("vendor/autoload.php"))
            .unwrap()
            .matches("vendor-prefixed/autoload.php")
            .count(),
        1
    );
}

#[test]
fn leaves_vendor_autoload_alone_by_default() {
    let project = with_include_autoloader("include-off", false);
    let output = prefix(&project, &[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(project.join("vendor/autoload.php")).unwrap(),
        COMPOSER_AUTOLOAD
    );
}

#[test]
fn a_dry_run_does_not_include_the_autoloader() {
    let project = with_include_autoloader("include-dry", true);
    let output = prefix(&project, &["--dry-run"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Would update vendor/autoload.php"));
    assert_eq!(
        fs::read_to_string(project.join("vendor/autoload.php")).unwrap(),
        COMPOSER_AUTOLOAD
    );
}
