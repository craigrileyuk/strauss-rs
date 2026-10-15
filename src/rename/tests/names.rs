use super::*;
use crate::rename::names::{namespace_replacement, strip_prefix};

fn config(json: &str) -> Config {
    Config::from_json(&fixture_dir(), Some(json), None).unwrap()
}

// ---- namespaces ----

#[test]
fn a_namespace_gets_the_prefix_in_front() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Psr4"),
        Some("Prefixed\\Vendor\\Acme\\Psr4")
    );
    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Psr4\\Sub"),
        Some("Prefixed\\Vendor\\Acme\\Psr4\\Sub")
    );
}

#[test]
fn a_namespace_replacement_pattern_with_a_literal_key_replaces_it() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Pattern"),
        Some("Custom\\Pattern")
    );
}

#[test]
fn a_namespace_replacement_pattern_with_a_regex_key_can_use_capture_groups() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Rx\\Thing"),
        Some("Rx\\Thing")
    );
}

#[test]
fn a_pattern_replaces_the_prefix_rather_than_adding_to_it() {
    let run = default_run();

    // `Custom\Pattern`, not `Prefixed\Vendor\Custom\Pattern`.
    assert!(
        !new_name(&run.plan, SymbolKind::Namespace, "Acme\\Pattern")
            .unwrap()
            .starts_with("Prefixed")
    );
}

#[test]
fn the_first_pattern_that_changes_the_namespace_wins() {
    let config = config(
        r#"{
            "namespace_prefix": "Pfx",
            "namespace_replacement_patterns": {
                "Other": "Nope",
                "Acme": "First",
                "~^Acme~": "Second"
            }
        }"#,
    );

    assert_eq!(
        namespace_replacement(&config, "Acme\\Lib"),
        Ok("First\\Lib".to_string())
    );
}

#[test]
fn a_pattern_that_does_not_match_falls_through_to_the_prefix() {
    let config = config(
        r#"{"namespace_prefix": "Pfx", "namespace_replacement_patterns": {"Other": "Nope"}}"#,
    );

    assert_eq!(
        namespace_replacement(&config, "Acme\\Lib"),
        Ok("Pfx\\Acme\\Lib".to_string())
    );
}

#[test]
fn a_literal_pattern_key_is_not_a_regular_expression() {
    let config = config(
        r#"{"namespace_prefix": "Pfx", "namespace_replacement_patterns": {"A.me": "Nope"}}"#,
    );

    assert_eq!(
        namespace_replacement(&config, "Acme\\Lib"),
        Ok("Pfx\\Acme\\Lib".to_string())
    );
    assert_eq!(
        namespace_replacement(&config, "A.me\\Lib"),
        Ok("Nope\\Lib".to_string())
    );
}

#[test]
fn without_a_prefix_or_a_matching_pattern_a_namespace_has_no_new_name() {
    let config = config(r#"{"namespace_replacement_patterns": {"Other": "Nope"}}"#);

    assert_eq!(
        namespace_replacement(&config, "Acme\\Lib"),
        Err(Reason::NoNamespaceReplacement)
    );
}

#[test]
fn a_namespace_that_already_has_the_prefix_does_not_get_it_again() {
    let config = config(r#"{"namespace_prefix": "Pfx\\Vendor"}"#);

    assert_eq!(
        namespace_replacement(&config, "Pfx\\Vendor\\Acme"),
        Ok("Pfx\\Vendor\\Acme".to_string())
    );
    assert_eq!(
        namespace_replacement(&config, "Pfx\\Vendor\\Pfx\\Vendor\\Acme"),
        Ok("Pfx\\Vendor\\Acme".to_string())
    );
}

#[test]
fn the_prefix_is_only_recognised_as_whole_segments() {
    let config = config(r#"{"namespace_prefix": "Pfx"}"#);

    assert_eq!(
        namespace_replacement(&config, "PfxCorp\\Lib"),
        Ok("Pfx\\PfxCorp\\Lib".to_string())
    );
}

#[test]
fn the_prefix_itself_is_not_given_a_trailing_backslash() {
    let config = config(r#"{"namespace_prefix": "Pfx\\Vendor"}"#);

    assert_eq!(
        namespace_replacement(&config, "Pfx\\Vendor"),
        Ok("Pfx\\Vendor".to_string())
    );
}

#[test]
fn stripping_a_prefix() {
    assert_eq!(strip_prefix("Pfx\\Acme", "Pfx"), "Acme");
    assert_eq!(strip_prefix("Pfx\\Pfx\\Acme", "Pfx"), "Acme");
    assert_eq!(strip_prefix("Acme", "Pfx"), "Acme");
    assert_eq!(strip_prefix("PfxAcme", "Pfx"), "PfxAcme");
    assert_eq!(strip_prefix("Pfx", "Pfx"), "");
    assert_eq!(strip_prefix("Acme\\Pfx", "Pfx"), "Acme\\Pfx");
    assert_eq!(strip_prefix("", "Pfx"), "");
}

// ---- what is in a namespace ----

#[test]
fn everything_in_a_namespace_follows_it() {
    let run = default_run();

    for (kind, name, new) in [
        (
            SymbolKind::Class,
            "Acme\\Psr4\\Client",
            "Prefixed\\Vendor\\Acme\\Psr4\\Client",
        ),
        (
            SymbolKind::Interface,
            "Acme\\Psr4\\Sub\\Deep",
            "Prefixed\\Vendor\\Acme\\Psr4\\Sub\\Deep",
        ),
        (
            SymbolKind::Function,
            "Acme\\Psr4\\helper",
            "Prefixed\\Vendor\\Acme\\Psr4\\helper",
        ),
        (
            SymbolKind::Constant,
            "Acme\\Psr4\\VERSION",
            "Prefixed\\Vendor\\Acme\\Psr4\\VERSION",
        ),
        (SymbolKind::Class, "Acme\\Pattern\\P", "Custom\\Pattern\\P"),
        (SymbolKind::Class, "Acme\\Rx\\Thing\\R", "Rx\\Thing\\R"),
    ] {
        assert_eq!(new_name(&run.plan, kind, name), Some(new), "{name}");
    }
}

#[test]
fn a_symbol_in_a_namespace_that_is_not_renamed_is_not_renamed() {
    // No prefix and no matching pattern, so no namespace is renamed.
    let run = run(|config| {
        config.namespace_prefix = None;
        config.namespace_replacement_patterns.clear();
    });

    assert_eq!(
        symbol(&run.plan, SymbolKind::Namespace, "Acme\\Psr4").reason,
        Reason::NoNamespaceReplacement
    );
    assert_eq!(
        symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").reason,
        Reason::NamespaceNotRenamed("Acme\\Psr4".to_string())
    );
    assert!(
        run.plan
            .symbols
            .iter()
            .filter(|symbol| !symbol.is_global())
            .all(|symbol| !symbol.rename)
    );
    // The global namespace has its own prefixes.
    assert!(symbol(&run.plan, SymbolKind::Class, "Mapped").rename);
}

// ---- the global namespace ----

#[test]
fn global_classes_interfaces_traits_and_enums_get_the_classmap_prefix() {
    let run = default_run();

    for (kind, name, new) in [
        (SymbolKind::Class, "Mapped", "Prefixed_Mapped"),
        (
            SymbolKind::Interface,
            "Mapped_Interface",
            "Prefixed_Mapped_Interface",
        ),
        (SymbolKind::Trait, "MappedTrait", "Prefixed_MappedTrait"),
        (SymbolKind::Enum, "Status", "Prefixed_Status"),
        (
            SymbolKind::Class,
            "Acme_Zero_Thing",
            "Prefixed_Acme_Zero_Thing",
        ),
    ] {
        assert_eq!(new_name(&run.plan, kind, name), Some(new), "{name}");
    }
}

#[test]
fn a_global_class_that_already_has_the_prefix_is_left_alone() {
    let run = default_run();
    let class = symbol(&run.plan, SymbolKind::Class, "Prefixed_Already");

    assert!(!class.rename);
    assert_eq!(class.reason, Reason::AlreadyPrefixed);
    assert!(class.autoloaded);
}

#[test]
fn global_functions_get_the_functions_prefix() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Function, "files_helper"),
        Some("pfx_files_helper")
    );
    assert_eq!(
        symbol(&run.plan, SymbolKind::Function, "pfx_already").reason,
        Reason::AlreadyPrefixed
    );
}

#[test]
fn global_constants_get_the_constant_prefix() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Constant, "FILES_CONST"),
        Some("PFX_FILES_CONST")
    );
    // A `const` statement in the global namespace is a global constant too.
    assert_eq!(
        new_name(&run.plan, SymbolKind::Constant, "NS_LESS"),
        Some("PFX_NS_LESS")
    );
}

#[test]
fn a_constant_that_starts_with_the_prefix_is_still_prefixed() {
    // As in PHP Strauss, which doesn't check.
    let run = run(|config| config.constant_prefix = Some("FILES_".to_string()));

    assert_eq!(
        new_name(&run.plan, SymbolKind::Constant, "FILES_CONST"),
        Some("FILES_FILES_CONST")
    );
}

#[test]
fn without_a_classmap_prefix_global_classes_are_not_renamed() {
    let run = run(|config| config.classmap_prefix = None);

    let class = symbol(&run.plan, SymbolKind::Class, "Mapped");

    assert!(!class.rename);
    assert_eq!(class.reason, Reason::NoPrefixConfigured("classmap_prefix"));
    assert_eq!(class.reason.to_string(), "classmap_prefix is not set");
}

#[test]
fn without_a_functions_prefix_global_functions_are_not_renamed() {
    let run = run(|config| config.functions_prefix = None);

    assert_eq!(
        symbol(&run.plan, SymbolKind::Function, "files_helper").reason,
        Reason::NoPrefixConfigured("functions_prefix")
    );
    // But functions in namespaces follow their namespace.
    assert!(symbol(&run.plan, SymbolKind::Function, "Acme\\Psr4\\helper").rename);
}

#[test]
fn without_a_constant_prefix_global_constants_are_not_renamed() {
    let run = run(|config| config.constant_prefix = None);

    assert_eq!(
        symbol(&run.plan, SymbolKind::Constant, "FILES_CONST").reason,
        Reason::NoPrefixConfigured("constant_prefix")
    );
    assert!(symbol(&run.plan, SymbolKind::Constant, "Acme\\Psr4\\VERSION").rename);
}

#[test]
fn functions_can_be_left_alone_inside_strings() {
    let run = default_run();

    // `exclude_from_string_rename.functions` has `files_helper`.
    assert!(!symbol(&run.plan, SymbolKind::Function, "files_helper").replace_in_strings);
    assert!(symbol(&run.plan, SymbolKind::Function, "pfx_already").replace_in_strings);
    assert!(symbol(&run.plan, SymbolKind::Function, "Acme\\Psr4\\helper").replace_in_strings);
    assert!(symbol(&run.plan, SymbolKind::Class, "Mapped").replace_in_strings);

    // It is still renamed in code.
    assert!(symbol(&run.plan, SymbolKind::Function, "files_helper").rename);
}

// ---- PSR-0 functions ----

#[test]
fn a_function_in_a_psr0_package_is_named_for_the_psr0_namespace() {
    let run = default_run();

    // Not named for a namespace: the prefix goes in front of the namespace's.
    assert_eq!(
        new_name(&run.plan, SymbolKind::Function, "zero_helper"),
        Some("Prefixed_Vendor_Acme_Zero_zero_helper")
    );
}

#[test]
fn a_psr0_function_named_for_the_namespace_swaps_the_namespace_for_the_new_one() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Function, "Acme_Zero_helper2"),
        Some("Prefixed_Vendor_Acme_Zero_helper2")
    );
}

// ---- the plan as a whole ----

#[test]
fn a_symbol_is_renamed_exactly_when_it_has_a_new_name() {
    let run = default_run();

    for symbol in &run.plan.symbols {
        assert_eq!(
            symbol.rename,
            symbol.replacement.is_some(),
            "{}",
            symbol.name
        );
    }
}

#[test]
fn a_new_name_is_never_the_old_name() {
    let run = default_run();

    for symbol in run.plan.renamed() {
        assert_ne!(
            symbol.replacement.as_deref(),
            Some(symbol.name.as_str()),
            "{}",
            symbol.name
        );
    }
}

#[test]
fn only_symbols_marked_for_a_reason_to_rename_are_renamed() {
    let run = default_run();

    for symbol in run.plan.renamed() {
        assert!(
            symbol.reason.is_rename(),
            "{}: {}",
            symbol.name,
            symbol.reason
        );
    }

    for symbol in run.plan.symbols.iter().filter(|symbol| !symbol.rename) {
        assert!(
            !symbol.reason.is_rename() || symbol.replacement.is_none(),
            "{}",
            symbol.name
        );
    }
}

#[test]
fn no_two_symbols_are_renamed_to_the_same_name() {
    let run = default_run();

    let mut seen = std::collections::HashSet::new();

    for symbol in run.plan.renamed() {
        let key = (
            symbol.kind,
            symbol.replacement.as_deref().unwrap().to_ascii_lowercase(),
        );

        assert!(seen.insert(key), "{} collides", symbol.name);
    }
}

#[test]
fn reasons_have_a_description() {
    assert_eq!(Reason::Autoloaded.to_string(), "it is autoloaded");
    assert_eq!(Reason::NotAutoloaded.to_string(), "it is not autoloaded");
    assert_eq!(
        Reason::AlreadyPrefixed.to_string(),
        "it already has the prefix"
    );
    assert_eq!(
        Reason::ConstantExcluded.to_string(),
        "it is in exclude_constants.constants"
    );
    assert_eq!(
        Reason::NamespaceNotRenamed("Acme".to_string()).to_string(),
        "its namespace Acme is not renamed"
    );
}

#[test]
fn every_reason_has_a_category() {
    let categories = [
        Reason::Autoloaded.category(),
        Reason::MatchesNamespaceReplacementPattern.category(),
        Reason::NotAutoloaded.category(),
        Reason::PackageExcludedFromCopy(String::new()).category(),
        Reason::PackageExcludedFromPrefix(String::new()).category(),
        Reason::NamespaceExcludedFromPrefix(String::new()).category(),
        Reason::FileExcludedFromPrefix {
            pattern: String::new(),
            file: String::new(),
        }
        .category(),
        Reason::ConstantPackageExcluded(String::new()).category(),
        Reason::ConstantNamespaceExcluded(String::new()).category(),
        Reason::ConstantFileExcluded {
            pattern: String::new(),
            file: String::new(),
        }
        .category(),
        Reason::ConstantExcluded.category(),
        Reason::OnlyInFilesNotCopied.category(),
        Reason::NamespaceNotRenamed(String::new()).category(),
        Reason::AlreadyPrefixed.category(),
        Reason::NoNamespaceReplacement.category(),
        Reason::NoPrefixConfigured("x").category(),
    ];

    let distinct: std::collections::HashSet<_> = categories.iter().collect();

    assert_eq!(distinct.len(), categories.len());
}
