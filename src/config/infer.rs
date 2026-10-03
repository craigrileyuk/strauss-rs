//! Values PHP Strauss infers from `composer.json` when an option is not set.

use crate::config::composer::{Autoload, ComposerJson};

/// The prefix PHP Strauss uses when `namespace_prefix` isn't set: the
/// project's first `psr-4` namespace, else its first `psr-0` namespace, else
/// its package name (`acme/my-plugin` -> `Acme\My_Plugin`), else the
/// classmap prefix without its trailing underscore.
pub(super) fn infer_namespace_prefix(
    composer: &ComposerJson,
    classmap_prefix: Option<&str>,
) -> Option<String> {
    if let Some(key) = first_autoload_namespace(composer) {
        return Some(key.trim_matches('\\').to_string());
    }

    if let Some(name) = package_name(composer) {
        let prefix = capitalise_words(&words_with_separators(name).replace('/', "\\"));

        return Some(prefix.trim_matches('\\').to_string());
    }

    classmap_prefix
        .map(|prefix| prefix.trim_end_matches('_').to_string())
        .filter(|prefix| !prefix.is_empty())
}

/// The prefix PHP Strauss uses when `classmap_prefix` isn't set: the
/// project's first `psr-4` or `psr-0` namespace with underscores for
/// backslashes (`Acme\Plugin\` -> `Acme_Plugin_`), else its package name
/// (`acme/my-plugin` -> `Acme_My_Plugin`), else the namespace prefix.
pub(super) fn infer_classmap_prefix(
    composer: &ComposerJson,
    namespace_prefix: Option<&str>,
) -> Option<String> {
    if let Some(key) = first_autoload_namespace(composer) {
        return Some(key.replace('\\', "_"));
    }

    if let Some(name) = package_name(composer) {
        return Some(capitalise_words(
            &words_with_separators(name).replace('/', "_"),
        ));
    }

    namespace_prefix
        .map(|prefix| format!("{}_", words_with_separators(prefix).trim_end_matches('_')))
        .filter(|prefix| prefix != "_")
}

fn first_autoload_namespace(composer: &ComposerJson) -> Option<&str> {
    composer
        .autoload
        .psr4
        .0
        .iter()
        .chain(composer.autoload.psr0.0.iter())
        .map(|(namespace, _)| namespace.as_str())
        .find(|namespace| !namespace.trim_matches('\\').is_empty())
}

/// The root package's name. A `composer.json` without a name is
/// `__root__` to Composer, which Strauss ignores.
fn package_name(composer: &ComposerJson) -> Option<&str> {
    composer
        .name
        .as_deref()
        .filter(|name| !name.is_empty() && *name != "__root__")
}

/// `preg_replace('/[^\w\/]+/', '_', $name)`: every run of characters that
/// aren't word characters or `/` becomes a single underscore.
fn words_with_separators(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut in_separator_run = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '/' {
            output.push(character);
            in_separator_run = false;
        } else if !in_separator_run {
            output.push('_');
            in_separator_run = true;
        }
    }

    output
}

/// Uppercases a lowercase letter at the start, or after `_` or `\`
/// (`preg_replace_callback('/(?<=^|_|\\\\)[a-z]/', 'strtoupper', …)`).
fn capitalise_words(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut previous: Option<char> = None;

    for character in name.chars() {
        let at_word_start = matches!(previous, None | Some('_') | Some('\\'));

        if at_word_start && character.is_ascii_lowercase() {
            output.push(character.to_ascii_uppercase());
        } else {
            output.push(character);
        }

        previous = Some(character);
    }

    output
}

/// Whether any of the project's autoload paths is `directory`.
pub(super) fn autoloads_directory(autoload: &Autoload, directory: &str) -> bool {
    autoload.paths().any(|path| {
        path.trim_matches(|character| character == '/' || character == '\\') == directory
    })
}
