//! Making the edits to a file's text.

use std::fmt;

use crate::edits::model::Edit;

#[derive(Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// The edit runs past the end of the text, or ends before it starts.
    OutOfRange { start: u32, end: u32, length: usize },
    /// The edit starts or ends inside a character.
    NotOnACharacter { offset: u32 },
    /// Two edits change the same text.
    Overlap {
        first: (u32, u32),
        second: (u32, u32),
    },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApplyError::OutOfRange { start, end, length } => {
                write!(
                    formatter,
                    "the edit {start}..{end} is outside the {length} bytes of the file"
                )
            }
            ApplyError::NotOnACharacter { offset } => {
                write!(
                    formatter,
                    "the edit at {offset} is in the middle of a character"
                )
            }
            ApplyError::Overlap { first, second } => write!(
                formatter,
                "the edits {}..{} and {}..{} change the same text",
                first.0, first.1, second.0, second.1
            ),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Makes every edit, which can be in any order, to `source`.
///
/// All of the edits refer to the original text, so none moves another. Edits
/// that overlap, or that don't fit the text, are an error and nothing is
/// changed.
pub fn apply_edits(source: &str, edits: &[Edit]) -> Result<String, ApplyError> {
    let mut ordered: Vec<&Edit> = edits.iter().collect();

    ordered.sort_by_key(|edit| (edit.start, edit.end));

    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    let mut previous: Option<&Edit> = None;

    for edit in ordered {
        let (start, end) = (edit.start as usize, edit.end as usize);

        if start > end || end > source.len() {
            return Err(ApplyError::OutOfRange {
                start: edit.start,
                end: edit.end,
                length: source.len(),
            });
        }

        for offset in [edit.start, edit.end] {
            if !source.is_char_boundary(offset as usize) {
                return Err(ApplyError::NotOnACharacter { offset });
            }
        }

        if let Some(previous) = previous.filter(|previous| (previous.end as usize) > start) {
            return Err(ApplyError::Overlap {
                first: (previous.start, previous.end),
                second: (edit.start, edit.end),
            });
        }

        output.push_str(&source[cursor..start]);
        output.push_str(&edit.replacement);

        cursor = end;
        previous = Some(edit);
    }

    output.push_str(&source[cursor..]);

    Ok(output)
}
