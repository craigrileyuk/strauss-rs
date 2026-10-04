//! Package enumeration against projects built on the fly, for the cases a committed fixture
//! can't hold: `.git`, `.gitignore`, symlinks. Each test builds a Composer-shaped project in
//! `target/tmp` and runs the real binary against it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// A project with one package, `acme/lib`, installed in `vendor/acme/lib`.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("packages")
            .join(name);

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let project = Project { root };

        project.write("composer.json", r#"{"require": {"acme/lib": "*"}}"#);
        project.write(
            "vendor/composer/installed.json",
            r#"{"packages": [{"name": "acme/lib", "version": "1.0.0", "install-path": "../acme/lib"}]}"#,
        );

        project
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// A file inside the package.
    fn package_file(&self, relative: &str, contents: &str) {
        self.write(&format!("vendor/acme/lib/{relative}"), contents);
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
            .args(args)
            .arg("-C")
            .arg(&self.root)
            .output()
            .unwrap()
    }

    /// The package-relative paths `files` lists.
    fn files(&self) -> Vec<String> {
        let output = self.run(&["files"]);

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let json: Value = serde_json::from_slice(&output.stdout).unwrap();

        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["package_relative_path"].as_str().unwrap().to_string())
            .collect()
    }
}

#[test]
fn excludes_the_git_directory() {
    let project = Project::new("git-directory");
    project.package_file("src/A.php", "<?php");
    project.package_file(".git/HEAD", "ref: refs/heads/main");
    project.package_file(".git/objects/ab/cdef", "x");

    assert_eq!(project.files(), ["src/A.php"]);
}

#[test]
fn excludes_a_git_file() {
    // A submodule or worktree has a `.git` file, not a directory.
    let project = Project::new("git-file");
    project.package_file("src/A.php", "<?php");
    project.package_file(".git", "gitdir: ../../.git/modules/lib");

    assert_eq!(project.files(), ["src/A.php"]);
}

#[test]
fn keeps_other_hidden_files() {
    let project = Project::new("hidden-files");
    project.package_file("src/A.php", "<?php");
    project.package_file(".editorconfig", "root = true");
    project.package_file("src/.gitkeep", "");
    project.package_file(".github/workflows/ci.yml", "on: push");

    assert_eq!(
        project.files(),
        [
            ".editorconfig",
            ".github/workflows/ci.yml",
            "src/.gitkeep",
            "src/A.php"
        ]
    );
}

#[test]
fn excludes_files_matched_by_the_packages_gitignore() {
    let project = Project::new("gitignore");
    project.package_file(".gitignore", "build/\n*.log\n!keep.log\n/secret.php\n");
    project.package_file("src/A.php", "<?php");
    project.package_file("build/out.js", "x");
    project.package_file("src/build/nested.js", "x");
    project.package_file("debug.log", "x");
    project.package_file("keep.log", "x");
    project.package_file("secret.php", "<?php");
    project.package_file("src/secret.php", "<?php");

    // Only the root `/secret.php` is ignored; `build/` matches at any depth.
    assert_eq!(
        project.files(),
        [".gitignore", "keep.log", "src/A.php", "src/secret.php"]
    );
}

#[test]
fn a_nested_gitignore_applies_only_within_its_directory() {
    let project = Project::new("nested-gitignore");
    project.package_file("src/.gitignore", "generated.php\n");
    project.package_file("src/generated.php", "<?php");
    project.package_file("src/A.php", "<?php");
    project.package_file("generated.php", "<?php");

    assert_eq!(
        project.files(),
        ["generated.php", "src/.gitignore", "src/A.php"]
    );
}

#[test]
fn the_projects_own_gitignore_does_not_apply_to_its_packages() {
    // A project usually ignores `vendor/` and often much more. Packages are listed
    // regardless.
    let project = Project::new("project-gitignore");
    project.write(".gitignore", "*\n");
    project.write("vendor/.gitignore", "*\n");
    project.package_file("src/A.php", "<?php");

    assert_eq!(project.files(), ["src/A.php"]);
}

#[test]
fn excludes_files_marked_export_ignore() {
    let project = Project::new("export-ignore");
    project.package_file(
        ".gitattributes",
        "/tests export-ignore\n*.md export-ignore\n/.github export-ignore\n*.php text\n",
    );
    project.package_file("src/A.php", "<?php");
    project.package_file("tests/ATest.php", "<?php");
    project.package_file("tests/deep/BTest.php", "<?php");
    project.package_file("README.md", "# Lib");
    project.package_file("docs/guide.md", "# Guide");
    project.package_file(".github/ci.yml", "on: push");
    project.package_file("src/tests/Kept.php", "<?php");

    // `/tests` is anchored to the package root.
    assert_eq!(
        project.files(),
        [".gitattributes", "src/A.php", "src/tests/Kept.php"]
    );
}

#[test]
fn git_exclusions_combine() {
    let project = Project::new("git-combined");
    project.package_file(".gitignore", "*.log\n");
    project.package_file(".gitattributes", "tests export-ignore\n");
    project.package_file(".git/HEAD", "x");
    project.package_file("src/A.php", "<?php");
    project.package_file("tests/ATest.php", "<?php");
    project.package_file("a.log", "x");

    assert_eq!(
        project.files(),
        [".gitattributes", ".gitignore", "src/A.php"]
    );
}

#[test]
fn exclude_git_files_false_lists_everything() {
    let project = Project::new("git-files-kept");
    project.write("strauss.json", r#"{"exclude_git_files": false}"#);
    project.package_file(".gitignore", "*.log\n");
    project.package_file(".gitattributes", "tests export-ignore\n");
    project.package_file(".git/HEAD", "x");
    project.package_file("src/A.php", "<?php");
    project.package_file("tests/ATest.php", "<?php");
    project.package_file("a.log", "x");

    assert_eq!(
        project.files(),
        [
            ".git/HEAD",
            ".gitattributes",
            ".gitignore",
            "a.log",
            "src/A.php",
            "tests/ATest.php"
        ]
    );
}

#[cfg(unix)]
#[test]
fn skips_symlinks_inside_a_package() {
    let project = Project::new("symlinks");
    project.package_file("src/A.php", "<?php");
    project.write("outside/Secret.php", "<?php");

    let package = project.root.join("vendor/acme/lib");

    std::os::unix::fs::symlink(
        project.root.join("outside/Secret.php"),
        package.join("link.php"),
    )
    .unwrap();
    std::os::unix::fs::symlink(project.root.join("outside"), package.join("linked-dir")).unwrap();

    assert_eq!(project.files(), ["src/A.php"]);
}

#[cfg(unix)]
#[test]
fn follows_a_symlinked_package_to_its_real_directory() {
    // A path repository: `vendor/acme/lib` is a symlink to a package elsewhere.
    let project = Project::new("symlinked-package");
    project.write("packages/lib/src/A.php", "<?php");
    project.write("packages/lib/composer.json", "{}");

    fs::create_dir_all(project.root.join("vendor/acme")).unwrap();
    std::os::unix::fs::symlink(
        project.root.join("packages/lib"),
        project.root.join("vendor/acme/lib"),
    )
    .unwrap();

    let output = project.run(&["files"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let files = json["files"].as_array().unwrap();
    let real = fs::canonicalize(project.root.join("packages/lib/src/A.php")).unwrap();

    assert_eq!(files.len(), 2);

    let source = files
        .iter()
        .find(|file| file["package_relative_path"] == "src/A.php")
        .unwrap();

    // Found at its real location, but named by the package, as everywhere else.
    assert_eq!(Path::new(source["source_path"].as_str().unwrap()), real);
    assert_eq!(source["vendor_relative_path"], "acme/lib/src/A.php");
}

#[test]
fn files_outside_the_project_are_listed_by_package_name() {
    let project = Project::new("outside-vendor");
    project.write(
        "vendor/composer/installed.json",
        r#"{"packages": [{"name": "acme/lib", "type": "wordpress-plugin", "install-path": "../../web/plugins/lib"}]}"#,
    );
    project.write("web/plugins/lib/plugin.php", "<?php");

    assert_eq!(project.files(), ["plugin.php"]);
}

#[test]
fn the_packages_command_lists_packages_and_warns_on_stderr() {
    let project = Project::new("packages-command");
    project.write(
        "composer.json",
        r#"{"require": {"acme/lib": "*", "missing/pkg": "*"}}"#,
    );
    project.package_file("src/A.php", "<?php");

    let output = project.run(&["packages"]);

    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(json["packages"][0]["name"], "acme/lib");
    assert_eq!(json["packages"][0]["version"], "1.0.0");
    assert_eq!(json["skipped"][0]["name"], "missing/pkg");
    assert_eq!(json["skipped"][0]["reason"], "not installed");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        stderr.trim(),
        "warning: missing/pkg was skipped: not installed"
    );
}

#[test]
fn explains_how_to_fix_a_project_without_installed_dependencies() {
    let project = Project::new("not-installed");
    fs::remove_dir_all(project.root.join("vendor")).unwrap();

    for command in ["packages", "files"] {
        let output = project.run(&[command]);

        assert_eq!(output.status.code(), Some(1), "{command}");

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(stderr.starts_with("error: "), "{stderr}");
        assert!(
            stderr
                .trim()
                .ends_with("does not exist; run `composer install` first"),
            "{stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn the_committed_fixture_is_listed_as_expected() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/packages-fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .args(["files", "-C"])
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let files = json["files"].as_array().unwrap();

    assert_eq!(files.len(), 11);
    assert!(files.iter().all(|file| file["copy"] == true));
    assert_eq!(files[0]["vendor_relative_path"], "Acme/Dep/lib/Dep.php");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("warning: missing/pkg was skipped: not installed"));
    assert!(stderr.contains("warning: acme/missing-dir:"));
}

#[test]
fn files_not_copied_say_why() {
    let project = Project::new("not-copied");
    project.write(
        "strauss.json",
        r#"{"exclude_from_copy": {"file_patterns": ["~/tests/~"]}}"#,
    );
    project.package_file("src/A.php", "<?php");
    project.package_file("tests/ATest.php", "<?php");

    let output = project.run(&["files"]);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let files = json["files"].as_array().unwrap();

    let test = files
        .iter()
        .find(|file| file["package_relative_path"] == "tests/ATest.php")
        .unwrap();
    let source = files
        .iter()
        .find(|file| file["package_relative_path"] == "src/A.php")
        .unwrap();

    assert_eq!(test["copy"], false);
    assert_eq!(
        test["not_copied_because"],
        "it matches exclude_from_copy.file_patterns `~/tests/~`"
    );
    assert_eq!(source["copy"], true);
    assert!(source.get("not_copied_because").is_none());
}
