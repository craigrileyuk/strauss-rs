//! `strauss-rs explain`, `init` and `migrate`, as the user runs them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

fn run(command: &str, project: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_strauss-rs"))
        .arg(command)
        .arg("-C")
        .arg(project)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn fixture(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "test", name].iter().collect()
}

/// An empty project directory that is removed afterwards.
struct Project(PathBuf);

impl Project {
    fn new(composer: Option<&str>) -> Project {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        let directory = std::env::temp_dir().join(format!(
            "strauss-rs-commands-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));

        fs::create_dir_all(&directory).unwrap();

        if let Some(composer) = composer {
            fs::write(directory.join("composer.json"), composer).unwrap();
        }

        Project(directory)
    }

    fn read(&self, file: &str) -> String {
        fs::read_to_string(self.0.join(file)).unwrap()
    }

    fn exists(&self, file: &str) -> bool {
        self.0.join(file).exists()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PLUGIN: &str = r#"{
    "name": "acme/my-plugin",
    "autoload": {"psr-4": {"Acme\\Plugin\\": "src/"}}
}
"#;

// ---- explain ----

#[test]
fn explains_a_symbol_wordpress_provides() {
    let output = run("explain", &fixture("presets-fixture"), &["WP_Error"]);

    assert!(output.status.success(), "{}", stderr(&output));

    let text = stdout(&output);

    assert!(text.starts_with("class WP_Error"), "{text}");
    assert!(text.contains("declared by  acme/shim"), "{text}");
    assert!(text.contains("Wordpress provides it"), "{text}");
    assert!(
        text.contains("remove `wordpress` from exclude_presets"),
        "{text}"
    );
}

#[test]
fn explains_a_name_no_package_declares_but_wordpress_does() {
    let output = run(
        "explain",
        &fixture("presets-fixture"),
        &["add_action", "--json"],
    );
    let explanation: Value = serde_json::from_slice(&output.stdout).unwrap();
    let matches = explanation["matches"].as_array().unwrap();

    // The shim's `Acme\Shim\add_action` is a different function.
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["kind"], "function");
    assert_eq!(matches[0]["provided_by"], "wordpress");
    assert_eq!(matches[0]["rename"], false);
    assert_eq!(matches[0]["packages"], serde_json::json!([]));
}

#[test]
fn explains_why_a_symbol_is_not_renamed() {
    let output = run(
        "explain",
        &fixture("symbols-fixture"),
        &["\\acme\\app\\base", "--json"],
    );

    assert!(output.status.success(), "{}", stderr(&output));

    let explanation: Value = serde_json::from_slice(&output.stdout).unwrap();
    let found = &explanation["matches"][0];

    assert_eq!(found["name"], "Acme\\App\\Base");
    assert_eq!(found["rename"], false);
    assert_eq!(found["reason_category"], "not_autoloaded");
    assert_eq!(
        found["packages"],
        serde_json::json!(["acme/app-lib", "acme/dup"])
    );
}

#[test]
fn explain_can_narrow_to_a_kind() {
    let output = run(
        "explain",
        &fixture("symbols-fixture"),
        &["Acme\\App\\Base", "--kind", "function"],
    );

    assert!(!output.status.success());
    assert!(stderr(&output).contains("no package declares `Acme\\App\\Base`"));
}

#[test]
fn explain_fails_for_an_unknown_symbol_and_suggests_similar_names() {
    let output = run("explain", &fixture("symbols-fixture"), &["Other\\Base"]);

    assert!(!output.status.success());

    let message = stderr(&output);

    assert!(
        message.contains("no package declares `Other\\Base`"),
        "{message}"
    );
    assert!(message.contains("similar names"), "{message}");
    assert!(message.contains("Acme\\App\\Base"), "{message}");
}

// ---- init ----

#[test]
fn init_writes_a_strauss_json_from_composer_json() {
    let project = Project::new(Some(PLUGIN));

    let output = run("init", &project.0, &["--no-composer-scripts"]);

    assert!(output.status.success(), "{}", stderr(&output));

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    assert_eq!(
        written,
        serde_json::json!({
            "namespace_prefix": "Acme\\Plugin\\Vendor",
            "classmap_prefix": "Acme_Plugin_",
            "include_autoloader": true
        })
    );
    // Untouched.
    assert_eq!(project.read("composer.json"), PLUGIN);
}

#[test]
fn init_output_is_a_valid_configuration() {
    let project = Project::new(Some(PLUGIN));

    run("init", &project.0, &["--no-composer-scripts"]);

    let config = run("config", &project.0, &[]);

    assert!(config.status.success(), "{}", stderr(&config));

    let config: Value = serde_json::from_slice(&config.stdout).unwrap();

    assert_eq!(config["namespace_prefix"], "Acme\\Plugin\\Vendor");
    assert_eq!(config["include_autoloader"], true);
}

#[test]
fn init_uses_the_options_it_is_given() {
    let project = Project::new(Some(PLUGIN));

    let output = run(
        "init",
        &project.0,
        &[
            "--no-composer-scripts",
            "--namespace-prefix",
            "My\\Deps",
            "--classmap-prefix",
            "My_Deps_",
            "--target-directory",
            "lib/prefixed",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    assert_eq!(written["namespace_prefix"], "My\\Deps");
    assert_eq!(written["classmap_prefix"], "My_Deps_");
    assert_eq!(written["target_directory"], "lib/prefixed");
}

#[test]
fn init_does_not_add_vendor_twice() {
    let project = Project::new(Some(
        r#"{"autoload": {"psr-4": {"Acme\\Vendor\\": "src/"}}}"#,
    ));

    run("init", &project.0, &["--no-composer-scripts"]);

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    assert_eq!(written["namespace_prefix"], "Acme\\Vendor");
}

#[test]
fn init_refuses_to_replace_strauss_json_without_force() {
    let project = Project::new(Some(PLUGIN));

    fs::write(project.0.join("strauss.json"), "{}").unwrap();

    let output = run("init", &project.0, &["--no-composer-scripts"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("already exists"));
    assert_eq!(project.read("strauss.json"), "{}");

    let output = run("init", &project.0, &["--no-composer-scripts", "--force"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(project.read("strauss.json").contains("namespace_prefix"));
}

#[test]
fn init_points_to_migrate_when_php_strauss_is_configured() {
    let project = Project::new(Some(r#"{"extra": {"strauss": {}}}"#));

    let output = run("init", &project.0, &["--no-composer-scripts"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("strauss-rs migrate"));
    assert!(!project.exists("strauss.json"));
}

#[test]
fn init_dry_run_writes_nothing() {
    let project = Project::new(Some(PLUGIN));

    let output = run("init", &project.0, &["--dry-run", "--composer-scripts"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("namespace_prefix"));
    assert!(!project.exists("strauss.json"));
    assert_eq!(project.read("composer.json"), PLUGIN);
}

#[test]
fn init_adds_the_composer_scripts_when_asked() {
    let project = Project::new(Some(PLUGIN));

    let output = run("init", &project.0, &["--composer-scripts"]);

    assert!(output.status.success(), "{}", stderr(&output));

    let composer: Value = serde_json::from_str(&project.read("composer.json")).unwrap();

    assert_eq!(
        composer["scripts"]["post-install-cmd"],
        serde_json::json!(["@prefix-namespaces"])
    );
    assert_eq!(
        composer["scripts"]["prefix-namespaces"],
        serde_json::json!(["sh bin/strauss-rs.sh prefix"])
    );
    // The rest is where it was.
    assert_eq!(composer["name"], "acme/my-plugin");
}

#[test]
fn init_rejects_an_invalid_prefix_and_writes_nothing() {
    let project = Project::new(Some(PLUGIN));

    let output = run(
        "init",
        &project.0,
        &["--no-composer-scripts", "--namespace-prefix", "Not Valid"],
    );

    assert!(!output.status.success());
    assert!(stderr(&output).contains("namespace_prefix"));
    assert!(!project.exists("strauss.json"));
}

#[test]
fn init_says_so_when_it_cannot_work_out_a_prefix() {
    let project = Project::new(None);

    let output = run("init", &project.0, &["--no-composer-scripts"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("no `namespace_prefix` could be worked out"));
    assert!(!project.read("strauss.json").contains("namespace_prefix"));
}

// ---- migrate ----

const PHP_STRAUSS: &str = r#"{
  "name": "acme/my-plugin",
  "extra": {
    "strauss": {
      "namespace_prefix": "Acme\\Vendor",
      "exclude_from_prefix": {"packages": ["a/b"]}
    },
    "other": true
  },
  "scripts": {
    "post-install-cmd": ["vendor/bin/strauss"]
  }
}
"#;

#[test]
fn migrate_moves_extra_strauss_into_strauss_json() {
    let project = Project::new(Some(PHP_STRAUSS));

    let output = run("migrate", &project.0, &[]);

    assert!(output.status.success(), "{}", stderr(&output));

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    assert_eq!(written["namespace_prefix"], "Acme\\Vendor");
    assert_eq!(
        written["exclude_from_prefix"],
        serde_json::json!({"packages": ["a/b"]})
    );

    let composer: Value = serde_json::from_str(&project.read("composer.json")).unwrap();

    assert!(composer["extra"].get("strauss").is_none());
    // Other `extra` entries stay.
    assert_eq!(composer["extra"]["other"], true);
    // Two-space indentation is kept.
    assert!(project.read("composer.json").contains("\n  \"name\""));
}

#[test]
fn migrate_keeps_class_names_by_pinning_the_classmap_prefix() {
    let project = Project::new(Some(PHP_STRAUSS));

    let output = run("migrate", &project.0, &[]);

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    // PHP Strauss would have inferred this from the package name, without the
    // trailing underscore strauss-rs adds.
    assert_eq!(written["classmap_prefix"], "Acme_My_Plugin");
    assert!(
        stderr(&output).contains("Acme_My_Plugin_"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn migrate_leaves_an_explicit_classmap_prefix_alone() {
    let project = Project::new(Some(
        r#"{"name": "acme/my-plugin", "extra": {"strauss": {"classmap_prefix": "Mine_"}}}"#,
    ));

    run("migrate", &project.0, &[]);

    let written: Value = serde_json::from_str(&project.read("strauss.json")).unwrap();

    assert_eq!(written["classmap_prefix"], "Mine_");
}

#[test]
fn migrate_warns_about_scripts_that_still_run_php_strauss() {
    let project = Project::new(Some(PHP_STRAUSS));

    let output = run("migrate", &project.0, &[]);

    assert!(
        stderr(&output).contains("`post-install-cmd` script runs `vendor/bin/strauss`"),
        "{}",
        stderr(&output)
    );
    // Not rewritten for them.
    assert!(project.read("composer.json").contains("vendor/bin/strauss"));
}

#[test]
fn migrate_writes_nothing_for_an_option_it_does_not_know() {
    let project = Project::new(Some(
        r#"{"name": "a/b", "extra": {"strauss": {"exclude_packages": ["x/y"]}}}"#,
    ));
    let before = project.read("composer.json");

    let output = run("migrate", &project.0, &[]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("exclude_packages"),
        "{}",
        stderr(&output)
    );
    assert!(!project.exists("strauss.json"));
    assert_eq!(project.read("composer.json"), before);
}

#[test]
fn migrate_dry_run_writes_nothing() {
    let project = Project::new(Some(PHP_STRAUSS));

    let output = run("migrate", &project.0, &["--dry-run"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("namespace_prefix"));
    assert!(!project.exists("strauss.json"));
    assert_eq!(project.read("composer.json"), PHP_STRAUSS);
}

#[test]
fn migrate_refuses_to_replace_strauss_json_without_force() {
    let project = Project::new(Some(PHP_STRAUSS));

    fs::write(project.0.join("strauss.json"), "{}").unwrap();

    let output = run("migrate", &project.0, &[]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("already exists"));
    assert_eq!(project.read("composer.json"), PHP_STRAUSS);

    let output = run("migrate", &project.0, &["--force"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(project.read("strauss.json").contains("Acme\\\\Vendor"));
}

#[test]
fn migrate_has_nothing_to_do_without_extra_strauss() {
    let project = Project::new(Some(PLUGIN));

    let output = run("migrate", &project.0, &[]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("nothing to migrate"));
}

#[test]
fn migrate_explains_why_extra_mozart_is_not_moved() {
    let project = Project::new(Some(r#"{"extra": {"mozart": {"dep_namespace": "A\\"}}}"#));

    let output = run("migrate", &project.0, &[]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("extra.mozart"));
}

#[test]
fn migrate_needs_a_composer_json() {
    let project = Project::new(None);

    let output = run("migrate", &project.0, &[]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("no composer.json"));
}
