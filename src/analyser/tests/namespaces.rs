use super::*;

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
