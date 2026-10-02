use super::*;

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
