//! Strings and comments, which may name a symbol without being code the parser resolves.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{CodeRegion, FileAnalysis, RegionKind};
use php_ast::{
    Span,
    owned::{ExprKind, FunctionCallExpr},
};
use php_lexer::TokenKind;

/// PHP functions whose first argument is a string naming a function.
const CALLABLE_STRING_FUNCTIONS: &[&str] = &[
    "function_exists",
    "call_user_func",
    "call_user_func_array",
    "forward_static_call",
    "forward_static_call_array",
    "register_shutdown_function",
    "register_tick_function",
    "unregister_tick_function",
];

/// PHP functions whose first argument is a string naming a constant.
const CONSTANT_STRING_FUNCTIONS: &[&str] = &["define", "defined"];

/// Records every string and comment in the file against the namespace it
/// appears in. This works from the token stream rather than the AST, so a
/// string is found wherever it appears, including places the reference walk
/// doesn't visit.
pub(super) fn assign_lexical_regions(source: &str, analysis: &mut FileAnalysis) {
    let (tokens, _errors) = php_lexer::lex_all(source);

    for token in tokens {
        let kind = match token.kind {
            TokenKind::SingleQuotedString
            | TokenKind::DoubleQuotedString
            | TokenKind::Heredoc
            | TokenKind::Nowdoc
            | TokenKind::BacktickString => RegionKind::String,

            TokenKind::LineComment | TokenKind::HashComment | TokenKind::BlockComment => {
                RegionKind::Comment
            }

            TokenKind::DocComment => RegionKind::DocComment,

            _ => continue,
        };

        let Some(text) = source.get(token.span.start as usize..token.span.end as usize) else {
            continue;
        };

        let Some(namespace) = analysis.namespaces.iter_mut().find(|namespace| {
            namespace.span.start <= token.span.start && token.span.start < namespace.span.end
        }) else {
            continue;
        };

        namespace.regions.push(CodeRegion {
            kind,
            span: token.span,
            text: text.to_string(),
            value: None,
        });
    }

    for namespace in &mut analysis.namespaces {
        namespace.regions.sort_by_key(|region| region.span.start);
    }
}

/// Records the first argument of `define('NAME', …)`, `function_exists('name')`
/// and similar, which name a symbol in a string.
pub(super) fn collect_string_arg_region(call: &FunctionCallExpr, context: &AnalysisContext) {
    let ExprKind::Identifier(name) = &call.name.kind else {
        return;
    };

    let name = name.strip_prefix('\\').unwrap_or(name);

    let kind = if CALLABLE_STRING_FUNCTIONS
        .iter()
        .any(|function| function.eq_ignore_ascii_case(name))
    {
        RegionKind::CallableStringArg
    } else if CONSTANT_STRING_FUNCTIONS
        .iter()
        .any(|function| function.eq_ignore_ascii_case(name))
    {
        RegionKind::DefineStringArg
    } else {
        return;
    };

    let Some(first) = call.args.first() else {
        return;
    };

    // A named first argument may not be the one that names the symbol.
    if first.name.is_some() {
        return;
    }

    let Some(value) = &first.value else {
        return;
    };

    let ExprKind::String(string) = &value.kind else {
        return;
    };

    let Some(text) = context
        .source
        .get(value.span.start as usize..value.span.end as usize)
    else {
        return;
    };

    // Only plain quoted strings. The quotes are excluded from the region so
    // that a replacement can't change them.
    let bytes = text.as_bytes();

    if bytes.len() < 2 || !matches!(bytes[0], b'\'' | b'"') || bytes[bytes.len() - 1] != bytes[0] {
        return;
    }

    context.regions.borrow_mut().push(CodeRegion {
        kind,
        span: Span::new(value.span.start + 1, value.span.end - 1),
        text: text[1..text.len() - 1].to_string(),
        value: Some(string.to_string()),
    });
}
