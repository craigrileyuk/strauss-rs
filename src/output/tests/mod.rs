//! Tests for writing the output.
//!
//! The ones that write work on a copy of a fixture in a scratch directory, since
//! writing changes files; `header.rs` is only text.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::config::{Config, Parallel};
use crate::edits;
use crate::packages::{enumerate_files, enumerate_packages};
use crate::rename;
use crate::symbols::scan;

mod clean;
mod follow;
mod header;
mod write;

pub(super) const DATE: &str = "05-October-2026";

/// A directory that is removed when the test ends.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A copy of `test/<name>` to change.
pub(super) fn scratch_copy(name: &str) -> Scratch {
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    let root = std::env::temp_dir().join(format!(
        "strauss-rs-output-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join(name);

    copy_directory(&source, &root);

    Scratch(fs::canonicalize(root).unwrap())
}

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();

    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());

        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Everything worked out for a project, ready to be written.
pub(super) struct Pipeline {
    pub config: Config,
    pub packages: PackageSet,
    pub files: Vec<PackageFile>,
    pub rename: rename::RenamePlan,
    pub edits: edits::EditPlan,
}

pub(super) fn pipeline(root: &Path, change: impl FnOnce(&mut Config)) -> Pipeline {
    let mut config = Config::load(root).unwrap();
    config.parallel = Parallel::Disabled;

    change(&mut config);

    let packages = enumerate_packages(&config).unwrap();
    let mut files = enumerate_files(&config, &packages).unwrap();
    let table = scan(&config, &files);

    rename::exclude_namespaces_from_copy(&config, &mut files, &table);

    let rename = rename::plan(&config, &packages, &files, &table);
    let edits = edits::plan(&config, &files, &table, &rename);

    Pipeline {
        config,
        packages,
        files,
        rename,
        edits,
    }
}

impl Pipeline {
    pub fn input(&self) -> Input<'_> {
        Input {
            config: &self.config,
            packages: &self.packages,
            files: &self.files,
            rename: &self.rename,
            edits: &self.edits,
        }
    }

    pub fn write(&self, dry_run: bool) -> Report {
        let mut options = Options::new(&self.config, dry_run, DATE.to_string());

        // Not the Composer of whoever runs the tests.
        options.composer = None;

        write(&self.input(), &options).unwrap()
    }
}

/// The fixture, written: the scratch copy and the report.
pub(super) fn written_fixture(change: impl FnOnce(&mut Config)) -> (Scratch, Pipeline, Report) {
    let scratch = scratch_copy("edit-fixture");
    let pipeline = pipeline(scratch.path(), change);
    let report = pipeline.write(false);

    (scratch, pipeline, report)
}

pub(super) fn read(scratch: &Scratch, path: &str) -> String {
    fs::read_to_string(scratch.path().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

pub(super) fn exists(scratch: &Scratch, path: &str) -> bool {
    scratch.path().join(path).exists()
}
