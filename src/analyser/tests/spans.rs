use php_ast::Span;

use super::*;

fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

fn import_texts(source: &str) -> Vec<(String, Option<String>, Option<String>)> {
    analyse_php(source).namespaces[0]
        .imports
        .iter()
        .map(|import| {
            (
                import.name.clone(),
                import.span.map(|span| text(source, span).to_string()),
                import.alias_span.map(|span| text(source, span).to_string()),
            )
        })
        .collect()
}

fn declaration_texts(source: &str) -> Vec<(String, Option<String>)> {
    analyse_php(source).namespaces[0]
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.name.clone(),
                symbol.name_span.map(|span| text(source, span).to_string()),
            )
        })
        .collect()
}

// ---- namespaces ----

#[test]
fn locates_the_name_of_a_simple_namespace() {
    let source = "<?php\nnamespace Foo\\Bar;\nclass A {}\n";
    let analysis = analyse_php(source);

    assert_eq!(
        text(source, analysis.namespaces[0].name_span.unwrap()),
        "Foo\\Bar"
    );
}

#[test]
fn locates_the_name_of_a_braced_namespace() {
    let source = "<?php\nnamespace Foo\\Bar {\n    class A {}\n}\n";
    let analysis = analyse_php(source);

    assert_eq!(
        text(source, analysis.namespaces[0].name_span.unwrap()),
        "Foo\\Bar"
    );
}

#[test]
fn locates_each_of_several_namespaces() {
    let source = "<?php\nnamespace One;\nclass A {}\nnamespace Two\\Deep;\nclass B {}\n";
    let analysis = analyse_php(source);

    assert_eq!(
        text(source, analysis.namespaces[0].name_span.unwrap()),
        "One"
    );
    assert_eq!(
        text(source, analysis.namespaces[1].name_span.unwrap()),
        "Two\\Deep"
    );
}

#[test]
fn the_global_namespace_has_no_name_to_locate() {
    let analysis = analyse_php("<?php\nclass A {}\n");

    assert_eq!(analysis.namespaces[0].name_span, None);

    let braced = analyse_php("<?php\nnamespace {\n    class A {}\n}\n");

    assert_eq!(braced.namespaces[0].name_span, None);
}

// ---- use ----

#[test]
fn locates_the_names_in_a_use_statement() {
    let source = "<?php\nuse A\\B, C\\D as E;\nuse function X\\y as z;\nuse const X\\Z;\n";

    assert_eq!(
        import_texts(source),
        [
            ("A\\B".into(), Some("A\\B".into()), None),
            ("C\\D".into(), Some("C\\D".into()), Some("E".into())),
            ("X\\y".into(), Some("X\\y".into()), Some("z".into())),
            ("X\\Z".into(), Some("X\\Z".into()), None),
        ]
    );
}

#[test]
fn a_leading_backslash_is_not_part_of_the_name() {
    let source = "<?php\nuse \\A\\B;\nuse function \\A\\c;\n";

    assert_eq!(
        import_texts(source),
        [
            ("A\\B".into(), Some("A\\B".into()), None),
            ("A\\c".into(), Some("A\\c".into()), None),
        ]
    );
}

#[test]
fn locates_the_items_of_a_group_use() {
    let source = "<?php\nuse A\\B\\{C, D\\E as F, function g, const H};\n";
    let analysis = analyse_php(source);
    let namespace = &analysis.namespaces[0];

    // Each item is just what is inside the braces.
    assert_eq!(
        import_texts(source),
        [
            ("A\\B\\C".into(), Some("C".into()), None),
            ("A\\B\\D\\E".into(), Some("D\\E".into()), Some("F".into())),
            ("A\\B\\g".into(), Some("g".into()), None),
            ("A\\B\\H".into(), Some("H".into()), None),
        ]
    );

    // And the statement and its prefix are known.
    assert_eq!(namespace.group_uses.len(), 1);
    assert_eq!(text(source, namespace.group_uses[0].prefix_span), "A\\B");
    assert_eq!(
        text(source, namespace.group_uses[0].span),
        "use A\\B\\{C, D\\E as F, function g, const H};"
    );
    assert!(
        namespace
            .imports
            .iter()
            .all(|import| import.group == Some(0))
    );
}

#[test]
fn a_group_use_can_have_a_trailing_comma_and_span_lines() {
    let source = "<?php\nuse A\\{\n    B,\n    C as D,\n};\n";

    assert_eq!(
        import_texts(source),
        [
            ("A\\B".into(), Some("B".into()), None),
            ("A\\C".into(), Some("C".into()), Some("D".into())),
        ]
    );
}

#[test]
fn a_group_use_of_functions_locates_its_items() {
    let source = "<?php\nuse function A\\{b, c};\n";

    assert_eq!(
        import_texts(source),
        [
            ("A\\b".into(), Some("b".into()), None),
            ("A\\c".into(), Some("c".into()), None),
        ]
    );
}

#[test]
fn imports_know_which_group_they_are_in() {
    let source = "<?php\nuse A\\B;\nuse C\\{D, E};\nuse F\\G;\nuse H\\{I};\n";
    let analysis = analyse_php(source);
    let namespace = &analysis.namespaces[0];

    let groups: Vec<Option<usize>> = namespace
        .imports
        .iter()
        .map(|import| import.group)
        .collect();

    assert_eq!(groups, [None, Some(0), Some(0), None, Some(1)]);
    assert_eq!(text(source, namespace.group_uses[0].prefix_span), "C");
    assert_eq!(text(source, namespace.group_uses[1].prefix_span), "H");
}

#[test]
fn names_that_lex_as_keywords_are_located() {
    let source = "<?php\nuse A\\Enum;\nuse B\\{Match, Readonly as R};\n";

    assert_eq!(
        import_texts(source),
        [
            ("A\\Enum".into(), Some("A\\Enum".into()), None),
            ("B\\Match".into(), Some("Match".into()), None),
            (
                "B\\Readonly".into(),
                Some("Readonly".into()),
                Some("R".into())
            ),
        ]
    );
}

// ---- declarations ----

#[test]
fn locates_the_name_of_each_kind_of_declaration() {
    let source = "<?php\nnamespace Foo;\nclass A {}\ninterface B {}\ntrait C {}\nenum D {}\nfunction e() {}\nconst F = 1;\n";

    assert_eq!(
        declaration_texts(source),
        [
            ("Foo\\A".into(), Some("A".into())),
            ("Foo\\B".into(), Some("B".into())),
            ("Foo\\C".into(), Some("C".into())),
            ("Foo\\D".into(), Some("D".into())),
            ("Foo\\e".into(), Some("e".into())),
            ("Foo\\F".into(), Some("F".into())),
        ]
    );
}

#[test]
fn an_attribute_mentioning_class_does_not_confuse_the_search() {
    let source = "<?php\n#[Attr(Foo::class, Bar::class)]\nabstract class Foo {}\n";

    let analysis = analyse_php(source);
    let span = analysis.namespaces[0].symbols[0].name_span.unwrap();

    assert_eq!(text(source, span), "Foo");
    // The declaration, not the `Foo` in the attribute.
    assert_eq!(span.start as usize, source.rfind("Foo").unwrap());
}

#[test]
fn locates_the_name_of_a_function_returned_by_reference() {
    let source = "<?php\nfunction &foo() {}\n";

    assert_eq!(
        declaration_texts(source),
        [("foo".into(), Some("foo".into()))]
    );
}

#[test]
fn locates_each_constant_in_a_const_statement() {
    let source = "<?php\nconst A = 1, B = 2;\nconst C = 3;\n";

    assert_eq!(
        declaration_texts(source),
        [
            ("A".into(), Some("A".into())),
            ("B".into(), Some("B".into())),
            ("C".into(), Some("C".into())),
        ]
    );
}

#[test]
fn locates_declarations_in_blocks_and_function_bodies() {
    let source = "<?php\nif (true) {\n    function guarded() {}\n    class Guarded {}\n}\nfunction outer() {\n    function inner() {}\n}\n";

    assert_eq!(
        declaration_texts(source),
        [
            ("guarded".into(), Some("guarded".into())),
            ("Guarded".into(), Some("Guarded".into())),
            ("outer".into(), Some("outer".into())),
            ("inner".into(), Some("inner".into())),
        ]
    );
}

#[test]
fn locates_declaration_names_that_lex_as_keywords() {
    // `enum` is a soft keyword: valid as a name, but lexed as a keyword.
    let source = "<?php\nclass Enum {}\nfunction enum() {}\n";

    assert_eq!(
        declaration_texts(source),
        [
            ("Enum".into(), Some("Enum".into())),
            ("enum".into(), Some("enum".into())),
        ]
    );
}

#[test]
fn a_constant_made_with_define_has_no_declaration_name() {
    let analysis = analyse_php("<?php\ndefine('FOO', 1);\n");

    assert_eq!(analysis.namespaces[0].symbols[0].name_span, None);
}

#[test]
fn a_method_with_the_same_name_is_not_mistaken_for_a_function() {
    // The function is found before the class's method of the same name.
    let source = "<?php\nfunction run() {}\nclass A { public function run() {} }\n";

    let analysis = analyse_php(source);
    let function = &analysis.namespaces[0].symbols[0];

    assert_eq!(function.name, "run");
    assert_eq!(function.name_span.unwrap().start, 15);
}

#[test]
fn every_located_declaration_is_found_by_its_local_name() {
    // Over a sample with all the awkward cases at once.
    let source = r#"<?php
namespace Foo;

#[Attr(Foo::class)]
final class Client extends Base implements A, B {}

interface Contract {}
trait Helps {}
enum Suit: string { case Hearts = 'h'; }

function &make() { function nested() {} }

const ONE = 1, TWO = 2;
"#;

    for symbol in &analyse_php(source).namespaces[0].symbols {
        let span = symbol
            .name_span
            .unwrap_or_else(|| panic!("{} has no span", symbol.name));
        let local = symbol.name.rsplit('\\').next().unwrap();

        assert_eq!(text(source, span), local, "{}", symbol.name);
    }
}
