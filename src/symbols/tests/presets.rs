use super::*;

fn scan_presets(presets: &[&str]) -> SymbolTable {
    let dir =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/presets-fixture"))
            .unwrap();
    let mut config = Config::load(&dir).unwrap();
    config.exclude_presets = presets.iter().map(|name| name.to_string()).collect();

    let set = enumerate_packages(&config).unwrap();
    let files = enumerate_files(&config, &set).unwrap();

    scan(&config, &files)
}

fn provided(table: &SymbolTable, kind: SymbolKind, name: &str) -> Option<String> {
    table
        .built_ins
        .iter()
        .find(|symbol| symbol.kind == kind && symbol.name == name)
        .map(|symbol| symbol.provided_by.clone())
}

#[test]
fn wordpress_symbols_are_recognised() {
    let wordpress = PresetSymbols::get("wordpress").unwrap();

    assert!(wordpress.is_class_like("WP_Error"));
    assert!(wordpress.is_class_like("\\wpdb"));
    assert!(wordpress.is_class_like("wp_query"));
    assert!(wordpress.is_function("add_action"));
    assert!(wordpress.is_function("WP_JSON_ENCODE"));
    assert!(wordpress.is_constant("ABSPATH"));
    assert!(wordpress.is_constant("WP_DEBUG"));
}

#[test]
fn wordpress_constants_are_case_sensitive_and_names_are_exact() {
    let wordpress = PresetSymbols::get("wordpress").unwrap();

    assert!(!wordpress.is_constant("abspath"));
    assert!(!wordpress.is_class_like("Acme\\WP_Error"));
    assert!(!wordpress.is_function("Acme\\add_action"));
    assert!(!wordpress.is_class_like("Acme_Shim_Thing"));
    assert!(!wordpress.is_function("add_action_not"));
}

#[test]
fn there_is_no_preset_for_an_unknown_name() {
    assert!(PresetSymbols::get("drupal").is_none());
}

#[test]
fn every_named_preset_has_symbols() {
    for name in crate::symbols::presets::PRESET_NAMES {
        assert!(PresetSymbols::get(name).is_some(), "{name}");
    }
}

#[test]
fn leaves_out_what_wordpress_provides() {
    let table = scan_presets(&["wordpress"]);

    assert_eq!(
        provided(&table, SymbolKind::Class, "WP_Error").as_deref(),
        Some("wordpress")
    );
    // Spelled differently from Wordpress', which doesn't matter to PHP.
    assert_eq!(
        provided(&table, SymbolKind::Function, "WP_JSON_ENCODE").as_deref(),
        Some("wordpress")
    );
    assert_eq!(
        provided(&table, SymbolKind::Constant, "ABSPATH").as_deref(),
        Some("wordpress")
    );
    assert_eq!(
        provided(&table, SymbolKind::Constant, "WP_DEBUG").as_deref(),
        Some("wordpress")
    );

    assert!(table.get(SymbolKind::Class, "WP_Error").is_none());
    assert!(table.get(SymbolKind::Function, "wp_json_encode").is_none());
    assert!(table.get(SymbolKind::Constant, "ABSPATH").is_none());
    assert_eq!(table.built_ins.len(), 4);
}

#[test]
fn keeps_symbols_wordpress_does_not_provide() {
    let table = scan_presets(&["wordpress"]);

    assert!(table.get(SymbolKind::Class, "Acme_Shim_Thing").is_some());
    assert!(
        table
            .get(SymbolKind::Function, "acme_shim_helper")
            .is_some()
    );
    assert!(table.get(SymbolKind::Constant, "wp_debug_lower").is_some());
    // Same name as Wordpress', in a namespace.
    assert!(
        table
            .get(SymbolKind::Class, "Acme\\Shim\\WP_Error")
            .is_some()
    );
    assert!(
        table
            .get(SymbolKind::Function, "Acme\\Shim\\add_action")
            .is_some()
    );
}

#[test]
fn without_the_preset_wordpress_symbols_are_ordinary() {
    let table = scan_presets(&[]);

    assert!(table.built_ins.is_empty());
    assert!(table.get(SymbolKind::Class, "WP_Error").is_some());
    assert!(table.get(SymbolKind::Function, "wp_json_encode").is_some());
    assert!(table.get(SymbolKind::Constant, "ABSPATH").is_some());
}

#[test]
fn php_built_ins_are_attributed_to_php() {
    // A preset doesn't change what PHP provides.
    let table = scan_fixture(Parallel::Disabled);

    assert!(
        table
            .built_ins
            .iter()
            .all(|symbol| symbol.provided_by == "php")
    );
}
