//! References made by expressions.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{Reference, ReferenceKind};
use crate::analyser::references::classes::collect_class;
use crate::analyser::references::names::{
    build_reference, resolve_class_expr_name, resolve_identifier,
};
use crate::analyser::references::signatures::{
    collect_attributes, collect_params, collect_type_hint,
};
use crate::analyser::references::statements::collect_stmt;
use crate::analyser::regions::collect_string_arg_region;
use php_ast::{
    ast::BinaryOp,
    owned::{Arg, ArrowFunctionExpr, CallableCreateKind, ClosureExpr, Expr, ExprKind, MatchExpr},
    resolve::NameContext,
};

pub(super) fn collect_arg(arg: &Arg, context: &AnalysisContext, references: &mut Vec<Reference>) {
    if let Some(value) = &arg.value {
        collect_expr(value, context, references);
    }
}

pub(super) fn collect_expr(
    expr: &Expr,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    match &expr.kind {
        ExprKind::VariableVariable(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::UnaryPrefix(inner) => {
            collect_expr(inner.operand.as_ref(), context, references);
        }

        ExprKind::UnaryPostfix(inner) => {
            collect_expr(inner.operand.as_ref(), context, references);
        }

        ExprKind::Cast(_, inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::ErrorSuppress(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Print(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Isset(exprs) => {
            for expr in exprs.iter() {
                collect_expr(expr, context, references);
            }
        }

        ExprKind::Empty(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Include(_, inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Eval(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Exit(Some(inner)) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Clone(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::CloneWith(left, right) => {
            collect_expr(left.as_ref(), context, references);
            collect_expr(right.as_ref(), context, references);
        }

        ExprKind::ArrayAccess(access) => {
            collect_expr(access.array.as_ref(), context, references);

            if let Some(index) = &access.index {
                collect_expr(index.as_ref(), context, references);
            }
        }

        ExprKind::NullsafePropertyAccess(access) => {
            collect_expr(access.object.as_ref(), context, references);
            collect_member_name(access.property.as_ref(), context, references);
        }

        ExprKind::NullsafeMethodCall(call) => {
            collect_expr(call.object.as_ref(), context, references);
            collect_member_name(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::StaticDynMethodCall(call) => {
            collect_class_expr(call.class.as_ref(), context, references);
            collect_member_name(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::Closure(closure) => {
            collect_closure(closure, context, references);
        }

        ExprKind::ArrowFunction(arrow) => {
            collect_arrow_function(arrow, context, references);
        }

        ExprKind::Match(match_expr) => {
            collect_match(match_expr, context, references);
        }

        ExprKind::Yield(yield_expr) => {
            if let Some(key) = &yield_expr.key {
                collect_expr(key.as_ref(), context, references);
            }

            if let Some(value) = &yield_expr.value {
                collect_expr(value.as_ref(), context, references);
            }
        }

        ExprKind::Assign(assign) => {
            collect_expr(assign.target.as_ref(), context, references);
            collect_expr(assign.value.as_ref(), context, references);
        }

        ExprKind::New(new_expr) => {
            collect_class_expr(&new_expr.class, context, references);

            for arg in new_expr.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::Binary(binary) => {
            collect_expr(binary.left.as_ref(), context, references);

            if binary.op == BinaryOp::Instanceof {
                collect_class_expr(binary.right.as_ref(), context, references);
            } else {
                collect_expr(binary.right.as_ref(), context, references);
            }
        }

        ExprKind::StaticMethodCall(call) => {
            collect_class_expr(&call.class, context, references);
            collect_member_name(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::StaticPropertyAccess(access) => {
            collect_class_expr(&access.class, context, references);
        }

        ExprKind::ClassConstAccess(access) => {
            collect_class_expr(&access.class, context, references);

            if let ExprKind::Identifier(name) = &access.member.kind {
                // `Foo::class` is the class name itself, already collected above.
                if name.eq_ignore_ascii_case("class") {
                    return;
                }

                if let Some(class_name) = resolve_class_expr_name(&access.class, context) {
                    references.push(Reference {
                        name: format!("{class_name}::{name}"),
                        fallback: None,
                        span: access.member.span,
                        kind: ReferenceKind::ClassConstantName,
                    });
                }
            } else {
                collect_expr(&access.member, context, references);
            }
        }

        ExprKind::Parenthesized(inner) => {
            collect_expr(inner.as_ref(), context, references);
        }

        ExprKind::Ternary(ternary) => {
            collect_expr(ternary.condition.as_ref(), context, references);

            if let Some(then_expr) = &ternary.then_expr {
                collect_expr(then_expr.as_ref(), context, references);
            }

            collect_expr(ternary.else_expr.as_ref(), context, references);
        }

        ExprKind::NullCoalesce(coalesce) => {
            collect_expr(coalesce.left.as_ref(), context, references);
            collect_expr(coalesce.right.as_ref(), context, references);
        }

        ExprKind::FunctionCall(call) => {
            collect_function_name(call.name.as_ref(), context, references);
            collect_string_arg_region(call, context);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::MethodCall(call) => {
            collect_expr(call.object.as_ref(), context, references);
            collect_member_name(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::PropertyAccess(access) => {
            collect_expr(access.object.as_ref(), context, references);
            collect_member_name(access.property.as_ref(), context, references);
        }

        ExprKind::Array(elements) => {
            for element in elements {
                if let Some(key) = &element.key {
                    collect_expr(key, context, references);
                }

                collect_expr(&element.value, context, references);
            }
        }

        ExprKind::StaticPropertyAccessDynamic { class, member } => {
            collect_class_expr(class.as_ref(), context, references);
            collect_member_name(member.as_ref(), context, references);
        }

        ExprKind::ClassConstAccessDynamic { class, member } => {
            collect_class_expr(class.as_ref(), context, references);
            collect_member_name(member.as_ref(), context, references);
        }

        ExprKind::AnonymousClass(class) => {
            collect_class(class.as_ref(), context, references);
        }

        ExprKind::CallableCreate(callable) => match &callable.kind {
            CallableCreateKind::Function(function) => {
                collect_expr(function.as_ref(), context, references);
            }

            CallableCreateKind::Method { object, method }
            | CallableCreateKind::NullsafeMethod { object, method } => {
                collect_expr(object.as_ref(), context, references);
                collect_member_name(method.as_ref(), context, references);
            }

            CallableCreateKind::StaticMethod { class, method } => {
                collect_class_expr(class.as_ref(), context, references);
                collect_member_name(method.as_ref(), context, references);
            }
        },

        ExprKind::Identifier(name) => {
            references.extend(build_reference(
                resolve_identifier(name, NameContext::Const, context),
                expr.span,
                ReferenceKind::ConstantName,
            ));
        }

        _ => {}
    }
}

fn collect_closure(
    closure: &ClosureExpr,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&closure.attributes, context, references);
    collect_params(&closure.params, context, references);

    if let Some(return_type) = &closure.return_type {
        collect_type_hint(return_type, context, references);
    }

    for stmt in closure.body.stmts.iter() {
        collect_stmt(stmt, context, references);
    }
}

fn collect_arrow_function(
    arrow: &ArrowFunctionExpr,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&arrow.attributes, context, references);
    collect_params(&arrow.params, context, references);

    if let Some(return_type) = &arrow.return_type {
        collect_type_hint(return_type, context, references);
    }

    collect_expr(arrow.body.as_ref(), context, references);
}

fn collect_match(
    match_expr: &MatchExpr,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_expr(match_expr.subject.as_ref(), context, references);

    for arm in match_expr.arms.iter() {
        if let Some(conditions) = &arm.conditions {
            for condition in conditions.iter() {
                collect_expr(condition, context, references);
            }
        }

        collect_expr(&arm.body, context, references);
    }
}

fn collect_class_expr(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
    match &expr.kind {
        ExprKind::Identifier(name) => {
            references.extend(build_reference(
                resolve_identifier(name, NameContext::Class, context),
                expr.span,
                ReferenceKind::ClassName,
            ));
        }

        // Dynamic class expressions (`new $class`, anonymous classes, etc.).
        _ => collect_expr(expr, context, references),
    }
}

/// Collects the member side of `->`, `?->` and `::`. A bare identifier is a
/// method/property name, not a symbol; anything else is a dynamic expression.
fn collect_member_name(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
    if !matches!(expr.kind, ExprKind::Identifier(_)) {
        collect_expr(expr, context, references);
    }
}

fn collect_function_name(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
    match &expr.kind {
        ExprKind::Identifier(name) => {
            references.extend(build_reference(
                resolve_identifier(name, NameContext::Function, context),
                expr.span,
                ReferenceKind::FunctionName,
            ));
        }

        // Dynamic callees, e.g. `Foo::{$method}()` or `$callable()`.
        _ => collect_expr(expr, context, references),
    }
}
