//! `strauss-rs init`: write a starting `strauss.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::commands::composer_file::{ComposerFile, PREFIX_COMMAND, render_strauss_json};
use crate::config::{COMPOSER_FILE, CONFIG_FILE, Config};

/// The namespace the dependencies are put under, inside the project's own.
const VENDOR_SEGMENT: &str = "Vendor";

pub struct Options {
    pub project_dir: PathBuf,
    pub force: bool,
    pub dry_run: bool,
    pub namespace_prefix: Option<String>,
    pub classmap_prefix: Option<String>,
    pub target_directory: Option<String>,
    /// Whether to add the Composer scripts that run `prefix`.
    pub composer_scripts: bool,
}

#[derive(Debug)]
pub struct Outcome {
    /// What `strauss.json` is, or would be.
    pub strauss_json: String,
    /// What changed in `composer.json`, or would.
    pub composer_changes: Vec<String>,
    /// Things the user should know.
    pub notes: Vec<String>,
}

pub fn run(options: &Options) -> Result<Outcome, String> {
    let strauss_path = options.project_dir.join(CONFIG_FILE);
    let composer_path = options.project_dir.join(COMPOSER_FILE);

    if strauss_path.exists() && !options.force {
        return Err(format!(
            "{CONFIG_FILE} already exists; use --force to replace it"
        ));
    }

    let composer = ComposerFile::read(&composer_path)?;

    if let Some((file, _)) = &composer
        && file.extra("strauss").is_some()
    {
        return Err(format!(
            "{COMPOSER_FILE} has an `extra.strauss` section from PHP Strauss; run `strauss-rs migrate` to move it to {CONFIG_FILE}"
        ));
    }

    let composer_text = composer.as_ref().map(|(_, text)| text.as_str());

    // What would be used with no settings at all.
    let inferred = Config::from_json(&options.project_dir, None, composer_text)
        .map_err(|error| error.to_string())?;

    let mut notes = Vec::new();
    let mut settings = Map::new();

    if options
        .project_dir
        .join("bin/strauss.schema.json")
        .is_file()
    {
        settings.insert("$schema".into(), json!("./bin/strauss.schema.json"));
    }

    if let Some(directory) = &options.target_directory {
        settings.insert("target_directory".into(), json!(directory));
    }

    let namespace_prefix = options
        .namespace_prefix
        .clone()
        .or_else(|| inferred.namespace_prefix.as_deref().map(under_vendor));

    match namespace_prefix {
        Some(prefix) => {
            settings.insert("namespace_prefix".into(), json!(prefix));
        }
        None => notes.push(
            "no `namespace_prefix` could be worked out: give a `name` or `autoload` in composer.json, or use --namespace-prefix"
                .to_string(),
        ),
    }

    match options
        .classmap_prefix
        .clone()
        .or_else(|| inferred.classmap_prefix.clone())
    {
        Some(prefix) => {
            settings.insert("classmap_prefix".into(), json!(prefix));
        }
        None => notes
            .push("no `classmap_prefix` could be worked out: use --classmap-prefix".to_string()),
    }

    settings.insert("include_autoloader".into(), json!(true));

    let strauss_json = render_strauss_json(&Value::Object(settings));

    // Catches a bad --namespace-prefix and the like before anything is written.
    Config::from_json(&options.project_dir, Some(&strauss_json), composer_text)
        .map_err(|error| error.to_string())?;

    let mut composer_changes = Vec::new();
    let mut composer_after = None;

    if options.composer_scripts {
        match composer {
            Some((mut file, _)) => {
                composer_changes = file.add_prefix_scripts();
                composer_after = Some(file.render());
            }
            None => notes.push(format!(
                "there is no {COMPOSER_FILE}, so the Composer scripts weren't added"
            )),
        }
    }

    if !options.dry_run {
        write(&strauss_path, &strauss_json)?;

        if let Some(text) = composer_after.filter(|_| !composer_changes.is_empty()) {
            write(&composer_path, &text)?;
        }
    }

    if !options.composer_scripts {
        notes.push(format!(
            "to prefix on `composer install` and `composer update`, add a script that runs `{PREFIX_COMMAND}` (or run again with --composer-scripts)"
        ));
    }

    Ok(Outcome {
        strauss_json,
        composer_changes,
        notes,
    })
}

/// Dependencies go under `<your namespace>\Vendor`, not straight into your own
/// namespace where they could meet your classes.
fn under_vendor(prefix: &str) -> String {
    let last = prefix.rsplit('\\').next().unwrap_or(prefix);

    if last == VENDOR_SEGMENT {
        prefix.to_string()
    } else {
        format!("{prefix}\\{VENDOR_SEGMENT}")
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("could not write {}: {error}", path.display()))
}
