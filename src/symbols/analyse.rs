//! Reading and analysing PHP files, in parallel.

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use php_rs_parser::parse;
use rayon::prelude::*;

use crate::analyser::{self, FileAnalysis};
use crate::parallel;

/// A file that was analysed.
pub struct Analysed {
    pub analysis: FileAnalysis,

    /// The file isn't valid UTF-8, so it was analysed with the invalid bytes
    /// replaced. Its symbols are right, but its spans are offsets into the
    /// altered text, not the file.
    pub lossy: bool,
}

/// Analyses each file on `workers` threads, returning the results in the
/// order of `paths`.
pub fn analyse_files(paths: &[&Path], workers: usize) -> Vec<Result<Analysed, String>> {
    let stage = crate::progress::stage("Analysing", paths.len());

    parallel::pool(workers).install(|| {
        paths
            .par_iter()
            .map(|path| {
                let analysed = analyse_file(path);

                stage.tick();

                analysed
            })
            .collect()
    })
}

/// Analyses one file. A file that can't be read or parsed is an error message,
/// not a failure of the whole run: templates and files for newer syntax than
/// the parser knows turn up in packages.
fn analyse_file(path: &Path) -> Result<Analysed, String> {
    let bytes = fs::read(path).map_err(|error| format!("could not read the file: {error}"))?;

    // Old files in other encodings turn up in packages. Their symbols still
    // matter, but edits are made by byte offset into the text, so text that
    // had to be altered to be read as UTF-8 can't be edited safely.
    let (source, lossy) = match String::from_utf8(bytes) {
        Ok(source) => (source, false),
        Err(error) => (String::from_utf8_lossy(error.as_bytes()).into_owned(), true),
    };

    // A bug in the analyser, or a file that hits one, shouldn't stop the other
    // files being analysed.
    panic::catch_unwind(AssertUnwindSafe(|| {
        let result = parse(&source);

        if let Some(error) = result.errors.first() {
            let more = result.errors.len() - 1;

            return Err(match more {
                0 => format!("parse error: {error}"),
                more => format!("parse error: {error} (and {more} more)"),
            });
        }

        Ok(Analysed {
            analysis: analyser::analyse(&source, &result.program),
            lossy,
        })
    }))
    .unwrap_or_else(|_| Err("the analyser failed on this file".to_string()))
}
