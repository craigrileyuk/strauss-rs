//! The state shared while walking one namespace's statements.

use std::cell::RefCell;

use php_ast::resolve::NameResolver;

use crate::analyser::model::CodeRegion;

pub(super) struct AnalysisContext<'a> {
    /// Resolves names against the namespace and the imports collected so far.
    pub(super) resolver: &'a NameResolver,
    /// The file's source, for reading the text behind a span.
    pub(super) source: &'a str,
    /// String argument regions found while walking expressions.
    pub(super) regions: RefCell<Vec<CodeRegion>>,
}
