use super::*;
use crate::packages::CopyExclusion;
use crate::rename::is_within_namespace;

fn file<'a>(run: &'a Run, path: &str) -> &'a PackageFile {
    run.files
        .iter()
        .find(|file| file.vendor_relative_path == path)
        .unwrap_or_else(|| panic!("{path} not listed"))
}

#[test]
fn a_file_that_declares_an_excluded_namespace_is_not_copied() {
    let run = default_run();
    let excluded = file(&run, "acme/psr4-lib/src/CopyExcluded/X.php");

    assert!(!excluded.copy);
    assert_eq!(
        excluded.not_copied_because,
        Some(CopyExclusion::NamespaceExcluded(
            "Acme\\Psr4\\CopyExcluded".to_string()
        ))
    );
}

#[test]
fn other_files_are_still_copied() {
    let run = default_run();

    for path in [
        "acme/psr4-lib/src/Client.php",
        "acme/psr4-lib/src/Sub/Deep.php",
        "acme/psr4-lib/src/functions.php",
        "acme/psr4-lib/tests/NotAutoloaded.php",
        "acme/classmap-lib/classes/Mapped.php",
    ] {
        assert!(file(&run, path).copy, "{path}");
    }
}

#[test]
fn an_excluded_namespace_covers_the_namespaces_inside_it() {
    let run = run(|config| config.exclude_from_copy.namespaces = vec!["Acme\\Psr4".to_string()]);

    for path in [
        "acme/psr4-lib/src/Client.php",
        "acme/psr4-lib/src/Sub/Deep.php",
        "acme/psr4-lib/src/functions.php",
        "acme/psr4-lib/src/CopyExcluded/X.php",
    ] {
        assert!(!file(&run, path).copy, "{path}");
    }

    // `Acme\Other` and `Acme\Pattern` are different namespaces.
    assert!(file(&run, "acme/psr4-lib/tests/NotAutoloaded.php").copy);
    assert!(file(&run, "acme/pattern/lib/P.php").copy);
}

#[test]
fn an_excluded_namespace_is_matched_by_whole_segments() {
    let run = run(|config| config.exclude_from_copy.namespaces = vec!["Acme\\Psr".to_string()]);

    assert!(file(&run, "acme/psr4-lib/src/Client.php").copy);
}

#[test]
fn a_file_keeps_the_first_reason_it_is_not_copied() {
    let run = run(|config| {
        config.exclude_from_copy.packages = vec!["acme/psr4-lib".to_string()];
        config.exclude_from_copy.namespaces = vec!["Acme\\Psr4".to_string()];
    });

    assert_eq!(
        file(&run, "acme/psr4-lib/src/Client.php").not_copied_because,
        Some(CopyExclusion::PackageExcluded)
    );
}

#[test]
fn files_that_are_not_php_are_never_excluded_by_namespace() {
    let run = run(|config| config.exclude_from_copy.namespaces = vec!["Acme\\Psr4".to_string()]);

    assert!(file(&run, "acme/psr4-lib/src/Legacy/data.json").copy);
}

#[test]
fn nothing_changes_without_excluded_namespaces() {
    let run = run(|config| config.exclude_from_copy.namespaces = Vec::new());

    assert!(!run.files.iter().any(|file| matches!(
        file.not_copied_because,
        Some(CopyExclusion::NamespaceExcluded(_))
    )));

    // The file the fixture's namespace exclusion would have stopped.
    assert!(file(&run, "acme/psr4-lib/src/CopyExcluded/X.php").copy);

    // Excluded for another reason, as before.
    assert_eq!(
        file(&run, "acme/not-copied/src/N.php").not_copied_because,
        Some(CopyExclusion::PackageExcluded)
    );
}

#[test]
fn describes_the_reason() {
    assert_eq!(
        CopyExclusion::NamespaceExcluded("Acme".to_string()).to_string(),
        "it declares a namespace in exclude_from_copy.namespaces `Acme`"
    );
}

#[test]
fn a_namespace_is_within_itself_and_what_is_inside_it() {
    assert!(is_within_namespace("Psr\\Log", "Psr\\Log"));
    assert!(is_within_namespace("Psr\\Log\\Foo", "Psr\\Log"));
    assert!(is_within_namespace("Psr\\Log\\Foo\\Bar", "Psr\\Log"));
}

#[test]
fn a_namespace_is_not_within_a_namespace_that_only_starts_the_same() {
    assert!(!is_within_namespace("Psr\\LogExtra", "Psr\\Log"));
    assert!(!is_within_namespace("Psr", "Psr\\Log"));
    assert!(!is_within_namespace("Other\\Log", "Psr\\Log"));
}

#[test]
fn leading_and_trailing_backslashes_are_ignored() {
    assert!(is_within_namespace("\\Psr\\Log\\", "Psr\\Log"));
    assert!(is_within_namespace("Psr\\Log\\Foo", "\\Psr\\Log\\"));
}
