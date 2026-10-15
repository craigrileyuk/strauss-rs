#![allow(dead_code)]
#![allow(unused_imports)]
use php_ast::{
    Span,
    ast::{BinaryOp, NameKind, UseKind},
    owned::{
        Arg, ArrowFunctionExpr, Attribute, CallableCreateKind, CatchClause, ClassConstDecl,
        ClassDecl, ClassMember, ClassMemberKind, ClosureExpr, DoWhileStmt, EnumDecl,
        EnumMemberKind, Expr, ExprKind, ForStmt, ForeachStmt, FunctionDecl, IfStmt, InterfaceDecl,
        MatchExpr, MethodDecl, Name, NamespaceBody, Param, Program, PropertyDecl, PropertyHook,
        PropertyHookBody, StaticAccessExpr, StaticDynMethodCallExpr, Stmt, StmtKind, SwitchBody,
        SwitchCase, SwitchStmt, TraitDecl, TraitUseDecl, TryCatchStmt, TypeHint, TypeHintKind,
        WhileStmt, YieldExpr,
    },
    resolve::{NameContext, NameResolver, ResolvedName},
};

#[derive(Debug)]
pub struct Reference {
    pub name: String,
    /// Global name PHP falls back to at runtime when `name` isn't defined.
    /// Only set for unqualified function and constant references inside
    /// a namespace with no matching import.
    pub fallback: Option<String>,
    pub span: Span,
    pub kind: ReferenceKind,
}

#[derive(Debug)]
pub enum ReferenceKind {
    ClassName,
    FunctionName,
    ConstantName,
    ClassConstantName,
}

#[derive(Debug)]
pub struct FileAnalysis {
    pub namespaces: Vec<NamespaceAnalysis>,
}

#[derive(Debug)]
pub struct NamespaceAnalysis {
    pub namespace: Option<String>,
    pub imports: Vec<Import>,
    pub symbols: Vec<DeclaredSymbol>,
    pub references: Vec<Reference>,
}

#[derive(Debug)]
pub struct Import {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, Copy)]
pub enum ImportKind {
    Class,
    Function,
    Constant,
}

#[derive(Debug)]
pub struct DeclaredSymbol {
    pub name: String,
    pub kind: SymbolKind,
}

#[derive(Debug)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Constant,
}

struct AnalysisContext<'a> {
    resolver: &'a NameResolver,
}

pub fn analyse(program: &Program) -> FileAnalysis {
    let mut analysis = FileAnalysis {
        namespaces: Vec::new(),
    };

    let mut current_namespace: Option<String> = None;
    let mut current_start: usize = 0;

    for (index, stmt) in program.stmts.iter().enumerate() {
        match &stmt.kind {
            StmtKind::Namespace(namespace) => {
                // Analyse statements belonging to the preceding
                // simple namespace.
                if current_start < index {
                    analyse_statements(
                        &program.stmts[current_start..index],
                        current_namespace.as_deref(),
                        &mut analysis,
                    );
                }

                current_namespace = namespace.name.as_ref().map(|name| name.parts.join("\\"));

                match &namespace.body {
                    NamespaceBody::Braced(block) => {
                        analyse_statements(
                            block.stmts.as_ref(),
                            current_namespace.as_deref(),
                            &mut analysis,
                        );

                        current_namespace = None;
                        current_start = index + 1;
                    }

                    NamespaceBody::Simple => {
                        current_start = index + 1;
                    }
                }
            }

            _ => {}
        }
    }

    // Analyse remaining statements belonging to the final
    // simple namespace.
    if current_start < program.stmts.len() {
        analyse_statements(
            &program.stmts[current_start..],
            current_namespace.as_deref(),
            &mut analysis,
        );
    }

    analysis
}

fn analyse_statements(stmts: &[Stmt], namespace: Option<&str>, analysis: &mut FileAnalysis) {
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
    };

    let mut references = Vec::new();

    // Pass 2: collect references from every statement, including
    // top-level code and nested declarations.
    for stmt in stmts.iter() {
        collect_stmt(stmt, &context, &mut references);
    }

    // Code before the first `namespace` statement can only be things like
    // `declare(strict_types=1);`. Don't report an empty global namespace.
    if namespace.is_none() && imports.is_empty() && symbols.is_empty() && references.is_empty() {
        return;
    }

    analysis.namespaces.push(NamespaceAnalysis {
        namespace: namespace.map(String::from),
        imports,
        symbols,
        references,
    });
}

/// Records every symbol declared by `stmts`, including declarations nested
/// in blocks, conditionals, `try` statements and function bodies.
fn collect_declarations(
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

fn collect_arg(arg: &Arg, context: &AnalysisContext, references: &mut Vec<Reference>) {
    if let Some(value) = &arg.value {
        collect_expr(value, context, references);
    }
}

fn collect_expr(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
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

fn collect_attributes(
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

fn collect_params(params: &[Param], context: &AnalysisContext, references: &mut Vec<Reference>) {
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

fn collect_property_hooks(
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

fn resolve_class_expr_name(expr: &Expr, context: &AnalysisContext) -> Option<String> {
    match &expr.kind {
        ExprKind::Identifier(name) => match resolve_identifier(name, NameContext::Class, context) {
            ResolvedName::Fqn(name) => Some(name),
            _ => None,
        },

        _ => None,
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

/// Resolves a name written as a plain identifier (`Foo`, `Bar\Baz`, `\Foo`,
/// `namespace\Foo`) against the current namespace and imports.
fn resolve_identifier(
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

fn build_reference(resolved: ResolvedName, span: Span, kind: ReferenceKind) -> Option<Reference> {
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

fn collect_stmt(stmt: &Stmt, context: &AnalysisContext, references: &mut Vec<Reference>) {
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

fn collect_class(class: &ClassDecl, context: &AnalysisContext, references: &mut Vec<Reference>) {
    collect_attributes(&class.attributes, context, references);

    if let Some(parent) = &class.extends {
        collect_class_name(parent, context, references);
    }

    for interface in class.implements.iter() {
        collect_class_name(interface, context, references);
    }

    for member in class.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

fn collect_class_name(name: &Name, context: &AnalysisContext, references: &mut Vec<Reference>) {
    let resolved = context
        .resolver
        .resolve_owned_name(name, NameContext::Class);

    references.extend(build_reference(
        resolved,
        name.span,
        ReferenceKind::ClassName,
    ));
}

fn collect_class_member(
    member: &ClassMember,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    match &member.kind {
        ClassMemberKind::Property(property) => {
            collect_property(property, context, references);
        }

        ClassMemberKind::Method(method) => {
            collect_method(method, context, references);
        }

        ClassMemberKind::TraitUse(trait_use) => {
            collect_trait_use(trait_use, context, references);
        }

        ClassMemberKind::ClassConst(class_const) => {
            collect_class_const(class_const, context, references);
        }
    }
}

fn collect_class_const(
    class_const: &ClassConstDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&class_const.attributes, context, references);

    if let Some(type_hint) = &class_const.type_hint {
        collect_type_hint(type_hint, context, references);
    }

    collect_expr(&class_const.value, context, references);
}

fn collect_property(
    property: &PropertyDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&property.attributes, context, references);

    if let Some(type_hint) = &property.type_hint {
        collect_type_hint(type_hint, context, references);
    }

    if let Some(default) = &property.default {
        collect_expr(default, context, references);
    }

    collect_property_hooks(&property.hooks, context, references);
}

fn collect_interface(
    interface: &InterfaceDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&interface.attributes, context, references);

    for parent in interface.extends.iter() {
        collect_class_name(parent, context, references);
    }

    for member in interface.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

fn collect_trait(
    trait_decl: &TraitDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&trait_decl.attributes, context, references);

    for member in trait_decl.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

fn collect_trait_use(
    trait_use: &TraitUseDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for name in trait_use.traits.iter() {
        collect_class_name(name, context, references);
    }
}

fn collect_enum(enum_decl: &EnumDecl, context: &AnalysisContext, references: &mut Vec<Reference>) {
    collect_attributes(&enum_decl.attributes, context, references);

    for interface in enum_decl.implements.iter() {
        collect_class_name(interface, context, references);
    }

    for member in enum_decl.body.members.iter() {
        match &member.kind {
            EnumMemberKind::Method(method) => {
                collect_method(method, context, references);
            }

            EnumMemberKind::TraitUse(trait_use) => {
                collect_trait_use(trait_use, context, references);
            }

            EnumMemberKind::ClassConst(class_const) => {
                collect_class_const(class_const, context, references);
            }

            EnumMemberKind::Case(case) => {
                collect_attributes(&case.attributes, context, references);

                if let Some(value) = &case.value {
                    collect_expr(value, context, references);
                }
            }
        }
    }
}

fn collect_function(
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

fn collect_method(method: &MethodDecl, context: &AnalysisContext, references: &mut Vec<Reference>) {
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

fn collect_type_hint(
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

fn inspect_name(name: &Name) {
    println!(
        "NAME: kind={:?}, parts={:?}, span={:?}",
        name.kind, name.parts, name.span,
    );
}

/* *********************************************************
 * TESTS
 ********************************************************* */

#[cfg(test)]
mod tests {
    use super::*;
    use php_ast::owned::Program;
    use php_rs_parser::parse;

    fn analyse_php(source: &str) -> FileAnalysis {
        let result = parse(source);

        assert!(
            result.errors.is_empty(),
            "PHP parse errors: {:?}",
            result.errors
        );

        analyse(&result.program)
    }

    #[test]
    fn collects_namespace_and_class_import() {
        let source = r#"<?php

namespace Foo;

use Vendor\Bar;

class Example extends Bar
{
}
"#;

        let analysis = analyse_php(source);

        assert_eq!(analysis.namespaces.len(), 1);

        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.namespace.as_deref(), Some("Foo"));

        assert_eq!(namespace.imports.len(), 1);
        assert_eq!(namespace.imports[0].name, "Vendor\\Bar");
        assert!(namespace.imports[0].alias.is_none());
        assert!(matches!(namespace.imports[0].kind, ImportKind::Class));

        assert_eq!(namespace.symbols.len(), 1);
        assert_eq!(namespace.symbols[0].name, "Foo\\Example");
        assert!(matches!(namespace.symbols[0].kind, SymbolKind::Class));

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\Bar");
        assert!(matches!(
            namespace.references[0].kind,
            ReferenceKind::ClassName
        ));
    }

    #[test]
    fn resolves_class_alias() {
        let source = r#"<?php

namespace Foo;

use Vendor\Bar as Baz;

class Example extends Baz
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 1);
        assert_eq!(namespace.imports[0].name, "Vendor\\Bar");
        assert_eq!(namespace.imports[0].alias.as_deref(), Some("Baz"));
        assert!(matches!(namespace.imports[0].kind, ImportKind::Class));

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\Bar");
    }

    #[test]
    fn resolves_unqualified_class_in_current_namespace() {
        let source = r#"<?php

namespace Foo;

class Example extends ParentClass
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Foo\\ParentClass");
    }

    #[test]
    fn resolves_qualified_class_in_current_namespace() {
        let source = r#"<?php

namespace Foo;

class Example extends Bar\ParentClass
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Foo\\Bar\\ParentClass");
    }

    #[test]
    fn resolves_fully_qualified_class() {
        let source = r#"<?php

namespace Foo;

class Example extends \Vendor\ParentClass
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\ParentClass");
    }

    #[test]
    fn resolves_namespace_relative_class() {
        let source = r#"<?php

namespace Foo;

class Example extends namespace\Bar\ParentClass
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Foo\\Bar\\ParentClass");
    }

    #[test]
    fn resolves_imported_qualified_class() {
        let source = r#"<?php

namespace Foo;

use Vendor\Bar;
 
class Example extends Bar\Nested
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\Bar\\Nested");
    }

    #[test]
    fn collects_function_import() {
        let source = r#"<?php

namespace Foo;

use function Vendor\helper;

function test(): void
{
    helper();
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 1);
        assert_eq!(namespace.imports[0].name, "Vendor\\helper");
        assert!(namespace.imports[0].alias.is_none());
        assert!(matches!(namespace.imports[0].kind, ImportKind::Function));

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\helper");
        assert!(matches!(
            namespace.references[0].kind,
            ReferenceKind::FunctionName
        ));
    }

    #[test]
    fn resolves_function_import_alias() {
        let source = r#"<?php

namespace Foo;

use function Vendor\helper as custom_helper;

function test(): void
{
    custom_helper();
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 1);
        assert_eq!(namespace.imports[0].name, "Vendor\\helper");
        assert_eq!(namespace.imports[0].alias.as_deref(), Some("custom_helper"));
        assert!(matches!(namespace.imports[0].kind, ImportKind::Function));

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Vendor\\helper");
    }

    #[test]
    fn class_import_does_not_resolve_function_call() {
        let source = r#"<?php

namespace Foo;

use Vendor\helper;

function test(): void
{
    helper();
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 1);
        assert!(matches!(namespace.imports[0].kind, ImportKind::Class));

        assert_eq!(namespace.references.len(), 1);
        assert_eq!(namespace.references[0].name, "Foo\\helper");
        assert!(matches!(
            namespace.references[0].kind,
            ReferenceKind::FunctionName
        ));
    }

    #[test]
    fn collects_const_import() {
        let source = r#"<?php

namespace Foo;

use const Vendor\VALUE;

class Example
{
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 1);
        assert_eq!(namespace.imports[0].name, "Vendor\\VALUE");
        assert!(namespace.imports[0].alias.is_none());
        assert!(matches!(namespace.imports[0].kind, ImportKind::Constant));
    }

    #[test]
    fn collects_function_calls() {
        let source = r#"<?php

namespace Foo;

function test(): void
{
    foo();
    Bar\baz();
    \Global\thing();
    $callable();
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.references.len(), 3);

        assert_eq!(namespace.references[0].name, "Foo\\foo");
        assert!(matches!(
            namespace.references[0].kind,
            ReferenceKind::FunctionName
        ));

        assert_eq!(namespace.references[1].name, "Foo\\Bar\\baz");
        assert!(matches!(
            namespace.references[1].kind,
            ReferenceKind::FunctionName
        ));

        assert_eq!(namespace.references[2].name, "Global\\thing");
        assert!(matches!(
            namespace.references[2].kind,
            ReferenceKind::FunctionName
        ));
    }

    #[test]
    fn collects_multiple_import_kinds() {
        let source = r#"<?php

namespace Foo;

use Vendor\SomeClass;
use function Vendor\helper;
use const Vendor\VALUE;

class Example extends SomeClass
{
    public function test(): void
    {
        helper();
    }
}
"#;

        let analysis = analyse_php(source);
        let namespace = &analysis.namespaces[0];

        assert_eq!(namespace.imports.len(), 3);

        assert!(matches!(namespace.imports[0].kind, ImportKind::Class));
        assert!(matches!(namespace.imports[1].kind, ImportKind::Function));
        assert!(matches!(namespace.imports[2].kind, ImportKind::Constant));

        assert_eq!(namespace.references.len(), 2);

        assert_eq!(namespace.references[0].name, "Vendor\\SomeClass");
        assert!(matches!(
            namespace.references[0].kind,
            ReferenceKind::ClassName
        ));

        assert_eq!(namespace.references[1].name, "Vendor\\helper");
        assert!(matches!(
            namespace.references[1].kind,
            ReferenceKind::FunctionName
        ));
    }

    #[test]
    fn collects_const_references() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            const LOCAL = 123;

            function test(): void
            {
                $a = LOCAL;
                $b = Bar\VALUE;
                $c = \Global\VALUE;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 3);

        assert_eq!(references[0].name, "Foo\\LOCAL");
        assert!(matches!(references[0].kind, ReferenceKind::ConstantName));

        assert_eq!(references[1].name, "Foo\\Bar\\VALUE");
        assert!(matches!(references[1].kind, ReferenceKind::ConstantName));

        assert_eq!(references[2].name, "Global\\VALUE");
        assert!(matches!(references[2].kind, ReferenceKind::ConstantName));
    }

    #[test]
    fn resolves_const_import() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use const Vendor\VALUE;

            function test(): void
            {
                $a = VALUE;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name, "Vendor\\VALUE");
        assert!(matches!(references[0].kind, ReferenceKind::ConstantName));
    }

    #[test]
    fn resolves_const_import_alias() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use const Vendor\VALUE as OTHER;

            function test(): void
            {
                $a = OTHER;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name, "Vendor\\VALUE");
        assert!(matches!(references[0].kind, ReferenceKind::ConstantName));
    }

    #[test]
    fn inspect_const_expression() {
        let result = parse(
            r#"<?php
            namespace Foo;

            use const Vendor\VALUE;

            function test(): void
            {
                $a = VALUE;
                $b = Vendor\OTHER;
                $c = \Global\VALUE;
            }
            "#,
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);

        println!("{:#?}", result.program);
    }

    #[test]
    fn collects_class_constant_reference() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            class Example
            {
                public const VALUE = 123;
            }

            function test(): void
            {
                $value = Example::VALUE;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 2);

        assert_eq!(references[0].name, "Foo\\Example");
        assert!(matches!(references[0].kind, ReferenceKind::ClassName));

        assert_eq!(references[1].name, "Foo\\Example::VALUE");
        assert!(matches!(
            references[1].kind,
            ReferenceKind::ClassConstantName
        ));
    }

    #[test]
    fn resolves_imported_class_constant() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            function test(): void
            {
                $value = Bar::VALUE;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 2);

        assert_eq!(references[0].name, "Vendor\\Bar");
        assert!(matches!(references[0].kind, ReferenceKind::ClassName));

        assert_eq!(references[1].name, "Vendor\\Bar::VALUE");
        assert!(matches!(
            references[1].kind,
            ReferenceKind::ClassConstantName
        ));
    }

    fn reference_names(analysis: &FileAnalysis) -> Vec<&str> {
        analysis.namespaces[0]
            .references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect()
    }

    #[test]
    fn ignores_method_and_property_names() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            function test($o): void
            {
                $o->method();
                $o?->other();
                $a = $o->prop;
                $b = $o?->nullable;
                $c = $o->method(...);
            }
            "#,
        );

        assert!(reference_names(&analysis).is_empty());
    }

    #[test]
    fn collects_instanceof_class_once() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            function test($x): void
            {
                $a = $x instanceof Bar;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].name, "Vendor\\Bar");
        assert!(matches!(references[0].kind, ReferenceKind::ClassName));
    }

    #[test]
    fn resolves_qualified_names_through_class_imports() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            function test(): void
            {
                $a = new Bar\Nested();
                Bar\helper();
                $b = Bar\VALUE;
            }
            "#,
        );

        assert_eq!(
            reference_names(&analysis),
            [
                "Vendor\\Bar\\Nested",
                "Vendor\\Bar\\helper",
                "Vendor\\Bar\\VALUE"
            ]
        );
    }

    #[test]
    fn resolves_class_imports_case_insensitively() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\{Baz, Qux as Q};

            function test(): void
            {
                $a = new baz();
                $b = new q();
            }
            "#,
        );

        assert_eq!(reference_names(&analysis), ["Vendor\\Baz", "Vendor\\Qux"]);
    }

    #[test]
    fn ignores_special_class_names() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            class Example
            {
                public function test(): void
                {
                    $a = new self();
                    $b = new static();
                    $c = self::VALUE;
                    parent::test();
                }
            }
            "#,
        );

        assert!(reference_names(&analysis).is_empty());
    }

    #[test]
    fn records_global_fallback_for_unqualified_functions_and_constants() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use function Vendor\helper;
            use const Vendor\VALUE;

            function test(): void
            {
                strlen('a');
                $a = PHP_EOL;
                helper();
                $b = VALUE;
                Bar\baz();
                \Global\thing();
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        let pairs: Vec<(&str, Option<&str>)> = references
            .iter()
            .map(|reference| (reference.name.as_str(), reference.fallback.as_deref()))
            .collect();

        assert_eq!(
            pairs,
            [
                ("Foo\\strlen", Some("strlen")),
                ("Foo\\PHP_EOL", Some("PHP_EOL")),
                ("Vendor\\helper", None),
                ("Vendor\\VALUE", None),
                ("Foo\\Bar\\baz", None),
                ("Global\\thing", None),
            ]
        );
    }

    #[test]
    fn no_fallback_in_global_namespace() {
        let analysis = analyse_php(
            r#"<?php
            function test(): void
            {
                strlen(PHP_EOL);
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 2);
        assert_eq!(references[0].name, "strlen");
        assert!(references[0].fallback.is_none());
        assert_eq!(references[1].name, "PHP_EOL");
        assert!(references[1].fallback.is_none());
    }

    #[test]
    fn class_keyword_is_not_a_constant() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            function test(): void
            {
                $a = Bar::class;
                $b = Bar::CLASS;
            }
            "#,
        );

        let references = &analysis.namespaces[0].references;

        assert_eq!(references.len(), 2);

        for reference in references {
            assert_eq!(reference.name, "Vendor\\Bar");
            assert!(matches!(reference.kind, ReferenceKind::ClassName));
        }
    }

    fn symbols(analysis: &FileAnalysis) -> Vec<(&str, &SymbolKind)> {
        analysis.namespaces[0]
            .symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), &symbol.kind))
            .collect()
    }

    #[test]
    fn collects_guarded_declarations() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            if (!function_exists('Foo\\guarded_fn')) {
                function guarded_fn() {}
            }

            if (!class_exists('Foo\\Guarded')) {
                class Guarded {}
            } elseif (true) {
                interface ElseIfInterface {}
            } else {
                trait ElseTrait {}
            }
            "#,
        );

        let symbols = symbols(&analysis);

        assert_eq!(symbols.len(), 4);
        assert_eq!(symbols[0].0, "Foo\\guarded_fn");
        assert!(matches!(symbols[0].1, SymbolKind::Function));
        assert_eq!(symbols[1].0, "Foo\\Guarded");
        assert!(matches!(symbols[1].1, SymbolKind::Class));
        assert_eq!(symbols[2].0, "Foo\\ElseIfInterface");
        assert!(matches!(symbols[2].1, SymbolKind::Interface));
        assert_eq!(symbols[3].0, "Foo\\ElseTrait");
        assert!(matches!(symbols[3].1, SymbolKind::Trait));
    }

    #[test]
    fn collects_declarations_in_try_and_blocks() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            try {
                class InTry {}
            } catch (\Exception $e) {
                class InCatch {}
            } finally {
                class InFinally {}
            }

            {
                enum InBlock {}
            }
            "#,
        );

        let names: Vec<&str> = symbols(&analysis).iter().map(|(name, _)| *name).collect();

        assert_eq!(
            names,
            [
                "Foo\\InTry",
                "Foo\\InCatch",
                "Foo\\InFinally",
                "Foo\\InBlock"
            ]
        );
    }

    #[test]
    fn collects_nested_declarations_in_function_bodies() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            function outer()
            {
                function inner() {}

                class InnerClass {}
            }
            "#,
        );

        let names: Vec<&str> = symbols(&analysis).iter().map(|(name, _)| *name).collect();

        assert_eq!(names, ["Foo\\outer", "Foo\\inner", "Foo\\InnerClass"]);
    }

    #[test]
    fn collects_const_declarations() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            const A = 1, B = 2;
            "#,
        );

        let symbols = symbols(&analysis);

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].0, "Foo\\A");
        assert!(matches!(symbols[0].1, SymbolKind::Constant));
        assert_eq!(symbols[1].0, "Foo\\B");
        assert!(matches!(symbols[1].1, SymbolKind::Constant));
    }

    #[test]
    fn collects_define_calls_without_namespacing_them() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            define('GLOBAL_CONST', 1);
            \define('Vendor\\NS_CONST', 2);
            define('\\Leading', 3);

            if (!defined('GUARDED')) {
                define('GUARDED', 4);
            }

            define($dynamic, 5);
            "#,
        );

        let names: Vec<&str> = symbols(&analysis).iter().map(|(name, _)| *name).collect();

        assert_eq!(
            names,
            ["GLOBAL_CONST", "Vendor\\NS_CONST", "Leading", "GUARDED"]
        );
    }

    #[test]
    fn collects_declarations_in_global_namespace() {
        let analysis = analyse_php(
            r#"<?php
            if (!class_exists('Plain')) {
                class Plain {}
            }

            const TOP = 1;
            "#,
        );

        let names: Vec<&str> = symbols(&analysis).iter().map(|(name, _)| *name).collect();

        assert_eq!(names, ["Plain", "TOP"]);
    }

    #[test]
    fn collects_references_from_top_level_code() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            $a = new Bar();
            $b = new TopLevel();
            Bar::run(new InArgs());
            $c = fn(Hint $h) => new InArrow();
            "#,
        );

        let names = reference_names(&analysis);

        assert!(names.contains(&"Vendor\\Bar"));
        assert!(names.contains(&"Foo\\TopLevel"));
        assert!(names.contains(&"Foo\\InArrow"));
    }

    #[test]
    fn collects_references_from_guarded_and_nested_declarations() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            if (!class_exists('Guarded')) {
                class Guarded extends GuardedParent {}
            }

            function outer()
            {
                function inner(Nested $n) {}

                class InnerClass extends InnerParent {}
            }
            "#,
        );

        let names = reference_names(&analysis);

        assert!(names.contains(&"Foo\\GuardedParent"));
        assert!(names.contains(&"Foo\\Nested"));
        assert!(names.contains(&"Foo\\InnerParent"));
    }

    #[test]
    fn collects_references_from_const_and_static_values() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            const A = Other::VALUE;

            function test()
            {
                static $s = new InStatic();
            }
            "#,
        );

        let names = reference_names(&analysis);

        assert!(names.contains(&"Foo\\Other"));
        assert!(names.contains(&"Foo\\Other::VALUE"));
        assert!(names.contains(&"Foo\\InStatic"));
    }

    #[test]
    fn does_not_collect_references_from_use_statements() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;
            use function Vendor\helper;
            "#,
        );

        assert!(reference_names(&analysis).is_empty());
    }

    #[test]
    fn collects_static_call_arguments_and_dynamic_members() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Bar;

            function test($method): void
            {
                Bar::call(new InArgs());
                Bar::{$method}(new InDynArgs());
                Bar::$method(new InVarArgs());
            }
            "#,
        );

        assert_eq!(
            reference_names(&analysis),
            [
                "Vendor\\Bar",
                "Foo\\InArgs",
                "Vendor\\Bar",
                "Foo\\InDynArgs",
                "Vendor\\Bar",
                "Foo\\InVarArgs"
            ]
        );
    }

    #[test]
    fn collects_method_parameter_defaults() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            class Example
            {
                public function run($a = new InDefault(), $b = Other::VALUE) {}

                public function __construct(private $c = new InPromoted()) {}
            }
            "#,
        );

        let names = reference_names(&analysis);

        assert!(names.contains(&"Foo\\InDefault"));
        assert!(names.contains(&"Foo\\Other"));
        assert!(names.contains(&"Foo\\Other::VALUE"));
        assert!(names.contains(&"Foo\\InPromoted"));
    }

    #[test]
    fn collects_enum_constant_and_case_values() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            enum Status: string
            {
                case Active = Config::ACTIVE;
                case Plain = 'plain';

                const DEFAULT = Other::VALUE;
            }
            "#,
        );

        assert_eq!(
            reference_names(&analysis),
            [
                "Foo\\Config",
                "Foo\\Config::ACTIVE",
                "Foo\\Other",
                "Foo\\Other::VALUE"
            ]
        );
    }

    #[test]
    fn analyses_braced_namespaces_separately() {
        let analysis = analyse_php(
            r#"<?php
            namespace One {
                use Vendor\Bar;

                class A extends Bar {}
            }

            namespace Two {
                class B extends Bar {}
            }
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 2);

        let one = &analysis.namespaces[0];
        assert_eq!(one.namespace.as_deref(), Some("One"));
        assert_eq!(one.imports.len(), 1);
        assert_eq!(one.symbols[0].name, "One\\A");
        assert_eq!(one.references[0].name, "Vendor\\Bar");

        // The import from `One` must not leak into `Two`.
        let two = &analysis.namespaces[1];
        assert_eq!(two.namespace.as_deref(), Some("Two"));
        assert!(two.imports.is_empty());
        assert_eq!(two.symbols[0].name, "Two\\B");
        assert_eq!(two.references[0].name, "Two\\Bar");
    }

    #[test]
    fn analyses_multiple_simple_namespaces_separately() {
        let analysis = analyse_php(
            r#"<?php
            namespace One;

            use Vendor\Bar;

            class A extends Bar {}

            namespace Two;

            class B extends Bar {}
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 2);

        let one = &analysis.namespaces[0];
        assert_eq!(one.namespace.as_deref(), Some("One"));
        assert_eq!(one.symbols[0].name, "One\\A");
        assert_eq!(one.references[0].name, "Vendor\\Bar");

        let two = &analysis.namespaces[1];
        assert_eq!(two.namespace.as_deref(), Some("Two"));
        assert!(two.imports.is_empty());
        assert_eq!(two.symbols[0].name, "Two\\B");
        assert_eq!(two.references[0].name, "Two\\Bar");
    }

    #[test]
    fn analyses_global_braced_namespace() {
        let analysis = analyse_php(
            r#"<?php
            namespace {
                class Plain extends Base {}

                function helper() {}
            }

            namespace Named {
                class Other {}
            }
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 2);

        let global = &analysis.namespaces[0];
        assert_eq!(global.namespace, None);
        assert_eq!(global.symbols[0].name, "Plain");
        assert_eq!(global.symbols[1].name, "helper");
        assert_eq!(global.references[0].name, "Base");

        let named = &analysis.namespaces[1];
        assert_eq!(named.namespace.as_deref(), Some("Named"));
        assert_eq!(named.symbols[0].name, "Named\\Other");
    }

    #[test]
    fn analyses_nested_namespace_names() {
        let analysis = analyse_php(
            r#"<?php
            namespace Vendor\Package\Sub;

            class Example extends Sibling {}
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 1);
        assert_eq!(
            analysis.namespaces[0].namespace.as_deref(),
            Some("Vendor\\Package\\Sub")
        );
        assert_eq!(
            analysis.namespaces[0].symbols[0].name,
            "Vendor\\Package\\Sub\\Example"
        );
        assert_eq!(
            analysis.namespaces[0].references[0].name,
            "Vendor\\Package\\Sub\\Sibling"
        );
    }

    #[test]
    fn declare_before_namespace_does_not_add_an_empty_entry() {
        let analysis = analyse_php(
            r#"<?php
            declare(strict_types=1);

            namespace Foo;

            class Example {}
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 1);
        assert_eq!(analysis.namespaces[0].namespace.as_deref(), Some("Foo"));
    }

    #[test]
    fn analyses_file_without_namespace() {
        let analysis = analyse_php(
            r#"<?php
            use Vendor\Bar;

            class Example extends Bar {}
            "#,
        );

        assert_eq!(analysis.namespaces.len(), 1);
        assert_eq!(analysis.namespaces[0].namespace, None);
        assert_eq!(analysis.namespaces[0].symbols[0].name, "Example");
        assert_eq!(analysis.namespaces[0].references[0].name, "Vendor\\Bar");
    }

    #[test]
    fn analyses_empty_file() {
        let analysis = analyse_php("<?php\n");

        assert!(analysis.namespaces.is_empty());
    }

    #[test]
    fn collects_attribute_class_and_arguments() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            use Vendor\Route;

            #[Route('/x', handler: Handler::class), Plain]
            class Example {}
            "#,
        );

        assert_eq!(
            reference_names(&analysis),
            ["Vendor\\Route", "Foo\\Handler", "Foo\\Plain"]
        );
    }

    #[test]
    fn collects_attributes_on_every_declaration_kind() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            #[OnInterface]
            interface I {}

            #[OnTrait]
            trait T {}

            #[OnEnum]
            enum E {
                #[OnCase]
                case A;

                #[OnEnumConst]
                const K = 1;
            }

            #[OnFunction]
            function f(#[OnParam] $p) {}

            #[OnConst]
            const C = 1;

            class Example {
                #[OnConstant]
                const X = 1;

                #[OnProperty]
                public $prop;

                #[OnMethod]
                public function m(#[OnMethodParam] $p) {}

                public function __construct(#[OnPromoted] private $q) {}
            }

            $a = #[OnClosure] function (#[OnClosureParam] $p) {};
            $b = #[OnArrow] fn(#[OnArrowParam] $p) => 1;
            $c = new #[OnAnon] class {};
            "#,
        );

        let names = reference_names(&analysis);

        for expected in [
            "OnInterface",
            "OnTrait",
            "OnEnum",
            "OnCase",
            "OnEnumConst",
            "OnFunction",
            "OnParam",
            "OnConst",
            "OnConstant",
            "OnProperty",
            "OnMethod",
            "OnMethodParam",
            "OnPromoted",
            "OnClosure",
            "OnClosureParam",
            "OnArrow",
            "OnArrowParam",
            "OnAnon",
        ] {
            let qualified = format!("Foo\\{expected}");

            assert!(
                names.contains(&qualified.as_str()),
                "missing attribute reference {qualified}; got {names:?}"
            );
        }
    }

    #[test]
    fn collects_property_hook_references() {
        let analysis = analyse_php(
            r#"<?php
            namespace Foo;

            class Example {
                public string $name {
                    #[OnHook]
                    get => Formatter::format($this->name);

                    set(Value $value) {
                        $this->name = new InHookBody();
                    }
                }
            }
            "#,
        );

        let names = reference_names(&analysis);

        assert!(names.contains(&"Foo\\OnHook"));
        assert!(names.contains(&"Foo\\Formatter"));
        assert!(names.contains(&"Foo\\Value"));
        assert!(names.contains(&"Foo\\InHookBody"));
    }
}
