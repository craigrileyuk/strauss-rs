//! Finding names in the text of strings and docblocks.
//!
//! Neither is parsed by PHP, so the names in them are found by reading the text.
//! A string is only changed where it spells a name unambiguously: with a
//! backslash in it (`'Acme\Core\Client'`, `'Acme\\Core\\'`). A docblock is only
//! read where a tag holds a type (`@param Foo $x`), not in prose.

use crate::edits::names::NameMap;

/// Replace `start..end` of the text that was scanned.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn starts_name(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

/// The names in a quoted string literal (`raw` includes the quotes) that have
/// to change.
pub(super) fn string_edits(names: &NameMap, raw: &str) -> Vec<TextEdit> {
    let bytes = raw.as_bytes();

    let (Some(&quote), Some(&last)) = (bytes.first(), bytes.last()) else {
        return Vec::new();
    };

    // Heredocs and nowdocs are left alone.
    if bytes.len() < 2 || !matches!(quote, b'\'' | b'"') || last != quote {
        return Vec::new();
    }

    let content = &bytes[1..bytes.len() - 1];
    let mut edits = Vec::new();
    let mut at = 0;
    // Just after an escape such as `\n`, a name can start again.
    let mut after_escape = false;

    while at < content.len() {
        // A backslash that is not a separator is an escape: skip what it
        // escapes, and a name can start again after it.
        if content[at] == b'\\' && separator(content, at, quote) == 0 {
            at += 2;
            after_escape = true;
            continue;
        }

        // The middle of a word or a variable.
        let inside = !after_escape
            && at > 0
            && (is_name_byte(content[at - 1]) || matches!(content[at - 1], b'$' | b'\\'));

        after_escape = false;

        match (inside, read_run(content, at, quote)) {
            (false, Some(run)) => {
                at = run.end;

                if let Some(edit) = run_edit(names, content, &run, quote) {
                    edits.push(TextEdit {
                        start: edit.start + 1,
                        end: edit.end + 1,
                        replacement: edit.replacement,
                    });
                }
            }

            _ => at += 1,
        }
    }

    edits
}

/// A name as written in a string: `\Acme\Core\Client`, `Acme\\Core\\`.
struct Run {
    start: usize,
    end: usize,

    /// The length of the separator before the first segment, 0 if none.
    leading: usize,

    /// The length of the separator after the last segment, 0 if none.
    trailing: usize,

    /// The length of the separator the name is written with: 1 for `\`, 2 for `\\`.
    unit: usize,

    /// Where each segment is, in the content.
    segments: Vec<(usize, usize)>,
}

/// The length of the namespace separator at `at`: one backslash, or two or
/// four where the string escapes them (`'A\\B'`, and `'/^A\\\\B/'` in a regular
/// expression). A single one is not a separator if it makes an escape: `\'` in
/// a single quoted string, `\n` and the like in a double quoted one.
fn separator(content: &[u8], at: usize, quote: u8) -> usize {
    let backslashes = content[at..]
        .iter()
        .take_while(|byte| **byte == b'\\')
        .count();

    match backslashes {
        2 | 4 => backslashes,
        1 => match content.get(at + 1) {
            Some(next) if quote == b'"' && starts_escape(*next) => 0,
            Some(b'\'') if quote == b'\'' => 0,
            Some(_) => 1,
            None => 0,
        },
        _ => 0,
    }
}

/// What can follow a backslash in a double quoted string to make an escape.
fn starts_escape(byte: u8) -> bool {
    matches!(
        byte,
        b'n' | b't' | b'r' | b'v' | b'e' | b'f' | b'x' | b'u' | b'0'..=b'7' | b'$' | b'"'
    )
}

fn read_run(content: &[u8], at: usize, quote: u8) -> Option<Run> {
    let leading = separator(content, at, quote);
    let mut position = at + leading;
    let mut unit = leading;
    let mut trailing = 0;
    let mut segments = Vec::new();

    loop {
        let start = position;

        if !content.get(position).is_some_and(|byte| starts_name(*byte)) {
            return None;
        }

        while content
            .get(position)
            .is_some_and(|byte| is_name_byte(*byte))
        {
            position += 1;
        }

        segments.push((start, position));

        let separator = separator(content, position, quote);

        if separator == 0 {
            break;
        }

        if unit == 0 {
            unit = separator;
        }

        position += separator;

        if !content.get(position).is_some_and(|byte| starts_name(*byte)) {
            trailing = separator;
            break;
        }
    }

    // A separator that ends a name is a namespace, but only if the string
    // ends there. Elsewhere it is more likely an escape in a regular expression
    // (`google\.com`).
    if trailing > 0 && position != content.len() {
        position -= trailing;
        trailing = 0;
    }

    Some(Run {
        start: at,
        end: position,
        leading,
        trailing,
        unit: unit.max(1),
        segments,
    })
}

fn run_edit(names: &NameMap, content: &[u8], run: &Run, quote: u8) -> Option<TextEdit> {
    let text = |(start, end): (usize, usize)| std::str::from_utf8(&content[start..end]).ok();

    let segments: Vec<&str> = run
        .segments
        .iter()
        .map(|segment| text(*segment))
        .collect::<Option<_>>()?;

    let name = segments.join("\\");

    let new = if run.trailing > 0 {
        // `Acme\\Core\\`, as in a PSR-4 map.
        names.new_namespace_text(&name)
    } else if segments.len() > 1 {
        names.new_text_name(&name)
    } else if run.leading > 0 && run.start == 0 && run.end == content.len() {
        // `'\GlobalClass'`. Anywhere else, `\d` and `\e` are as likely to be a
        // regular expression as a class.
        names.new_class_text(&name)
    } else {
        // A bare word is text.
        return None;
    };

    if new == name {
        return None;
    }

    // Written the way it was, unless that would make an escape.
    let new_segments: Vec<&str> = new.split('\\').collect();
    let mut unit = run.unit;

    if quote == b'"' && unit == 1 {
        let separated_first = run.leading > 0;
        let hazard = new_segments.iter().enumerate().any(|(index, segment)| {
            (index > 0 || separated_first) && starts_escape(segment.as_bytes()[0])
        });

        if hazard {
            unit = 2;
        }
    }

    let separator = "\\".repeat(unit);
    let leading = if run.leading > 0 {
        separator.clone()
    } else {
        String::new()
    };

    Some(TextEdit {
        start: run.start,
        end: run.segments.last()?.1,
        replacement: format!("{leading}{}", new_segments.join(&separator)),
    })
}

/// The tags whose text starts with a type, or a name.
const TYPE_TAGS: &[&str] = &[
    "param",
    "return",
    "var",
    "throws",
    "property",
    "property-read",
    "property-write",
    "method",
    "extends",
    "implements",
    "mixin",
    "use",
    "see",
    "covers",
    "uses",
];

/// Words in a type that are not class names.
const TYPE_WORDS: &[&str] = &[
    "int", "integer", "string", "bool", "boolean", "float", "double", "array", "object", "mixed",
    "null", "void", "never", "callable", "iterable", "resource", "true", "false", "self", "static",
    "parent", "list", "numeric", "scalar", "of", "is", "not",
];

/// Where the names are in the types of a docblock's tags, as ranges of `raw`.
pub(super) fn doc_type_names(raw: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut offset = 0;

    for line in raw.split_inclusive('\n') {
        let base = offset;
        offset += line.len();

        let trimmed = line.trim_start_matches([' ', '\t', '*', '/']);
        let tag_start = line.len() - trimmed.len();

        let Some(tag_text) = trimmed.strip_prefix('@') else {
            continue;
        };

        let tag_length = tag_text
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(tag_text.len());

        let tag = tag_text[..tag_length].to_ascii_lowercase();
        let tag = tag
            .strip_prefix("psalm-")
            .or_else(|| tag.strip_prefix("phpstan-"))
            .unwrap_or(&tag);

        if !TYPE_TAGS.contains(&tag) {
            continue;
        }

        let rest_start = tag_start + 1 + tag_length;
        let rest = &line[rest_start..];
        let mut expression_start = rest_start + (rest.len() - rest.trim_start().len());

        // `@method static Foo bar()`
        if tag == "method" {
            let word_end = line[expression_start..]
                .find(char::is_whitespace)
                .map_or(line.len(), |at| expression_start + at);

            if line[expression_start..word_end].eq_ignore_ascii_case("static") {
                expression_start =
                    word_end + (line[word_end..].len() - line[word_end..].trim_start().len());
            }
        }

        let expression = type_expression(&line[expression_start..], tag == "see");

        scan_type(expression, base + expression_start, &mut found);
    }

    found
}

/// The type at the start of `text`: up to the first space outside brackets.
fn type_expression(text: &str, single_word: bool) -> &str {
    let mut depth = 0i32;

    for (at, character) in text.char_indices() {
        match character {
            '<' | '(' | '{' | '[' if !single_word => depth += 1,
            '>' | ')' | '}' | ']' if !single_word => depth -= 1,
            '*' if text[at..].starts_with("*/") => return &text[..at],
            c if c.is_whitespace() && (depth <= 0 || single_word) => return &text[..at],
            _ => {}
        }
    }

    text
}

fn scan_type(expression: &str, base: usize, found: &mut Vec<(usize, usize)>) {
    let bytes = expression.as_bytes();
    let mut at = 0;

    while at < bytes.len() {
        let byte = bytes[at];

        if matches!(byte, b'\'' | b'"') {
            at = bytes[at + 1..]
                .iter()
                .position(|candidate| *candidate == byte)
                .map_or(bytes.len(), |end| at + 1 + end + 1);
            continue;
        }

        if !(starts_name(byte) || byte == b'\\') {
            at += 1;
            continue;
        }

        let start = at;

        while at < bytes.len() && (is_name_byte(bytes[at]) || matches!(bytes[at], b'\\' | b'-')) {
            at += 1;
        }

        let word = &expression[start..at];
        let before = start.checked_sub(1).map(|i| bytes[i]);
        let after = expression[at..].trim_start().as_bytes();

        // A variable, a class constant, a key in an array shape, or a keyword.
        let skip = matches!(before, Some(b'$' | b':'))
            || (after.first() == Some(&b':') && after.get(1) != Some(&b':'))
            || after.first() == Some(&b'?') && after.get(1) == Some(&b':')
            || TYPE_WORDS.contains(&word.to_ascii_lowercase().as_str())
            || word.trim_start_matches('\\').is_empty()
            || word.ends_with('\\');

        if !skip {
            found.push((base + start, base + at));
        }
    }
}
