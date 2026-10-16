//! Symbols that something other than PHP provides, which are never renamed
//! when the preset for them is enabled (`exclude_presets`).
//!
//! A library can declare what the platform it runs on already has, usually in
//! a shim guarded by `function_exists()` or `class_exists()`. A renamed copy
//! would no longer be what the platform's own code expects.

use std::collections::HashSet;
use std::sync::OnceLock;

/// The presets that can be named in `exclude_presets`.
pub const PRESET_NAMES: &[&str] = &["wordpress", "woocommerce", "wp-cli", "php-constants"];

/// The presets on when `exclude_presets` isn't set. WooCommerce and WP-CLI are
/// only for plugins that extend them, and `php-constants` can leave out a
/// constant that a package means to define, so they are opt-in.
pub const DEFAULT_PRESETS: &[&str] = &["wordpress"];

/// `wordpress.txt`, `woocommerce.txt`, `wp-cli.txt` and `php-constants.txt`: one symbol per line, `c` for a class, interface or trait,
/// `f` for a function and `k` for a constant. `#` starts a comment. Generated
/// by `scripts/generate-presets.sh`.
const WORDPRESS: &str = include_str!("presets/wordpress.txt");
const WOOCOMMERCE: &str = include_str!("presets/woocommerce.txt");
const WP_CLI: &str = include_str!("presets/wp-cli.txt");
const PHP_CONSTANTS: &str = include_str!("presets/php-constants.txt");

#[derive(Debug, Default)]
pub struct PresetSymbols {
    /// Lower-cased: PHP's class and function names are case-insensitive.
    class_likes: HashSet<String>,
    functions: HashSet<String>,
    /// As written: constants are case-sensitive.
    constants: HashSet<String>,
}

impl PresetSymbols {
    /// The symbols of a preset, parsed on first use. `None` if there is no
    /// such preset.
    pub fn get(preset: &str) -> Option<&'static PresetSymbols> {
        static WORDPRESS_SYMBOLS: OnceLock<PresetSymbols> = OnceLock::new();
        static WOOCOMMERCE_SYMBOLS: OnceLock<PresetSymbols> = OnceLock::new();
        static WP_CLI_SYMBOLS: OnceLock<PresetSymbols> = OnceLock::new();
        static PHP_CONSTANTS_SYMBOLS: OnceLock<PresetSymbols> = OnceLock::new();

        match preset {
            "wordpress" => Some(WORDPRESS_SYMBOLS.get_or_init(|| PresetSymbols::parse(WORDPRESS))),
            "woocommerce" => {
                Some(WOOCOMMERCE_SYMBOLS.get_or_init(|| PresetSymbols::parse(WOOCOMMERCE)))
            }
            "wp-cli" => Some(WP_CLI_SYMBOLS.get_or_init(|| PresetSymbols::parse(WP_CLI))),
            "php-constants" => {
                Some(PHP_CONSTANTS_SYMBOLS.get_or_init(|| PresetSymbols::parse(PHP_CONSTANTS)))
            }
            _ => None,
        }
    }

    /// What the platform is called in a sentence: `WooCommerce`, not
    /// `woocommerce`.
    pub fn title(preset: &str) -> &str {
        match preset {
            "php" => "PHP",
            "wordpress" => "Wordpress",
            "woocommerce" => "WooCommerce",
            "wp-cli" => "WP-CLI",
            "php-constants" => "PHP",
            other => other,
        }
    }

    pub fn parse(data: &str) -> PresetSymbols {
        let mut symbols = PresetSymbols::default();

        for line in data.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            match line.split_once(' ') {
                Some(("c", name)) => symbols.class_likes.insert(name.trim().to_ascii_lowercase()),
                Some(("f", name)) => symbols.functions.insert(name.trim().to_ascii_lowercase()),
                Some(("k", name)) => symbols.constants.insert(name.trim().to_string()),
                _ => continue,
            };
        }

        symbols
    }

    pub fn is_class_like(&self, name: &str) -> bool {
        self.class_likes
            .contains(&name.trim_start_matches('\\').to_ascii_lowercase())
    }

    pub fn is_function(&self, name: &str) -> bool {
        self.functions
            .contains(&name.trim_start_matches('\\').to_ascii_lowercase())
    }

    pub fn is_constant(&self, name: &str) -> bool {
        self.constants.contains(name.trim_start_matches('\\'))
    }
}
