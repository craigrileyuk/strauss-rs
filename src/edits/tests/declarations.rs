use super::*;

fn global_names() -> NameMap {
    names()
        .with(
            SymbolKind::Interface,
            "GlobalContract",
            Some("P_GlobalContract"),
        )
        .with(SymbolKind::Trait, "GlobalTrait", Some("P_GlobalTrait"))
        .with(SymbolKind::Enum, "GlobalEnum", Some("P_GlobalEnum"))
        .with(SymbolKind::Constant, "G_DEFINED", Some("P_C_G_DEFINED"))
}

#[test]
fn global_declarations_are_renamed() {
    let source = r#"<?php
class GlobalClass {}
interface GlobalContract {}
trait GlobalTrait {}
enum GlobalEnum {}
function g_helper() {}
const G_CONST = 1;
"#;

    assert_eq!(
        rewrite(&global_names(), source),
        r#"<?php
class P_GlobalClass {}
interface P_GlobalContract {}
trait P_GlobalTrait {}
enum P_GlobalEnum {}
function p_g_helper() {}
const P_C_G_CONST = 1;
"#
    );
}

#[test]
fn a_declaration_in_a_namespace_keeps_its_name() {
    let source = r#"<?php
namespace Acme\Core;

class Client {}
function helper() {}
const VERSION = 1;
"#;

    // The namespace is what changes.
    assert_eq!(
        rewrite(&names(), source),
        source.replace("namespace Acme\\Core;", "namespace P\\V\\Acme\\Core;")
    );
}

#[test]
fn only_the_declaration_name_changes() {
    let source = "<?php\n#[Attr(GlobalClass::class)]\nfinal class GlobalClass extends Base implements Countable {}\n";

    let changed = rewrite(&names(), source);

    assert!(
        changed.contains("final class P_GlobalClass extends Base"),
        "{changed}"
    );
    // The mention in the attribute is a reference, in the global namespace.
    assert!(
        changed.contains("#[Attr(P_GlobalClass::class)]"),
        "{changed}"
    );
}

#[test]
fn a_global_name_declared_in_a_file_that_keeps_its_namespaces_is_left_alone() {
    let source = "<?php\nclass GlobalClass {}\n";

    assert_eq!(rewrite_with(&names(), source, true), source);
}

#[test]
fn constants_made_with_define_are_renamed_in_the_string() {
    let source = "<?php\ndefine('G_DEFINED', 1);\nvar_dump(defined(\"G_DEFINED\"));\n\\define('G_DEFINED', 2);\n";

    assert_eq!(
        rewrite(&global_names(), source),
        "<?php\ndefine('P_C_G_DEFINED', 1);\nvar_dump(defined(\"P_C_G_DEFINED\"));\n\\define('P_C_G_DEFINED', 2);\n"
    );
}

#[test]
fn a_string_naming_a_function_is_renamed() {
    let source = r#"<?php
function_exists('g_helper');
call_user_func("\\g_helper");
function_exists('Acme\Core\helper');
function_exists('Acme\\Core\\helper');
function_exists('\Acme\Core\helper');
function_exists('Other\func');
function_exists('not_a_symbol');
"#;

    assert_eq!(
        rewrite(&names(), source),
        r#"<?php
function_exists('p_g_helper');
call_user_func("\\p_g_helper");
function_exists('P\V\Acme\Core\helper');
function_exists('P\\V\\Acme\\Core\\helper');
function_exists('\P\V\Acme\Core\helper');
function_exists('Other\func');
function_exists('not_a_symbol');
"#
    );
}

#[test]
fn a_string_is_renamed_only_where_it_names_a_function() {
    // A string that is a function's name is only that where something takes a
    // callable. Everywhere else it is as likely an array key.
    let source = "<?php\n$x = 'g_helper';\nstrtoupper('g_helper');\nfunction_exists('g_helper_not');\narray_map('g_helper', $a);\nusort($a, 'g_helper');\nadd_action('init', 'g_helper');\n";

    assert_eq!(
        rewrite(&names(), source),
        "<?php\n$x = 'g_helper';\nstrtoupper('g_helper');\nfunction_exists('g_helper_not');\narray_map('p_g_helper', $a);\nusort($a, 'p_g_helper');\nadd_action('init', 'p_g_helper');\n"
    );
}

#[test]
fn a_function_name_that_is_not_the_one_renamed_is_not_a_match() {
    // `G_CONST` is a constant; `function_exists` asks about functions.
    let source = "<?php\nfunction_exists('G_CONST');\ndefined('g_helper');\n";

    assert_eq!(rewrite(&names(), source), source);
}

// ---- when something can't be found ----

fn plan_with_analysis(
    source: &str,
    change: impl FnOnce(&mut crate::analyser::FileAnalysis),
) -> Planned {
    let result = parse(source);
    let mut analysis = analyse(source, &result.program);

    change(&mut analysis);

    plan_file(
        &names(),
        &FileInput {
            file: "pkg/file.php",
            source,
            analysis: &analysis,
            keeps_namespaces: false,
        },
    )
}

#[test]
fn says_when_it_cannot_find_a_declaration() {
    let planned = plan_with_analysis("<?php\nclass GlobalClass {}\n", |analysis| {
        analysis.namespaces[0].symbols[0].name_span = None;
    });

    assert!(planned.edits.is_empty());
    assert_eq!(
        planned.warnings,
        ["pkg/file.php: could not find where GlobalClass is declared"]
    );
}

#[test]
fn says_when_it_cannot_find_a_define() {
    // A constant to rename, but no `define()` string for it.
    let planned = plan_with_analysis("<?php\ndefine('G_CONST', 1);\n", |analysis| {
        analysis.namespaces[0].regions.clear();
    });

    assert_eq!(
        planned.warnings,
        ["pkg/file.php: could not find where the constant G_CONST is defined"]
    );
}

#[test]
fn says_when_it_cannot_find_a_namespace_declaration() {
    let planned = plan_with_analysis("<?php\nnamespace Acme\\Core;\n", |analysis| {
        analysis.namespaces[0].name_span = None;
    });

    assert_eq!(
        planned.warnings,
        ["pkg/file.php: could not find where `namespace Acme\\Core;` is written"]
    );
}

#[test]
fn says_when_it_cannot_find_an_import() {
    let planned = plan_with_analysis(
        "<?php\nnamespace Acme\\Consumer;\nuse Acme\\Core\\Client;\n",
        |analysis| {
            analysis.namespaces[0].imports[0].span = None;
        },
    );

    assert_eq!(
        planned.warnings,
        ["pkg/file.php: could not find where `use Acme\\Core\\Client;` is written"]
    );
}

#[test]
fn says_when_it_cannot_find_the_items_of_a_group_use() {
    let planned = plan_with_analysis(
        "<?php\nnamespace Acme\\Consumer;\nuse Acme\\Core\\{Client};\n",
        |analysis| {
            analysis.namespaces[0].imports[0].span = None;
        },
    );

    assert_eq!(
        planned.warnings,
        ["pkg/file.php: could not find where the items of a group `use` are written"]
    );
}

#[test]
fn the_rest_of_the_file_is_still_planned() {
    let planned = plan_with_analysis(
        "<?php\nnamespace Acme\\Consumer;\nuse Acme\\Core\\Client;\n$x = new \\Acme\\Core\\Client();\n",
        |analysis| {
            analysis.namespaces[0].imports[0].span = None;
        },
    );

    assert_eq!(planned.warnings.len(), 1);
    assert_eq!(planned.edits.len(), 1);
    assert_eq!(planned.edits[0].replacement, "\\P\\V\\Acme\\Core\\Client");
}

#[test]
fn clashing_edits_keep_the_first_and_say_so() {
    use crate::edits::file::tidy;

    let edit = |start, end, replacement: &str| Edit {
        start,
        end,
        replacement: replacement.to_string(),
        kind: EditKind::Reference,
    };

    let mut planned = Planned {
        edits: vec![
            edit(10, 14, "b"),
            edit(0, 4, "a"),
            edit(0, 4, "a"),
            edit(12, 16, "clash"),
        ],
        warnings: Vec::new(),
    };

    tidy(&mut planned, "pkg/file.php");

    // Ordered, with the exact repeat dropped and the clash left out.
    assert_eq!(planned.edits, [edit(0, 4, "a"), edit(10, 14, "b")]);
    assert_eq!(planned.warnings.len(), 1);
    assert!(
        planned.warnings[0].contains("10..14 and 12..16 clash"),
        "{:?}",
        planned.warnings
    );
}
