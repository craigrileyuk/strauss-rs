//! References made by statements, and a walk over their bodies.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::Reference;
use crate::analyser::references::classes::{
    collect_class, collect_class_name, collect_enum, collect_interface, collect_trait,
};
use crate::analyser::references::expressions::collect_expr;
use crate::analyser::references::signatures::{collect_attributes, collect_function};
use php_ast::owned::{CatchClause, IfStmt, Stmt, StmtKind, SwitchBody};

pub(in crate::analyser) fn collect_stmt(
    stmt: &Stmt,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    match &stmt.kind {
        StmtKind::Class(class) => {
            collect_class(class, context, references);
        }

        StmtKind::Interface(interface) => {
            collect_interface(interface, context, references);
        }

        StmtKind::Trait(trait_decl) => {
            collect_trait(trait_decl, context, references);
        }

        StmtKind::Enum(enum_decl) => {
            collect_enum(enum_decl, context, references);
        }

        StmtKind::Function(function) => {
            collect_function(function, context, references);
        }

        StmtKind::Const(items) => {
            for item in items.iter() {
                collect_attributes(&item.attributes, context, references);
                collect_expr(&item.value, context, references);
            }
        }

        StmtKind::StaticVar(vars) => {
            for var in vars.iter() {
                if let Some(default) = &var.default {
                    collect_expr(default, context, references);
                }
            }
        }

        StmtKind::Declare(declare) => {
            if let Some(body) = &declare.body {
                collect_stmt(body.as_ref(), context, references);
            }
        }

        StmtKind::Switch(switch_stat) => {
            collect_expr(&switch_stat.expr, context, references);

            collect_switch_body(&switch_stat.body, context, references);
        }

        StmtKind::TryCatch(try_catch) => {
            for stmt in try_catch.body.stmts.iter() {
                collect_stmt(stmt, context, references);
            }

            for catch in try_catch.catches.iter() {
                collect_catch_clause(catch, context, references);
            }

            if let Some(finally) = &try_catch.finally {
                for stmt in finally.stmts.iter() {
                    collect_stmt(stmt, context, references);
                }
            }
        }

        StmtKind::While(while_stmt) => {
            collect_expr(&while_stmt.condition, context, references);
            collect_stmt(&while_stmt.body.as_ref(), context, references);
        }

        StmtKind::For(for_stmt) => {
            for expr in for_stmt.init.iter() {
                collect_expr(expr, context, references);
            }

            for expr in for_stmt.condition.iter() {
                collect_expr(expr, context, references);
            }

            for expr in for_stmt.update.iter() {
                collect_expr(expr, context, references);
            }

            collect_stmt(for_stmt.body.as_ref(), context, references);
        }

        StmtKind::Foreach(foreach_stmt) => {
            collect_expr(&foreach_stmt.expr, context, references);

            if let Some(key) = &foreach_stmt.key {
                collect_expr(key, context, references);
            }

            collect_expr(&foreach_stmt.value, context, references);
            collect_stmt(&foreach_stmt.body.as_ref(), context, references);
        }

        StmtKind::DoWhile(do_while_stmt) => {
            collect_stmt(do_while_stmt.body.as_ref(), context, references);
            collect_expr(&do_while_stmt.condition, context, references);
        }

        StmtKind::Expression(expr) => {
            collect_expr(expr, context, references);
        }

        StmtKind::Echo(exprs) => {
            for expr in exprs.iter() {
                collect_expr(expr, context, references);
            }
        }

        StmtKind::Return(Some(expr)) => {
            collect_expr(expr.as_ref(), context, references);
        }

        StmtKind::Break(Some(expr)) => {
            collect_expr(expr.as_ref(), context, references);
        }

        StmtKind::Continue(Some(expr)) => {
            collect_expr(expr.as_ref(), context, references);
        }

        StmtKind::Throw(expr) => {
            collect_expr(expr.as_ref(), context, references);
        }

        StmtKind::Unset(exprs) => {
            for expr in exprs.iter() {
                collect_expr(expr, context, references);
            }
        }

        StmtKind::Global(exprs) => {
            for expr in exprs.iter() {
                collect_expr(expr, context, references);
            }
        }

        StmtKind::Block(block) => {
            for stmt in block.stmts.iter() {
                collect_stmt(stmt, context, references);
            }
        }

        StmtKind::If(if_stmt) => {
            collect_if(if_stmt, context, references);
        }

        _ => {}
    }
}

fn collect_switch_body(
    body: &SwitchBody,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for case in body.cases.iter() {
        if let Some(value) = &case.value {
            collect_expr(value, context, references);
        }

        for stmt in case.body.iter() {
            collect_stmt(stmt, context, references);
        }
    }
}

fn collect_catch_clause(
    catch: &CatchClause,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for type_name in catch.types.iter() {
        collect_class_name(type_name, context, references);
    }

    for stmt in catch.body.stmts.iter() {
        collect_stmt(stmt, context, references);
    }
}

fn collect_if(if_stmt: &IfStmt, context: &AnalysisContext, references: &mut Vec<Reference>) {
    collect_expr(&if_stmt.condition, context, references);

    collect_stmt(if_stmt.then_branch.as_ref(), context, references);

    for branch in if_stmt.elseif_branches.iter() {
        collect_expr(&branch.condition, context, references);
        collect_stmt(&branch.body, context, references);
    }

    if let Some(branch) = &if_stmt.else_branch {
        collect_stmt(branch.as_ref(), context, references);
    }
}
