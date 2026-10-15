use super::*;

#[test]
fn the_packages_to_prefix_are_all_but_those_excluded_from_prefixing() {
    let run = default_run();

    assert_eq!(
        run.plan.packages_to_prefix,
        [
            "acme/psr4-lib",
            "acme/psr0-lib",
            "acme/classmap-lib",
            "acme/files-lib",
            "acme/not-copied",
            "acme/pattern"
        ]
    );
}

#[test]
fn package_exclusion_ignores_case_when_choosing_packages_to_prefix() {
    let run = run(|config| {
        config.exclude_from_prefix.packages =
            vec!["ACME/KEEP".to_string(), "Acme/Pattern".to_string()]
    });

    assert!(
        !run.plan
            .packages_to_prefix
            .contains(&"acme/keep".to_string())
    );
    assert!(
        !run.plan
            .packages_to_prefix
            .contains(&"acme/pattern".to_string())
    );
    assert_eq!(run.plan.packages_to_prefix.len(), 5);
}

#[test]
fn symbols_say_which_packages_declare_them() {
    let run = default_run();

    assert_eq!(
        symbol(&run.plan, SymbolKind::Class, "Acme\\Psr4\\Client").packages,
        ["acme/psr4-lib"]
    );
    assert_eq!(
        symbol(&run.plan, SymbolKind::Class, "Mapped").packages,
        ["acme/classmap-lib"]
    );
}

#[test]
fn counts_symbols_by_kind_and_by_reason() {
    let summary = default_run().plan.summary();

    assert_eq!(summary.symbols, 43);
    assert_eq!(summary.renamed, 27);

    let kind = |name: &str| (summary.by_kind[name].total, summary.by_kind[name].renamed);

    assert_eq!(kind("namespace"), (12, 8));
    assert_eq!(kind("class"), (16, 7));
    assert_eq!(kind("interface"), (2, 2));
    assert_eq!(kind("trait"), (1, 1));
    assert_eq!(kind("enum"), (1, 1));
    assert_eq!(kind("function"), (7, 5));
    assert_eq!(kind("constant"), (4, 3));

    assert_eq!(summary.reasons["autoloaded"], 23);
    assert_eq!(summary.reasons["namespace_replacement_pattern"], 4);
    assert_eq!(summary.reasons["not_autoloaded"], 5);
    assert_eq!(summary.reasons["already_prefixed"], 2);
    assert_eq!(summary.reasons["package_excluded_from_prefix"], 2);
    assert_eq!(summary.reasons["package_excluded_from_copy"], 2);
    assert_eq!(summary.reasons["namespace_excluded_from_prefix"], 2);
    assert_eq!(summary.reasons["file_excluded_from_prefix"], 1);
    assert_eq!(summary.reasons["only_in_files_not_copied"], 1);
    assert_eq!(summary.reasons["constant_excluded"], 1);

    // Every symbol has exactly one reason.
    assert_eq!(summary.reasons.values().sum::<usize>(), summary.symbols);
}

#[test]
fn looks_symbols_up_by_kind_and_name() {
    let plan = default_run().plan;

    assert!(plan.get(SymbolKind::Class, "acme\\psr4\\CLIENT").is_some());
    assert!(
        plan.get(SymbolKind::Class, "\\Acme\\Psr4\\Client")
            .is_some()
    );
    // Not a class.
    assert!(
        plan.get(SymbolKind::Interface, "Acme\\Psr4\\Client")
            .is_none()
    );
    // Constants are case-sensitive.
    assert!(plan.get(SymbolKind::Constant, "FILES_CONST").is_some());
    assert!(plan.get(SymbolKind::Constant, "files_const").is_none());
}

#[test]
fn the_plan_does_not_depend_on_the_number_of_workers() {
    let describe = |parallel: Parallel| -> String {
        let run = run(|config| config.parallel = parallel);

        serde_json::to_string(&run.plan).unwrap()
    };

    let sequential = describe(Parallel::Disabled);

    assert!(sequential.len() > 5000);
    assert_eq!(describe(Parallel::Workers(4)), sequential);
    assert_eq!(describe(Parallel::Auto), sequential);
}

#[test]
fn serialises_without_the_source_files() {
    let plan = default_run().plan;

    let json = serde_json::to_value(&plan).unwrap();
    let client = json["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|symbol| symbol["name"] == "Acme\\Psr4\\Client")
        .unwrap();

    assert_eq!(client["kind"], "class");
    assert_eq!(client["rename"], true);
    assert_eq!(
        client["replacement"],
        "Prefixed\\Vendor\\Acme\\Psr4\\Client"
    );
    assert_eq!(client["reason"], "it is autoloaded");
    assert!(client.get("sources").is_none());
    // Only shown when unusual.
    assert!(client.get("replace_in_strings").is_none());
    assert!(client.get("psr").is_none());
}

#[test]
fn serialises_what_is_unusual() {
    let json = serde_json::to_value(&default_run().plan).unwrap();
    let find = |name: &str| {
        json["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .find(|symbol| symbol["name"] == name)
            .unwrap()
            .clone()
    };

    assert_eq!(find("files_helper")["replace_in_strings"], false);
    assert_eq!(find("Zero")["psr"], "psr-0");
    assert_eq!(find("Acme\\Psr4")["psr"], "psr-4");
    assert!(find("Loose").get("replacement").is_none());
}
