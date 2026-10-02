use super::*;

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
