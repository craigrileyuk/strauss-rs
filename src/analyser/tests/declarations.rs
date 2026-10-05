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

fn declared<'a>(analysis: &'a FileAnalysis, name: &str) -> &'a DeclaredSymbol {
    analysis.namespaces[0]
        .symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| panic!("{name} not declared"))
}

#[test]
fn records_what_a_class_extends_and_implements() {
    let analysis = analyse_php(
        r#"<?php
        namespace Foo;

        use Vendor\Base;
        use Vendor\Contracts\{Countable as Countable2, Jsonable};

        abstract class Example extends Base implements \Stringable, Jsonable, Countable2, Local {}
        final class Plain {}
        "#,
    );

    let example = declared(&analysis, "Foo\\Example");

    assert!(example.is_abstract);
    assert_eq!(example.extends, ["Vendor\\Base"]);
    assert_eq!(
        example.implements,
        [
            "Stringable",
            "Vendor\\Contracts\\Jsonable",
            "Vendor\\Contracts\\Countable",
            "Foo\\Local"
        ]
    );

    let plain = declared(&analysis, "Foo\\Plain");

    assert!(!plain.is_abstract);
    assert!(plain.extends.is_empty());
    assert!(plain.implements.is_empty());
}

#[test]
fn records_what_an_interface_extends() {
    let analysis = analyse_php(
        r#"<?php
        namespace Foo;

        use Vendor\Base;

        interface Example extends Base, \Countable, Sibling {}
        "#,
    );

    assert_eq!(
        declared(&analysis, "Foo\\Example").extends,
        ["Vendor\\Base", "Countable", "Foo\\Sibling"]
    );
}

#[test]
fn records_an_enums_backing_type_and_interfaces() {
    let analysis = analyse_php(
        r#"<?php
        namespace Foo;

        use Vendor\HasLabel;

        enum Status: string implements HasLabel {}
        enum Level: int {}
        enum Plain {}
        "#,
    );

    let status = declared(&analysis, "Foo\\Status");

    assert_eq!(status.backing_type.as_deref(), Some("string"));
    assert_eq!(status.implements, ["Vendor\\HasLabel"]);
    assert_eq!(
        declared(&analysis, "Foo\\Level").backing_type.as_deref(),
        Some("int")
    );
    assert_eq!(declared(&analysis, "Foo\\Plain").backing_type, None);
}

#[test]
fn parents_resolve_through_imports_declared_after_the_class() {
    // The resolver sees every import in the namespace, as references do.
    let analysis = analyse_php(
        r#"<?php
        namespace Foo;

        class Example extends Base {}

        use Vendor\Base;
        "#,
    );

    assert_eq!(
        declared(&analysis, "Foo\\Example").extends,
        ["Vendor\\Base"]
    );
}

#[test]
fn only_classes_interfaces_and_enums_have_parents() {
    let analysis = analyse_php(
        r#"<?php
        namespace Foo;

        trait T {}
        function f() {}
        const C = 1;
        "#,
    );

    for name in ["Foo\\T", "Foo\\f", "Foo\\C"] {
        let symbol = declared(&analysis, name);

        assert!(!symbol.is_abstract && symbol.extends.is_empty() && symbol.implements.is_empty());
        assert_eq!(symbol.backing_type, None);
    }
}
