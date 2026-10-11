//! Telling Composer about the packages that were deleted.
//!
//! Composer's generated autoloader (`vendor/composer/autoload_files.php` and
//! the rest) names the files of every package in `installed.json`, and requires
//! the `files` ones on every request. Once a package's directory is gone that is
//! a fatal error. So the deleted packages are removed from `installed.json`, and
//! the autoloader is regenerated from what is left.

use std::fs;
use std::process::Command;

use serde::Serialize;
use serde_json::{Value, ser::PrettyFormatter};

use crate::config::Config;

/// What to run to regenerate Composer's autoloader, when the environment doesn't say.
const COMPOSER: &str = "composer";

/// The command that runs Composer: `STRAUSS_RS_COMPOSER`, else `composer`.
pub fn composer_command() -> String {
    std::env::var("STRAUSS_RS_COMPOSER")
        .ok()
        .filter(|command| !command.trim().is_empty())
        .unwrap_or_else(|| COMPOSER.to_string())
}

/// Removes `deleted` (package names) from `vendor/composer/installed.json`, then
/// runs `composer dump-autoload` with `composer`, if given. Problems are
/// warnings: the files are already deleted, and what to run by hand is said.
pub(super) fn forget_packages(
    config: &Config,
    deleted: &[String],
    composer: Option<&str>,
    warnings: &mut Vec<String>,
) {
    if deleted.is_empty() {
        return;
    }

    let vendor = config.vendor_directory.trim_matches('/');
    let installed = config
        .project_dir
        .join(vendor)
        .join("composer/installed.json");

    let by_hand = format!(
        "remove the deleted packages from {vendor}/composer/installed.json and run `composer dump-autoload`"
    );

    match remove_from_installed(&installed, deleted) {
        Ok(()) => {}
        Err(message) => {
            warnings.push(format!(
                "{vendor}/composer/installed.json was not updated ({message}); Composer's autoloader will fail on the deleted packages until you {by_hand}"
            ));
            return;
        }
    }

    let Some(composer) = composer else {
        return;
    };

    let result = Command::new(composer)
        .args(["dump-autoload", "--no-scripts", "--no-interaction"])
        .current_dir(&config.project_dir)
        .output();

    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => warnings.push(format!(
            "`{composer} dump-autoload` failed, so Composer's autoloader still names the deleted packages: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => warnings.push(format!(
            "could not run `{composer} dump-autoload` ({error}); run it yourself, or Composer's autoloader will fail on the deleted packages"
        )),
    }
}

fn remove_from_installed(path: &std::path::Path, deleted: &[String]) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut installed: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;

    let gone = |name: &str| deleted.iter().any(|d| d.eq_ignore_ascii_case(name));

    let packages = match &mut installed {
        // Composer 2.
        Value::Object(map) => {
            if let Some(Value::Array(names)) = map.get_mut("dev-package-names") {
                names.retain(|name| !name.as_str().is_some_and(gone));
            }

            map.get_mut("packages")
        }
        // Composer 1 wrote the list itself.
        list => Some(list),
    };

    let Some(Value::Array(packages)) = packages else {
        return Err("it has no list of packages".to_string());
    };

    packages.retain(|package| {
        !package
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(gone)
    });

    // Composer writes four spaces.
    let mut out = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(b"    "));

    installed
        .serialize(&mut serializer)
        .map_err(|error| error.to_string())?;
    out.push(b'\n');

    fs::write(path, out).map_err(|error| error.to_string())
}
