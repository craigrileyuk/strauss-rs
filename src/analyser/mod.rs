//! Extracts what Strauss needs from a PHP file: the symbols it declares, the symbols it
//! refers to, and the strings and comments that might mention symbols.
#![allow(dead_code)]

use std::cell::RefCell;

use crate::analyser::context::AnalysisContext;
use crate::analyser::declarations::collect_declarations;
use crate::analyser::references::collect_stmt;
use crate::analyser::regions::assign_lexical_regions;
use crate::analyser::spans::SourceTokens;
use php_ast::{
    Span,
    ast::UseKind,
    owned::{NamespaceBody, Program, Stmt, StmtKind},
    resolve::NameResolver,
};

mod context;
mod declarations;
mod model;
mod references;
mod regions;
mod spans;

// The result types are this module's API. Nothing outside it names them yet
// (`main` only prints the result), so the re-export looks unused.
#[allow(unused_imports)]
pub use model::*;
#[allow(unused_imports)]
pub use references::resolve_written_name;

pub fn analyse(source: &str, program: &Program) -> FileAnalysis {
    let mut analysis = FileAnalysis {
        namespaces: Vec::new(),
    };

    // The tokens locate what the parser's positions don't: the names of
    // declarations, the items of a group `use`, and every string and comment.
    let (tokens, _errors) = php_lexer::lex_all(source);
    let located = SourceTokens::new(source, &tokens);

    let mut current_namespace: Option<String> = None;
    // Where the name of the simple namespace being read is written.
    let mut pending_name_span: Option<Span> = None;
    let mut current_start: usize = 0;
    let mut segment_start: u32 = 0;

    for (index, stmt) in program.stmts.iter().enumerate() {
        let StmtKind::Namespace(namespace) = &stmt.kind else {
            continue;
        };

        // Analyse statements belonging to the preceding simple namespace.
        analyse_statements(
            &program.stmts[current_start..index],
            current_namespace.as_deref(),
            pending_name_span,
            Span::new(segment_start, stmt.span.start),
            source,
            &located,
            &mut analysis,
        );

        current_namespace = namespace.name.as_ref().map(|name| name.parts.join("\\"));
        let name_span = namespace.name.as_ref().map(|name| name.span);

        match &namespace.body {
            NamespaceBody::Braced(block) => {
                analyse_statements(
                    block.stmts.as_ref(),
                    current_namespace.as_deref(),
                    name_span,
                    stmt.span,
                    source,
                    &located,
                    &mut analysis,
                );

                current_namespace = None;
                pending_name_span = None;
                current_start = index + 1;
                segment_start = stmt.span.end;
            }

            NamespaceBody::Simple => {
                current_start = index + 1;
                segment_start = stmt.span.start;
                pending_name_span = name_span;
            }
        }
    }

    // Analyse remaining statements belonging to the final simple namespace.
    analyse_statements(
        &program.stmts[current_start..],
        current_namespace.as_deref(),
        pending_name_span,
        Span::new(segment_start, source.len() as u32),
        source,
        &located,
        &mut analysis,
    );

    assign_lexical_regions(source, &tokens, &mut analysis);

    // Code before the first `namespace` statement can only be things like
    // `declare(strict_types=1);`, and nothing may follow a braced namespace.
    // Don't report global segments that turned out to be empty.
    analysis.namespaces.retain(|namespace| {
        namespace.namespace.is_some()
            || !(namespace.imports.is_empty()
                && namespace.symbols.is_empty()
                && namespace.references.is_empty()
                && namespace.regions.is_empty())
    });

    analysis
}

fn analyse_statements(
    stmts: &[Stmt],
    namespace: Option<&str>,
    name_span: Option<Span>,
    span: Span,
    source: &str,
    located: &SourceTokens,
    analysis: &mut FileAnalysis,
) {
    let mut resolver = NameResolver::new();

    if let Some(namespace) = namespace {
        resolver.enter_namespace(Some(namespace));
    }
    let mut imports = Vec::new();
    let mut group_uses = Vec::new();
    let mut symbols = Vec::new();

    // Pass 1: collect imports and declarations.
    for stmt in stmts.iter() {
        match &stmt.kind {
            StmtKind::Use(use_decl) => {
                // Where each name is written, if the tokens agree with the parser
                // about how many there are.
                let spans = located
                    .use_statement(stmt.span)
                    .filter(|spans| spans.items.len() == use_decl.uses.len());

                let group =
                    spans
                        .as_ref()
                        .and_then(|spans| spans.group_prefix)
                        .map(|prefix_span| {
                            group_uses.push(GroupUse {
                                span: stmt.span,
                                prefix_span,
                            });

                            group_uses.len() - 1
                        });

                for (position, item) in use_decl.uses.iter().enumerate() {
                    let kind = item.kind.unwrap_or(use_decl.kind);

                    resolver.add_import(kind, &item.name.parts.join("\\"), item.alias.as_deref());

                    let import_kind = match kind {
                        UseKind::Normal => ImportKind::Class,
                        UseKind::Function => ImportKind::Function,
                        UseKind::Const => ImportKind::Constant,
                    };

                    imports.push(Import {
                        name: item.name.parts.join("\\"),
                        alias: item.alias.as_deref().map(String::from),
                        kind: import_kind,
                        span: spans.as_ref().map(|spans| spans.items[position].name),
                        alias_span: spans.as_ref().and_then(|spans| spans.items[position].alias),
                        group,
                    });
                }
            }

            _ => {}
        }
    }

    // Declarations may be nested (inside `if` blocks, function bodies, etc.),
    // so they're collected recursively. This waits for every import, so a
    // declaration's parents resolve through all of them.
    collect_declarations(stmts, namespace, &resolver, located, &mut symbols);

    let context = AnalysisContext {
        resolver: &resolver,
        source,
        regions: RefCell::new(Vec::new()),
    };

    let mut references = Vec::new();

    // Pass 2: collect references from every statement, including
    // top-level code and nested declarations.
    for stmt in stmts.iter() {
        collect_stmt(stmt, &context, &mut references);
    }

    analysis.namespaces.push(NamespaceAnalysis {
        namespace: namespace.map(String::from),
        name_span,
        span,
        imports,
        group_uses,
        symbols,
        references,
        regions: context.regions.into_inner(),
    });
}

#[cfg(test)]
mod tests;
