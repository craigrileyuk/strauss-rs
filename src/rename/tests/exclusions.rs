use super::*;

fn reason<'a>(plan: &'a RenamePlan, kind: SymbolKind, name: &str) -> &'a Reason {
    &symbol(plan, kind, name).reason
}

#[test]
fn a_package_in_exclude_from_prefix_is_not_renamed() {
    let run = default_run();

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Keep\\Kept"),
        &Reason::PackageExcludedFromPrefix("acme/keep".to_string())
    );
    assert_eq!(
        new_name(&run.plan, SymbolKind::Class, "Acme\\Keep\\Kept"),
        None
    );
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Keep\\Kept").autoloaded);
}

#[test]
fn package_names_are_case_insensitive() {
    let run = run(|config| config.exclude_from_prefix.packages = vec!["ACME/Keep".to_string()]);

    assert!(matches!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Keep\\Kept"),
        Reason::PackageExcludedFromPrefix(_)
    ));
}

#[test]
fn a_package_in_exclude_from_copy_is_not_renamed() {
    let run = default_run();

    for (kind, name) in [
        (SymbolKind::Class, "Acme\\NotCopied\\N"),
        (SymbolKind::Namespace, "Acme\\NotCopied"),
    ] {
        assert_eq!(
            reason(&run.plan, kind, name),
            &Reason::PackageExcludedFromCopy("acme/not-copied".to_string()),
            "{name}"
        );
    }
}

#[test]
fn not_copying_a_package_takes_precedence_over_not_prefixing_it() {
    let run = run(|config| {
        config.exclude_from_copy.packages = vec!["acme/keep".to_string()];
    });

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Keep\\Kept"),
        &Reason::PackageExcludedFromCopy("acme/keep".to_string())
    );
}

#[test]
fn an_exclusion_beats_a_namespace_replacement_pattern() {
    let run = run(|config| config.exclude_from_prefix.packages = vec!["acme/pattern".to_string()]);

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Pattern\\P"),
        &Reason::PackageExcludedFromPrefix("acme/pattern".to_string())
    );
    assert_eq!(
        new_name(&run.plan, SymbolKind::Class, "Acme\\Pattern\\P"),
        None
    );
}

#[test]
fn a_namespace_in_exclude_from_prefix_covers_what_is_in_it() {
    let run = default_run();

    for (kind, name) in [
        (SymbolKind::Namespace, "Acme\\Psr4\\Skip"),
        (SymbolKind::Class, "Acme\\Psr4\\Skip\\Skipped"),
    ] {
        assert_eq!(
            reason(&run.plan, kind, name),
            &Reason::NamespaceExcludedFromPrefix("Acme\\Psr4\\Skip".to_string()),
            "{name}"
        );
    }

    // Its siblings are not covered.
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").rename);
}

#[test]
fn an_excluded_namespace_covers_the_namespaces_inside_it() {
    let run = run(|config| config.exclude_from_prefix.namespaces = vec!["Acme\\Psr4".to_string()]);

    for (kind, name) in [
        (SymbolKind::Namespace, "Acme\\Psr4"),
        (SymbolKind::Namespace, "Acme\\Psr4\\Sub"),
        (SymbolKind::Class, "Acme\\Psr4\\Client"),
        (SymbolKind::Interface, "Acme\\Psr4\\Sub\\Deep"),
        (SymbolKind::Function, "Acme\\Psr4\\helper"),
    ] {
        assert!(
            matches!(
                reason(&run.plan, kind, name),
                Reason::NamespaceExcludedFromPrefix(_)
            ),
            "{name}"
        );
    }

    // A different namespace entirely.
    assert!(symbol(&run.plan, SymbolKind::Namespace, "Acme\\Pattern").rename);
}

#[test]
fn an_excluded_namespace_is_matched_by_whole_segments() {
    // PHP Strauss compares the start of the strings, so `Acme\Psr4\Sk` would
    // exclude `Acme\Psr4\Skip`.
    let run = run(|config| {
        config.exclude_from_prefix.namespaces =
            vec!["Acme\\Psr4\\Sk".to_string(), "Acme\\Ps".to_string()]
    });

    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").rename);
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Skip\\Skipped").rename);
}

#[test]
fn a_file_pattern_excludes_symbols_only_declared_in_matching_files() {
    let run = default_run();

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Old"),
        &Reason::FileExcludedFromPrefix {
            pattern: "~/Legacy/~".to_string(),
            file: "acme/psr4-lib/src/Legacy/Old.php".to_string(),
        }
    );
}

#[test]
fn a_namespace_shared_with_other_files_is_still_renamed() {
    // `Legacy/Old.php` declares `Acme\Psr4`, as do other files. PHP Strauss
    // would leave the namespace alone because of that one file.
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Psr4"),
        Some("Prefixed\\Vendor\\Acme\\Psr4")
    );
}

#[test]
fn a_namespace_only_declared_in_matching_files_is_excluded() {
    let run =
        run(|config| config.exclude_from_prefix.file_patterns = vec![pattern("~^acme/psr4-lib/~")]);

    for (kind, name) in [
        (SymbolKind::Namespace, "Acme\\Psr4"),
        (SymbolKind::Class, "Acme\\Psr4\\Client"),
        (SymbolKind::Namespace, "Acme\\Psr4\\Sub"),
    ] {
        assert!(
            matches!(
                reason(&run.plan, kind, name),
                Reason::FileExcludedFromPrefix { .. }
            ),
            "{name}"
        );
    }
}

#[test]
fn every_file_matching_a_pattern_is_left_alone_even_if_it_declares_nothing() {
    let run = default_run();

    let files: Vec<(&str, &str)> = run
        .plan
        .files_excluded_from_prefix
        .iter()
        .map(|file| (file.file.as_str(), file.pattern.as_str()))
        .collect();

    assert_eq!(
        files,
        [
            ("acme/psr4-lib/src/Legacy/Old.php", "~/Legacy/~"),
            // Not PHP, so it declares no symbols.
            ("acme/psr4-lib/src/Legacy/data.json", "~/Legacy/~"),
        ]
    );
}

#[test]
fn constants_can_be_excluded_by_name() {
    let run = default_run();

    assert_eq!(
        reason(&run.plan, SymbolKind::Constant, "ACME_KEEP_CONST"),
        &Reason::ConstantExcluded
    );
    assert_eq!(
        new_name(&run.plan, SymbolKind::Constant, "ACME_KEEP_CONST"),
        None
    );
    // Others in the same file are not affected.
    assert_eq!(
        new_name(&run.plan, SymbolKind::Constant, "FILES_CONST"),
        Some("PFX_FILES_CONST")
    );
}

#[test]
fn constants_can_be_excluded_by_package() {
    let run = run(|config| {
        config.exclude_constants.exclusions.packages = vec!["acme/files-lib".to_string()]
    });

    assert_eq!(
        reason(&run.plan, SymbolKind::Constant, "FILES_CONST"),
        &Reason::ConstantPackageExcluded("acme/files-lib".to_string())
    );
    // Only constants.
    assert!(symbol(&run.plan, SymbolKind::Class, "FilesClass").rename);
    assert!(symbol(&run.plan, SymbolKind::Function, "files_helper").rename);
}

#[test]
fn a_constant_in_a_renamed_namespace_cannot_be_excluded_by_namespace() {
    let run = run(|config| {
        config.exclude_constants.exclusions.namespaces = vec!["Acme\\Psr4".to_string()]
    });

    // `const VERSION` is in the namespace, which is renamed in the file that
    // declares it, so the constant goes with it.
    assert_eq!(
        reason(&run.plan, SymbolKind::Constant, "Acme\\Psr4\\VERSION"),
        &Reason::FollowsNamespace
    );
    assert!(symbol(&run.plan, SymbolKind::Constant, "Acme\\Psr4\\VERSION").rename);
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").rename);
}

#[test]
fn constants_can_be_excluded_by_file() {
    let run = run(|config| {
        config.exclude_constants.exclusions.file_patterns = vec![pattern("~bootstrap~")]
    });

    assert_eq!(
        reason(&run.plan, SymbolKind::Constant, "FILES_CONST"),
        &Reason::ConstantFileExcluded {
            pattern: "~bootstrap~".to_string(),
            file: "acme/files-lib/bootstrap.php".to_string(),
        }
    );
    assert!(symbol(&run.plan, SymbolKind::Class, "FilesClass").rename);
}

#[test]
fn a_symbol_only_in_files_that_are_not_copied_is_not_renamed() {
    let run = default_run();

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Acme\\Psr4\\CopyExcluded\\X"),
        &Reason::OnlyInFilesNotCopied
    );
}

#[test]
fn a_namespace_is_renamed_even_if_its_files_are_not_copied() {
    let run = default_run();

    assert_eq!(
        new_name(&run.plan, SymbolKind::Namespace, "Acme\\Psr4\\CopyExcluded"),
        Some("Prefixed\\Vendor\\Acme\\Psr4\\CopyExcluded")
    );
}

#[test]
fn a_file_pattern_in_exclude_from_copy_keeps_its_symbols_unrenamed() {
    let run = run(|config| {
        config.exclude_from_copy.file_patterns = vec![pattern("~classes/Mapped\\.php~")]
    });

    assert_eq!(
        reason(&run.plan, SymbolKind::Class, "Mapped"),
        &Reason::OnlyInFilesNotCopied
    );
    // The rest of the package is copied.
    assert!(symbol(&run.plan, SymbolKind::Interface, "Mapped_Interface").rename);
}

#[test]
fn when_the_target_is_the_vendor_directory_nothing_is_left_out_for_not_being_copied() {
    // Files are changed where they are, so there is no copy to be missing from.
    let run = run(|config| config.target_directory = config.vendor_directory.clone());

    assert!(run.files.iter().all(|file| !file.copy));
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\CopyExcluded\\X").rename);
}
