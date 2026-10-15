use super::*;

fn edit(start: u32, end: u32, replacement: &str) -> Edit {
    Edit {
        start,
        end,
        replacement: replacement.to_string(),
        kind: EditKind::Reference,
    }
}

#[test]
fn no_edits_leaves_the_text_alone() {
    assert_eq!(apply_edits("<?php echo 1;", &[]).unwrap(), "<?php echo 1;");
}

#[test]
fn replaces_text() {
    assert_eq!(
        apply_edits("hello world", &[edit(6, 11, "there")]).unwrap(),
        "hello there"
    );
}

#[test]
fn the_replacement_can_be_longer_or_shorter() {
    assert_eq!(apply_edits("abcdef", &[edit(1, 4, "X")]).unwrap(), "aXef");
    assert_eq!(
        apply_edits("abcdef", &[edit(1, 2, "XYZ")]).unwrap(),
        "aXYZcdef"
    );
}

#[test]
fn all_edits_refer_to_the_original_text() {
    // The first edit makes the text longer, which doesn't move the second.
    let edits = [edit(0, 1, "AAAA"), edit(4, 5, "E")];

    assert_eq!(apply_edits("abcde", &edits).unwrap(), "AAAAbcdE");
}

#[test]
fn edits_can_be_given_in_any_order() {
    let ascending = [edit(0, 1, "1"), edit(2, 3, "2"), edit(4, 5, "3")];
    let descending = [edit(4, 5, "3"), edit(2, 3, "2"), edit(0, 1, "1")];

    assert_eq!(apply_edits("a b c", &ascending).unwrap(), "1 2 3");
    assert_eq!(apply_edits("a b c", &descending).unwrap(), "1 2 3");
}

#[test]
fn edits_can_touch() {
    assert_eq!(
        apply_edits("abcd", &[edit(0, 2, "X"), edit(2, 4, "Y")]).unwrap(),
        "XY"
    );
}

#[test]
fn an_empty_range_inserts_and_an_empty_replacement_deletes() {
    assert_eq!(apply_edits("ac", &[edit(1, 1, "b")]).unwrap(), "abc");
    assert_eq!(apply_edits("abc", &[edit(1, 2, "")]).unwrap(), "ac");
}

#[test]
fn edits_at_the_ends_of_the_text() {
    assert_eq!(
        apply_edits("bc", &[edit(0, 0, "a"), edit(2, 2, "d")]).unwrap(),
        "abcd"
    );
}

#[test]
fn overlapping_edits_are_an_error() {
    let error = apply_edits("abcdef", &[edit(0, 4, "X"), edit(2, 5, "Y")]).unwrap_err();

    assert_eq!(
        error,
        ApplyError::Overlap {
            first: (0, 4),
            second: (2, 5)
        }
    );
    assert_eq!(
        error.to_string(),
        "the edits 0..4 and 2..5 change the same text"
    );
}

#[test]
fn two_edits_to_the_same_range_overlap() {
    assert!(matches!(
        apply_edits("abc", &[edit(0, 2, "X"), edit(0, 2, "Y")]),
        Err(ApplyError::Overlap { .. })
    ));
}

#[test]
fn an_edit_past_the_end_is_an_error() {
    assert_eq!(
        apply_edits("abc", &[edit(2, 9, "X")]).unwrap_err(),
        ApplyError::OutOfRange {
            start: 2,
            end: 9,
            length: 3
        }
    );
}

#[test]
fn an_edit_that_ends_before_it_starts_is_an_error() {
    assert!(matches!(
        apply_edits("abcdef", &[edit(4, 2, "X")]),
        Err(ApplyError::OutOfRange { .. })
    ));
}

#[test]
fn an_edit_inside_a_character_is_an_error() {
    // `é` is two bytes.
    assert_eq!(
        apply_edits("aéb", &[edit(2, 3, "X")]).unwrap_err(),
        ApplyError::NotOnACharacter { offset: 2 }
    );
}

#[test]
fn multibyte_text_around_edits_is_kept() {
    let source = "// café\n$x = Foo;\n// naïve ✓\n";
    let start = source.find("Foo").unwrap() as u32;

    assert_eq!(
        apply_edits(source, &[edit(start, start + 3, "Bar")]).unwrap(),
        "// café\n$x = Bar;\n// naïve ✓\n"
    );
}

#[test]
fn nothing_is_changed_if_any_edit_fails() {
    // The first edit is fine; the result is an error, not half a change.
    assert!(apply_edits("abc", &[edit(0, 1, "X"), edit(0, 9, "Y")]).is_err());
}
