use super::*;
use crate::config::{Autoload, StringOrList};
use crate::rename::autoload::{in_classmap, in_files, in_psr_directory, package_relative_path};

#[test]
fn everything_within_a_psr4_namespace_is_autoloaded() {
    let run = default_run();

    for (kind, name) in [
        (SymbolKind::Namespace, "Acme\\Psr4"),
        (SymbolKind::Namespace, "Acme\\Psr4\\Sub"),
        (SymbolKind::Class, "Acme\\Psr4\\Client"),
        (SymbolKind::Interface, "Acme\\Psr4\\Sub\\Deep"),
        (SymbolKind::Function, "Acme\\Psr4\\helper"),
        (SymbolKind::Constant, "Acme\\Psr4\\VERSION"),
    ] {
        assert!(symbol(&run.plan, kind, name).autoloaded, "{name}");
    }
}

#[test]
fn a_namespace_outside_every_autoload_rule_in_a_file_outside_its_directories_is_not_autoloaded() {
    let run = default_run();

    // `tests/NotAutoloaded.php`, in namespace `Acme\Other`: psr-4 maps `Acme\Psr4\` to `src/`.
    assert!(!symbol(&run.plan, SymbolKind::Namespace, "Acme\\Other").autoloaded);
    assert!(!symbol(&run.plan, SymbolKind::Class, "Acme\\Other\\NotAutoloaded").autoloaded);
}

#[test]
fn a_file_in_a_psr4_directory_is_autoloaded_with_what_it_declares_whatever_its_namespace() {
    let run = default_run();

    // `src/global.php` declares a function in the global namespace, which no
    // namespace key reaches. It is autoloaded because of where the file is.
    let helper = symbol(&run.plan, SymbolKind::Function, "psr4_global_helper");

    assert!(helper.autoloaded);
    assert_eq!(
        helper.replacement.as_deref(),
        Some("pfx_psr4_global_helper")
    );
}

#[test]
fn a_namespace_within_a_psr4_key_is_autoloaded_with_the_files_that_declare_it_wherever_they_are() {
    let run = default_run();

    // `tests/UnderKey.php` is outside the `src/` directory, but declares
    // `Acme\Psr4\Tests`, which is within the `Acme\Psr4\` key.
    assert!(symbol(&run.plan, SymbolKind::Namespace, "Acme\\Psr4\\Tests").autoloaded);
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Tests\\UnderKey").autoloaded);

    // `tests/NotAutoloaded.php` is in the same directory, in another namespace.
    assert!(!symbol(&run.plan, SymbolKind::Class, "Acme\\Other\\NotAutoloaded").autoloaded);
}

#[test]
fn a_file_in_a_psr0_directory_is_autoloaded_with_what_it_declares() {
    let run = default_run();

    assert!(symbol(&run.plan, SymbolKind::Class, "Acme_Zero_Thing").autoloaded);
    assert!(symbol(&run.plan, SymbolKind::Function, "zero_helper").autoloaded);
}

#[test]
fn a_classmap_path_autoloads_what_it_holds_except_what_is_excluded() {
    let run = default_run();

    for name in ["Mapped", "Mapped_Interface", "MappedTrait", "Status"] {
        let kind = match name {
            "Mapped" => SymbolKind::Class,
            "Mapped_Interface" => SymbolKind::Interface,
            "MappedTrait" => SymbolKind::Trait,
            _ => SymbolKind::Enum,
        };

        assert!(symbol(&run.plan, kind, name).autoloaded, "{name}");
    }

    // `exclude-from-classmap`, and outside the classmap path.
    assert!(!symbol(&run.plan, SymbolKind::Class, "SkippedByClassmap").autoloaded);
    assert!(!symbol(&run.plan, SymbolKind::Class, "Loose").autoloaded);
}

#[test]
fn a_files_autoload_entry_autoloads_everything_in_that_file() {
    let run = default_run();

    for (kind, name) in [
        (SymbolKind::Function, "files_helper"),
        (SymbolKind::Constant, "FILES_CONST"),
        (SymbolKind::Constant, "NS_LESS"),
        (SymbolKind::Class, "FilesClass"),
    ] {
        assert!(symbol(&run.plan, kind, name).autoloaded, "{name}");
    }

    // Another file in the package, which `bootstrap.php` doesn't include.
    assert!(!symbol(&run.plan, SymbolKind::Function, "other_helper").autoloaded);
}

#[test]
fn the_namespaces_of_autoload_rules_are_added_even_if_no_file_declares_them() {
    let run = default_run();

    for (name, psr) in [("Acme_Zero_", Psr::Psr0), ("Zero", Psr::Psr0)] {
        let namespace = symbol(&run.plan, SymbolKind::Namespace, name);

        assert_eq!(namespace.psr, Some(psr), "{name}");
        assert!(namespace.autoloaded);
        assert!(namespace.sources.is_empty());
        assert_eq!(namespace.packages, ["acme/psr0-lib"]);
    }
}

#[test]
fn an_autoload_namespace_that_a_file_declares_is_one_symbol() {
    let run = default_run();

    let matching: Vec<&PlannedSymbol> = run
        .plan
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == SymbolKind::Namespace && symbol.name == "Acme\\Psr4")
        .collect();

    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].psr, Some(Psr::Psr4));
    assert!(!matching[0].sources.is_empty());
    assert_eq!(matching[0].packages, ["acme/psr4-lib"]);
}

#[test]
fn a_package_that_is_not_prefixed_adds_no_autoload_namespaces() {
    let run = default_run();

    // acme/keep is in exclude_from_prefix.packages. Its namespace is there only
    // because a file declares it.
    let keep = symbol(&run.plan, SymbolKind::Namespace, "Acme\\Keep");

    assert_eq!(keep.psr, None);
    assert!(
        !run.plan
            .packages_to_prefix
            .contains(&"acme/keep".to_string())
    );
}

#[test]
fn override_autoload_decides_what_is_autoloaded() {
    let run = run(|config| {
        let package = "acme/psr4-lib";

        config.override_autoload.insert(
            package.to_string(),
            Autoload {
                files: vec!["tests/NotAutoloaded.php".to_string()],
                ..Autoload::default()
            },
        );
    });

    // With the psr-4 rule replaced by a `files` entry, the tests file is
    // autoloaded and `src/` is not.
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\Other\\NotAutoloaded").autoloaded);
    assert!(!symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").autoloaded);
}

#[test]
fn package_relative_paths() {
    assert_eq!(
        package_relative_path("acme/lib", "acme/lib/src/A.php"),
        "src/A.php"
    );
    assert_eq!(package_relative_path("acme/lib", "acme/lib/A.php"), "A.php");
    // Not under that package: unchanged.
    assert_eq!(
        package_relative_path("acme/lib", "other/src/A.php"),
        "other/src/A.php"
    );
    assert_eq!(package_relative_path("acme/lib", "acme/lib"), "acme/lib");
}

fn psr(directories: &[&str]) -> Vec<(String, StringOrList)> {
    vec![(
        "Ns\\".to_string(),
        StringOrList::Many(
            directories
                .iter()
                .map(|directory| directory.to_string())
                .collect(),
        ),
    )]
}

#[test]
fn psr_directories() {
    let rules = psr(&["src/", "lib"]);

    assert!(in_psr_directory(rules.iter(), "src/A.php", "/\\"));
    assert!(in_psr_directory(rules.iter(), "lib/deep/B.php", "/\\"));
    assert!(!in_psr_directory(rules.iter(), "tests/A.php", "/\\"));

    // A leading slash on either is ignored.
    assert!(in_psr_directory(psr(&["/src"]).iter(), "src/A.php", "/\\"));
    assert!(in_psr_directory(rules.iter(), "/src/A.php", "/\\"));

    // The empty directory is the package root.
    assert!(in_psr_directory(
        psr(&[""]).iter(),
        "anything/at/all.php",
        "/\\"
    ));

    assert!(!in_psr_directory(std::iter::empty(), "src/A.php", "/\\"));
}

#[test]
fn a_psr0_directory_trims_backslashes_only() {
    let rules = psr(&["\\lib"]);

    assert!(in_psr_directory(rules.iter(), "lib/A.php", "\\"));
    // A slash is part of the path, as in PHP Strauss.
    assert!(!in_psr_directory(psr(&["/lib"]).iter(), "lib/A.php", "\\"));
}

#[test]
fn files_autoload_entries_must_match_exactly() {
    let autoload = Autoload {
        files: vec!["bootstrap.php".to_string(), "inc/a.php".to_string()],
        ..Autoload::default()
    };

    assert!(in_files(&autoload, "bootstrap.php"));
    assert!(in_files(&autoload, "inc/a.php"));
    assert!(!in_files(&autoload, "inc/b.php"));
    assert!(!in_files(&autoload, "bootstrap.php.bak"));
}

#[test]
fn classmap_paths_are_prefixes() {
    let autoload = |classmap: &[&str], exclude: &[&str]| Autoload {
        classmap: classmap.iter().map(|path| path.to_string()).collect(),
        exclude_from_classmap: exclude.iter().map(|path| path.to_string()).collect(),
        ..Autoload::default()
    };

    assert!(in_classmap(&autoload(&["classes/"], &[]), "classes/A.php"));
    assert!(!in_classmap(&autoload(&["classes/"], &[]), "other/A.php"));
    assert!(in_classmap(&autoload(&["."], &[]), "anything/A.php"));
    assert!(!in_classmap(&autoload(&[], &[]), "A.php"));

    // Excluded paths are skipped, whatever else matches.
    assert!(!in_classmap(
        &autoload(&["classes/"], &["classes/skip/"]),
        "classes/skip/A.php"
    ));
    assert!(in_classmap(
        &autoload(&["classes/"], &["classes/skip/"]),
        "classes/A.php"
    ));
    assert!(!in_classmap(&autoload(&["."], &["tests/"]), "tests/A.php"));
}

#[test]
fn a_package_without_psr_rules_has_every_namespace_it_declares_autoloaded_once_any_file_is() {
    let run = default_run();

    // `includes/namespaced.php` is not in the `files` list, but the package
    // finds its classes by namespace at run time.
    assert!(symbol(&run.plan, SymbolKind::Namespace, "Acme\\FilesLib\\Sub").autoloaded);
    assert!(symbol(&run.plan, SymbolKind::Class, "Acme\\FilesLib\\Sub\\Extra").autoloaded);

    // Global symbols in it keep following the rules for their own file.
    assert!(!symbol(&run.plan, SymbolKind::Function, "other_helper").autoloaded);
}
