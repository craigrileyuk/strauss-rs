use crate::packages::gitattributes::GitAttributes;

fn ignored(attributes: &str, path: &str) -> bool {
    GitAttributes::parse(attributes).is_export_ignored(path)
}

#[test]
fn a_pattern_without_a_slash_matches_any_path_segment() {
    let attributes = "tests export-ignore";

    assert!(ignored(attributes, "tests"));
    assert!(ignored(attributes, "tests/FooTest.php"));
    assert!(ignored(attributes, "src/tests/FooTest.php"));
    assert!(!ignored(attributes, "src/Foo.php"));
    assert!(!ignored(attributes, "src/mytests/Foo.php"));
}

#[test]
fn a_trailing_slash_is_ignored() {
    assert!(ignored("tests/ export-ignore", "tests/FooTest.php"));
    assert!(ignored("tests/ export-ignore", "src/tests/FooTest.php"));
}

#[test]
fn globs_match_within_a_segment() {
    let attributes = "*.md export-ignore\n.git* export-ignore";

    assert!(ignored(attributes, "README.md"));
    assert!(ignored(attributes, "docs/guide.md"));
    assert!(ignored(attributes, ".gitattributes"));
    assert!(ignored(attributes, ".github/workflows/ci.yml"));
    assert!(!ignored(attributes, "src/Foo.php"));
}

#[test]
fn a_pattern_with_a_slash_is_anchored_to_the_package_root() {
    let attributes = "/docs export-ignore";

    assert!(ignored(attributes, "docs"));
    assert!(ignored(attributes, "docs/guide.md"));
    assert!(ignored(attributes, "docs/deep/guide.md"));
    assert!(!ignored(attributes, "src/docs/guide.md"));
}

#[test]
fn an_anchored_star_does_not_cross_a_directory_separator() {
    let attributes = "/build/*.js export-ignore";

    assert!(ignored(attributes, "build/app.js"));
    assert!(!ignored(attributes, "build/sub/app.js"));
    assert!(!ignored(attributes, "build/app.css"));
}

#[test]
fn a_directory_pattern_with_a_slash_covers_what_is_inside_it() {
    let attributes = "src/Legacy export-ignore";

    assert!(ignored(attributes, "src/Legacy/Old.php"));
    assert!(!ignored(attributes, "src/Current/New.php"));
}

#[test]
fn the_last_matching_line_wins() {
    let attributes = "tests export-ignore\ntests/keep.php -export-ignore";

    assert!(ignored(attributes, "tests/FooTest.php"));
    assert!(!ignored(attributes, "tests/keep.php"));
}

#[test]
fn only_setting_the_attribute_excludes_a_file() {
    assert!(ignored("a export-ignore", "a"));
    assert!(!ignored("a -export-ignore", "a"));
    assert!(!ignored("a !export-ignore", "a"));
    assert!(!ignored("a export-ignore=false", "a"));
}

#[test]
fn the_last_mention_on_a_line_wins() {
    assert!(!ignored("a export-ignore -export-ignore", "a"));
    assert!(ignored("a -export-ignore export-ignore", "a"));
}

#[test]
fn other_attributes_and_other_lines_are_ignored() {
    let attributes = "# a comment\n\n*.php text eol=lf\n*.png binary\ntests export-ignore text";

    assert!(ignored(attributes, "tests/a.php"));
    assert!(!ignored(attributes, "src/a.php"));
    assert!(!ignored(attributes, "logo.png"));
}

#[test]
fn a_leading_slash_on_the_path_is_ignored() {
    assert!(ignored("tests export-ignore", "/tests/a.php"));
}

#[test]
fn no_attributes_excludes_nothing() {
    assert!(!ignored("", "anything"));
}

#[test]
fn a_missing_file_means_no_attributes() {
    let attributes = GitAttributes::load(&super::fixture_dir()).unwrap();

    assert!(!attributes.is_export_ignored("src/main.rs"));
}
