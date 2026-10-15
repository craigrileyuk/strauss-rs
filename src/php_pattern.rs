#![allow(dead_code)]
//! Patterns as PHP Strauss accepts them in its config.
//!
//! Strauss hands the user's strings to `preg_match()` / `preg_replace()`, so
//! the same strings have to keep working here. Two things differ from a
//! plain regex crate: patterns may carry PCRE delimiters (`~foo~`), and
//! replacements use `preg_replace` syntax (`$1`, `${1}`, `\1`, `\\`).
//!
//! `fancy-regex` is used rather than `regex` because PCRE patterns commonly
//! use lookarounds and backreferences.

use fancy_regex::{Captures, Regex};

#[derive(Debug)]
pub struct PhpPattern {
    source: String,
    regex: Regex,
}

impl PhpPattern {
    /// A pattern that is always a regular expression, e.g. `file_patterns`.
    ///
    /// Mirrors Strauss' `preparePattern()`: a pattern that starts and ends
    /// with the same delimiter character is used as written; anything else is
    /// wrapped in `#…#`.
    pub fn regex(raw: &str) -> Result<Self, String> {
        let body = delimited_body(raw).unwrap_or(raw);

        Self::compile(raw, body)
    }

    /// A pattern that is literal text unless it is delimited, e.g. the keys of
    /// `namespace_replacement_patterns`.
    ///
    /// Mirrors the code in Strauss that performs the replacement: a key such
    /// as `Acme\Lib` is matched literally (`preg_quote`), while `~Acme\\(.*)~`
    /// is a regular expression.
    pub fn literal_or_regex(raw: &str) -> Result<Self, String> {
        match delimited_body(raw) {
            Some(body) => Self::compile(raw, body),
            None => Self::compile(raw, &fancy_regex::escape(raw)),
        }
    }

    fn compile(raw: &str, body: &str) -> Result<Self, String> {
        let regex =
            Regex::new(body).map_err(|error| format!("invalid pattern `{raw}`: {error}"))?;

        Ok(Self {
            source: raw.to_string(),
            regex,
        })
    }

    /// The pattern as the user wrote it.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the pattern matches anywhere in `subject`. A pattern that
    /// fails at match time (e.g. exceeds the backtracking limit) is treated as
    /// not matching, as `preg_match() === 1` does in Strauss.
    pub fn is_match(&self, subject: &str) -> bool {
        self.regex.is_match(subject).unwrap_or(false)
    }

    /// `preg_replace($pattern, $replacement, $subject)`.
    pub fn replace_all(&self, subject: &str, replacement: &Replacement) -> Result<String, String> {
        let mut output = String::with_capacity(subject.len());
        let mut last_end = 0;

        for captures in self.regex.captures_iter(subject) {
            let captures = captures.map_err(|error| error.to_string())?;

            let Some(whole) = captures.get(0) else {
                continue;
            };

            output.push_str(&subject[last_end..whole.start()]);
            replacement.expand(&captures, &mut output);
            last_end = whole.end();
        }

        output.push_str(&subject[last_end..]);

        Ok(output)
    }
}

/// The text between a pattern's delimiters, if it has them.
///
/// A delimiter is any character other than a letter, digit, backslash or
/// whitespace, and the pattern must start and end with the same one. PHP also
/// allows trailing modifiers (`/foo/i`), but Strauss' check — first character
/// equals last — never treats such a pattern as delimited, so neither do we.
fn delimited_body(raw: &str) -> Option<&str> {
    let first = raw.chars().next()?;
    let last = raw.chars().next_back()?;

    if raw.len() < first.len_utf8() + last.len_utf8() {
        return None;
    }

    let is_delimiter = !first.is_alphanumeric() && first != '\\' && !first.is_whitespace();

    if first != last || !is_delimiter {
        return None;
    }

    Some(&raw[first.len_utf8()..raw.len() - last.len_utf8()])
}

/// A `preg_replace` replacement string, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Group(usize),
}

impl Replacement {
    /// Parses a replacement string the way PHP reads it: `$n`, `${n}` and
    /// `\n` (up to two digits) insert a capture group, `\\` is a backslash,
    /// and anything else is literal. This never fails; a reference to a
    /// group that doesn't exist expands to nothing.
    pub fn parse(raw: &str) -> Self {
        let chars: Vec<char> = raw.chars().collect();
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut index = 0;

        let flush = |literal: &mut String, parts: &mut Vec<Part>| {
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(literal)));
            }
        };

        while index < chars.len() {
            match chars[index] {
                '\\' if chars.get(index + 1) == Some(&'\\') => {
                    literal.push('\\');
                    index += 2;
                }

                '\\' | '$' if chars.get(index + 1).is_some_and(char::is_ascii_digit) => {
                    let (group, length) = read_digits(&chars[index + 1..]);

                    flush(&mut literal, &mut parts);
                    parts.push(Part::Group(group));
                    index += 1 + length;
                }

                '$' if chars.get(index + 1) == Some(&'{') => {
                    let (group, length) = read_digits(&chars[index + 2..]);

                    if length > 0 && chars.get(index + 2 + length) == Some(&'}') {
                        flush(&mut literal, &mut parts);
                        parts.push(Part::Group(group));
                        index += 3 + length;
                    } else {
                        literal.push('$');
                        index += 1;
                    }
                }

                character => {
                    literal.push(character);
                    index += 1;
                }
            }
        }

        flush(&mut literal, &mut parts);

        Self { parts }
    }

    fn expand(&self, captures: &Captures<'_, str>, output: &mut String) {
        for part in &self.parts {
            match part {
                Part::Literal(text) => output.push_str(text),
                Part::Group(group) => {
                    if let Some(capture) = captures.get(*group) {
                        output.push_str(capture.as_str());
                    }
                }
            }
        }
    }
}

/// Reads up to two leading digits, as PCRE replacement references do.
fn read_digits(chars: &[char]) -> (usize, usize) {
    let digits: String = chars
        .iter()
        .take(2)
        .take_while(|character| character.is_ascii_digit())
        .collect();

    (digits.parse().unwrap_or(0), digits.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace(pattern: &str, replacement: &str, subject: &str) -> String {
        PhpPattern::literal_or_regex(pattern)
            .unwrap()
            .replace_all(subject, &Replacement::parse(replacement))
            .unwrap()
    }

    #[test]
    fn file_patterns_with_and_without_delimiters() {
        let pattern = PhpPattern::regex("/tests/").unwrap();

        // First and last characters match, so this is delimited: body `tests`.
        assert!(pattern.is_match("vendor/pkg/tests/Foo.php"));
        assert!(pattern.is_match("tests"));

        let pattern = PhpPattern::regex("vendor/pkg/.*\\.md").unwrap();

        assert!(pattern.is_match("vendor/pkg/README.md"));
        assert!(!pattern.is_match("vendor/pkg/README.txt"));
    }

    #[test]
    fn delimited_file_pattern_is_used_as_written() {
        let pattern = PhpPattern::regex("~^vendor/pkg/(src|lib)/~").unwrap();

        assert!(pattern.is_match("vendor/pkg/src/Foo.php"));
        assert!(!pattern.is_match("other/vendor/pkg/src/Foo.php"));
    }

    #[test]
    fn supports_lookarounds() {
        let pattern = PhpPattern::regex("~^(?!.*/tests/).*\\.php$~").unwrap();

        assert!(pattern.is_match("vendor/pkg/src/Foo.php"));
        assert!(!pattern.is_match("vendor/pkg/tests/FooTest.php"));
    }

    #[test]
    fn alphanumeric_first_and_last_characters_are_not_delimiters() {
        // `a.*a` starts and ends with `a`, which PHP would reject as a
        // delimiter. It's a plain regex here.
        let pattern = PhpPattern::regex("a.*a").unwrap();

        assert!(pattern.is_match("banana"));
    }

    #[test]
    fn rejects_invalid_patterns() {
        let error = PhpPattern::regex("~(unclosed~").unwrap_err();

        assert!(error.contains("invalid pattern `~(unclosed~`"), "{error}");
    }

    #[test]
    fn undelimited_namespace_key_is_literal() {
        assert_eq!(
            replace("Acme\\Lib", "Prefixed\\Lib", "Acme\\Lib\\Thing"),
            "Prefixed\\Lib\\Thing"
        );

        // `.` must not act as a wildcard.
        assert_eq!(replace("A.me", "X", "AxmeA.me"), "AxmeX");
    }

    #[test]
    fn delimited_namespace_key_is_a_regex() {
        assert_eq!(
            replace(
                "~^Acme\\\\(.*)~",
                "Plugin\\\\Vendor\\\\$1",
                "Acme\\Lib\\Thing"
            ),
            "Plugin\\Vendor\\Lib\\Thing"
        );
    }

    #[test]
    fn replaces_every_match() {
        assert_eq!(replace("~a~", "b", "banana"), "bbnbnb");
    }

    #[test]
    fn replacement_syntax() {
        let cases = [
            ("$1", "x"),
            ("${1}1", "x1"),
            ("\\1", "x"),
            ("[$1$1]", "[xx]"),
            ("\\\\", "\\"),
            ("a\\b", "a\\b"),
            ("$$", "$$"),
            ("${x}", "${x}"),
            ("cost: $", "cost: $"),
            ("$9", ""),
            ("$0", "x"),
        ];

        for (replacement, expected) in cases {
            assert_eq!(
                replace("~(x)~", replacement, "x"),
                expected,
                "replacement {replacement:?}"
            );
        }
    }

    #[test]
    fn documented_readme_example() {
        // From Strauss' README: keeps the vendor name out of the namespace.
        assert_eq!(
            replace(
                "~BrianHenryIE\\\\(.*)~",
                "BrianHenryIE\\WC_Cash_App_Gateway\\\\$1",
                "BrianHenryIE\\Lib"
            ),
            "BrianHenryIE\\WC_Cash_App_Gateway\\Lib"
        );
    }

    #[test]
    fn single_character_pattern_is_not_delimited() {
        let pattern = PhpPattern::regex("/").unwrap();

        assert!(pattern.is_match("a/b"));
    }
}
