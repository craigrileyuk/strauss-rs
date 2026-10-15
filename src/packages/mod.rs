//! Finding the packages to process, and their files.

#![allow(dead_code)]

mod error;
mod files;
mod gitattributes;
mod installed;
mod model;
mod resolve;

// The module's API; the CLI uses some of it so far.
#[allow(unused_imports)]
pub use error::PackagesError;
#[allow(unused_imports)]
pub use files::{enumerate_files, is_target_the_vendor_directory};
#[allow(unused_imports)]
pub use model::*;
#[allow(unused_imports)]
pub use resolve::is_platform_package;

use crate::config::Config;
use crate::packages::installed::InstalledIndex;

/// The packages the configuration names, and everything they require, found
/// through `vendor/composer/installed.json`.
pub fn enumerate_packages(config: &Config) -> Result<PackageSet, PackagesError> {
    let vendor_dir = config.project_dir.join(&config.vendor_directory);
    let index = InstalledIndex::load(&vendor_dir)?;

    Ok(resolve::resolve(config, &index))
}

#[cfg(test)]
mod tests;
