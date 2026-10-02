//! Resolving the names written in code against the current namespace and imports.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{Reference, ReferenceKind};
use php_ast::{
    Span,
    ast::NameKind,
    owned::{Expr, ExprKind},
    resolve::{NameContext, ResolvedName},
};

/// Resolves a name written as a plain identifier (`Foo`, `Bar\Baz`, `\Foo`,
/// `namespace\Foo`) against the current namespace and imports.
pub(super) fn resolve_identifier(
    name: &str,
    name_context: NameContext,
    context: &AnalysisContext,
) -> ResolvedName {
    let (kind, rest) = if let Some(rest) = name.strip_prefix('\\') {
        (NameKind::FullyQualified, rest)
    } else if name
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("namespace\\"))
    {
        (NameKind::Relative, &name[10..])
    } else if name.contains('\\') {
        (NameKind::Qualified, name)
    } else {
        (NameKind::Unqualified, name)
    };

    let parts: Vec<&str> = rest.split('\\').collect();

    context.resolver.resolve(name_context, kind, &parts)
}

pub(super) fn build_reference(
    resolved: ResolvedName,
    span: Span,
    kind: ReferenceKind,
) -> Option<Reference> {
    let (name, fallback) = match resolved {
        ResolvedName::Fqn(name) => (name, None),

        // Unqualified functions/constants: PHP tries the namespaced name
        // first, then falls back to the global one at runtime.
        ResolvedName::Fallback { namespaced, global } => (namespaced, Some(global)),

        // self/static/parent and error-recovery names aren't symbols.
        ResolvedName::Special(_) | ResolvedName::Error => return None,
    };

    Some(Reference {
        name,
        fallback,
        span,
        kind,
    })
}

pub(super) fn resolve_class_expr_name(expr: &Expr, context: &AnalysisContext) -> Option<String> {
    match &expr.kind {
        ExprKind::Identifier(name) => match resolve_identifier(name, NameContext::Class, context) {
            ResolvedName::Fqn(name) => Some(name),
            _ => None,
        },

        _ => None,
    }
}
