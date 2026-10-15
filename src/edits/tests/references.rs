use super::*;

#[test]
fn names_relative_to_a_renamed_namespace_follow_it() {
    let source = r#"<?php
namespace Acme\Core;

class User
{
    public function f(Client $client, Sub\Thing $thing)
    {
        return new Client();
    }
}
"#;

    // Only the namespace is changed: the names inside it mean the same.
    assert_eq!(
        rewrite(&names(), source),
        source.replace("namespace Acme\\Core;", "namespace P\\V\\Acme\\Core;")
    );
}

#[test]
fn fully_qualified_names_are_rewritten() {
    let source = in_namespace(
        r#"$a = new \Acme\Core\Client();
$b = \Acme\Core\Sub\Thing::class;"#,
    );

    assert_eq!(
        rewrite(&names(), &source),
        in_namespace(
            r#"$a = new \P\V\Acme\Core\Client();
$b = \P\V\Acme\Core\Sub\Thing::class;"#
        )
    );
}

#[test]
fn a_name_through_an_alias_follows_the_import() {
    let source = in_namespace(
        r#"use Acme\Core\Sub as S;

$x = new S\Thing();"#,
    );

    assert_eq!(
        rewrite(&names(), &source),
        in_namespace(
            r#"use P\V\Acme\Core\Sub as S;

$x = new S\Thing();"#
        )
    );
}

#[test]
fn a_relative_name_that_would_stop_working_is_made_fully_qualified() {
    // `Excluded\Item` isn't renamed, but `Acme\Core` is. Left alone, the name would
    // now mean `P\V\Acme\Core\Excluded\Item`, which doesn't exist.
    let source = r#"<?php
namespace Acme\Core;

class Holder
{
    public function f(): Excluded\Item
    {
        return new Excluded\Item();
    }
}
"#;

    assert_eq!(
        rewrite(&names(), source),
        r#"<?php
namespace P\V\Acme\Core;

class Holder
{
    public function f(): \Acme\Core\Excluded\Item
    {
        return new \Acme\Core\Excluded\Item();
    }
}
"#
    );
}

#[test]
fn global_names_in_the_global_namespace_are_renamed_as_they_are() {
    let source = "<?php\ng_helper();\n$a = new GlobalClass();\necho G_CONST;\n";

    assert_eq!(
        rewrite(&names(), source),
        "<?php\np_g_helper();\n$a = new P_GlobalClass();\necho P_C_G_CONST;\n"
    );
}

#[test]
fn a_global_function_or_constant_used_from_a_namespace_keeps_working() {
    // PHP looks for them in the namespace, then the global namespace.
    let source = in_namespace("g_helper();\necho G_CONST;");

    assert_eq!(
        rewrite(&names(), &source),
        in_namespace("p_g_helper();\necho P_C_G_CONST;")
    );
}

#[test]
fn a_namespaced_function_with_the_new_name_would_be_called_instead_so_the_name_is_qualified() {
    let names = names().with(SymbolKind::Function, "Acme\\Consumer\\p_g_helper", None);

    assert_eq!(
        rewrite(&names, &in_namespace("g_helper();")),
        in_namespace("\\p_g_helper();")
    );
}

#[test]
fn a_namespaced_function_that_was_already_being_called_still_is() {
    // `g_helper()` here meant `Acme\Consumer\g_helper`, not the global one.
    let names = names().with(SymbolKind::Function, "Acme\\Consumer\\g_helper", None);

    assert_eq!(
        rewrite(&names, &in_namespace("g_helper();")),
        in_namespace("g_helper();")
    );
}

#[test]
fn functions_and_constants_php_provides_are_left_alone() {
    let source = in_namespace("echo strlen('x') . PHP_EOL;");

    assert_eq!(rewrite(&names(), &source), source);
}

#[test]
fn constants_are_matched_exactly() {
    // `G_CONST` is renamed; `g_const` is a different constant.
    let source = in_namespace("echo g_const;");

    assert_eq!(rewrite(&names(), &source), source);
}

#[test]
fn a_class_that_was_not_found_follows_its_namespace() {
    // Relative to a renamed namespace: nothing to do.
    let relative = r#"<?php
namespace Acme\Core;

$x = new Missing();
"#;

    assert_eq!(
        rewrite(&names(), relative),
        relative.replace("namespace Acme\\Core;", "namespace P\\V\\Acme\\Core;")
    );

    // Fully qualified: gets the new namespace.
    assert_eq!(
        rewrite(&names(), &in_namespace(r"$a = \Acme\Core\Missing::class;")),
        in_namespace(r"$a = \P\V\Acme\Core\Missing::class;")
    );

    // In no namespace that is known.
    let elsewhere = in_namespace(r"$a = \Other\Thing::class;");

    assert_eq!(rewrite(&names(), &elsewhere), elsewhere);
}

#[test]
fn a_name_relative_to_the_namespace_keyword_is_checked_too() {
    let in_renamed = r#"<?php
namespace Acme\Core;

$x = namespace\Client::class;
"#;

    // `namespace\Client` is in the new namespace, where `Client` now is.
    assert_eq!(
        rewrite(&names(), in_renamed),
        in_renamed.replace("namespace Acme\\Core;", "namespace P\\V\\Acme\\Core;")
    );

    let elsewhere = in_namespace(r"$x = namespace\Client::class;");

    assert_eq!(rewrite(&names(), &elsewhere), elsewhere);
}

#[test]
fn a_file_that_keeps_its_namespace_refers_to_what_moved_by_its_new_name() {
    let source = r#"<?php
namespace Acme\Core;

class Legacy
{
    public function f(Sub\Thing $thing)
    {
        return new Client();
    }
}
"#;

    // The file stays in `Acme\Core`, where `Client` no longer is.
    assert_eq!(
        rewrite_with(&names(), source, true),
        r#"<?php
namespace Acme\Core;

class Legacy
{
    public function f(\P\V\Acme\Core\Sub\Thing $thing)
    {
        return new \P\V\Acme\Core\Client();
    }
}
"#
    );
}

#[test]
fn names_are_rewritten_wherever_a_class_can_be_named() {
    let source = in_namespace(
        r#"class A extends \Acme\Core\Client
{
    public function f(?\Acme\Core\Client $a, int|\GlobalClass $b): \Acme\Core\Sub\Thing
    {
        try {
            return $a instanceof \Acme\Core\Client ? null : new class extends \GlobalClass {};
        } catch (\Acme\Core\Sub\Thing $e) {
            return $b;
        }
    }
}"#,
    );

    let expected = source
        .replace("\\Acme\\Core\\Client", "\\P\\V\\Acme\\Core\\Client")
        .replace("\\Acme\\Core\\Sub\\Thing", "\\P\\V\\Acme\\Core\\Sub\\Thing")
        .replace("\\GlobalClass", "\\P_GlobalClass");

    assert_eq!(rewrite(&names(), &source), expected);
}

#[test]
fn the_constant_after_a_class_is_not_a_reference() {
    assert_eq!(
        rewrite(&names(), &in_namespace(r"$a = \Acme\Core\Client::KIND;")),
        in_namespace(r"$a = \P\V\Acme\Core\Client::KIND;")
    );
}

#[test]
fn each_namespace_in_a_file_is_planned_on_its_own() {
    let source = r#"<?php
namespace Acme\Core {
    class A
    {
        public function f(Client $client) {}
    }
}

namespace Acme\Consumer {
    class B
    {
        public function f(\Acme\Core\Client $client) {}
    }
}
"#;

    assert_eq!(
        rewrite(&names(), source),
        r#"<?php
namespace P\V\Acme\Core {
    class A
    {
        public function f(Client $client) {}
    }
}

namespace Acme\Consumer {
    class B
    {
        public function f(\P\V\Acme\Core\Client $client) {}
    }
}
"#
    );
}

#[test]
fn a_name_is_qualified_when_the_namespaces_do_not_move_together() {
    // Patterns can send a namespace and the one inside it to unrelated places.
    let names = NameMap::default()
        .with(SymbolKind::Namespace, "Acme\\Core", Some("Custom"))
        .with(SymbolKind::Namespace, "Acme\\Core\\Sub", Some("Other\\Sub"))
        .with(
            SymbolKind::Class,
            "Acme\\Core\\Sub\\Thing",
            Some("Other\\Sub\\Thing"),
        );

    let source = r#"<?php
namespace Acme\Core;

class U
{
    public function f(Sub\Thing $thing) {}
}
"#;

    // Left alone it would mean `Custom\Sub\Thing`.
    assert_eq!(
        rewrite(&names, source),
        r#"<?php
namespace Custom;

class U
{
    public function f(\Other\Sub\Thing $thing) {}
}
"#
    );
}

#[test]
fn an_unqualified_class_in_a_namespace_is_not_the_global_class() {
    // `GlobalClass` unqualified in a namespace would mean `Acme\Consumer\GlobalClass`
    // - not the global class - so it isn't a reference to it, and is left alone.
    let source = in_namespace("$a = new GlobalClass();");

    assert_eq!(rewrite(&names(), &source), source);
}

#[test]
fn an_unqualified_name_is_not_changed_when_it_is_already_right() {
    let source = r#"<?php
namespace Acme\Core\Sub;

class Thing2 extends Thing {}
"#;

    // `Thing` is `Acme\Core\Sub\Thing`, renamed along with its namespace.
    assert_eq!(
        rewrite(&names(), source),
        source.replace(
            "namespace Acme\\Core\\Sub;",
            "namespace P\\V\\Acme\\Core\\Sub;"
        )
    );
}

#[test]
fn traits_named_in_trait_adaptations_are_rewritten() {
    let source = in_namespace(
        r#"class User
{
    use \Acme\Core\Client, \Acme\Core\Sub\Thing {
        \Acme\Core\Client::hello insteadof \Acme\Core\Sub\Thing;
        \Acme\Core\Sub\Thing::hello as protected hi;
    }
}"#,
    );

    assert_eq!(
        rewrite(&names(), &source),
        in_namespace(
            r#"class User
{
    use \P\V\Acme\Core\Client, \P\V\Acme\Core\Sub\Thing {
        \P\V\Acme\Core\Client::hello insteadof \P\V\Acme\Core\Sub\Thing;
        \P\V\Acme\Core\Sub\Thing::hello as protected hi;
    }
}"#
        )
    );
}
