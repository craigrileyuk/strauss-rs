use super::*;

#[test]
fn nothing_is_deleted_unless_asked() {
    let (scratch, _, report) = written_fixture(|_| {});

    assert_eq!(report.deleted_files, 0);
    assert!(exists(&scratch, "vendor/acme/core/src/Client.php"));
}

#[test]
fn delete_vendor_files_removes_what_was_copied_and_the_directories_it_leaves_empty() {
    let (scratch, _, report) = written_fixture(|config| config.delete_vendor_files = true);

    assert_eq!(report.deleted_files, 13);
    assert!(report.deleted_packages.is_empty());
    assert!(!exists(&scratch, "vendor/acme"));

    // Composer's own files are not touched, and the copies are there.
    assert!(exists(&scratch, "vendor/composer/installed.json"));
    assert!(exists(&scratch, "vendor-prefixed/acme/core/src/Client.php"));
}

#[test]
fn delete_vendor_packages_removes_the_packages() {
    let (scratch, _, report) = written_fixture(|config| config.delete_vendor_packages = true);

    assert_eq!(report.deleted_files, 13);
    assert_eq!(
        report.deleted_packages,
        ["acme/core", "acme/globals", "acme/consumer", "acme/keep"]
    );
    assert!(!exists(&scratch, "vendor/acme"));
    assert!(exists(&scratch, "vendor/composer/installed.json"));
}

#[test]
fn a_package_with_files_that_are_not_copied_loses_only_those_that_are() {
    let (scratch, _, report) = written_fixture(|config| {
        config.delete_vendor_packages = true;
        config
            .exclude_from_copy
            .file_patterns
            .push(crate::php_pattern::PhpPattern::regex("#acme/core/src/Util\\.php#").unwrap());
    });

    assert!(
        report
            .deleted_packages
            .iter()
            .all(|name| name != "acme/core")
    );
    assert!(report.warnings.iter().any(|warning| {
        warning.starts_with("acme/core is not deleted: some of its files are not copied")
    }));

    assert!(exists(&scratch, "vendor/acme/core/src/Util.php"));
    assert!(!exists(&scratch, "vendor/acme/core/src/Client.php"));
    assert!(!exists(&scratch, "vendor/acme/core/src/Sub"));
    assert!(!exists(&scratch, "vendor/acme/consumer"));
}

#[test]
fn nothing_is_deleted_by_a_dry_run() {
    let scratch = scratch_copy("edit-fixture");
    let pipeline = pipeline(scratch.path(), |config| {
        config.delete_vendor_packages = true
    });

    let report = pipeline.write(true);

    assert_eq!(report.deleted_files, 13);
    assert!(exists(&scratch, "vendor/acme/core/src/Client.php"));
    assert!(!exists(&scratch, "vendor-prefixed"));
}

#[test]
fn nothing_is_deleted_when_the_files_are_changed_in_place() {
    let (scratch, _, report) = written_fixture(|config| {
        config.target_directory = config.vendor_directory.clone();
        config.delete_vendor_files = true;
    });

    assert_eq!(report.deleted_files, 0);
    assert!(exists(&scratch, "vendor/acme/core/src/Client.php"));
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("are ignored"))
    );
}

#[cfg(unix)]
#[test]
fn a_package_that_is_a_link_is_not_deleted() {
    let scratch = scratch_copy("edit-fixture");
    let elsewhere = scratch.path().join("elsewhere");

    fs::create_dir_all(&elsewhere).unwrap();
    fs::rename(
        scratch.path().join("vendor/acme/core"),
        elsewhere.join("core"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        elsewhere.join("core"),
        scratch.path().join("vendor/acme/core"),
    )
    .unwrap();

    let report = pipeline(scratch.path(), |config| {
        config.delete_vendor_packages = true
    })
    .write(false);

    assert!(elsewhere.join("core/src/Client.php").exists());
    assert!(
        report
            .warnings
            .contains(&"acme/core is not deleted: it is a link to another directory".to_string())
    );
    assert!(!exists(&scratch, "vendor/acme/consumer"));
}

#[test]
fn a_package_with_nothing_copied_is_left_alone() {
    let (scratch, _, report) = written_fixture(|config| {
        config.delete_vendor_packages = true;
        config
            .exclude_from_copy
            .packages
            .push("acme/keep".to_string());
    });

    assert!(exists(&scratch, "vendor/acme/keep/src/Kept.php"));
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("acme/keep"))
    );
    assert!(!report.deleted_packages.contains(&"acme/keep".to_string()));
    assert!(!exists(&scratch, "vendor/acme/core"));
}

fn installed_names(scratch: &Scratch) -> Vec<String> {
    let installed: serde_json::Value =
        serde_json::from_str(&read(scratch, "vendor/composer/installed.json")).unwrap();

    installed["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| package["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn deleted_packages_are_removed_from_composers_installed_json() {
    let (scratch, _, report) = written_fixture(|config| config.delete_vendor_packages = true);

    assert!(report.composer_updated);
    assert!(installed_names(&scratch).is_empty());
}

#[test]
fn only_the_packages_that_were_deleted_are_removed_from_installed_json() {
    let (scratch, _, _) = written_fixture(|config| {
        config.delete_vendor_packages = true;
        config
            .exclude_from_copy
            .packages
            .push("acme/keep".to_string());
    });

    assert_eq!(installed_names(&scratch), ["acme/keep"]);
}

#[test]
fn installed_json_is_left_alone_unless_something_was_deleted() {
    let scratch = scratch_copy("edit-fixture");
    let before = read(&scratch, "vendor/composer/installed.json");
    let pipeline = pipeline(scratch.path(), |_| {});
    let report = pipeline.write(false);

    assert!(!report.composer_updated);
    assert_eq!(read(&scratch, "vendor/composer/installed.json"), before);
}

#[test]
fn a_dry_run_leaves_installed_json_alone() {
    let scratch = scratch_copy("edit-fixture");
    let before = read(&scratch, "vendor/composer/installed.json");
    let pipeline = pipeline(scratch.path(), |config| {
        config.delete_vendor_packages = true
    });

    pipeline.write(true);

    assert_eq!(read(&scratch, "vendor/composer/installed.json"), before);
}

#[test]
fn a_composer_that_cannot_be_run_is_a_warning_that_says_what_to_do() {
    let scratch = scratch_copy("edit-fixture");
    let pipeline = pipeline(scratch.path(), |config| {
        config.delete_vendor_packages = true
    });
    let mut options = Options::new(&pipeline.config, false, DATE.to_string());

    options.composer = Some("/no/such/composer".to_string());

    let report = write(&pipeline.input(), &options).unwrap();

    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("run it yourself")),
        "{:?}",
        report.warnings
    );
}
