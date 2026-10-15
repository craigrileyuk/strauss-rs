use super::*;

fn regions_of(analysis: &FileAnalysis, index: usize) -> Vec<(RegionKind, &str)> {
    analysis.namespaces[index]
        .regions
        .iter()
        .map(|region| (region.kind, region.text.as_str()))
        .collect()
}

#[test]
fn collects_every_kind_of_string() {
    let source = "<?php\nnamespace Foo;\n$a = 'Foo\\\\Bar';\n$b = \"Foo\\\\Bar $x {$y}\";\n$c = <<<EOT\nFoo\\Bar $x\nEOT;\n$d = <<<'EOT'\nFoo\\Bar\nEOT;\n$e = `ls`;\n";

    let analysis = analyse_php(source);

    assert_eq!(
        regions_of(&analysis, 0),
        [
            (RegionKind::String, "'Foo\\\\Bar'"),
            (RegionKind::String, "\"Foo\\\\Bar $x {$y}\""),
            (RegionKind::String, "<<<EOT\nFoo\\Bar $x\nEOT"),
            (RegionKind::String, "<<<'EOT'\nFoo\\Bar\nEOT"),
            (RegionKind::String, "`ls`"),
        ]
    );
}

#[test]
fn region_spans_index_the_source_text() {
    let source = "<?php\nnamespace Foo;\n/** doc */\n$a = 'x';\n";
    let analysis = analyse_php(source);

    for region in &analysis.namespaces[0].regions {
        assert_eq!(
            &source[region.span.start as usize..region.span.end as usize],
            region.text
        );
    }
}

#[test]
fn collects_every_kind_of_comment() {
    let analysis = analyse_php(
        "<?php\nnamespace Foo;\n/** doc */\n// line\n# hash\n/* block */\nclass A {}\n",
    );

    assert_eq!(
        regions_of(&analysis, 0),
        [
            (RegionKind::DocComment, "/** doc */"),
            (RegionKind::Comment, "// line"),
            (RegionKind::Comment, "# hash"),
            (RegionKind::Comment, "/* block */"),
        ]
    );
}

#[test]
fn collects_docblocks_attached_to_declarations_and_members() {
    let analysis = analyse_php(
        r#"<?php
            namespace Foo;

            /** @see \Vendor\Thing */
            class Example
            {
                /** @var \Vendor\Other */
                public $prop;

                /**
                 * @param \Vendor\Arg $arg
                 * @return \Vendor\Ret
                 */
                public function run($arg) {}
            }
            "#,
    );

    let regions = &analysis.namespaces[0].regions;

    assert_eq!(regions.len(), 3);
    assert!(
        regions
            .iter()
            .all(|region| region.kind == RegionKind::DocComment)
    );
    assert!(regions[0].text.contains("\\Vendor\\Thing"));
    assert!(regions[1].text.contains("\\Vendor\\Other"));
    assert!(regions[2].text.contains("\\Vendor\\Ret"));
}

#[test]
fn collects_strings_in_places_the_reference_walk_does_not_visit() {
    let analysis = analyse_php(
        r#"<?php
            namespace Foo;

            #[Attr('Vendor\\InAttribute')]
            class Example
            {
                const A = 'Vendor\\InConst';
                public $p = ['Vendor\\InArray' => 'Vendor\\InValue'];
            }

            switch ($x) {
                case 'Vendor\\InCase':
                    break;
            }
            "#,
    );

    let texts: Vec<&str> = analysis.namespaces[0]
        .regions
        .iter()
        .map(|region| region.text.as_str())
        .collect();

    for expected in ["InAttribute", "InConst", "InArray", "InValue", "InCase"] {
        assert!(
            texts.iter().any(|text| text.contains(expected)),
            "missing {expected}: {texts:?}"
        );
    }
}

#[test]
fn assigns_regions_to_the_namespace_they_appear_in() {
    let analysis = analyse_php(
        r#"<?php
            namespace One;

            $a = 'in one';

            namespace Two;

            // in two
            $b = 'also in two';
            "#,
    );

    assert_eq!(analysis.namespaces.len(), 2);
    assert_eq!(regions_of(&analysis, 0), [(RegionKind::String, "'in one'")]);
    assert_eq!(
        regions_of(&analysis, 1),
        [
            (RegionKind::Comment, "// in two"),
            (RegionKind::String, "'also in two'"),
        ]
    );
}

#[test]
fn assigns_regions_in_braced_namespaces() {
    let analysis = analyse_php(
        r#"<?php
            namespace One {
                $a = 'in one';
            }

            namespace {
                $b = 'in global';
            }
            "#,
    );

    assert_eq!(analysis.namespaces.len(), 2);
    assert_eq!(analysis.namespaces[0].namespace.as_deref(), Some("One"));
    assert_eq!(regions_of(&analysis, 0), [(RegionKind::String, "'in one'")]);
    assert_eq!(analysis.namespaces[1].namespace, None);
    assert_eq!(
        regions_of(&analysis, 1),
        [(RegionKind::String, "'in global'")]
    );
}

#[test]
fn comment_above_namespace_belongs_to_the_global_namespace() {
    let analysis = analyse_php(
        r#"<?php
            /**
             * File header.
             */
            declare(strict_types=1);

            namespace Foo;

            class Example {}
            "#,
    );

    assert_eq!(analysis.namespaces.len(), 2);
    assert_eq!(analysis.namespaces[0].namespace, None);
    assert_eq!(analysis.namespaces[0].regions.len(), 1);
    assert_eq!(
        analysis.namespaces[0].regions[0].kind,
        RegionKind::DocComment
    );
    assert_eq!(analysis.namespaces[1].namespace.as_deref(), Some("Foo"));
    assert!(analysis.namespaces[1].regions.is_empty());
}

#[test]
fn collects_callable_and_define_string_arguments() {
    let analysis = analyse_php(
        r#"<?php
            namespace Foo;

            function_exists('Foo\\helper');
            \call_user_func("Foo\\other", 1);
            define('MY_CONST', 1);
            defined('MY_CONST');
            "#,
    );

    let args: Vec<(RegionKind, &str, &str)> = analysis.namespaces[0]
        .regions
        .iter()
        .filter(|region| region.value.is_some())
        .map(|region| {
            (
                region.kind,
                region.text.as_str(),
                region.value.as_deref().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        args,
        [
            (
                RegionKind::CallableStringArg,
                "Foo\\\\helper",
                "Foo\\helper"
            ),
            (RegionKind::CallableStringArg, "Foo\\\\other", "Foo\\other"),
            (RegionKind::DefineStringArg, "MY_CONST", "MY_CONST"),
            (RegionKind::DefineStringArg, "MY_CONST", "MY_CONST"),
        ]
    );
}

#[test]
fn string_argument_regions_exclude_the_quotes() {
    let source = "<?php\nfunction_exists('foo');\n";
    let analysis = analyse_php(source);

    let region = analysis.namespaces[0]
        .regions
        .iter()
        .find(|region| region.kind == RegionKind::CallableStringArg)
        .unwrap();

    assert_eq!(
        &source[region.span.start as usize..region.span.end as usize],
        "foo"
    );
}

#[test]
fn ignores_string_argument_regions_for_other_calls() {
    let analysis = analyse_php(
        r#"<?php
            namespace Foo;

            strlen('Foo\\helper');
            Bar\define('X', 1);
            $o->define('X');
            define($dynamic, 1);
            define("interp $x", 1);
            define(value: 1, constant_name: 'X');
            define();
            "#,
    );

    assert!(
        analysis.namespaces[0]
            .regions
            .iter()
            .all(|region| region.value.is_none()),
        "{:?}",
        analysis.namespaces[0].regions
    );
}
