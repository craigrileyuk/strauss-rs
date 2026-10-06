use super::*;
use crate::edits::names::{Group, NameMap};

#[test]
fn a_symbol_that_was_found_is_renamed_as_decided() {
    let names = names();

    assert_eq!(
        names.new_name(Group::ClassLike, "Acme\\Core\\Client"),
        "P\\V\\Acme\\Core\\Client"
    );
    assert_eq!(
        names.new_name(Group::ClassLike, "GlobalClass"),
        "P_GlobalClass"
    );
    assert_eq!(names.new_name(Group::Function, "g_helper"), "p_g_helper");
    assert_eq!(names.new_name(Group::Constant, "G_CONST"), "P_C_G_CONST");
}

#[test]
fn a_symbol_that_was_found_and_is_not_renamed_keeps_its_name_even_in_a_renamed_namespace() {
    // `Acme\Core` is renamed, but `Acme\Core\Excluded\Item` is not.
    assert_eq!(
        names().new_name(Group::ClassLike, "Acme\\Core\\Excluded\\Item"),
        "Acme\\Core\\Excluded\\Item"
    );
}

#[test]
fn a_symbol_that_was_not_found_follows_its_namespace() {
    let names = names();

    assert_eq!(
        names.new_name(Group::ClassLike, "Acme\\Core\\Missing"),
        "P\\V\\Acme\\Core\\Missing"
    );
    assert_eq!(
        names.new_name(Group::Function, "Acme\\Core\\Sub\\missing"),
        "P\\V\\Acme\\Core\\Sub\\missing"
    );
    assert_eq!(
        names.new_name(Group::Constant, "Acme\\Core\\MISSING"),
        "P\\V\\Acme\\Core\\MISSING"
    );
}

#[test]
fn a_namespace_that_is_not_renamed_holds_back_what_is_in_it() {
    // `Acme\Core` is renamed; `Acme\Core\Excluded` is found and isn't.
    assert_eq!(
        names().new_name(Group::ClassLike, "Acme\\Core\\Excluded\\Deeper\\Missing"),
        "Acme\\Core\\Excluded\\Deeper\\Missing"
    );
}

#[test]
fn a_name_outside_every_known_namespace_is_unchanged() {
    let names = names();

    assert_eq!(
        names.new_name(Group::ClassLike, "Other\\Thing"),
        "Other\\Thing"
    );
    assert_eq!(names.new_name(Group::ClassLike, "Countable"), "Countable");
    assert_eq!(names.new_name(Group::Function, "strlen"), "strlen");
}

#[test]
fn names_are_looked_up_ignoring_case_and_a_leading_backslash_except_constants() {
    let names = names();

    assert_eq!(
        names.new_name(Group::ClassLike, "\\globalclass"),
        "P_GlobalClass"
    );
    assert_eq!(
        names.new_name(Group::ClassLike, "ACME\\core\\CLIENT"),
        "P\\V\\Acme\\Core\\Client"
    );
    assert_eq!(names.new_name(Group::Function, "G_HELPER"), "p_g_helper");
    assert_eq!(names.new_name(Group::Constant, "G_CONST"), "P_C_G_CONST");
    assert_eq!(names.new_name(Group::Constant, "g_const"), "g_const");
}

#[test]
fn knows_what_was_found() {
    let names = names();

    assert!(names.known(Group::ClassLike, "Acme\\Core\\Client"));
    assert!(names.known(Group::ClassLike, "Acme\\Core\\Excluded\\Item"));
    assert!(!names.known(Group::ClassLike, "Acme\\Core\\Missing"));
    assert!(!names.known(Group::Function, "Acme\\Core\\Client"));
}

#[test]
fn says_what_a_symbol_is_renamed_to_only_if_it_is() {
    let names = names();

    assert_eq!(
        names.replacement(Group::ClassLike, "GlobalClass"),
        Some("P_GlobalClass")
    );
    assert_eq!(
        names.replacement(Group::ClassLike, "Acme\\Core\\Excluded\\Item"),
        None
    );
    assert_eq!(names.replacement(Group::ClassLike, "Missing"), None);
}

#[test]
fn a_class_import_can_name_a_class_or_a_namespace() {
    let names = names();

    // A class.
    assert_eq!(
        names.new_import_name(Group::ClassLike, "Acme\\Core\\Client"),
        "P\\V\\Acme\\Core\\Client"
    );
    // A namespace, so that `use Acme\Core\Sub as S;` works.
    assert_eq!(
        names.new_import_name(Group::ClassLike, "Acme\\Core\\Sub"),
        "P\\V\\Acme\\Core\\Sub"
    );
    // A namespace that isn't renamed.
    assert_eq!(
        names.new_import_name(Group::ClassLike, "Acme\\Core\\Excluded"),
        "Acme\\Core\\Excluded"
    );
    // Neither was found.
    assert_eq!(
        names.new_import_name(Group::ClassLike, "Acme\\Core\\Missing"),
        "P\\V\\Acme\\Core\\Missing"
    );
    assert_eq!(
        names.new_import_name(Group::ClassLike, "Countable"),
        "Countable"
    );
}

#[test]
fn a_class_beats_a_namespace_of_the_same_name() {
    let names = NameMap::default()
        .with(SymbolKind::Namespace, "Lib\\Thing", Some("P\\Lib\\Thing"))
        .with(SymbolKind::Class, "Lib\\Thing", None);

    assert_eq!(
        names.new_import_name(Group::ClassLike, "Lib\\Thing"),
        "Lib\\Thing"
    );
}

#[test]
fn function_and_constant_imports_do_not_look_at_namespaces() {
    let names = names();

    assert_eq!(
        names.new_import_name(Group::Function, "Acme\\Core\\helper"),
        "P\\V\\Acme\\Core\\helper"
    );
    assert_eq!(
        names.new_import_name(Group::Function, "Acme\\Core\\Sub"),
        "P\\V\\Acme\\Core\\Sub"
    );
}

#[test]
fn says_what_a_declared_namespace_is_called() {
    let names = names();

    assert_eq!(names.namespace("Acme\\Core"), "P\\V\\Acme\\Core");
    assert_eq!(names.namespace("acme\\core"), "P\\V\\Acme\\Core");
    assert_eq!(
        names.namespace("Acme\\Core\\Excluded"),
        "Acme\\Core\\Excluded"
    );
    assert_eq!(names.namespace("Unknown"), "Unknown");
}

#[test]
fn knows_which_names_exist_once_everything_is_renamed() {
    let names = names();

    assert!(names.exists_after(Group::Function, "p_g_helper"));
    assert!(names.exists_after(Group::Function, "P_G_HELPER"));
    // The old name is gone.
    assert!(!names.exists_after(Group::Function, "g_helper"));
    // A symbol that isn't renamed keeps existing.
    assert!(names.exists_after(Group::ClassLike, "Acme\\Core\\Excluded\\Item"));
    assert!(names.exists_after(Group::Constant, "P_C_G_CONST"));
    assert!(!names.exists_after(Group::Constant, "p_c_g_const"));
}

#[test]
fn groups_come_from_symbol_kinds() {
    assert_eq!(Group::of(SymbolKind::Class), Some(Group::ClassLike));
    assert_eq!(Group::of(SymbolKind::Interface), Some(Group::ClassLike));
    assert_eq!(Group::of(SymbolKind::Trait), Some(Group::ClassLike));
    assert_eq!(Group::of(SymbolKind::Enum), Some(Group::ClassLike));
    assert_eq!(Group::of(SymbolKind::Function), Some(Group::Function));
    assert_eq!(Group::of(SymbolKind::Constant), Some(Group::Constant));
    assert_eq!(Group::of(SymbolKind::Namespace), None);
}

#[test]
fn names_are_the_same_ignoring_case_except_constants() {
    assert!(Group::ClassLike.same("Foo\\Bar", "foo\\bar"));
    assert!(Group::Function.same("Foo", "FOO"));
    assert!(!Group::Constant.same("FOO", "foo"));
    assert!(Group::Constant.same("FOO", "FOO"));
}
