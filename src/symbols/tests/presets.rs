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

#[test]
fn woocommerce_symbols_are_recognised() {
    let woocommerce = PresetSymbols::get("woocommerce").unwrap();

    assert!(woocommerce.is_class_like("WooCommerce"));
    assert!(woocommerce.is_class_like("wc_order"));
    assert!(woocommerce.is_function("wc_get_order"));
    assert!(woocommerce.is_constant("WC_ABSPATH"));
    assert!(!woocommerce.is_constant("wc_abspath"));
    assert!(!woocommerce.is_class_like("Acme\\WC_Order"));
}

#[test]
fn each_preset_only_has_its_own_symbols() {
    let wordpress = PresetSymbols::get("wordpress").unwrap();
    let woocommerce = PresetSymbols::get("woocommerce").unwrap();

    assert!(!wordpress.is_class_like("WC_Order"));
    assert!(!wordpress.is_function("wc_get_order"));
    // WooCommerce's stubs depend on Wordpress', but the lists don't mix.
    assert!(!woocommerce.is_class_like("WP_Error"));
    assert!(!woocommerce.is_function("add_action"));
}

#[test]
fn leaves_out_what_woocommerce_provides_when_it_is_on() {
    let table = scan_presets(&["wordpress", "woocommerce"]);

    assert_eq!(
        provided(&table, SymbolKind::Class, "WC_Order").as_deref(),
        Some("woocommerce")
    );
    assert_eq!(
        provided(&table, SymbolKind::Function, "WC_GET_ORDER").as_deref(),
        Some("woocommerce")
    );
    assert_eq!(
        provided(&table, SymbolKind::Constant, "WC_ABSPATH").as_deref(),
        Some("woocommerce")
    );
    assert!(table.get(SymbolKind::Class, "WC_Order").is_none());
}

#[test]
fn woocommerce_symbols_are_ordinary_unless_the_preset_is_on() {
    let table = scan_presets(&["wordpress"]);

    assert!(table.get(SymbolKind::Class, "WC_Order").is_some());
    assert!(table.get(SymbolKind::Constant, "WC_ABSPATH").is_some());
    assert_eq!(provided(&table, SymbolKind::Class, "WC_Order"), None);
}

#[test]
fn presets_have_titles() {
    assert_eq!(PresetSymbols::title("woocommerce"), "WooCommerce");
    assert_eq!(PresetSymbols::title("wordpress"), "Wordpress");
    assert_eq!(PresetSymbols::title("wp-cli"), "WP-CLI");
    assert_eq!(PresetSymbols::title("php"), "PHP");
}

#[test]
fn wp_cli_symbols_are_recognised() {
    let wp_cli = PresetSymbols::get("wp-cli").unwrap();

    assert!(wp_cli.is_class_like("WP_CLI"));
    assert!(wp_cli.is_class_like("wp_cli_command"));
    assert!(wp_cli.is_class_like("Plugin_Command"));
    assert!(wp_cli.is_constant("WP_CLI"));
    assert!(!wp_cli.is_constant("wp_cli"));
    // Namespaced, so a different class.
    assert!(!wp_cli.is_class_like("WP_CLI\\Utils"));
    assert!(!wp_cli.is_class_like("WP_Error"));
    assert!(
        !PresetSymbols::get("wordpress")
            .unwrap()
            .is_class_like("WP_CLI_Command")
    );
}

#[test]
fn leaves_out_what_wp_cli_provides_when_it_is_on() {
    let table = scan_presets(&["wp-cli"]);

    assert_eq!(
        provided(&table, SymbolKind::Class, "WP_CLI_Command").as_deref(),
        Some("wp-cli")
    );
    assert_eq!(
        provided(&table, SymbolKind::Constant, "WP_CLI").as_deref(),
        Some("wp-cli")
    );
    assert!(table.get(SymbolKind::Class, "WP_CLI_Command").is_none());

    let table = scan_presets(&["wordpress"]);

    assert!(table.get(SymbolKind::Class, "WP_CLI_Command").is_some());
    assert!(table.get(SymbolKind::Constant, "WP_CLI").is_some());
}

#[test]
fn php_constants_are_recognised_and_case_sensitive() {
    let php = PresetSymbols::get("php-constants").unwrap();

    for name in [
        "MB_CASE_TITLE",
        "E_ALL",
        "PHP_EOL",
        "JSON_THROW_ON_ERROR",
        "\\MB_CASE_UPPER",
    ] {
        assert!(php.is_constant(name), "{name}");
    }

    // From scripts/php-constants-extra.txt.
    assert!(php.is_constant("UUID_TYPE_RANDOM"));
    assert!(!php.is_constant("mb_case_title"));
    assert!(!php.is_constant("ACME_OWN_CONSTANT"));
    // Constants only: classes and functions are always handled as PHP's own.
    assert!(!php.is_class_like("ArrayObject"));
    assert!(!php.is_function("strlen"));
}

#[test]
fn the_php_constants_preset_is_opt_in() {
    let table = scan_presets(&["wordpress"]);

    assert!(table.get(SymbolKind::Constant, "MB_CASE_TITLE").is_some());

    let table = scan_presets(&["php-constants"]);

    assert_eq!(
        provided(&table, SymbolKind::Constant, "MB_CASE_TITLE").as_deref(),
        Some("php-constants")
    );
    assert!(table.get(SymbolKind::Constant, "MB_CASE_TITLE").is_none());
    // A constant of the package's own is still renamed.
    assert!(
        table
            .get(SymbolKind::Constant, "ACME_OWN_CONSTANT")
            .is_some()
    );
}
