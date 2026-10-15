//! `include-autoloader`: making Composer's `vendor/autoload.php` load the
//! prefixed packages' autoloader too.

use std::fs;

use crate::config::Config;
use crate::output::error::OutputError;

/// Marks the line, so running the command again changes nothing.
const MARKER: &str = "// strauss-rs: load the prefixed packages";

#[derive(Debug, PartialEq, Eq)]
pub enum Included {
    /// The line was added.
    Added,
    /// It was there already.
    AlreadyIncluded,
}

#[derive(Debug)]
pub enum IncludeError {
    Output(OutputError),
    /// `vendor/autoload.php` isn't one Composer wrote, so where to add the line is not known.
    Unrecognised(String),
}

impl std::fmt::Display for IncludeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IncludeError::Output(error) => error.fmt(formatter),
            IncludeError::Unrecognised(path) => write!(
                formatter,
                "{path} has no `return ComposerAutoloaderInit…::getLoader();` line to add the include before; has `composer install` run?"
            ),
        }
    }
}

impl std::error::Error for IncludeError {}

/// Adds `require_once` of the target directory's `autoload.php` to the vendor
/// directory's `autoload.php`, before it returns the Composer loader.
pub fn include_autoloader(config: &Config, dry_run: bool) -> Result<Included, IncludeError> {
    let vendor = config.vendor_directory.trim_matches('/');
    let target = config.target_directory.trim_matches('/');
    let path = config.project_dir.join(vendor).join("autoload.php");

    let source = fs::read_to_string(&path)
        .map_err(|error| IncludeError::Output(OutputError::io("read", &path, error)))?;

    if source.contains(MARKER) {
        return Ok(Included::AlreadyIncluded);
    }

    let (before, after) = source
        .split_once("\nreturn ComposerAutoloaderInit")
        .ok_or_else(|| IncludeError::Unrecognised(format!("{vendor}/autoload.php")))?;

    // From the vendor directory up to the project, then down to the target.
    let up = "../".repeat(vendor.split('/').filter(|part| !part.is_empty()).count());

    let changed = format!(
        "{before}\n{MARKER}\nrequire_once __DIR__ . '/{up}{target}/autoload.php';\n\nreturn ComposerAutoloaderInit{after}"
    );

    if !dry_run {
        fs::write(&path, changed)
            .map_err(|error| IncludeError::Output(OutputError::io("write", &path, error)))?;
    }

    Ok(Included::Added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(test: &str, autoload: &str) -> Config {
        let dir = std::env::temp_dir().join(format!("strauss-rs-include-{test}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("vendor")).unwrap();
        fs::write(dir.join("composer.json"), r#"{"name":"a/b"}"#).unwrap();
        fs::write(dir.join("vendor/autoload.php"), autoload).unwrap();

        Config::load(&fs::canonicalize(dir).unwrap()).unwrap()
    }

    const COMPOSER: &str = "<?php\n\nrequire_once __DIR__ . '/composer/autoload_real.php';\n\nreturn ComposerAutoloaderInitabc::getLoader();\n";

    #[test]
    fn adds_the_include_before_the_loader_is_returned() {
        let config = project("adds", COMPOSER);

        assert_eq!(include_autoloader(&config, false).unwrap(), Included::Added);

        let changed = fs::read_to_string(config.project_dir.join("vendor/autoload.php")).unwrap();

        assert!(changed.contains(
            "require_once __DIR__ . '/../vendor-prefixed/autoload.php';\n\nreturn ComposerAutoloaderInitabc::getLoader();"
        ), "{changed}");
    }

    #[test]
    fn running_it_again_changes_nothing() {
        let config = project("again", COMPOSER);

        include_autoloader(&config, false).unwrap();
        let once = fs::read_to_string(config.project_dir.join("vendor/autoload.php")).unwrap();

        assert_eq!(
            include_autoloader(&config, false).unwrap(),
            Included::AlreadyIncluded
        );
        assert_eq!(
            fs::read_to_string(config.project_dir.join("vendor/autoload.php")).unwrap(),
            once
        );
    }

    #[test]
    fn a_dry_run_writes_nothing() {
        let config = project("dry", COMPOSER);

        assert_eq!(include_autoloader(&config, true).unwrap(), Included::Added);
        assert_eq!(
            fs::read_to_string(config.project_dir.join("vendor/autoload.php")).unwrap(),
            COMPOSER
        );
    }

    #[test]
    fn a_file_composer_did_not_write_is_an_error() {
        let config = project("foreign", "<?php echo 1;\n");

        assert!(matches!(
            include_autoloader(&config, false),
            Err(IncludeError::Unrecognised(_))
        ));
    }
}
