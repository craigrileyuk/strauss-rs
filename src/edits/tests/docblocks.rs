use super::*;

fn expect(body: &str, expected: &str) {
    assert_eq!(
        rewrite(&names(), &in_namespace(body)),
        in_namespace(expected)
    );
}

#[test]
fn types_in_tags_follow_the_rename() {
    expect(
        r#"use Acme\Core\Client;

/**
 * Makes a thing.
 *
 * @param \Acme\Core\Client $a
 * @param Client $b
 * @param array<string, \Acme\Core\Sub\Thing>|null $c
 * @return \GlobalClass
 * @throws \Acme\Core\Client|\Acme\Core\Sub\Thing
 * @see \Acme\Core\Client::create()
 */
function make($a, $b, $c) {}"#,
        r#"use P\V\Acme\Core\Client;

/**
 * Makes a thing.
 *
 * @param \P\V\Acme\Core\Client $a
 * @param Client $b
 * @param array<string, \P\V\Acme\Core\Sub\Thing>|null $c
 * @return \P_GlobalClass
 * @throws \P\V\Acme\Core\Client|\P\V\Acme\Core\Sub\Thing
 * @see \P\V\Acme\Core\Client::create()
 */
function make($a, $b, $c) {}"#,
    );
}

#[test]
fn prose_is_left_alone() {
    let body = r#"/**
 * Wraps \Acme\Core\Client so that Acme\Core\Sub\Thing works.
 *
 * Returns a \GlobalClass.
 */
class C {}"#;

    assert_eq!(rewrite(&names(), &in_namespace(body)), in_namespace(body));
}

#[test]
fn names_are_read_as_they_are_in_code() {
    // Imported, the name follows the import; relative to the namespace, it
    // follows the namespace; neither is touched.
    let source = r#"<?php
namespace Acme\Core;

/**
 * @var Client $client
 * @var Sub\Thing $thing
 * @param \Acme\Core\Client $x
 */
class User {}
"#;

    assert_eq!(
        rewrite(&names(), source),
        source
            .replace("namespace Acme\\Core;", "namespace P\\V\\Acme\\Core;")
            .replace("\\Acme\\Core\\Client $x", "\\P\\V\\Acme\\Core\\Client $x")
    );
}

#[test]
fn a_global_class_in_a_global_file_is_renamed() {
    let source =
        "<?php\n/**\n * @param GlobalClass $a\n * @return GlobalClass[]\n */\nfunction f($a) {}\n";

    assert_eq!(
        rewrite(&names(), source),
        "<?php\n/**\n * @param P_GlobalClass $a\n * @return P_GlobalClass[]\n */\nfunction f($a) {}\n"
    );
}

#[test]
fn types_that_are_not_classes_are_left_alone() {
    let body = r#"/**
 * @param int|string|null $a
 * @param class-string<\Acme\Core\Client> $b
 * @param array{key: int, other: \GlobalClass} $c
 * @param self|static|$this $d
 * @return list<int>
 */
function f($a, $b, $c, $d) {}"#;

    let expected = body
        .replace("<\\Acme\\Core\\Client>", "<\\P\\V\\Acme\\Core\\Client>")
        .replace("other: \\GlobalClass", "other: \\P_GlobalClass");

    assert_eq!(
        rewrite(&names(), &in_namespace(body)),
        in_namespace(&expected)
    );
}

#[test]
fn a_one_line_docblock_is_read() {
    expect(
        "/** @var \\Acme\\Core\\Client $x */\n$x = null;",
        "/** @var \\P\\V\\Acme\\Core\\Client $x */\n$x = null;",
    );
}

#[test]
fn a_class_constant_is_not_a_class() {
    // `GlobalClass` after `::` is a constant that happens to share a name.
    let source =
        "<?php\n/**\n * @see Client::GlobalClass\n * @return GlobalClass\n */\nfunction f() {}\n";

    assert_eq!(
        rewrite(&names(), source),
        "<?php\n/**\n * @see Client::GlobalClass\n * @return P_GlobalClass\n */\nfunction f() {}\n"
    );
}
