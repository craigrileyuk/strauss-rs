//! The changes to make to each file.

use std::fmt;
use std::path::PathBuf;

use serde::{Serialize, Serializer};

/// Replace the text from `start` to `end` (byte offsets into the file) with
/// `replacement`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub replacement: String,
    pub kind: EditKind,
}

/// What an edit changes, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    /// The name in `namespace Foo;`.
    Namespace,
    /// The name in a `use` statement.
    Import,
    /// The prefix of, or all of, a group `use A\{B, C};`.
    GroupUse,
    /// The name in `class Foo`, `function foo()` or `const FOO`.
    Declaration,
    /// A use of a class, function or constant in code.
    Reference,
    /// The name in `define('FOO', …)` or `defined('FOO')`.
    DefineString,
    /// The name in `function_exists('foo')` and the like.
    CallableString,
    /// A name with a backslash in it, inside a string.
    String,
    /// A class named in the type of a docblock tag.
    Docblock,
}

/// The edits for one file, ascending by position and not overlapping.
#[derive(Debug, Serialize)]
pub struct FileEdits {
    pub package: String,

    /// `league/flysystem/src/Client.php`.
    pub file: String,

    pub source_path: PathBuf,

    pub edits: Vec<Edit>,
}

/// Why a file is not edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The file isn't copied, so it isn't written.
    NotCopied,
    /// `exclude_files_from_update` covers it.
    ExcludedFromUpdate(String),
    /// It could not be parsed, so what to change is not known.
    NotAnalysed,
    /// It isn't valid UTF-8, so positions in it can't be trusted.
    NotUtf8,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SkipReason::NotCopied => formatter.write_str("the file is not copied"),
            SkipReason::ExcludedFromUpdate(by) => {
                write!(
                    formatter,
                    "it is excluded by exclude_files_from_update {by}"
                )
            }
            SkipReason::NotAnalysed => formatter.write_str("it could not be analysed"),
            SkipReason::NotUtf8 => formatter.write_str("it is not valid UTF-8"),
        }
    }
}

impl Serialize for SkipReason {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[derive(Debug, Serialize)]
pub struct SkippedFile {
    pub file: String,
    pub reason: SkipReason,
}

/// Something that stopped part of a file being planned. The rest of the file
/// is still planned, so it may be left half changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditWarning {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct EditPlan {
    /// Files with something to change.
    pub files: Vec<FileEdits>,

    /// How many files were planned and need no change.
    pub unchanged_files: usize,

    /// Files that are not edited, and why.
    pub skipped: Vec<SkippedFile>,

    pub warnings: Vec<EditWarning>,
}

impl EditPlan {
    pub fn edit_count(&self) -> usize {
        self.files.iter().map(|file| file.edits.len()).sum()
    }

    pub fn get(&self, file: &str) -> Option<&FileEdits> {
        self.files.iter().find(|candidate| candidate.file == file)
    }
}
