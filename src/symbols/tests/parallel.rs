use super::*;
use crate::symbols::analyse::analyse_files;

fn report(parallel: Parallel) -> String {
    serde_json::to_string_pretty(&scan_fixture(parallel).report()).unwrap()
}

#[test]
fn the_result_does_not_depend_on_the_number_of_workers() {
    let sequential = report(Parallel::Disabled);

    for workers in [2, 3, 4, 16] {
        assert_eq!(
            report(Parallel::Workers(workers)),
            sequential,
            "{workers} workers"
        );
    }
}

#[test]
fn results_come_back_in_the_order_of_the_files() {
    let directory = fixture_dir().join("vendor/acme/app-lib/src");

    // Alternate a file that analyses with one that doesn't.
    let good = directory.join("Child.php");
    let broken = directory.join("broken.php");
    let missing = directory.join("does-not-exist.php");

    let paths: Vec<&std::path::Path> = (0..40)
        .map(|index| match index % 3 {
            0 => good.as_path(),
            1 => broken.as_path(),
            _ => missing.as_path(),
        })
        .collect();

    for workers in [1, 4, 7] {
        let results = analyse_files(&paths, workers);

        assert_eq!(results.len(), 40);

        for (index, result) in results.iter().enumerate() {
            match index % 3 {
                0 => assert!(result.is_ok(), "{index} with {workers} workers"),
                1 => assert!(
                    result
                        .as_ref()
                        .err()
                        .is_some_and(|message| message.starts_with("parse error")),
                    "{index} with {workers} workers"
                ),
                _ => assert!(
                    result
                        .as_ref()
                        .err()
                        .is_some_and(|message| message.starts_with("could not read")),
                    "{index} with {workers} workers"
                ),
            }
        }
    }
}

#[test]
fn no_files_is_not_an_error() {
    assert!(analyse_files(&[], 4).is_empty());
}
