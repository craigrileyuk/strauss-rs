use super::*;

fn rewritten(body: &str) -> String {
    rewrite(&names(), &in_namespace(body))
}

#[test]
fn an_import_of_a_renamed_class_is_rewritten_and_its_uses_are_not() {
    assert_eq!(
        rewritten("use Acme\\Core\\Client;\n\n$c = new Client();"),
        in_namespace("use P\\V\\Acme\\Core\\Client;\n\n$c = new Client();")
    );
}

#[test]
fn an_alias_is_kept() {
    assert_eq!(
        rewritten("use Acme\\Core\\Client as C;\n\n$c = new C();"),
        in_namespace("use P\\V\\Acme\\Core\\Client as C;\n\n$c = new C();")
    );
}

#[test]
fn function_and_constant_imports_are_rewritten() {
    assert_eq!(
        rewritten(
            "use function Acme\\Core\\helper;\nuse const Acme\\Core\\VERSION;\n\nhelper();\necho VERSION;"
        ),
        in_namespace(
            "use function P\\V\\Acme\\Core\\helper;\nuse const P\\V\\Acme\\Core\\VERSION;\n\nhelper();\necho VERSION;"
        )
    );
}

#[test]
fn a_global_symbol_imported_keeps_the_name_the_code_uses_for_it() {
    // `use GlobalClass;` makes `GlobalClass` mean it. Importing `P_GlobalClass` would
    // make `P_GlobalClass` mean it, and break the code that says `GlobalClass`.
    assert_eq!(
        rewritten(
            "use GlobalClass;\nuse function g_helper;\nuse const G_CONST;\n\nnew GlobalClass();\ng_helper();\necho G_CONST;"
        ),
        in_namespace(
            "use P_GlobalClass as GlobalClass;\nuse function p_g_helper as g_helper;\nuse const P_C_G_CONST as G_CONST;\n\nnew GlobalClass();\ng_helper();\necho G_CONST;"
        )
    );
}

#[test]
fn an_alias_that_was_already_there_is_used_as_it_is() {
    assert_eq!(
        rewritten("use GlobalClass as G;\n\nnew G();"),
        in_namespace("use P_GlobalClass as G;\n\nnew G();")
    );
}

#[test]
fn a_leading_backslash_is_kept() {
    assert_eq!(
        rewritten("use \\Acme\\Core\\Client;"),
        in_namespace("use \\P\\V\\Acme\\Core\\Client;")
    );
}

#[test]
fn imports_that_are_not_renamed_are_left_alone() {
    let source = in_namespace(
        "use Countable;\nuse Other\\Thing;\nuse function strlen;\nuse Acme\\Core\\Excluded\\Item;",
    );

    assert_eq!(rewrite(&names(), &source), source);
}

#[test]
fn several_imports_in_one_statement_are_each_handled() {
    assert_eq!(
        rewritten("use Acme\\Core\\Client, Other\\Thing, GlobalClass;"),
        in_namespace("use P\\V\\Acme\\Core\\Client, Other\\Thing, P_GlobalClass as GlobalClass;")
    );
}

#[test]
fn a_namespace_can_be_imported() {
    assert_eq!(
        rewritten("use Acme\\Core\\Sub;\n\n$x = new Sub\\Thing();"),
        in_namespace("use P\\V\\Acme\\Core\\Sub;\n\n$x = new Sub\\Thing();")
    );
}

#[test]
fn an_import_of_a_class_not_found_in_a_renamed_namespace_follows_it() {
    assert_eq!(
        rewritten("use Acme\\Core\\Missing;"),
        in_namespace("use P\\V\\Acme\\Core\\Missing;")
    );
}

// ---- group use ----

#[test]
fn a_group_use_that_moves_as_one_has_its_prefix_changed() {
    assert_eq!(
        rewritten("use Acme\\Core\\Sub\\{Thing, Other as O};\n\nnew Thing();\nnew O();"),
        in_namespace("use P\\V\\Acme\\Core\\Sub\\{Thing, Other as O};\n\nnew Thing();\nnew O();")
    );
}

#[test]
fn a_group_use_of_functions_and_constants_has_its_prefix_changed() {
    assert_eq!(
        rewritten("use Acme\\Core\\{function helper, const VERSION};"),
        in_namespace("use P\\V\\Acme\\Core\\{function helper, const VERSION};")
    );
}

#[test]
fn a_group_use_keeps_a_leading_backslash() {
    assert_eq!(
        rewritten("use \\Acme\\Core\\{Client, Sub\\Thing};"),
        in_namespace("use \\P\\V\\Acme\\Core\\{Client, Sub\\Thing};")
    );
}

#[test]
fn a_group_use_that_does_not_change_is_left_alone() {
    let source = in_namespace("use Other\\{A, B as C};\nuse function Other\\{d, e};");

    assert_eq!(rewrite(&names(), &source), source);
}

#[test]
fn a_group_use_whose_items_go_different_ways_becomes_one_statement_each() {
    let source = r#"<?php
namespace Acme\Consumer {
    use Acme\Core\{Client, Excluded\Item, function helper, const VERSION};

    class A {}
}
"#;

    assert_eq!(
        rewrite(&names(), source),
        r#"<?php
namespace Acme\Consumer {
    use P\V\Acme\Core\Client;
    use Acme\Core\Excluded\Item;
    use function P\V\Acme\Core\helper;
    use const P\V\Acme\Core\VERSION;

    class A {}
}
"#
    );
}

#[test]
fn an_expanded_group_use_keeps_aliases() {
    assert_eq!(
        rewritten("use Acme\\Core\\{Client as C, Excluded\\Item as I};\n\nnew C();\nnew I();"),
        in_namespace(
            "use P\\V\\Acme\\Core\\Client as C;\nuse Acme\\Core\\Excluded\\Item as I;\n\nnew C();\nnew I();"
        )
    );
}

#[test]
fn imports_are_updated_in_a_file_that_keeps_its_own_namespace() {
    let source = r#"<?php
namespace Acme\Keep;

use Acme\Core\Client;
use Acme\Core\Sub\{Thing};

class Kept extends Client {}
"#;

    assert_eq!(
        rewrite_with(&names(), source, true),
        r#"<?php
namespace Acme\Keep;

use P\V\Acme\Core\Client;
use P\V\Acme\Core\Sub\{Thing};

class Kept extends Client {}
"#
    );
}
