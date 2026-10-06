#![allow(dead_code)]
#![allow(unused_imports)]
use php_ast::{
    Span,
    ast::{BinaryOp, UseKind},
    owned::{
        Arg, ArrowFunctionExpr, CallableCreateKind, CatchClause, ClassConstDecl, ClassDecl,
        ClassMember, ClassMemberKind, ClosureExpr, DoWhileStmt, EnumDecl, EnumMemberKind, Expr,
        ExprKind, ForStmt, ForeachStmt, FunctionDecl, IfStmt, InterfaceDecl, MatchExpr, MethodDecl,
        Name, NamespaceBody, Program, PropertyDecl, StaticAccessExpr, StaticDynMethodCallExpr,
        Stmt, StmtKind, SwitchBody, SwitchCase, SwitchStmt, TraitDecl, TraitUseDecl, TryCatchStmt,
        TypeHint, TypeHintKind, WhileStmt, YieldExpr,
    },
    resolve::{NameContext, NameResolver, ResolvedName},
};

#[derive(Debug)]
pub struct Reference {
    pub name: String,
    pub span: Span,
    pub kind: ReferenceKind,
}

#[derive(Debug)]
pub enum ReferenceKind {
    ClassName,
    FunctionName,
    ConstantName,
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
}

struct AnalysisContext<'a> {
    namespace: Option<&'a str>,
    imports: &'a [Import],
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

            StmtKind::Class(class) => {
                if let Some(Some(name)) = &class.name {
                    let full_name = qualify_declared_name(name, namespace);

                    symbols.push(DeclaredSymbol {
                        name: full_name,
                        kind: SymbolKind::Class,
                    });
                }
            }

            StmtKind::Interface(interface) => {
                if let Some(name) = &interface.name {
                    let full_name = qualify_declared_name(name, namespace);

                    symbols.push(DeclaredSymbol {
                        name: full_name,
                        kind: SymbolKind::Interface,
                    });
                }
            }

            StmtKind::Trait(trait_decl) => {
                if let Some(name) = &trait_decl.name {
                    let full_name = qualify_declared_name(name, namespace);

                    symbols.push(DeclaredSymbol {
                        name: full_name,
                        kind: SymbolKind::Trait,
                    });
                }
            }

            StmtKind::Enum(enum_decl) => {
                if let Some(name) = &enum_decl.name {
                    let full_name = qualify_declared_name(name, namespace);

                    symbols.push(DeclaredSymbol {
                        name: full_name,
                        kind: SymbolKind::Enum,
                    });
                }
            }

            StmtKind::Function(function) => {
                if let Some(name) = &function.name {
                    let full_name = qualify_declared_name(name, namespace);

                    symbols.push(DeclaredSymbol {
                        name: full_name,
                        kind: SymbolKind::Function,
                    });
                }
            }

            _ => {}
        }
    }

    let context = AnalysisContext {
        namespace,
        imports: &imports,
        resolver: &resolver,
    };

    let mut references = Vec::new();

    // Pass 2: collect references.
    for stmt in stmts.iter() {
        match &stmt.kind {
            StmtKind::Class(class) => {
                collect_class(class, &context, &mut references);
            }

            StmtKind::Interface(interface) => {
                collect_interface(interface, &context, &mut references);
            }

            StmtKind::Trait(trait_decl) => {
                collect_trait(trait_decl, &context, &mut references);
            }

            StmtKind::Enum(enum_decl) => {
                collect_enum(enum_decl, &context, &mut references);
            }

            StmtKind::Function(function) => {
                collect_function(function, &context, &mut references);
            }

            _ => {}
        }
    }

    analysis.namespaces.push(NamespaceAnalysis {
        namespace: namespace.map(String::from),
        imports,
        symbols,
        references,
    });
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
            collect_expr(access.property.as_ref(), context, references);
        }

        ExprKind::NullsafeMethodCall(call) => {
            collect_expr(call.object.as_ref(), context, references);
            collect_expr(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::StaticDynMethodCall(call) => {
            collect_expr(call.class.as_ref(), context, references);
            collect_expr(call.method.as_ref(), context, references);

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
            if binary.op == BinaryOp::Instanceof {
                collect_class_expr(binary.right.as_ref(), context, references);
            }

            collect_expr(binary.left.as_ref(), context, references);
            collect_expr(binary.right.as_ref(), context, references);
        }

        ExprKind::StaticMethodCall(call) => {
            collect_class_expr(&call.class, context, references);
        }

        ExprKind::StaticPropertyAccess(access) => {
            collect_class_expr(&access.class, context, references);
        }

        ExprKind::ClassConstAccess(access) => {
            collect_class_expr(&access.class, context, references);

            if let ExprKind::Identifier(name) = &access.member.kind {
                if let Some(class_name) = resolve_class_expr_name(&access.class, context) {
                    references.push(Reference {
                        name: format!("{class_name}::{name}"),
                        span: access.member.span,
                        kind: ReferenceKind::ConstantName,
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
            collect_expr(call.method.as_ref(), context, references);

            for arg in call.args.iter() {
                collect_arg(arg, context, references);
            }
        }

        ExprKind::PropertyAccess(access) => {
            collect_expr(access.object.as_ref(), context, references);
            collect_expr(access.property.as_ref(), context, references);
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
            collect_expr(member.as_ref(), context, references);
        }

        ExprKind::ClassConstAccessDynamic { class, member } => {
            collect_class_expr(class.as_ref(), context, references);
            collect_expr(member.as_ref(), context, references);
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
                collect_expr(method.as_ref(), context, references);
            }

            CallableCreateKind::StaticMethod { class, method } => {
                collect_class_expr(class.as_ref(), context, references);
                collect_expr(method.as_ref(), context, references);
            }
        },

        ExprKind::Identifier(name) => {
            references.push(Reference {
                name: resolve_constant_name(name, context.namespace, context.imports),
                span: expr.span,
                kind: ReferenceKind::ConstantName,
            });
        }

        _ => {}
    }
}

fn collect_closure(
    closure: &ClosureExpr,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for param in closure.params.iter() {
        if let Some(type_hint) = &param.type_hint {
            collect_type_hint(type_hint, context, references);
        }

        if let Some(default) = &param.default {
            collect_expr(default, context, references);
        }
    }

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
    for param in arrow.params.iter() {
        if let Some(type_hint) = &param.type_hint {
            collect_type_hint(type_hint, context, references);
        }

        if let Some(default) = &param.default {
            collect_expr(default, context, references);
        }
    }

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
        ExprKind::Identifier(name) => {
            Some(resolve_class_name(name, context.namespace, context.imports))
        }

        _ => None,
    }
}

fn collect_class_expr(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
    match &expr.kind {
        ExprKind::Identifier(name) => {
            references.push(Reference {
                name: resolve_class_name(name, context.namespace, context.imports),
                span: expr.span,
                kind: ReferenceKind::ClassName,
            });
        }

        ExprKind::AnonymousClass(class) => {
            collect_class(class.as_ref(), context, references);
        }

        _ => {}
    }
}

fn resolve_class_name(name: &str, namespace: Option<&str>, imports: &[Import]) -> String {
    if let Some(name) = name.strip_prefix('\\') {
        return name.to_string();
    }

    if let Some(name) = name.strip_prefix("namespace\\") {
        return match namespace {
            Some(namespace) if !namespace.is_empty() => {
                format!("{namespace}\\{name}")
            }
            _ => name.to_string(),
        };
    }

    for import in imports {
        if !matches!(import.kind, ImportKind::Class) {
            continue;
        }

        let imported_name = import.name.rsplit('\\').next().unwrap_or(&import.name);

        let matches = match &import.alias {
            Some(alias) => alias == name,
            None => imported_name == name,
        };

        if matches {
            return import.name.clone();
        }
    }

    if name.contains('\\') {
        return match namespace {
            Some(namespace) if !namespace.is_empty() => {
                format!("{namespace}\\{name}")
            }
            _ => name.to_string(),
        };
    }

    match namespace {
        Some(namespace) if !namespace.is_empty() => {
            format!("{namespace}\\{name}")
        }
        _ => name.to_string(),
    }
}

fn collect_function_name(expr: &Expr, context: &AnalysisContext, references: &mut Vec<Reference>) {
    if let ExprKind::Identifier(name) = &expr.kind {
        references.push(Reference {
            name: resolve_function_name(name, context.namespace, context.imports),
            span: expr.span,
            kind: ReferenceKind::FunctionName,
        });
    }
}

fn resolve_function_name(name: &str, namespace: Option<&str>, imports: &[Import]) -> String {
    // Fully-qualified
    // \Foo\bar -> Foo\bar
    if let Some(name) = name.strip_prefix('\\') {
        return name.to_string();
    }

    // Imported function:
    //
    // use function Vendor\foo;
    // foo() -> Vendor\foo
    //
    // use function Vendor\foo as bar;
    // bar() -> Vendor\foo
    for import in imports {
        if !matches!(import.kind, ImportKind::Function) {
            continue;
        }

        let imported_name = import.name.rsplit('\\').next().unwrap_or(&import.name);

        let matches = match &import.alias {
            Some(alias) => alias == name,
            None => imported_name == name,
        };

        if matches {
            return import.name.clone();
        }
    }

    // Function imports will be handled once Import
    // preserves the import kind.
    match namespace {
        Some(namespace) if !namespace.is_empty() => {
            format!("{namespace}\\{name}")
        }
        _ => name.to_string(),
    }
}

fn resolve_constant_name(name: &str, namespace: Option<&str>, imports: &[Import]) -> String {
    if let Some(name) = name.strip_prefix('\\') {
        return name.to_string();
    }

    // An imported constant is only matched when the identifier
    // itself is the imported name/alias.
    if !name.contains('\\') {
        for import in imports {
            if !matches!(import.kind, ImportKind::Constant) {
                continue;
            }

            let imported_name = import.name.rsplit('\\').next().unwrap_or(&import.name);

            let matches = match &import.alias {
                Some(alias) => alias == name,
                None => imported_name == name,
            };

            if matches {
                return import.name.clone();
            }
        }
    }

    match namespace {
        Some(namespace) if !namespace.is_empty() => {
            format!("{namespace}\\{name}")
        }
        _ => name.to_string(),
    }
}

fn collect_stmt(stmt: &Stmt, context: &AnalysisContext, references: &mut Vec<Reference>) {
    match &stmt.kind {
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

    if let ResolvedName::Fqn(resolved_name) = resolved {
        references.push(Reference {
            name: resolved_name,
            span: name.span,
            kind: ReferenceKind::ClassName,
        });
    }
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
    if let Some(type_hint) = &property.type_hint {
        collect_type_hint(type_hint, context, references);
    }

    if let Some(default) = &property.default {
        collect_expr(default, context, references);
    }
}

fn collect_interface(
    interface: &InterfaceDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
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

            EnumMemberKind::ClassConst(_) => {
                // Constants may contain class references through their
                // values. We'll handle those when we add constant
                // expression traversal.
            }

            EnumMemberKind::Case(_) => {
                // Enum case declarations don't introduce class references.
            }
        }
    }
}

fn collect_function(
    function: &FunctionDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for param in function.params.iter() {
        if let Some(type_hint) = &param.type_hint {
            collect_type_hint(type_hint, context, references);
        }

        if let Some(default) = &param.default {
            collect_expr(default, context, references);
        }
    }

    if let Some(return_type) = &function.return_type {
        collect_type_hint(return_type, context, references);
    }

    for stmt in function.body.stmts.iter() {
        collect_stmt(stmt, context, references);
    }
}

fn collect_method(method: &MethodDecl, context: &AnalysisContext, references: &mut Vec<Reference>) {
    for param in method.params.iter() {
        if let Some(type_hint) = &param.type_hint {
            collect_type_hint(type_hint, context, references);
        }
    }

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

            if let ResolvedName::Fqn(resolved_name) = resolved {
                references.push(Reference {
                    name: resolved_name,
                    span: name.span,
                    kind: ReferenceKind::ClassName,
                });
            }
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
        assert!(matches!(references[1].kind, ReferenceKind::ConstantName));
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
        assert!(matches!(references[1].kind, ReferenceKind::ConstantName));
    }
}
