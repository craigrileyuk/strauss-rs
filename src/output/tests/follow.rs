//! A symbol nothing autoloads, in a namespace that is renamed because something
//! in it is autoloaded (`test/loader-fixture`: `load.php` is in `files`, and
//! requires the rest). The namespace is renamed in every file that declares it,
//! so the symbols in the others have to move with it.

use super::*;
use crate::rename::Reason;
use crate::symbols::SymbolKind;

fn loader() -> (Scratch, Pipeline, Report) {
    let scratch = scratch_copy("loader-fixture");
    let pipeline = pipeline(scratch.path(), |_| {});
    let report = pipeline.write(false);

    (scratch, pipeline, report)
}

#[test]
fn symbols_that_are_not_autoloaded_move_with_their_namespace() {
    let (_, pipeline, _) = loader();
    let get = |kind, name| pipeline.rename.get(kind, name).unwrap();

    let helper = get(SymbolKind::Class, "Acme\\Lib\\Helper");

    assert!(!helper.autoloaded);
    assert_eq!(helper.reason, Reason::FollowsNamespace);
    assert_eq!(helper.replacement.as_deref(), Some("P\\Acme\\Lib\\Helper"));

    let limit = get(SymbolKind::Constant, "Acme\\Lib\\LIMIT");

    assert_eq!(limit.reason, Reason::FollowsNamespace);
    assert_eq!(limit.replacement.as_deref(), Some("P\\Acme\\Lib\\LIMIT"));

    // The one that is autoloaded is renamed for that.
    assert_eq!(
        get(SymbolKind::Function, "Acme\\Lib\\boot").reason,
        Reason::Autoloaded
    );
}

#[test]
fn the_files_agree_about_the_new_names() {
    let (scratch, _, report) = loader();

    assert!(report.warnings.is_empty(), "{:?}", report.warnings);

    // Declared in a renamed namespace...
    assert!(
        read(&scratch, "vendor-prefixed/acme/lib/src/Helper.php")
            .contains("namespace P\\Acme\\Lib;")
    );

    // ...and named by the file that requires it, relative and fully qualified.
    let load = read(&scratch, "vendor-prefixed/acme/lib/load.php");

    assert!(load.contains("namespace P\\Acme\\Lib;"));
    assert!(load.contains("[new Helper(), new \\P\\Acme\\Lib\\Helper(), LIMIT]"));
}

#[test]
fn only_what_is_autoloaded_is_in_the_classmap_and_the_files_are_required() {
    let (scratch, _, report) = loader();

    assert_eq!(report.classmap_entries, 0);
    assert!(
        read(&scratch, "vendor-prefixed/autoload.php").contains("__DIR__ . '/acme/lib/load.php',")
    );
}
