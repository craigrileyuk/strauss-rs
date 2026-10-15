use super::*;

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
fn collects_the_traits_named_in_trait_adaptations() {
    let analysis = analyse_php(
        r#"<?php
            namespace Foo;

            use Vendor\A;
            use Vendor\B;

            class User
            {
                use A, B, \Other\C {
                    A::hello insteadof B, \Other\C;
                    B::hello as protected hi;
                    \Other\C::hello as other;
                    hello as private;
                }
            }
            "#,
    );

    // The trait list, then each rule; method names are not classes.
    assert_eq!(
        reference_names(&analysis),
        [
            "Vendor\\A",
            "Vendor\\B",
            "Other\\C",
            "Vendor\\A",
            "Vendor\\B",
            "Other\\C",
            "Vendor\\B",
            "Other\\C",
        ]
    );
}
