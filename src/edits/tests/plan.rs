use super::*;
use crate::edits::names::{Group, NameMap};

fn reasons(run: &Run) -> Vec<(String, String)> {
    run.edits
        .skipped
        .iter()
        .map(|skipped| (skipped.file.clone(), skipped.reason.to_string()))
        .collect()
}

#[test]
fn plans_the_files_that_have_something_to_change() {
    let run = edit_fixture();

    assert_eq!(run.edits.files.len(), 11);
    assert_eq!(run.edits.edit_count(), 53);
    // `Excluded/Item.php`: its namespace is not renamed and nothing in it refers to anything.
    // `Shared.php`: in a namespace that is renamed, but its package is not prefixed.
    assert_eq!(run.edits.unchanged_files, 2);
    assert!(run.edits.skipped.is_empty());
    assert!(run.edits.warnings.is_empty(), "{:?}", run.edits.warnings);
}

#[test]
fn each_files_edits_are_in_order_and_do_not_overlap() {
    let run = edit_fixture();

    for file in &run.edits.files {
        for pair in file.edits.windows(2) {
            assert!(
                pair[0].end <= pair[1].start,
                "{}: {:?} then {:?}",
                file.file,
                pair[0],
                pair[1]
            );
        }

        assert!(file.edits.iter().all(|edit| edit.start <= edit.end));
    }
}

#[test]
fn no_edit_changes_nothing() {
    let run = edit_fixture();

    for file in &run.edits.files {
        let source = std::fs::read_to_string(&file.source_path).unwrap();

        for edit in &file.edits {
            assert_ne!(
                &source[edit.start as usize..edit.end as usize],
                edit.replacement,
                "{}: {:?}",
                file.file,
                edit
            );
        }
    }
}

#[test]
fn planning_does_not_change_any_file() {
    let before: Vec<(String, String)> = run("edit-fixture", |_| {})
        .files
        .iter()
        .map(|file| {
            (
                file.vendor_relative_path.clone(),
                std::fs::read_to_string(&file.source_path).unwrap(),
            )
        })
        .collect();

    let after: Vec<(String, String)> = edit_fixture()
        .files
        .iter()
        .map(|file| {
            (
                file.vendor_relative_path.clone(),
                std::fs::read_to_string(&file.source_path).unwrap(),
            )
        })
        .collect();

    assert_eq!(before, after);
}

#[test]
fn every_rewritten_file_is_still_php() {
    let run = edit_fixture();

    for file in &run.edits.files {
        let changed = rewritten_file(&run, &file.file);
        let errors = parse(&changed).errors;

        assert!(errors.is_empty(), "{}: {errors:?}\n{changed}", file.file);
    }
}

#[test]
fn every_reference_in_the_rewritten_code_means_what_it_should() {
    // Analyse the rewritten files again, and check that each name in code now resolves
    // to the new name of what it resolved to before.
    let run = edit_fixture();
    let names = NameMap::from_plan(&run.rename);
    let mut checked = 0;

    for edited in &run.edits.files {
        let scanned = run
            .table
            .files
            .iter()
            .find(|file| file.vendor_relative_path == edited.file)
            .unwrap();

        let old = scanned.analysis.as_ref().unwrap();
        let changed = rewritten_file(&run, &edited.file);
        let new = analyse(&changed, &parse(&changed).program);

        assert_eq!(
            old.namespaces.len(),
            new.namespaces.len(),
            "{}",
            edited.file
        );

        for (old_segment, new_segment) in old.namespaces.iter().zip(&new.namespaces) {
            assert_eq!(
                old_segment.references.len(),
                new_segment.references.len(),
                "{}",
                edited.file
            );

            for (before, after) in old_segment.references.iter().zip(&new_segment.references) {
                let group = match before.kind {
                    crate::analyser::ReferenceKind::ClassName => Group::ClassLike,
                    crate::analyser::ReferenceKind::FunctionName => Group::Function,
                    crate::analyser::ReferenceKind::ConstantName => Group::Constant,
                    crate::analyser::ReferenceKind::ClassConstantName => continue,
                };

                // What it meant: the one in the namespace if there is one, else the global one.
                let meant = |reference: &crate::analyser::Reference,
                             exists: &dyn Fn(&str) -> bool| {
                    match &reference.fallback {
                        Some(global) if exists(&reference.name) => Some(reference.name.clone()),
                        Some(global) if exists(global) => Some(global.clone()),
                        Some(_) => None,
                        None => Some(reference.name.clone()),
                    }
                };

                let Some(target) = meant(before, &|name| names.known(group, name)) else {
                    continue;
                };

                let expected = names.new_name(group, &target);

                let actual =
                    meant(after, &|name| names.exists_after(group, name)).unwrap_or_else(|| {
                        panic!("{}: {} resolves to nothing", edited.file, after.name)
                    });

                assert!(
                    group.same(&actual, &expected),
                    "{}: `{}` should mean {expected} but means {actual}",
                    edited.file,
                    changed
                        .get(after.span.start as usize..after.span.end as usize)
                        .unwrap_or("?"),
                );

                checked += 1;
            }
        }
    }

    // Not a vacuous pass.
    assert!(checked > 40, "only {checked} references were checked");
}

// ---- which files are edited ----

#[test]
fn a_file_that_is_not_copied_is_not_edited() {
    let run = run("edit-fixture", |config| {
        config.exclude_from_copy.packages = vec!["acme/keep".to_string()]
    });

    assert_eq!(
        reasons(&run),
        [
            (
                "acme/keep/src/Kept.php".to_string(),
                "the file is not copied".to_string()
            ),
            (
                "acme/keep/src/Shared.php".to_string(),
                "the file is not copied".to_string()
            ),
        ]
    );
    assert!(run.edits.get("acme/keep/src/Kept.php").is_none());
    assert_eq!(run.edits.files.len(), 10);
}

#[test]
fn files_are_edited_in_place_when_the_target_is_the_vendor_directory() {
    let run = run("edit-fixture", |config| {
        config.target_directory = config.vendor_directory.clone()
    });

    // None of them are "copied", and all of them are edited.
    assert!(run.files.iter().all(|file| !file.copy));
    assert!(run.edits.skipped.is_empty());
    assert_eq!(run.edits.files.len(), 11);
}

#[test]
fn exclude_files_from_update_by_package_leaves_the_package_alone() {
    let run = run("edit-fixture", |config| {
        config.exclude_files_from_update.packages = vec!["ACME/Core".to_string()];
    });

    assert!(
        run.edits
            .skipped
            .iter()
            .all(|skipped| skipped.file.starts_with("acme/core/"))
    );
    assert_eq!(run.edits.skipped.len(), 7);
    assert_eq!(
        run.edits.skipped[0].reason.to_string(),
        "it is excluded by exclude_files_from_update package `ACME/Core`"
    );
    assert!(run.edits.get("acme/core/src/Client.php").is_none());
    // The others are still edited.
    assert!(run.edits.get("acme/consumer/src/UsesGlobals.php").is_some());
}

#[test]
fn exclude_files_from_update_by_file_pattern() {
    let run = run("edit-fixture", |config| {
        config.exclude_files_from_update.file_patterns = vec![pattern("~/Util\\.php$~")];
    });

    assert_eq!(
        reasons(&run),
        [(
            "acme/core/src/Util.php".to_string(),
            "it is excluded by exclude_files_from_update file pattern `~/Util\\.php$~`".to_string()
        )]
    );
}

#[test]
fn exclude_files_from_update_by_namespace_covers_the_namespaces_inside_it() {
    let run = run("edit-fixture", |config| {
        config.exclude_files_from_update.namespaces = vec!["Acme\\Core\\Sub".to_string()];
    });

    let files: Vec<&str> = run
        .edits
        .skipped
        .iter()
        .map(|skipped| skipped.file.as_str())
        .collect();

    assert_eq!(
        files,
        ["acme/core/src/Sub/Other.php", "acme/core/src/Sub/Thing.php"]
    );
    assert_eq!(
        run.edits.skipped[0].reason.to_string(),
        "it is excluded by exclude_files_from_update namespace `Acme\\Core\\Sub`"
    );
}

#[test]
fn a_file_that_does_not_parse_or_is_not_utf8_is_skipped_and_says_why() {
    let run = run("symbols-fixture", |_| {});

    let skipped: Vec<(&str, String)> = run
        .edits
        .skipped
        .iter()
        .map(|skipped| (skipped.file.as_str(), skipped.reason.to_string()))
        .collect();

    assert!(
        skipped.contains(&(
            "acme/app-lib/src/broken.php",
            "it could not be analysed".to_string()
        )),
        "{skipped:?}"
    );
    assert!(
        skipped.contains(&(
            "acme/app-lib/src/latin1.php",
            "it is not valid UTF-8".to_string()
        )),
        "{skipped:?}"
    );
}

// ---- files that keep their namespaces ----

#[test]
fn a_package_that_is_not_prefixed_keeps_its_namespace_but_its_references_follow() {
    let run = edit_fixture();
    let kept = run.edits.get("acme/keep/src/Kept.php").unwrap();

    assert!(
        kept.edits
            .iter()
            .all(|edit| edit.kind != EditKind::Namespace)
    );
    assert!(kept.edits.iter().any(|edit| edit.kind == EditKind::Import));
    assert!(
        kept.edits
            .iter()
            .any(|edit| edit.kind == EditKind::Reference)
    );
}

#[test]
fn a_package_that_is_not_prefixed_keeps_a_namespace_it_shares_with_one_that_is() {
    let run = edit_fixture();

    // `Acme\Core` is renamed for `acme/core`...
    assert_eq!(
        run.rename
            .get(SymbolKind::Namespace, "Acme\\Core")
            .unwrap()
            .replacement
            .as_deref(),
        Some("P\\V\\Acme\\Core")
    );

    // ...but `acme/keep`'s `Shared` is in it, is not renamed, and so stays where it is.
    assert!(
        !run.rename
            .get(SymbolKind::Class, "Acme\\Core\\Shared")
            .unwrap()
            .rename
    );
    assert!(run.edits.get("acme/keep/src/Shared.php").is_none());
}

#[test]
fn a_file_excluded_from_prefixing_keeps_its_namespace_but_its_references_follow() {
    let run = run("edit-fixture", |config| {
        config.exclude_from_prefix.file_patterns = vec![pattern("~Grouped~")];
    });

    let grouped = run.edits.get("acme/core/src/Grouped.php").unwrap();

    assert!(
        grouped
            .edits
            .iter()
            .all(|edit| edit.kind != EditKind::Namespace),
        "{:?}",
        grouped.edits
    );
    assert!(
        grouped
            .edits
            .iter()
            .any(|edit| edit.kind == EditKind::GroupUse)
    );

    // Its own namespace line is as it was.
    assert!(
        rewritten_file(&run, "acme/core/src/Grouped.php").contains("\nnamespace Acme\\Core;\n")
    );
}

// ---- the plan as a whole ----

#[test]
fn the_plan_does_not_depend_on_the_number_of_workers() {
    let describe = |parallel: Parallel| -> String {
        let run = run("edit-fixture", |config| config.parallel = parallel);

        serde_json::to_string(&run.edits).unwrap()
    };

    let sequential = describe(Parallel::Disabled);

    assert!(sequential.len() > 5000);
    assert_eq!(describe(Parallel::Workers(4)), sequential);
    assert_eq!(describe(Parallel::Auto), sequential);
}

#[test]
fn files_come_out_in_path_order() {
    let run = edit_fixture();
    let files: Vec<&str> = run
        .edits
        .files
        .iter()
        .map(|file| file.file.as_str())
        .collect();

    let mut sorted = files.clone();
    sorted.sort();

    assert_eq!(files, sorted);
}

#[test]
fn describes_why_a_file_is_skipped() {
    assert_eq!(SkipReason::NotCopied.to_string(), "the file is not copied");
    assert_eq!(
        SkipReason::NotAnalysed.to_string(),
        "it could not be analysed"
    );
    assert_eq!(SkipReason::NotUtf8.to_string(), "it is not valid UTF-8");
}

#[test]
fn serialises_for_the_json_output() {
    let run = edit_fixture();
    let json = serde_json::to_value(&run.edits).unwrap();

    assert_eq!(json["files"].as_array().unwrap().len(), 11);
    assert_eq!(json["unchanged_files"], 2);

    let first_edit = &json["files"][0]["edits"][0];

    assert!(first_edit["start"].is_u64());
    assert!(first_edit["replacement"].is_string());
    assert!(first_edit["kind"].is_string());
}
