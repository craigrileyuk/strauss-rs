use super::*;

fn table() -> SymbolTable {
    scan_fixture(Parallel::Disabled)
}

fn symbol<'a>(table: &'a SymbolTable, kind: SymbolKind, name: &str) -> &'a DiscoveredSymbol {
    table
        .get(kind, name)
        .unwrap_or_else(|| panic!("{kind:?} {name} not found"))
}

fn of_package<'a>(table: &'a SymbolTable, package: &'a str, kind: SymbolKind) -> Vec<String> {
    table
        .symbols_of(package)
        .filter(|symbol| symbol.kind == kind)
        .map(|symbol| symbol.name.clone())
        .collect()
}

#[test]
fn records_the_namespaces_a_package_declares() {
    let table = table();

    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Namespace),
        [
            "Acme\\App",
            "Acme\\App\\Concerns",
            "Acme\\App\\Contracts",
            "Acme\\App\\Deep"
        ]
    );
    assert_eq!(
        of_package(&table, "acme/poly", SymbolKind::Namespace),
        ["Acme\\Poly"]
    );
}

#[test]
fn the_global_namespace_is_not_a_symbol() {
    let table = table();

    assert!(
        table
            .symbols
            .iter()
            .all(|symbol| !(symbol.kind == SymbolKind::Namespace && symbol.name.is_empty()))
    );
    assert!(table.get(SymbolKind::Namespace, "").is_none());
}

#[test]
fn records_each_kind_of_symbol() {
    let table = table();

    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Class),
        [
            "Acme\\App\\Base",
            "Acme\\App\\Child",
            "Acme\\App\\Deep\\Inner",
            "GlobalClass",
            "Latin1Class"
        ]
    );
    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Interface),
        ["Acme\\App\\Contracts\\Marker"]
    );
    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Trait),
        ["Acme\\App\\Concerns\\Greets"]
    );
    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Enum),
        ["Acme\\App\\Status"]
    );
    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Function),
        ["global_function", "Acme\\App\\helper"]
    );
}

#[test]
fn constants_are_namespaced_unless_defined_with_define() {
    let table = table();

    assert_eq!(
        of_package(&table, "acme/app-lib", SymbolKind::Constant),
        ["Acme\\App\\VERSION", "ACME_APP_DEFINED"]
    );
    assert!(symbol(&table, SymbolKind::Constant, "ACME_APP_DEFINED").is_global());
    assert!(!symbol(&table, SymbolKind::Constant, "Acme\\App\\VERSION").is_global());
}

#[test]
fn a_symbol_knows_its_namespace_and_local_name() {
    let table = table();

    let helper = symbol(&table, SymbolKind::Function, "Acme\\App\\helper");
    assert_eq!(helper.namespace, "Acme\\App");
    assert_eq!(helper.local_name(), "helper");

    let global = symbol(&table, SymbolKind::Class, "GlobalClass");
    assert_eq!(global.namespace, "");
    assert_eq!(global.local_name(), "GlobalClass");
    assert!(global.is_global());

    let namespace = symbol(&table, SymbolKind::Namespace, "Acme\\App\\Deep");
    assert_eq!(namespace.local_name(), "Acme\\App\\Deep");
}

#[test]
fn records_what_classes_interfaces_and_enums_extend_and_implement() {
    let table = table();

    let base = symbol(&table, SymbolKind::Class, "Acme\\App\\Base");
    assert!(base.is_abstract);
    assert_eq!(
        base.implements,
        ["Countable", "Acme\\App\\Contracts\\Marker"]
    );

    let child = symbol(&table, SymbolKind::Class, "Acme\\App\\Child");
    assert!(!child.is_abstract);
    assert_eq!(child.extends, ["Acme\\App\\Base"]);

    let marker = symbol(
        &table,
        SymbolKind::Interface,
        "Acme\\App\\Contracts\\Marker",
    );
    assert_eq!(
        marker.extends,
        ["JsonSerializable", "Acme\\App\\Contracts\\Other"]
    );

    let status = symbol(&table, SymbolKind::Enum, "Acme\\App\\Status");
    assert_eq!(status.backing_type.as_deref(), Some("string"));
    assert_eq!(status.implements, ["Acme\\App\\Contracts\\Marker"]);
}

#[test]
fn records_where_each_symbol_is_declared() {
    let table = table();

    let child = symbol(&table, SymbolKind::Class, "Acme\\App\\Child");

    assert_eq!(
        child.sources,
        [SymbolSource {
            package: "acme/app-lib".to_string(),
            file: "acme/app-lib/src/Child.php".to_string(),
        }]
    );
}

#[test]
fn a_file_with_two_namespaces_declares_symbols_in_each() {
    let table = table();

    // Multi.php: `namespace Acme\App\Deep { … } namespace { … }`
    for (kind, name) in [
        (SymbolKind::Class, "Acme\\App\\Deep\\Inner"),
        (SymbolKind::Class, "GlobalClass"),
        (SymbolKind::Function, "global_function"),
    ] {
        assert_eq!(
            symbol(&table, kind, name).sources[0].file,
            "acme/app-lib/src/Multi.php"
        );
    }
}

#[test]
fn a_symbol_declared_in_two_packages_has_two_sources_and_the_first_ones_details() {
    let table = table();

    let base = symbol(&table, SymbolKind::Class, "Acme\\App\\Base");

    assert_eq!(base.packages(), ["acme/app-lib", "acme/dup"]);
    assert_eq!(base.sources.len(), 2);
    // acme/app-lib's file sorts first, and declares it abstract.
    assert!(base.is_abstract);

    assert_eq!(
        table
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Acme\\App\\Base")
            .count(),
        1
    );
}

#[test]
fn a_namespace_is_shared_by_the_packages_that_declare_it() {
    let table = table();

    let namespace = symbol(&table, SymbolKind::Namespace, "Acme\\App");

    assert_eq!(namespace.packages(), ["acme/app-lib", "acme/dup"]);
    assert_eq!(
        namespace
            .sources
            .iter()
            .map(|source| source.file.as_str())
            .collect::<Vec<_>>(),
        [
            "acme/app-lib/src/Base.php",
            "acme/app-lib/src/Child.php",
            "acme/app-lib/src/Status.php",
            "acme/app-lib/src/functions.php",
            "acme/dup/src/Base.php",
        ]
    );
}

#[test]
fn symbols_of_a_package_include_those_it_shares() {
    let table = table();

    assert_eq!(
        of_package(&table, "acme/dup", SymbolKind::Class),
        ["Acme\\App\\Base"]
    );
    assert_eq!(table.symbols_of("acme/nothing").count(), 0);
}

#[test]
fn looks_up_names_case_insensitively_except_constants() {
    let table = table();

    assert!(table.get(SymbolKind::Class, "acme\\app\\BASE").is_some());
    assert!(table.get(SymbolKind::Class, "\\Acme\\App\\Base").is_some());
    assert!(
        table
            .get(SymbolKind::Function, "ACME_POLY_HELPER")
            .is_some()
    );
    assert!(table.get(SymbolKind::Constant, "POLY_FLAG").is_some());
    assert!(table.get(SymbolKind::Constant, "poly_flag").is_none());
    // Not a class.
    assert!(
        table
            .get(SymbolKind::Interface, "Acme\\App\\Base")
            .is_none()
    );
}

#[test]
fn leaves_out_what_php_provides() {
    let table = table();

    let built_in = |kind: SymbolKind, name: &str| {
        table.built_ins.iter().any(|symbol| {
            symbol.kind == kind && symbol.name == name && symbol.package == "acme/poly"
        })
    };

    assert!(built_in(SymbolKind::Interface, "Stringable"));
    assert!(built_in(SymbolKind::Function, "str_contains"));
    assert!(built_in(SymbolKind::Class, "ArrayObject"));
    // Spelled differently from PHP's, which doesn't matter to PHP.
    assert!(built_in(SymbolKind::Function, "STR_STARTS_WITH"));

    for name in ["Stringable", "ArrayObject"] {
        assert!(table.get(SymbolKind::Interface, name).is_none());
        assert!(table.get(SymbolKind::Class, name).is_none());
    }

    assert!(table.get(SymbolKind::Function, "str_contains").is_none());
    assert!(table.get(SymbolKind::Function, "str_starts_with").is_none());
    assert_eq!(table.built_ins.len(), 4);
}

#[test]
fn a_polyfills_own_symbols_are_kept() {
    let table = table();

    assert_eq!(
        of_package(&table, "acme/poly", SymbolKind::Class),
        ["Acme\\Poly\\Polyfill"]
    );
    assert_eq!(
        of_package(&table, "acme/poly", SymbolKind::Function),
        ["acme_poly_helper"]
    );
    assert_eq!(
        of_package(&table, "acme/poly", SymbolKind::Constant),
        ["POLY_FLAG", "POLY_DEFINED"]
    );
}

#[test]
fn only_files_ending_in_php_are_analysed() {
    let table = table();

    // README.md, template.php.dist and Upper.PHP: PHP Strauss tests for a
    // lower-case `.php` suffix.
    assert!(table.get(SymbolKind::Class, "ShouldBeIgnored").is_none());
    assert!(table.get(SymbolKind::Class, "UpperExt").is_none());
    assert!(
        table
            .files
            .iter()
            .all(|file| file.vendor_relative_path.ends_with(".php"))
    );
    assert_eq!(table.files.len(), 12);
}

#[test]
fn a_file_that_does_not_parse_is_reported_and_its_symbols_are_missing() {
    let table = table();

    let warning = table
        .warnings
        .iter()
        .find(|warning| warning.file == "acme/app-lib/src/broken.php")
        .expect("a warning for broken.php");

    assert_eq!(warning.package, "acme/app-lib");
    assert!(
        warning.message.starts_with("parse error: "),
        "{}",
        warning.message
    );

    let file = table
        .files
        .iter()
        .find(|file| file.vendor_relative_path == "acme/app-lib/src/broken.php")
        .unwrap();

    assert!(file.analysis.is_none());
    assert!(!file.lossy);
}

#[test]
fn a_file_that_is_not_utf8_is_analysed_and_flagged() {
    let table = table();

    // Its class is known…
    assert_eq!(
        symbol(&table, SymbolKind::Class, "Latin1Class").sources[0].file,
        "acme/app-lib/src/latin1.php"
    );

    // …but it can't be edited safely.
    let file = table
        .files
        .iter()
        .find(|file| file.vendor_relative_path == "acme/app-lib/src/latin1.php")
        .unwrap();

    assert!(file.lossy);
    assert!(file.analysis.is_some());

    let warning = table
        .warnings
        .iter()
        .find(|warning| warning.file == "acme/app-lib/src/latin1.php")
        .unwrap();

    assert!(
        warning.message.contains("not valid UTF-8"),
        "{}",
        warning.message
    );
}

#[test]
fn there_are_no_other_warnings() {
    assert_eq!(table().warnings.len(), 2);
}

#[test]
fn keeps_each_files_analysis_for_later() {
    let table = table();

    let file = table
        .files
        .iter()
        .find(|file| file.vendor_relative_path == "acme/app-lib/src/Child.php")
        .unwrap();

    let analysis = file.analysis.as_ref().unwrap();

    assert_eq!(
        analysis.namespaces[0].namespace.as_deref(),
        Some("Acme\\App")
    );
    // `extends Base` is a reference to Acme\App\Base.
    assert_eq!(analysis.namespaces[0].references[0].name, "Acme\\App\\Base");
    assert_eq!(
        file.source_path,
        fixture_dir().join("vendor/acme/app-lib/src/Child.php")
    );
}

#[test]
fn summarises_each_package() {
    let packages = table().by_package();

    let app = &packages["acme/app-lib"];

    assert_eq!(app.php_files, 9);
    assert_eq!(app.unanalysed_files, 1);
    assert_eq!(app.lossy_files, 1);
    assert_eq!(app.classes.len(), 5);
    assert_eq!(app.interfaces, ["Acme\\App\\Contracts\\Marker"]);

    let poly = &packages["acme/poly"];

    assert_eq!(poly.php_files, 2);
    assert_eq!(poly.unanalysed_files, 0);
    assert_eq!(poly.lossy_files, 0);

    // Shared symbols are in each package.
    assert_eq!(packages["acme/dup"].classes, ["Acme\\App\\Base"]);
    assert_eq!(
        packages.keys().collect::<Vec<_>>(),
        ["acme/app-lib", "acme/dup", "acme/poly"]
    );
}

#[test]
fn symbols_are_in_the_order_the_files_are_found() {
    let table = table();

    // Files are ordered by path (byte-wise, so `Base.php` is before `broken.php`):
    // acme/app-lib's `src/Base.php`, `src/Child.php`, then `src/Concerns/Greets.php`.
    let first: Vec<&str> = table
        .symbols
        .iter()
        .take(5)
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert_eq!(
        first,
        [
            "Acme\\App",
            "Acme\\App\\Base",
            "Acme\\App\\Child",
            "Acme\\App\\Concerns",
            "Acme\\App\\Concerns\\Greets"
        ]
    );

    // acme/poly's files come last.
    assert_eq!(
        table.symbols.last().unwrap().sources[0].package,
        "acme/poly"
    );
}
