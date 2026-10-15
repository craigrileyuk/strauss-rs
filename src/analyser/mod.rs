//! Extracts what Strauss needs from a PHP file: the symbols it declares, the symbols it
//! refers to, and the strings and comments that might mention symbols.
#![allow(dead_code)]

use std::cell::RefCell;

use crate::analyser::context::AnalysisContext;
use crate::analyser::declarations::collect_declarations;
use crate::analyser::references::collect_stmt;
use crate::analyser::regions::assign_lexical_regions;
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

// The result types are this module's API. Nothing outside it names them yet
// (`main` only prints the result), so the re-export looks unused.
#[allow(unused_imports)]
pub use model::*;

pub fn analyse(source: &str, program: &Program) -> FileAnalysis {
    let mut analysis = FileAnalysis {
        namespaces: Vec::new(),
    };

    let mut current_namespace: Option<String> = None;
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
            Span::new(segment_start, stmt.span.start),
            source,
            &mut analysis,
        );

        current_namespace = namespace.name.as_ref().map(|name| name.parts.join("\\"));

        match &namespace.body {
            NamespaceBody::Braced(block) => {
                analyse_statements(
                    block.stmts.as_ref(),
                    current_namespace.as_deref(),
                    stmt.span,
                    source,
                    &mut analysis,
                );

                current_namespace = None;
                current_start = index + 1;
                segment_start = stmt.span.end;
            }

            NamespaceBody::Simple => {
                current_start = index + 1;
                segment_start = stmt.span.start;
            }
        }
    }

    // Analyse remaining statements belonging to the final simple namespace.
    analyse_statements(
        &program.stmts[current_start..],
        current_namespace.as_deref(),
        Span::new(segment_start, source.len() as u32),
        source,
        &mut analysis,
    );

    assign_lexical_regions(source, &mut analysis);

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
    span: Span,
    source: &str,
    analysis: &mut FileAnalysis,
) {
    let mut resolver = NameResolver::new();

    if let Some(namespace) = namespace {
        resolver.enter_namespace(Some(namespace));
    }
    let mut imports = Vec::new();
    let mut symbols = Vec::new();

    // Pass 1: collect imports and declarations.
    for stmt in stmts.iter() {
        match &stmt.kind {
            StmtKind::Use(use_decl) => {
                for item in &use_decl.uses {
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
                    });
                }
            }

            // Declarations may be nested (inside `if` blocks, function
            // bodies, etc.), so they're collected recursively.
            _ => collect_declarations(std::slice::from_ref(stmt), namespace, &mut symbols),
        }
    }

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
        span,
        imports,
        symbols,
        references,
        regions: context.regions.into_inner(),
    });
}

#[cfg(test)]
mod tests;
