//! The command line interface.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use php_rs_parser::parse;

use crate::analyser;
use crate::config::{Config, Parallel, UpdateCallSites};
use crate::packages::{self, PackageSet};

#[derive(Debug, Parser)]
#[command(
    name = "strauss-rs",
    version,
    about = "Prefix the namespaces of a WordPress plugin or theme's dependencies",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the resolved configuration as JSON.
    ///
    /// Every option is shown with its default or inferred value filled in, in
    /// the form `strauss.json` takes, after applying any options given here.
    Config(ConfigArgs),

    /// List the packages to process as JSON: those in `packages`, and
    /// everything they require.
    ///
    /// Reads `vendor/composer/installed.json`, so run `composer install`
    /// first. Warnings go to stderr.
    Packages(ProjectArgs),

    /// List the files of those packages as JSON, and which would be copied.
    ///
    /// Every file in a package is listed, except those git would leave out of
    /// an archive: `.git`, `.gitignore`d files and `export-ignore` files.
    Files(ProjectArgs),

    /// Analyse a PHP file and print the symbols it declares and refers to.
    Analyse {
        /// The PHP file to analyse.
        file: PathBuf,
    },
}

/// Finds the project. Shared by every command that reads `strauss.json`.
#[derive(Debug, Args)]
struct ProjectArgs {
    /// The plugin or theme root, containing `strauss.json` and `composer.json`.
    #[arg(short = 'C', long, value_name = "DIR", default_value = ".")]
    project_dir: PathBuf,
}

/// A project, and options that override its configuration.
#[derive(Debug, Args)]
struct ConfigArgs {
    #[command(flatten)]
    project: ProjectArgs,

    #[command(flatten)]
    overrides: Overrides,
}

/// Options that take precedence over `strauss.json`.
#[derive(Debug, Args)]
struct Overrides {
    /// Which project files to update where they use prefixed symbols,
    /// overriding `update_call_sites`: `true` for the files in the project's
    /// `autoload`, `false` for none, or a comma-separated list of files and
    /// directories.
    #[arg(
        long = "updateCallSites",
        alias = "update-call-sites",
        value_name = "true|false|PATHS",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true"
    )]
    update_call_sites: Option<UpdateCallSites>,

    /// How many workers analyse files, overriding `parallel`: `true` for one
    /// per spare CPU, `false` or `1` for a single process, or a number to cap
    /// the workers.
    #[arg(
        long,
        value_name = "true|false|N",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true"
    )]
    parallel: Option<Parallel>,
}

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn run() -> ExitCode {
    let cli = Cli::parse();

    match execute(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> Result {
    match command {
        Command::Config(args) => {
            println!("{}", render_config(&args)?);

            Ok(())
        }

        Command::Packages(args) => {
            println!("{}", render_packages(&args)?);

            Ok(())
        }

        Command::Files(args) => {
            println!("{}", render_files(&args)?);

            Ok(())
        }

        Command::Analyse { file } => analyse_file(&file),
    }
}

/// The configuration as `strauss.json` would spell it out.
fn render_config(args: &ConfigArgs) -> Result<String> {
    let mut config = load_config(&args.project)?;

    if let Some(update_call_sites) = &args.overrides.update_call_sites {
        config.update_call_sites = update_call_sites.clone();
    }

    if let Some(parallel) = args.overrides.parallel {
        config.parallel = parallel;
    }

    Ok(serde_json::to_string_pretty(&config)?)
}

fn render_packages(args: &ProjectArgs) -> Result<String> {
    let config = load_config(args)?;
    let set = packages::enumerate_packages(&config)?;

    report_problems(&set);

    Ok(serde_json::to_string_pretty(&set)?)
}

fn render_files(args: &ProjectArgs) -> Result<String> {
    let config = load_config(args)?;
    let set = packages::enumerate_packages(&config)?;
    let files = packages::enumerate_files(&config, &set)?;

    report_problems(&set);

    Ok(serde_json::to_string_pretty(
        &serde_json::json!({ "files": files }),
    )?)
}

/// Packages that were required but left out, or found with a problem, go to
/// stderr so the JSON on stdout stays clean.
fn report_problems(set: &PackageSet) {
    for skipped in &set.skipped {
        eprintln!("warning: {} was skipped: {}", skipped.name, skipped.reason);
    }

    for warning in &set.warnings {
        eprintln!("warning: {}: {}", warning.package, warning.message);
    }
}

/// Reads the project's configuration.
fn load_config(args: &ProjectArgs) -> Result<Config> {
    let project_dir = fs::canonicalize(&args.project_dir).map_err(|error| {
        format!(
            "could not open project directory {}: {error}",
            args.project_dir.display()
        )
    })?;

    if !project_dir.is_dir() {
        return Err(format!("{} is not a directory", project_dir.display()).into());
    }

    Ok(Config::load(&project_dir)?)
}

fn analyse_file(file: &Path) -> Result {
    let source = fs::read_to_string(file)
        .map_err(|error| format!("could not read {}: {error}", file.display()))?;

    let result = parse(&source);

    for error in &result.errors {
        eprintln!("Parse error: {error}");
    }

    let analysis = analyser::analyse(&source, &result.program);

    println!("{analysis:#?}");

    Ok(())
}

#[cfg(test)]
mod tests;
