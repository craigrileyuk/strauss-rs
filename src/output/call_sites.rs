//! Changing the project's own files for `update_call_sites`.

use std::fs;

use crate::edits::apply_edits;
use crate::output::error::OutputError;
use crate::output::{Input, Options};

/// Changes each file in place. Returns how many were (or would be) changed.
pub(super) fn run(input: &Input, options: &Options) -> Result<usize, OutputError> {
    // Apply everything in memory first, so a bad edit stops the run before any
    // project file has been touched.
    let mut changes = Vec::new();

    for file in &input.edits.call_sites {
        let original = fs::read_to_string(&file.source_path)
            .map_err(|error| OutputError::io("read", &file.source_path, error))?;

        let changed = apply_edits(&original, &file.edits).map_err(|error| OutputError::Edit {
            file: file.file.clone(),
            message: error.to_string(),
        })?;

        changes.push((&file.source_path, changed));
    }

    if !options.dry_run {
        for (path, changed) in &changes {
            fs::write(path, changed).map_err(|error| OutputError::io("write", path, error))?;
        }
    }

    Ok(changes.len())
}
