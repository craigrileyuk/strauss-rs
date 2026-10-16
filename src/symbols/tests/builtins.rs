use super::*;

#[test]
fn recognises_built_in_classes_interfaces_and_enums() {
    let built_ins = BuiltIns::get();

    for name in [
        "Stringable",
        "ArrayObject",
        "Countable",
        "JsonSerializable",
        "UnitEnum",
        "BackedEnum",
        "WeakMap",
        "Fiber",
        "Attribute",
    ] {
        assert!(built_ins.is_class_like(name), "{name}");
    }
}

#[test]
fn recognises_built_in_functions() {
    let built_ins = BuiltIns::get();

    for name in [
        "str_contains",
        "str_starts_with",
        "array_key_first",
        "array_is_list",
        "strlen",
        "mb_str_split",
    ] {
        assert!(built_ins.is_function(name), "{name}");
    }
}

#[test]
fn matching_ignores_case_as_php_does() {
    let built_ins = BuiltIns::get();

    assert!(built_ins.is_class_like("STRINGABLE"));
    assert!(built_ins.is_class_like("arrayobject"));
    assert!(built_ins.is_function("STR_CONTAINS"));
}

#[test]
fn a_leading_backslash_is_ignored() {
    let built_ins = BuiltIns::get();

    assert!(built_ins.is_class_like("\\Stringable"));
    assert!(built_ins.is_function("\\strlen"));
}

#[test]
fn a_name_in_another_namespace_is_not_built_in() {
    let built_ins = BuiltIns::get();

    assert!(!built_ins.is_class_like("Acme\\Stringable"));
    assert!(!built_ins.is_function("Acme\\str_contains"));
}

#[test]
fn classes_and_functions_are_separate() {
    let built_ins = BuiltIns::get();

    assert!(!built_ins.is_function("Stringable"));
    assert!(!built_ins.is_class_like("str_contains"));
}

#[test]
fn ordinary_names_are_not_built_in() {
    let built_ins = BuiltIns::get();

    assert!(!built_ins.is_class_like("Client"));
    assert!(!built_ins.is_function("acme_poly_helper"));
}

#[test]
fn parses_the_data_format() {
    let built_ins =
        BuiltIns::parse("# comment\n\nc Foo\nf bar\nx ignored\nc\nf  spaced  \nnonsense\n");

    assert!(built_ins.is_class_like("foo"));
    assert!(built_ins.is_function("BAR"));
    assert!(built_ins.is_function("spaced"));
    assert!(!built_ins.is_class_like("ignored"));
    assert!(!built_ins.is_function("foo"));
}

#[test]
fn recognises_functions_that_only_some_sapis_provide() {
    let built_ins = BuiltIns::get();

    for name in [
        "getallheaders",
        "apache_request_headers",
        "fastcgi_finish_request",
        "litespeed_request_headers",
        "frankenphp_handle_request",
    ] {
        assert!(built_ins.is_function(name), "{name}");
    }
}
