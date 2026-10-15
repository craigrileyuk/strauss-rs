use clap::error::ErrorKind;
use serde_json::Value;

use super::*;

fn parse_args(args: &[&str]) -> std::result::Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("strauss-rs").chain(args.iter().copied()))
}

fn config_args(args: &[&str]) -> ConfigArgs {
    let mut full = vec!["config"];
    full.extend_from_slice(args);

    match parse_args(&full).unwrap().command {
        Command::Config(args) => args,
        other => panic!("expected the config command, got {other:?}"),
    }
}

fn fixture() -> String {
    format!("{}/test/config-fixture", env!("CARGO_MANIFEST_DIR"))
}

fn rendered(args: &[&str]) -> Value {
    let mut full = vec!["-C"];
    let fixture = fixture();
    full.push(&fixture);
    full.extend_from_slice(args);

    serde_json::from_str(&render_config(&config_args(&full)).unwrap()).unwrap()
}

#[test]
fn shows_help_when_given_no_command() {
    let error = parse_args(&[]).unwrap_err();

    assert_eq!(
        error.kind(),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn project_dir_defaults_to_the_current_directory() {
    assert_eq!(config_args(&[]).project.project_dir, PathBuf::from("."));
    assert_eq!(
        config_args(&["-C", "plugin"]).project.project_dir,
        PathBuf::from("plugin")
    );
    assert_eq!(
        config_args(&["--project-dir", "plugin"])
            .project
            .project_dir,
        PathBuf::from("plugin")
    );
}

#[test]
fn no_overrides_by_default() {
    let args = config_args(&[]);

    assert_eq!(args.overrides.update_call_sites, None);
    assert_eq!(args.overrides.parallel, None);
}

#[test]
fn reads_update_call_sites_in_strauss_spelling() {
    assert_eq!(
        config_args(&["--updateCallSites"])
            .overrides
            .update_call_sites,
        Some(UpdateCallSites::FromAutoload)
    );
    assert_eq!(
        config_args(&["--updateCallSites=true"])
            .overrides
            .update_call_sites,
        Some(UpdateCallSites::FromAutoload)
    );
    assert_eq!(
        config_args(&["--updateCallSites=false"])
            .overrides
            .update_call_sites,
        Some(UpdateCallSites::Disabled)
    );
    assert_eq!(
        config_args(&["--updateCallSites=includes,templates"])
            .overrides
            .update_call_sites,
        Some(UpdateCallSites::Paths(vec![
            "includes".into(),
            "templates".into()
        ]))
    );
}

#[test]
fn update_call_sites_also_accepts_the_kebab_case_spelling() {
    assert_eq!(
        config_args(&["--update-call-sites=false"])
            .overrides
            .update_call_sites,
        Some(UpdateCallSites::Disabled)
    );
}

#[test]
fn reads_parallel() {
    assert_eq!(
        config_args(&["--parallel"]).overrides.parallel,
        Some(Parallel::Auto)
    );
    assert_eq!(
        config_args(&["--parallel=true"]).overrides.parallel,
        Some(Parallel::Auto)
    );
    assert_eq!(
        config_args(&["--parallel=false"]).overrides.parallel,
        Some(Parallel::Disabled)
    );
    assert_eq!(
        config_args(&["--parallel=1"]).overrides.parallel,
        Some(Parallel::Disabled)
    );
    assert_eq!(
        config_args(&["--parallel=4"]).overrides.parallel,
        Some(Parallel::Workers(4))
    );
}

#[test]
fn rejects_invalid_values() {
    for args in [
        &["config", "--parallel=lots"][..],
        &["config", "--parallel=-1"],
        &["config", "--updateCallSites="],
        &["config", "--updateCallSites=,"],
    ] {
        let error = parse_args(args).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::ValueValidation, "{args:?}");
    }
}

#[test]
fn values_must_be_given_with_an_equals_sign() {
    // `--parallel 4` would otherwise be ambiguous with a following argument.
    let error = parse_args(&["config", "--parallel", "4"]).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn analyse_takes_a_file() {
    match parse_args(&["analyse", "src/Plugin.php"]).unwrap().command {
        Command::Analyse { file } => assert_eq!(file, PathBuf::from("src/Plugin.php")),
        other => panic!("expected the analyse command, got {other:?}"),
    }

    assert_eq!(
        parse_args(&["analyse"]).unwrap_err().kind(),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn config_flags_are_not_accepted_by_analyse() {
    let error = parse_args(&["analyse", "a.php", "--parallel=2"]).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn renders_the_resolved_configuration() {
    let config = rendered(&[]);

    assert_eq!(config["target_directory"], "lib/prefixed");
    assert_eq!(config["namespace_prefix"], "Acme\\Plugin\\Vendor");
    // Inferred from composer.json's psr-4 key.
    assert_eq!(config["classmap_prefix"], "Acme_Plugin_");
    assert_eq!(config["constant_prefix"], "ACME_");
    assert_eq!(
        config["packages"],
        serde_json::json!(["league/flysystem", "psr/log"])
    );
    assert_eq!(config["update_call_sites"], false);
    assert_eq!(config["parallel"], true);
}

#[test]
fn command_line_options_override_the_file() {
    let config = rendered(&["--updateCallSites=includes,templates", "--parallel=4"]);

    assert_eq!(
        config["update_call_sites"],
        serde_json::json!(["includes", "templates"])
    );
    assert_eq!(config["parallel"], 4);
    // Everything else is untouched.
    assert_eq!(config["target_directory"], "lib/prefixed");
}

#[test]
fn reports_a_missing_project_directory() {
    let args = config_args(&["-C", "no/such/project"]);
    let error = render_config(&args).unwrap_err().to_string();

    assert!(
        error.starts_with("could not open project directory no/such/project:"),
        "{error}"
    );
}

#[test]
fn reports_a_project_directory_that_is_a_file() {
    let file = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let args = config_args(&["-C", &file]);
    let error = render_config(&args).unwrap_err().to_string();

    assert!(error.ends_with("is not a directory"), "{error}");
}

#[test]
fn reports_configuration_errors() {
    let invalid = format!("{}/test/config-fixture-invalid", env!("CARGO_MANIFEST_DIR"));
    let args = config_args(&["-C", &invalid]);
    let error = render_config(&args).unwrap_err().to_string();

    assert!(
        error.starts_with("strauss.json: unknown field `target_dir`"),
        "{error}"
    );
}
