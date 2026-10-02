//! Tests for the analyser, grouped by what they exercise. Each reads PHP source and checks
//! the `FileAnalysis` produced.

use super::*;
use php_rs_parser::parse;

mod attributes;
mod declarations;
mod names;
mod namespaces;
mod references;
mod regions;

pub(super) fn analyse_php(source: &str) -> FileAnalysis {
    let result = parse(source);

    assert!(
        result.errors.is_empty(),
        "PHP parse errors: {:?}",
        result.errors
    );

    analyse(source, &result.program)
}

pub(super) fn reference_names(analysis: &FileAnalysis) -> Vec<&str> {
    analysis.namespaces[0]
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect()
}
