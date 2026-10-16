use super::*;

fn rewritten(body: &str) -> String {
    rewrite(&names(), &in_namespace(body))
}

fn expect(body: &str, expected: &str) {
    assert_eq!(rewritten(body), in_namespace(expected));
}

#[test]
fn a_qualified_name_in_a_string_follows_the_rename() {
    expect(
        r#"$a = 'Acme\Core\Client';
$b = 'Acme\Core\Client::create';
$c = 'Acme\Core\Sub\Thing';"#,
        r#"$a = 'P\V\Acme\Core\Client';
$b = 'P\V\Acme\Core\Client::create';
$c = 'P\V\Acme\Core\Sub\Thing';"#,
    );
}

#[test]
fn the_way_the_backslashes_are_written_is_kept() {
    expect(
        r#"$a = 'Acme\\Core\\Client';
$b = "Acme\\Core\\Client";
$c = "\\Acme\\Core\\Client";
$d = "\Acme\Core\Client";"#,
        r#"$a = 'P\\V\\Acme\\Core\\Client';
$b = "P\\V\\Acme\\Core\\Client";
$c = "\\P\\V\\Acme\\Core\\Client";
$d = "\P\V\Acme\Core\Client";"#,
    );
}

#[test]
fn a_namespace_prefix_in_a_string_follows_the_rename() {
    // As in a PSR-4 map.
    expect(
        r#"$map = ['Acme\\Core\\' => 'src/', 'Acme\Core\Sub\\' => 'sub/'];"#,
        r#"$map = ['P\\V\\Acme\\Core\\' => 'src/', 'P\V\Acme\Core\Sub\\' => 'sub/'];"#,
    );
}

#[test]
fn a_name_that_is_not_renamed_is_left_alone() {
    let body = r#"$a = 'Acme\Core\Excluded\Item';
$b = 'Other\Thing';
$c = 'Acme\Corex\Client';
$d = 'Acme';"#;

    assert_eq!(rewritten(body), in_namespace(body));
}

#[test]
fn a_name_in_prose_is_found_between_words() {
    expect(
        r#"$m = "Expected Acme\Core\Client, got {$x}";
$n = 'See \Acme\Core\Sub\Thing.';"#,
        r#"$m = "Expected P\V\Acme\Core\Client, got {$x}";
$n = 'See \P\V\Acme\Core\Sub\Thing.';"#,
    );
}

#[test]
fn an_escape_is_not_taken_for_a_separator() {
    // `\n` is a newline and `\t` a tab; the names after them are names.
    expect(
        r#"$a = "line\nAcme\Core\Client\tend";
$b = "\$Acme\Core\Client";
$c = "$Acme\Core\Client";"#,
        r#"$a = "line\nP\V\Acme\Core\Client\tend";
$b = "\$P\V\Acme\Core\Client";
$c = "$Acme\Core\Client";"#,
    );
}

#[test]
fn a_new_name_that_would_make_an_escape_is_written_doubled() {
    let names = NameMap::default().with(SymbolKind::Namespace, "app", Some("vendor\\app"));

    // Written as it was, `\vendor` in double quotes is a vertical tab.
    assert_eq!(
        rewrite(&names, "<?php\n$a = \"\\app\\Foo\";\n$b = 'app\\Foo';\n"),
        "<?php\n$a = \"\\\\vendor\\\\app\\\\Foo\";\n$b = 'vendor\\app\\Foo';\n"
    );
}

#[test]
fn a_bare_word_is_never_changed() {
    // Whatever it is called, it is more likely a key than a symbol.
    let body = r#"$a = 'g_helper';
$d = ['g_helper' => 1, 'GlobalClass' => 2, 'G_CONST' => 3];"#;

    assert_eq!(rewritten(body), in_namespace(body));
}

#[test]
fn a_callback_is_renamed_unless_excluded_from_string_rename() {
    let source = in_namespace("add_action('init', 'g_helper');\nusort($a, 'g_helper');");

    assert_eq!(
        rewrite(&names(), &source),
        in_namespace("add_action('init', 'p_g_helper');\nusort($a, 'p_g_helper');")
    );

    let excluded = names().without_string_rename(SymbolKind::Function, "g_helper");

    assert_eq!(rewrite(&excluded, &source), source);
}

#[test]
fn a_global_class_is_renamed_in_a_string_only_when_it_is_unambiguous() {
    expect(
        r#"$a = '\GlobalClass';
$b = 'GlobalClass';
$c = 'The GlobalClass class';"#,
        r#"$a = '\P_GlobalClass';
$b = 'GlobalClass';
$c = 'The GlobalClass class';"#,
    );
}

#[test]
fn define_and_function_exists_strings_are_not_edited_twice() {
    expect(
        r#"define('Acme\Core\VERSION', 1);
if (function_exists('Acme\Core\helper')) {}
$a = 'Acme\Core\VERSION';"#,
        r#"define('P\V\Acme\Core\VERSION', 1);
if (function_exists('P\V\Acme\Core\helper')) {}
$a = 'P\V\Acme\Core\VERSION';"#,
    );
}

#[test]
fn heredocs_are_left_alone() {
    let body = "$a = <<<TXT\nAcme\\Core\\Client\nTXT;\n$b = <<<'TXT'\n\\Acme\\Core\\Client\nTXT;";

    assert_eq!(rewritten(body), in_namespace(body));
}

#[test]
fn names_in_comments_follow_the_rename() {
    expect(
        "// Acme\\Core\\Client\n# see \\Acme\\Core\\Sub\\Thing\n/* \\Acme\\Core\\Client */",
        "// P\\V\\Acme\\Core\\Client\n# see \\P\\V\\Acme\\Core\\Sub\\Thing\n/* \\P\\V\\Acme\\Core\\Client */",
    );
}

#[test]
fn escapes_in_a_regular_expression_are_not_names() {
    // `\.` and `\d` are regular expression syntax. `g_helper`, `GlobalClass` and
    // `Acme` would be names if read as `.g_helper\.`.
    let body = r#"$a = '#/\.g_helper\.[^/]+$#';
$b = '/\d+\.GlobalClass\/x/';
$c = 'g_helper\\1es';
$d = '/acme\.com\/Acme\.php/i';
$e = '\GlobalClass::create';"#;

    assert_eq!(rewritten(body), in_namespace(body));
}

#[test]
fn a_name_in_a_regular_expression_is_written_with_its_backslashes() {
    expect(
        r#"$a = '/^Acme\\\\Core\\\\Client::__construct$/';"#,
        r#"$a = '/^P\\\\V\\\\Acme\\\\Core\\\\Client::__construct$/';"#,
    );
}

#[test]
fn a_trailing_separator_in_the_middle_of_a_string_is_left_where_it_is() {
    expect(
        r#"$a = ['Acme\\Core\\', 'Acme\Core\ is a namespace'];"#,
        r#"$a = ['P\\V\\Acme\\Core\\', 'P\V\Acme\Core\ is a namespace'];"#,
    );
}

#[test]
fn a_regular_expression_next_to_a_one_word_namespace_is_left_alone() {
    let names = NameMap::default().with(SymbolKind::Namespace, "Google", Some("P\\Google"));

    let source = "<?php\n$a = '#developers\\.google\\.com#i';\n$b = 'Google\\\\';\n";

    assert_eq!(
        rewrite(&names, source),
        "<?php\n$a = '#developers\\.google\\.com#i';\n$b = 'P\\\\Google\\\\';\n"
    );
}

#[test]
fn a_function_excluded_from_string_rename_is_left_alone_even_when_namespaced() {
    let names = names().without_string_rename(SymbolKind::Function, "Acme\\Core\\helper");

    let source = in_namespace("if (function_exists('Acme\\Core\\helper')) {}");

    assert_eq!(rewrite(&names, &source), source);
}

#[test]
fn a_callback_given_by_name_is_not_guessed_at() {
    // With named arguments the second one is not necessarily the callback.
    let body = "add_action(callback: 'init', hook: 'g_helper');";

    assert_eq!(rewritten(body), in_namespace(body));
}
