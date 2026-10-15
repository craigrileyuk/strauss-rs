//! `strauss-rs migrate`: move PHP Strauss' `extra.strauss` into `strauss.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::composer_file::{ComposerFile, PREFIX_COMMAND, render_strauss_json};
use crate::config::{COMPOSER_FILE, CONFIG_FILE, Config, php_strauss_classmap_prefix};

pub struct Options {
    pub project_dir: PathBuf,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct Outcome {
    pub strauss_json: String,
    /// Things the user should know or do.
    pub notes: Vec<String>,
}

pub fn run(options: &Options) -> Result<Outcome, String> {
    let strauss_path = options.project_dir.join(CONFIG_FILE);
    let composer_path = options.project_dir.join(COMPOSER_FILE);

    let Some((mut composer, composer_text)) = ComposerFile::read(&composer_path)? else {
        return Err(format!(
            "there is no {COMPOSER_FILE} in {}",
            options.project_dir.display()
        ));
    };

    let Some(extra) = composer.extra("strauss").cloned() else {
        return Err(if composer.extra("mozart").is_some() {
            format!(
                "{COMPOSER_FILE} has `extra.mozart`, whose option names strauss-rs doesn't support; write a {CONFIG_FILE} with the current names (see the README) and delete `extra.mozart`"
            )
        } else {
            format!("nothing to migrate: {COMPOSER_FILE} has no `extra.strauss`")
        });
    };

    let Value::Object(mut settings) = extra else {
        return Err("`extra.strauss` must be an object".to_string());
    };

    if strauss_path.exists() && !options.force {
        return Err(format!(
            "{CONFIG_FILE} already exists; use --force to replace it"
        ));
    }

    let mut notes = Vec::new();

    // PHP Strauss leaves the underscore off a prefix inferred from the package
    // name. Say so, or class names change.
    if !settings.contains_key("classmap_prefix")
        && let Some(prefix) = php_strauss_classmap_prefix(&composer_text)
    {
        notes.push(format!(
            "set `classmap_prefix` to `{prefix}`, which PHP Strauss inferred, so your class names stay as they were (strauss-rs would have used `{prefix}_`)"
        ));
        settings.insert("classmap_prefix".to_string(), Value::String(prefix));
    }

    if !settings.contains_key("$schema")
        && options
            .project_dir
            .join("bin/strauss.schema.json")
            .is_file()
    {
        settings.shift_insert(
            0,
            "$schema".to_string(),
            Value::String("./bin/strauss.schema.json".to_string()),
        );
    }

    let strauss_json = render_strauss_json(&Value::Object(settings));

    composer.remove_extra_strauss();

    // Unknown or misspelt options are an error here, as they will be when
    // `prefix` runs, so nothing is written half-done.
    Config::from_json(
        &options.project_dir,
        Some(&strauss_json),
        Some(&composer.render()),
    )
    .map_err(|error| format!("`extra.strauss` can't be used as it is: {error}"))?;

    for (script, command) in composer.php_strauss_commands() {
        notes.push(format!(
            "the `{script}` script runs `{command}`; change it to run `{PREFIX_COMMAND}`"
        ));
    }

    if !options.dry_run {
        write(&strauss_path, &strauss_json)?;
        write(&composer_path, &composer.render())?;
    }

    Ok(Outcome {
        strauss_json,
        notes,
    })
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("could not write {}: {error}", path.display()))
}
