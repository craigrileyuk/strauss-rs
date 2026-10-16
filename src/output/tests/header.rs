use super::*;
use crate::output::header::{Header, add_header, format_date};

const CREDIT: &str = "{@see https://github.com/evo-mark/strauss-rs}";

fn full<'a>(licenses: &'a [String]) -> Header<'a> {
    Header {
        licenses,
        author: Some("Ada Lovelace"),
        date: Some(DATE),
    }
}

fn bare() -> Header<'static> {
    Header {
        licenses: &[],
        author: None,
        date: None,
    }
}

#[test]
fn a_file_without_a_comment_gets_one() {
    let licenses = vec!["MIT".to_string()];

    assert_eq!(
        add_header("<?php\nnamespace A;\n", &full(&licenses)),
        format!(
            "<?php\n/**\n * @license MIT\n *\n * Modified by Ada Lovelace on {DATE} using {CREDIT}.\n */\n\nnamespace A;\n"
        )
    );
}

#[test]
fn the_author_the_date_and_the_license_are_each_optional() {
    assert_eq!(
        add_header("<?php\nnamespace A;\n", &bare()),
        format!("<?php\n/**\n * Modified using {CREDIT}.\n */\n\nnamespace A;\n")
    );

    let header = Header {
        author: Some("Ada"),
        ..bare()
    };

    assert!(
        add_header("<?php\nA;", &header).contains(&format!(" * Modified by Ada using {CREDIT}."))
    );

    let header = Header {
        date: Some(DATE),
        ..bare()
    };

    assert!(
        add_header("<?php\nA;", &header)
            .contains(&format!(" * Modified on {DATE} using {CREDIT}."))
    );
}

#[test]
fn several_licenses_are_each_listed() {
    let licenses = vec!["MIT".to_string(), "GPL-2.0-or-later".to_string()];

    let changed = add_header("<?php\nA;", &full(&licenses));

    assert!(changed.contains(" * @license MIT\n * @license GPL-2.0-or-later\n *\n * Modified"));
}

#[test]
fn a_license_comment_is_added_to() {
    let source = "<?php\n/**\n * @license MIT\n */\n\nnamespace A;\n";

    assert_eq!(
        add_header(source, &full(&[])),
        format!(
            "<?php\n/**\n * @license MIT\n *\n * Modified by Ada Lovelace on {DATE} using {CREDIT}.\n */\n\nnamespace A;\n"
        )
    );
}

#[test]
fn a_comment_that_ends_with_a_blank_line_is_not_given_another() {
    let source = "<?php\n/*\n * Copyright\n *\n */\n\nnamespace A;\n";

    assert_eq!(
        add_header(source, &bare()),
        format!("<?php\n/*\n * Copyright\n *\n * Modified using {CREDIT}.\n */\n\nnamespace A;\n")
    );
}

#[test]
fn the_comment_of_a_class_is_not_a_license_comment() {
    // No blank line between it and what it describes.
    let source = "<?php\n/**\n * About A.\n */\nclass A {}\n";

    assert_eq!(
        add_header(source, &bare()),
        format!(
            "<?php\n/**\n * Modified using {CREDIT}.\n */\n\n/**\n * About A.\n */\nclass A {{}}\n"
        )
    );
}

#[test]
fn code_on_the_line_of_the_open_tag_moves_down() {
    assert_eq!(
        add_header("<?php declare(strict_types=1);\n", &bare()),
        format!("<?php\n/**\n * Modified using {CREDIT}.\n */\n\ndeclare(strict_types=1);\n")
    );
}

#[test]
fn line_endings_are_kept() {
    let changed = add_header("<?php\r\nnamespace A;\r\n", &bare());

    assert_eq!(
        changed,
        format!("<?php\r\n/**\r\n * Modified using {CREDIT}.\r\n */\r\n\r\nnamespace A;\r\n")
    );
    assert!(!changed.replace("\r\n", "").contains('\n'));
}

#[test]
fn a_file_with_the_note_is_left_alone() {
    let once = add_header("<?php\nnamespace A;\n", &bare());

    assert_eq!(add_header(&once, &full(&[])), once);
}

#[test]
fn a_file_that_does_not_start_as_php_is_left_alone() {
    for source in [
        "",
        "<html>\n<?php echo 1;",
        "#!/usr/bin/env php\n<?php\n",
        "<?php",
        "<?phpx\n",
        "<?= 1 ?>",
    ] {
        assert_eq!(add_header(source, &bare()), source, "{source:?}");
    }
}

#[test]
fn dates_are_written_as_php_does() {
    assert_eq!(format_date(0), "01-January-1970");
    assert_eq!(format_date(951_782_400), "29-February-2000");
    assert_eq!(format_date(1_000_000_000), "09-September-2001");
    assert_eq!(format_date(1_760_486_400), "15-October-2025");
    assert_eq!(format_date(86_399), "01-January-1970");
    assert_eq!(format_date(86_400), "02-January-1970");
    assert_eq!(format_date(-1), "31-December-1969");
    assert_eq!(format_date(4_102_444_800), "01-January-2100");
}

#[test]
fn a_comment_with_code_after_it_on_the_line_is_not_a_license_comment() {
    let source = "<?php\n/**\n * About A.\n */ class A {}\n\nclass B {}\n";

    assert_eq!(
        add_header(source, &bare()),
        format!(
            "<?php\n/**\n * Modified using {CREDIT}.\n */\n\n/**\n * About A.\n */ class A {{}}\n\nclass B {{}}\n"
        )
    );
}
