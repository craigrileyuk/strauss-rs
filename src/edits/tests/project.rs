//! The exact result of rewriting each file of `test/edit-fixture`.

use super::*;

fn assert_rewritten(file: &str, expected: &str) {
    assert_eq!(rewritten_file(&edit_fixture(), file), expected, "{file}");
}

#[test]
fn a_consumer_using_global_symbols() {
    assert_rewritten(
        "acme/consumer/src/UsesGlobals.php",
        r#"<?php
namespace P\V\Acme\Consumer;

use P_GlobalClass as GlobalClass;
use function p_g_helper as g_helper;
use P\V\Acme\Core\Client;

class UsesGlobals extends GlobalClass implements \P_GlobalContract
{
    public function run(Client $client)
    {
        g_helper();
        \p_g_helper();
        p_g_other();
        $x = new GlobalClass();
        $y = \P_GlobalClass::class;
        $z = P_C_G_CONST;
        $w = \P_C_G_CONST;

        defined("P_C_G_DEFINED");
        function_exists("p_g_helper");
        function_exists("\\p_g_other");

        return namespace\Missing::class;
    }
}
"#,
    );
}

#[test]
fn code_in_the_global_namespace() {
    assert_rewritten(
        "acme/consumer/src/GlobalFile.php",
        r#"<?php
use P\V\Acme\Core\Client as C;

$a = new P_GlobalClass();
p_g_helper();
echo P_C_G_CONST;
$b = new \P\V\Acme\Core\Client();
$c = new C();
"#,
    );
}

#[test]
fn the_globals_are_renamed_where_they_are_declared() {
    assert_rewritten(
        "acme/globals/bootstrap.php",
        r#"<?php
function p_g_helper() {}
function p_g_other() {}

const P_C_G_CONST = 1;
define("P_C_G_DEFINED", 2);

class P_GlobalClass {}
interface P_GlobalContract {}
"#,
    );
}

#[test]
fn a_class_whose_neighbour_is_not_renamed() {
    assert_rewritten(
        "acme/core/src/Client.php",
        r#"<?php
namespace P\V\Acme\Core;

class Client
{
    public function make(): Sub\Thing
    {
        helper();

        return new Sub\Thing();
    }

    public function item(): \Acme\Core\Excluded\Item
    {
        return new \Acme\Core\Excluded\Item();
    }
}
"#,
    );
}

#[test]
fn group_use_statements() {
    assert_rewritten(
        "acme/core/src/Grouped.php",
        r#"<?php
namespace P\V\Acme\Core;

use P\V\Acme\Core\Sub\{Thing, Other as O};
use P\V\Acme\Core\Client;
use Acme\Core\Excluded\Item;
use P\V\Acme\Core\{function helper, const VERSION};

class Grouped extends Client
{
    public function f(Thing $a, O $b, Item $c)
    {
        helper();

        return VERSION;
    }
}
"#,
    );
}

#[test]
fn imports_aliases_and_fully_qualified_names_together() {
    assert_rewritten(
        "acme/core/src/Util.php",
        r#"<?php
namespace P\V\Acme\Core;

use P\V\Acme\Core\Sub\Thing;
use P\V\Acme\Core\Sub as S;
use function P\V\Acme\Core\helper;
use const P\V\Acme\Core\VERSION;
use Countable;

class Util extends Client implements Countable
{
    const KIND = Thing::class;

    public function f(Thing $a, S\Thing $b, \P\V\Acme\Core\Sub\Thing $c, \Exception $d): ?Client
    {
        helper();
        \P\V\Acme\Core\helper();
        $v = VERSION . \P\V\Acme\Core\VERSION;
        $e = new \P\V\Acme\Core\Client();

        try {
            return strlen("x") > 0 ? null : $this;
        } catch (\InvalidArgumentException $x) {
            return $e;
        }
    }

    public function count(): int
    {
        return 0;
    }
}
"#,
    );
}

#[test]
fn a_package_that_is_not_prefixed_still_follows_what_is() {
    assert_rewritten(
        "acme/keep/src/Kept.php",
        r#"<?php
namespace Acme\Keep;

use P\V\Acme\Core\Client;

class Kept extends Client
{
    public function f(): \P\V\Acme\Core\Sub\Thing
    {
        return new Sub\Thing();
    }
}
"#,
    );
}

#[test]
fn files_that_only_declare_a_namespace_only_change_that() {
    let run = edit_fixture();

    for file in [
        "acme/core/src/Sub/Thing.php",
        "acme/core/src/Sub/Other.php",
        "acme/core/src/functions.php",
    ] {
        let edits = &run.edits.get(file).unwrap().edits;

        assert_eq!(edits.len(), 1, "{file}");
        assert_eq!(edits[0].kind, EditKind::Namespace, "{file}");
    }
}

#[test]
fn the_file_with_nothing_to_change_has_no_edits() {
    assert!(
        edit_fixture()
            .edits
            .get("acme/core/src/Excluded/Item.php")
            .is_none()
    );
}

#[test]
fn counts_each_kind_of_edit() {
    let run = edit_fixture();
    let mut counts = std::collections::BTreeMap::new();

    for edit in run.edits.files.iter().flat_map(|file| &file.edits) {
        *counts.entry(format!("{:?}", edit.kind)).or_insert(0) += 1;
    }

    // Counted from the rewritten files above.
    assert_eq!(counts["Namespace"], 8);
    assert_eq!(counts["GroupUse"], 3);
    assert_eq!(counts["DefineString"], 2);
    assert_eq!(counts["CallableString"], 3);
    assert_eq!(counts["Declaration"], 5);
    assert_eq!(counts["Import"], 10);
    assert_eq!(counts["Reference"], 17);
    assert_eq!(counts["Docblock"], 2);
    assert_eq!(counts["String"], 3);
    assert_eq!(counts.values().sum::<usize>(), 53);
}

#[test]
fn strings_and_docblocks_in_a_consumer() {
    assert_rewritten(
        "acme/consumer/src/Strings.php",
        r#"<?php
namespace P\V\Acme\Consumer;

use P\V\Acme\Core\Client;

class Strings
{
    /**
     * @param \P\V\Acme\Core\Client $client
     * @param Client $same
     * @return \P_GlobalClass|null
     */
    public function run($client, $same)
    {
        $map = ['P\\V\\Acme\\Core\\' => 'src/'];
        $class = 'P\V\Acme\Core\Client';
        $kept = 'Acme\Core\Excluded\Item';
        add_action('init', 'p_g_helper');
        $row = ['g_helper' => 1];

        return "Unknown P\V\Acme\Core\Sub\Thing: {$class}";
    }
}
"#,
    );
}
