//! The note added to the files that are changed.
//!
//! PHP Strauss adds a line saying the file was modified, to the license
//! comment at the top if there is one, or in a new comment if not. Licenses
//! often ask for exactly that.

/// What PHP Strauss puts after "Modified"; kept so that its notes and this
/// one look the same.
const CREDIT: &str = "using {@see https://github.com/evo-mark/strauss-rs}.";

pub struct Header<'a> {
    /// The package's licenses, for a comment made from scratch.
    pub licenses: &'a [String],
    pub author: Option<&'a str>,
    pub date: Option<&'a str>,
}

impl Header<'_> {
    fn note(&self) -> String {
        let mut note = String::from("Modified");

        if let Some(author) = self.author {
            note.push_str(&format!(" by {author}"));
        }

        if let Some(date) = self.date {
            note.push_str(&format!(" on {date}"));
        }

        note.push(' ');
        note.push_str(CREDIT);

        note
    }
}

/// `source` with the note added at the top. A file that isn't PHP code from
/// the first byte, or that already has the note, is returned as it is.
pub fn add_header(source: &str, header: &Header) -> String {
    let Some(tag) = source
        .get(..5)
        .filter(|tag| tag.eq_ignore_ascii_case("<?php"))
    else {
        return source.to_string();
    };

    let after_tag = tag.len();

    if source.contains(CREDIT) {
        return source.to_string();
    }

    let whitespace = &source[after_tag..];
    let whitespace_length = whitespace.len() - whitespace.trim_start().len();

    // `<?php` has to be followed by whitespace.
    if whitespace_length == 0 {
        return source.to_string();
    }

    let code_start = after_tag + whitespace_length;
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let note = header.note();

    if let Some(extended) = extend_comment(source, code_start, &note, newline) {
        return extended;
    }

    let mut comment = vec!["/**".to_string()];

    for license in header.licenses {
        comment.push(format!(" * @license {license}"));
    }

    if !header.licenses.is_empty() {
        comment.push(" *".to_string());
    }

    comment.push(format!(" * {note}"));
    comment.push(" */".to_string());

    format!(
        "{}{newline}{}{newline}{newline}{}",
        &source[..after_tag],
        comment.join(newline),
        &source[code_start..]
    )
}

/// Adds the note to the license comment that starts at `start`, if there is
/// one: a comment of several lines, followed by a blank line (so it isn't the
/// documentation of the class below it).
fn extend_comment(source: &str, start: usize, note: &str, newline: &str) -> Option<String> {
    let rest = &source[start..];

    if !rest.starts_with("/*") {
        return None;
    }

    let close = start + rest.find("*/")?;
    let body = &source[start..close];

    // The closing `*/` has to be on a line of its own.
    let last_line_start = body.rfind('\n')? + 1;
    let indent = &body[last_line_start..];

    if !indent.trim().is_empty() {
        return None;
    }

    // Followed by a blank line.
    let after = &source[close + 2..];
    let first_line_end = after.find('\n')?;

    if !after[..first_line_end].trim().is_empty()
        || !after[first_line_end + 1..]
            .lines()
            .next()?
            .trim()
            .is_empty()
    {
        return None;
    }

    let previous_line = body[..last_line_start].trim_end_matches(['\r', '\n']);
    let previous_line = previous_line.rsplit('\n').next().unwrap_or("");
    let previous_is_blank = previous_line
        .trim()
        .trim_start_matches('*')
        .trim()
        .is_empty();

    let mut added = String::new();

    if !previous_is_blank {
        added.push_str(&format!("{indent}*{newline}"));
    }

    added.push_str(&format!("{indent}* {note}{newline}"));

    Some(format!(
        "{}{added}{}",
        &source[..start + last_line_start],
        &source[start + last_line_start..]
    ))
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A date as PHP's `d-F-Y` writes it: `05-October-2026`.
pub fn format_date(unix_seconds: i64) -> String {
    // Days since 1970-01-01 to a calendar date, for the proleptic Gregorian
    // calendar.
    let z = unix_seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    format!("{day:02}-{}-{year}", MONTHS[(month - 1) as usize])
}

/// Today, in UTC.
pub fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);

    format_date(seconds)
}
