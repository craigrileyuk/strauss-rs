//! The symbols a file declares.

use crate::analyser::model::{DeclaredSymbol, SymbolKind};
use php_ast::owned::{Expr, ExprKind, Stmt, StmtKind};

/// Records every symbol declared by `stmts`, including declarations nested
/// in blocks, conditionals, `try` statements and function bodies.
pub(super) fn collect_declarations(
    stmts: &[Stmt],
    namespace: Option<&str>,
    symbols: &mut Vec<DeclaredSymbol>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Class(class) => {
                if let Some(Some(name)) = &class.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Class);
                }
            }

            StmtKind::Interface(interface) => {
                if let Some(name) = &interface.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Interface);
                }
            }

            StmtKind::Trait(trait_decl) => {
                if let Some(name) = &trait_decl.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Trait);
                }
            }

            StmtKind::Enum(enum_decl) => {
                if let Some(name) = &enum_decl.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Enum);
                }
            }

            StmtKind::Function(function) => {
                if let Some(name) = &function.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Function);
                }

                collect_declarations(&function.body.stmts, namespace, symbols);
            }

            // `const FOO = 1;` is namespaced like any other declaration.
            StmtKind::Const(items) => {
                for item in items.iter() {
                    if let Some(name) = &item.name {
                        push_symbol(symbols, name, namespace, SymbolKind::Constant);
                    }
                }
            }

            // `define('FOO', 1);`
            StmtKind::Expression(expr) => {
                if let Some(name) = defined_constant_name(expr) {
                    symbols.push(DeclaredSymbol {
                        name,
                        kind: SymbolKind::Constant,
                    });
                }
            }

            StmtKind::Block(block) => {
                collect_declarations(&block.stmts, namespace, symbols);
            }

            StmtKind::If(if_stmt) => {
                collect_declarations(
                    std::slice::from_ref(if_stmt.then_branch.as_ref()),
                    namespace,
                    symbols,
                );

                for branch in if_stmt.elseif_branches.iter() {
                    collect_declarations(std::slice::from_ref(&branch.body), namespace, symbols);
                }

                if let Some(branch) = &if_stmt.else_branch {
                    collect_declarations(std::slice::from_ref(branch.as_ref()), namespace, symbols);
                }
            }

            StmtKind::TryCatch(try_catch) => {
                collect_declarations(&try_catch.body.stmts, namespace, symbols);

                for catch in try_catch.catches.iter() {
                    collect_declarations(&catch.body.stmts, namespace, symbols);
                }

                if let Some(finally) = &try_catch.finally {
                    collect_declarations(&finally.stmts, namespace, symbols);
                }
            }

            StmtKind::Declare(declare) => {
                if let Some(body) = &declare.body {
                    collect_declarations(std::slice::from_ref(body.as_ref()), namespace, symbols);
                }
            }

            _ => {}
        }
    }
}

fn push_symbol(
    symbols: &mut Vec<DeclaredSymbol>,
    name: &str,
    namespace: Option<&str>,
    kind: SymbolKind,
) {
    symbols.push(DeclaredSymbol {
        name: qualify_declared_name(name, namespace),
        kind,
    });
}

/// The constant name declared by a `define('NAME', ...)` call.
///
/// The name is a string, so it isn't affected by the current namespace: it
/// is global unless the string itself contains a namespace.
fn defined_constant_name(expr: &Expr) -> Option<String> {
    let ExprKind::FunctionCall(call) = &expr.kind else {
        return None;
    };

    let ExprKind::Identifier(function) = &call.name.kind else {
        return None;
    };

    if !function
        .trim_start_matches('\\')
        .eq_ignore_ascii_case("define")
    {
        return None;
    }

    match &call.args.first()?.value.as_ref()?.kind {
        ExprKind::String(name) => Some(name.trim_start_matches('\\').to_string()),
        _ => None,
    }
}

fn qualify_declared_name(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) if !namespace.is_empty() => {
            format!("{namespace}\\{name}")
        }
        _ => name.to_string(),
    }
}
