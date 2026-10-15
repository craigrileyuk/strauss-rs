//! The symbols a file declares.

use crate::analyser::model::{DeclaredSymbol, SymbolKind};
use php_ast::owned::{Expr, ExprKind, Name, Stmt, StmtKind};
use php_ast::resolve::{NameContext, NameResolver, ResolvedName};

/// Records every symbol declared by `stmts`, including declarations nested
/// in blocks, conditionals, `try` statements and function bodies.
pub(super) fn collect_declarations(
    stmts: &[Stmt],
    namespace: Option<&str>,
    resolver: &NameResolver,
    symbols: &mut Vec<DeclaredSymbol>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Class(class) => {
                if let Some(Some(name)) = &class.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Class);

                    let symbol = symbols.last_mut().expect("just pushed");

                    symbol.is_abstract = class.modifiers.is_abstract;
                    symbol.extends = class_names(class.extends.iter(), resolver);
                    symbol.implements = class_names(class.implements.iter(), resolver);
                }
            }

            StmtKind::Interface(interface) => {
                if let Some(name) = &interface.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Interface);

                    symbols.last_mut().expect("just pushed").extends =
                        class_names(interface.extends.iter(), resolver);
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

                    let symbol = symbols.last_mut().expect("just pushed");

                    symbol.implements = class_names(enum_decl.implements.iter(), resolver);
                    symbol.backing_type = enum_decl
                        .scalar_type
                        .as_ref()
                        .map(|backing| backing.parts.join("\\"));
                }
            }

            StmtKind::Function(function) => {
                if let Some(name) = &function.name {
                    push_symbol(symbols, name, namespace, SymbolKind::Function);
                }

                collect_declarations(&function.body.stmts, namespace, resolver, symbols);
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
                    symbols.push(DeclaredSymbol::new(name, SymbolKind::Constant));
                }
            }

            StmtKind::Block(block) => {
                collect_declarations(&block.stmts, namespace, resolver, symbols);
            }

            StmtKind::If(if_stmt) => {
                collect_declarations(
                    std::slice::from_ref(if_stmt.then_branch.as_ref()),
                    namespace,
                    resolver,
                    symbols,
                );

                for branch in if_stmt.elseif_branches.iter() {
                    collect_declarations(
                        std::slice::from_ref(&branch.body),
                        namespace,
                        resolver,
                        symbols,
                    );
                }

                if let Some(branch) = &if_stmt.else_branch {
                    collect_declarations(
                        std::slice::from_ref(branch.as_ref()),
                        namespace,
                        resolver,
                        symbols,
                    );
                }
            }

            StmtKind::TryCatch(try_catch) => {
                collect_declarations(&try_catch.body.stmts, namespace, resolver, symbols);

                for catch in try_catch.catches.iter() {
                    collect_declarations(&catch.body.stmts, namespace, resolver, symbols);
                }

                if let Some(finally) = &try_catch.finally {
                    collect_declarations(&finally.stmts, namespace, resolver, symbols);
                }
            }

            StmtKind::Declare(declare) => {
                if let Some(body) = &declare.body {
                    collect_declarations(
                        std::slice::from_ref(body.as_ref()),
                        namespace,
                        resolver,
                        symbols,
                    );
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
    symbols.push(DeclaredSymbol::new(
        qualify_declared_name(name, namespace),
        kind,
    ));
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

/// The full names of the classes and interfaces a declaration refers to.
fn class_names<'a>(
    names: impl IntoIterator<Item = &'a Name>,
    resolver: &NameResolver,
) -> Vec<String> {
    names
        .into_iter()
        .filter_map(
            |name| match resolver.resolve_owned_name(name, NameContext::Class) {
                ResolvedName::Fqn(name) => Some(name),
                _ => None,
            },
        )
        .collect()
}
