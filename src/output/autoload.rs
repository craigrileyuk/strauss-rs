//! The autoloader for the prefixed packages: `autoload.php`, which loads
//! `autoload-classmap.php`.
//!
//! Every class, interface, trait and enum a package autoloads is listed under
//! its new name. With `optimize_autoloader` (the default) that is all there is:
//! it is classmap-authoritative, so loading a class never searches the disk.
//! Without it, a class that isn't in the classmap is also looked for through the
//! packages' PSR-4 directories, as Composer's unoptimised autoloader does. The
//! packages' `files` are required in dependency order.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use crate::edits::NameMap;
use crate::output::error::OutputError;
use crate::output::{Input, project_autoload_files};
use crate::packages::PackageSet;
use crate::symbols::SymbolKind;

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Autoloader {
    /// Each class-like's new name and its file, relative to the target directory.
    pub classmap: BTreeMap<String, String>,

    /// The files to require, relative to the target directory, in order.
    pub files: Vec<String>,

    /// PSR-4 prefixes (with a trailing backslash, as renamed) and the
    /// directories for each, relative to the target directory. Only without
    /// `optimize_autoloader`.
    pub psr4: Vec<(String, Vec<String>)>,
}

/// What the autoloader loads, from the files that were written.
pub(super) fn plan(
    input: &Input,
    written: &HashSet<String>,
    warnings: &mut Vec<String>,
) -> Autoloader {
    let mut autoloader = Autoloader::default();

    for symbol in &input.rename.symbols {
        if !matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum
        ) || !symbol.autoloaded
        {
            continue;
        }

        // Declared in several files (a polyfill, say): the first one that is
        // copied.
        let Some(source) = symbol
            .sources
            .iter()
            .find(|source| written.contains(&source.file))
        else {
            continue;
        };

        let name = symbol.replacement.as_deref().unwrap_or(&symbol.name);

        autoloader
            .classmap
            .entry(name.to_string())
            .or_insert_with(|| source.file.clone());
    }

    // The files of the project itself come first: the packages may rely on
    // what they define, as they could with Composer's autoloader.
    if input.config.include_root_autoload {
        let depth = input
            .config
            .target_directory
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != ".")
            .count();

        for file in project_autoload_files(input.config, warnings) {
            autoloader.files.push(format!(
                "{}{}",
                "../".repeat(depth),
                file.trim_start_matches("./")
            ));
        }
    }

    for name in dependency_order(input.packages) {
        let Some(package) = input.packages.get(&name) else {
            continue;
        };

        for file in &package.autoload.files {
            let path = format!("{}/{}", package.name, file.trim_start_matches("./"));

            if written.contains(&path) {
                autoloader.files.push(path);
            }
        }
    }

    if !input.config.optimize_autoloader {
        plan_psr4(input, written, &mut autoloader, warnings);
    }

    autoloader
}

/// The PSR-4 directories of the packages that were written, under the names the
/// namespaces now have. Longer prefixes come first, so the most specific wins.
fn plan_psr4(
    input: &Input,
    written: &HashSet<String>,
    autoloader: &mut Autoloader,
    warnings: &mut Vec<String>,
) {
    let names = NameMap::from_plan(input.rename);

    for name in dependency_order(input.packages) {
        let Some(package) = input.packages.get(&name) else {
            continue;
        };

        let prefix = format!("{}/", package.name);

        if !written.iter().any(|path| path.starts_with(&prefix)) {
            continue;
        }

        if !package.autoload.psr0.is_empty() {
            warnings.push(format!(
                "optimize_autoloader: {} uses psr-0, which is only autoloaded through the classmap",
                package.name
            ));
        }

        for (namespace, directories) in &package.autoload.psr4.0 {
            let namespace = namespace.trim_matches('\\');

            let renamed = if namespace.is_empty() {
                String::new()
            } else {
                format!("{}\\", names.new_namespace_text(namespace))
            };

            let directories = directories
                .iter()
                .map(
                    |directory| match directory.trim_start_matches("./").trim_matches('/') {
                        "" => package.name.clone(),
                        directory => format!("{}/{directory}", package.name),
                    },
                )
                .collect();

            autoloader.psr4.push((renamed, directories));
        }
    }

    autoloader
        .psr4
        .sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
}

/// The packages with what each requires before it.
pub(super) fn dependency_order(set: &PackageSet) -> Vec<String> {
    fn visit(set: &PackageSet, name: &str, seen: &mut HashSet<String>, order: &mut Vec<String>) {
        if !seen.insert(name.to_ascii_lowercase()) {
            return;
        }

        if let Some(package) = set.get(name) {
            for dependency in &package.dependencies {
                visit(set, dependency, seen, order);
            }

            order.push(package.name.clone());
        }
    }

    let mut seen = HashSet::new();
    let mut order = Vec::new();

    for package in &set.packages {
        visit(set, &package.name, &mut seen, &mut order);
    }

    order
}

/// A string for PHP source in single quotes.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

pub(super) fn classmap_source(autoloader: &Autoloader) -> String {
    let mut source = String::from(
        "<?php\n\n// autoload-classmap.php generated by strauss-rs\n\n$strauss_src = __DIR__;\n\nreturn array(\n",
    );

    for (name, file) in &autoloader.classmap {
        source.push_str(&format!(
            "    {} => $strauss_src . {},\n",
            quote(name),
            quote(&format!("/{file}"))
        ));
    }

    source.push_str(");\n");

    source
}

pub(super) fn autoload_source(autoloader: &Autoloader) -> String {
    let mut source = String::from(
        "<?php\n\n// autoload.php generated by strauss-rs\n\n$strauss_classmap = require __DIR__ . '/autoload-classmap.php';\n",
    );

    let fallback = !autoloader.psr4.is_empty();

    if fallback {
        source.push_str("\n$strauss_psr4 = array(\n");

        for (prefix, directories) in &autoloader.psr4 {
            source.push_str(&format!("    {} => array(\n", quote(prefix)));

            for directory in directories {
                source.push_str(&format!(
                    "        __DIR__ . {},\n",
                    quote(&format!("/{directory}"))
                ));
            }

            source.push_str("    ),\n");
        }

        source.push_str(");\n");
    }

    let uses = if fallback {
        "$strauss_classmap, $strauss_psr4"
    } else {
        "$strauss_classmap"
    };

    source.push_str(&format!(
        "\nspl_autoload_register(\n    static function ( $class ) use ( {uses} ) {{\n        $class = ltrim( $class, '\\\\' );\n\n        if ( isset( $strauss_classmap[ $class ] ) ) {{\n            require_once $strauss_classmap[ $class ];\n\n            return;\n        }}\n",
    ));

    if fallback {
        source.push_str(
            "\n        foreach ( $strauss_psr4 as $prefix => $directories ) {\n            if ( 0 !== strpos( $class, $prefix ) ) {\n                continue;\n            }\n\n            $relative = str_replace( '\\\\', '/', substr( $class, strlen( $prefix ) ) ) . '.php';\n\n            foreach ( $directories as $directory ) {\n                if ( is_file( $directory . '/' . $relative ) ) {\n                    require_once $directory . '/' . $relative;\n\n                    return;\n                }\n            }\n        }\n",
        );
    }

    source.push_str("    }\n);\n");

    if !autoloader.files.is_empty() {
        source.push_str("\nforeach (\n    array(\n");

        for file in &autoloader.files {
            source.push_str(&format!(
                "        __DIR__ . {},\n",
                quote(&format!("/{file}"))
            ));
        }

        source.push_str("    ) as $strauss_file\n) {\n    require_once $strauss_file;\n}\n\nunset( $strauss_file );\n");
    }

    source.push_str(if fallback {
        "\nunset( $strauss_classmap, $strauss_psr4 );\n"
    } else {
        "\nunset( $strauss_classmap );\n"
    });

    source
}

/// Writes both files into the target directory.
pub(super) fn write(
    autoloader: &Autoloader,
    target: &Path,
    dry_run: bool,
) -> Result<Vec<String>, OutputError> {
    let names = vec![
        "autoload-classmap.php".to_string(),
        "autoload.php".to_string(),
    ];

    if dry_run {
        return Ok(names);
    }

    fs::create_dir_all(target).map_err(|error| OutputError::io("create", target, error))?;

    for (name, source) in [
        ("autoload-classmap.php", classmap_source(autoloader)),
        ("autoload.php", autoload_source(autoloader)),
    ] {
        let path = target.join(name);

        fs::write(&path, source).map_err(|error| OutputError::io("write", path, error))?;
    }

    Ok(names)
}
