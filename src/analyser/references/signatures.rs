//! References made by functions, methods, parameters, attributes and type hints.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{Reference, ReferenceKind};
use crate::analyser::references::expressions::{collect_arg, collect_expr};
use crate::analyser::references::names::build_reference;
use crate::analyser::references::statements::collect_stmt;
use php_ast::{
    owned::{
        Attribute, FunctionDecl, MethodDecl, Param, PropertyHook, PropertyHookBody, TypeHint,
        TypeHintKind,
    },
    resolve::NameContext,
};

pub(super) fn collect_function(
    function: &FunctionDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&function.attributes, context, references);
    collect_params(&function.params, context, references);

    if let Some(return_type) = &function.return_type {
        collect_type_hint(return_type, context, references);
    }

    for stmt in function.body.stmts.iter() {
        collect_stmt(stmt, context, references);
    }
}

pub(super) fn collect_method(
    method: &MethodDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&method.attributes, context, references);
    collect_params(&method.params, context, references);

    if let Some(return_type) = &method.return_type {
        collect_type_hint(return_type, context, references);
    }

    if let Some(body) = &method.body {
        for stmt in body.stmts.iter() {
            collect_stmt(stmt, context, references);
        }
    }
}

pub(super) fn collect_params(
    params: &[Param],
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for param in params {
        collect_attributes(&param.attributes, context, references);

        if let Some(type_hint) = &param.type_hint {
            collect_type_hint(type_hint, context, references);
        }

        if let Some(default) = &param.default {
            collect_expr(default, context, references);
        }

        collect_property_hooks(&param.hooks, context, references);
    }
}

pub(super) fn collect_property_hooks(
    hooks: &[PropertyHook],
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for hook in hooks {
        collect_attributes(&hook.attributes, context, references);
        collect_params(&hook.params, context, references);

        match &hook.body {
            PropertyHookBody::Block(block) => {
                for stmt in block.stmts.iter() {
                    collect_stmt(stmt, context, references);
                }
            }

            PropertyHookBody::Expression(expr) => {
                collect_expr(expr, context, references);
            }

            PropertyHookBody::Abstract => {}
        }
    }
}

pub(super) fn collect_type_hint(
    type_hint: &TypeHint,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    match &type_hint.kind {
        TypeHintKind::Named(name) => {
            let resolved = context
                .resolver
                .resolve_owned_name(name, NameContext::Class);

            references.extend(build_reference(
                resolved,
                name.span,
                ReferenceKind::ClassName,
            ));
        }

        TypeHintKind::Keyword(_, _) => {}

        TypeHintKind::Nullable(inner) => {
            collect_type_hint(inner.as_ref(), context, references);
        }

        TypeHintKind::Union(types) => {
            for type_hint in types.iter() {
                collect_type_hint(type_hint, context, references);
            }
        }

        TypeHintKind::Intersection(types) => {
            for type_hint in types.iter() {
                collect_type_hint(type_hint, context, references);
            }
        }
    }
}

pub(super) fn collect_attributes(
    attributes: &[Attribute],
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for attribute in attributes {
        let resolved = context
            .resolver
            .resolve_owned_name(&attribute.name, NameContext::Class);

        references.extend(build_reference(
            resolved,
            attribute.name.span,
            ReferenceKind::ClassName,
        ));

        for arg in attribute.args.iter() {
            collect_arg(arg, context, references);
        }
    }
}
