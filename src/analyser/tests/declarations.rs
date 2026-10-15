use super::*;

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
