//! Strings and comments, which may name a symbol without being code the parser resolves.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{CodeRegion, FileAnalysis, RegionKind};
use php_ast::{
    Span,
    owned::{ExprKind, FunctionCallExpr},
};
use php_lexer::TokenKind;

/// Functions that take a callable, with the position of the argument. A string
/// there names a function, so it is a symbol and not just text.
const CALLABLE_STRING_FUNCTIONS: &[(&str, usize)] = &[
    ("function_exists", 0),
    ("is_callable", 0),
    ("call_user_func", 0),
    ("call_user_func_array", 0),
    ("forward_static_call", 0),
    ("forward_static_call_array", 0),
    ("register_shutdown_function", 0),
    ("register_tick_function", 0),
    ("unregister_tick_function", 0),
    ("spl_autoload_register", 0),
    ("set_error_handler", 0),
    ("set_exception_handler", 0),
    ("ob_start", 0),
    ("array_map", 0),
    ("array_filter", 1),
    ("array_walk", 1),
    ("array_walk_recursive", 1),
    ("array_reduce", 1),
    ("usort", 1),
    ("uasort", 1),
    ("uksort", 1),
    ("preg_replace_callback", 1),
    ("iterator_apply", 1),
    // Wordpress hooks, which most code that is prefixed is for.
    ("add_action", 1),
    ("add_filter", 1),
    ("remove_action", 1),
    ("remove_filter", 1),
    ("has_action", 1),
    ("has_filter", 1),
    ("register_activation_hook", 1),
    ("register_deactivation_hook", 1),
    ("register_uninstall_hook", 1),
];

/// PHP functions whose first argument is a string naming a constant.
const CONSTANT_STRING_FUNCTIONS: &[&str] = &["define", "defined"];

/// Records every string and comment in the file against the namespace it
/// appears in. This works from the token stream rather than the AST, so a
/// string is found wherever it appears, including places the reference walk
/// doesn't visit.
pub(super) fn assign_lexical_regions(
    source: &str,
    tokens: &[php_lexer::Token],
    analysis: &mut FileAnalysis,
) {
    for token in tokens.iter().copied() {
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

/// Records the argument of `define('NAME', …)`, `function_exists('name')`,
/// `add_action('hook', 'callback')` and similar that names a symbol in a string.
pub(super) fn collect_string_arg_region(call: &FunctionCallExpr, context: &AnalysisContext) {
    let ExprKind::Identifier(name) = &call.name.kind else {
        return;
    };

    let name = name.strip_prefix('\\').unwrap_or(name);

    let callable = CALLABLE_STRING_FUNCTIONS
        .iter()
        .find(|(function, _)| function.eq_ignore_ascii_case(name));

    let (kind, position) = if let Some((_, position)) = callable {
        (RegionKind::CallableStringArg, *position)
    } else if CONSTANT_STRING_FUNCTIONS
        .iter()
        .any(|function| function.eq_ignore_ascii_case(name))
    {
        (RegionKind::DefineStringArg, 0)
    } else {
        return;
    };

    // Positions count unnamed arguments; a named one may not be the one that
    // names the symbol, and shifts the rest.
    if call
        .args
        .iter()
        .take(position + 1)
        .any(|arg| arg.name.is_some())
    {
        return;
    }

    let Some(arg) = call.args.get(position) else {
        return;
    };

    let Some(value) = &arg.value else {
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
