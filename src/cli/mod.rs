//! The command line interface.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use php_rs_parser::parse;

use crate::analyser;
use crate::config::{Config, Parallel, UpdateCallSites};
use crate::packages::{self, PackageSet};
use crate::rename::{self, PlannedSymbol};
use crate::symbols::{self, SymbolTable};

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
    Files(ScanArgs),

    /// List the symbols each package declares as JSON: namespaces, classes,
    /// interfaces, traits, enums, functions and constants.
    ///
    /// Analyses every `.php` file of every package, so it can take a moment.
    /// PHP's own symbols, which polyfills redeclare, are listed separately.
    /// Files that can't be analysed are reported on stderr.
    Symbols(ScanArgs),

    /// Show which symbols would be renamed, to what, and why not the others, as
    /// JSON.
    ///
    /// Analyses every package like `symbols` does. Without `--package` this is
    /// every symbol, which for a large project is a lot; `summary` has the
    /// counts.
    Plan(PlanArgs),

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

/// How many workers analyse files. The commands that analyse files take it.
#[derive(Debug, Args)]
struct ParallelArgs {
    /// How many workers analyse files, overriding `parallel`: `auto` (or
    /// `true`) for one per available CPU, `false` or `1` for a single worker,
    /// or a number to cap the workers.
    #[arg(
        long,
        value_name = "auto|false|N",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true"
    )]
    parallel: Option<Parallel>,
}

/// A project, and how to analyse its files.
#[derive(Debug, Args)]
struct ScanArgs {
    #[command(flatten)]
    project: ProjectArgs,

    #[command(flatten)]
    parallel: ParallelArgs,
}

/// A project, how to analyse its files, and which symbols to show.
#[derive(Debug, Args)]
struct PlanArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Only show the symbols this package declares. Can be given more than
    /// once.
    #[arg(long = "package", value_name = "NAME")]
    packages: Vec<String>,
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

    #[command(flatten)]
    parallel: ParallelArgs,
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

        Command::Symbols(args) => {
            println!("{}", render_symbols(&args)?);

            Ok(())
        }

        Command::Plan(args) => {
            println!("{}", render_plan(&args)?);

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

    if let Some(parallel) = args.overrides.parallel.parallel {
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

fn render_files(args: &ScanArgs) -> Result<String> {
    let mut config = load_config(&args.project)?;

    if let Some(parallel) = args.parallel.parallel {
        config.parallel = parallel;
    }

    let set = packages::enumerate_packages(&config)?;
    let files = packages::enumerate_files(&config, &set)?;

    report_problems(&set);

    Ok(serde_json::to_string_pretty(
        &serde_json::json!({ "files": files }),
    )?)
}

fn render_symbols(args: &ScanArgs) -> Result<String> {
    let mut config = load_config(&args.project)?;

    if let Some(parallel) = args.parallel.parallel {
        config.parallel = parallel;
    }

    let set = packages::enumerate_packages(&config)?;
    let files = packages::enumerate_files(&config, &set)?;
    let table = symbols::scan(&config, &files);

    report_problems(&set);
    report_scan_warnings(&table);

    Ok(serde_json::to_string_pretty(&table.report())?)
}

/// What `strauss-rs plan` prints.
#[derive(serde::Serialize)]
struct PlanReport<'a> {
    packages_to_prefix: &'a [String],
    summary: rename::PlanSummary,
    files_excluded_from_prefix: &'a [rename::ExcludedFile],
    symbols: Vec<&'a PlannedSymbol>,
}

fn render_plan(args: &PlanArgs) -> Result<String> {
    let mut config = load_config(&args.scan.project)?;

    if let Some(parallel) = args.scan.parallel.parallel {
        config.parallel = parallel;
    }

    let set = packages::enumerate_packages(&config)?;
    let mut files = packages::enumerate_files(&config, &set)?;
    let table = symbols::scan(&config, &files);

    rename::exclude_namespaces_from_copy(&config, &mut files, &table);

    let plan = rename::plan(&config, &set, &files, &table);

    report_problems(&set);
    report_scan_warnings(&table);

    let selected: Vec<&PlannedSymbol> = plan
        .symbols
        .iter()
        .filter(|symbol| {
            args.packages.is_empty()
                || symbol.packages.iter().any(|package| {
                    args.packages
                        .iter()
                        .any(|wanted| wanted.eq_ignore_ascii_case(package))
                })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&PlanReport {
        packages_to_prefix: &plan.packages_to_prefix,
        summary: plan.summary(),
        files_excluded_from_prefix: &plan.files_excluded_from_prefix,
        symbols: selected,
    })?)
}

fn report_scan_warnings(table: &SymbolTable) {
    for warning in &table.warnings {
        eprintln!("warning: {}: {}", warning.file, warning.message);
    }
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
