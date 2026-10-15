//! PHP's own classes and functions, which are never renamed.
//!
//! A polyfill for an older PHP declares what a newer PHP has built in
//! (`Stringable`, `str_contains()`), and a renamed copy would no longer be
//! what other code expects.

use std::collections::HashSet;
use std::sync::OnceLock;

/// `builtins.txt`: one symbol per line, `c` for a class, interface, trait or
/// enum and `f` for a function. `#` starts a comment.
const DATA: &str = include_str!("builtins.txt");

#[derive(Debug)]
pub struct BuiltIns {
    /// Lower-cased: PHP's class and function names are case-insensitive.
    class_likes: HashSet<String>,
    functions: HashSet<String>,
}

impl BuiltIns {
    /// The built-in symbols, parsed on first use.
    pub fn get() -> &'static BuiltIns {
        static BUILT_INS: OnceLock<BuiltIns> = OnceLock::new();

        BUILT_INS.get_or_init(|| BuiltIns::parse(DATA))
    }

    pub fn parse(data: &str) -> BuiltIns {
        let mut built_ins = BuiltIns {
            class_likes: HashSet::new(),
            functions: HashSet::new(),
        };

        for line in data.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            match line.split_once(' ') {
                Some(("c", name)) => built_ins
                    .class_likes
                    .insert(name.trim().to_ascii_lowercase()),
                Some(("f", name)) => built_ins.functions.insert(name.trim().to_ascii_lowercase()),
                _ => continue,
            };
        }

        built_ins
    }

    /// Whether `name` is a built-in class, interface, trait or enum.
    pub fn is_class_like(&self, name: &str) -> bool {
        self.class_likes
            .contains(&name.trim_start_matches('\\').to_ascii_lowercase())
    }

    pub fn is_function(&self, name: &str) -> bool {
        self.functions
            .contains(&name.trim_start_matches('\\').to_ascii_lowercase())
    }
}
