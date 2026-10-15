//! `export-ignore` in a package's `.gitattributes`.
//!
//! A package marks the files it leaves out of its distributed archive
//! (tests, CI config…) with `export-ignore`. This follows PHP Strauss'
//! `GitAttributes`, which understands just that attribute.

use std::fs;
use std::io;
use std::path::Path;

use globset::{Glob, GlobBuilder, GlobMatcher};

#[derive(Debug, Default)]
pub struct GitAttributes {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    matcher: Matcher,
    /// `export-ignore` sets the attribute; `-export-ignore`, `!export-ignore`
    /// and `export-ignore=value` all leave the file in the archive.
    ignored: bool,
}

#[derive(Debug)]
enum Matcher {
    /// A pattern with no slash matches any single path segment.
    Segment(GlobMatcher),
    /// A pattern with a slash matches the whole path, or a directory above
    /// the file.
    Path(GlobMatcher),
}

impl GitAttributes {
    /// Reads `<directory>/.gitattributes`. Absent means no attributes.
    pub fn load(directory: &Path) -> io::Result<Self> {
        match fs::read_to_string(directory.join(".gitattributes")) {
            Ok(contents) => Ok(Self::parse(&contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn parse(contents: &str) -> Self {
        let mut entries = Vec::new();

        for line in contents.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut tokens = line.split_whitespace();

            let Some(pattern) = tokens.next() else {
                continue;
            };

            // The last mention of the attribute on the line wins.
            let mut ignored = None;

            for token in tokens {
                let (name, set) = if let Some(name) = token.strip_prefix('-') {
                    (name, false)
                } else if let Some(name) = token.strip_prefix('!') {
                    (name, false)
                } else if let Some((name, _value)) = token.split_once('=') {
                    (name, false)
                } else {
                    (token, true)
                };

                if name == "export-ignore" {
                    ignored = Some(set);
                }
            }

            let (Some(ignored), Some(matcher)) = (ignored, Matcher::new(pattern)) else {
                continue;
            };

            entries.push(Entry { matcher, ignored });
        }

        Self { entries }
    }

    /// Whether the package leaves `relative_path` (using `/`) out of its
    /// archive. The last matching line decides.
    pub fn is_export_ignored(&self, relative_path: &str) -> bool {
        let relative_path = relative_path.trim_start_matches('/');
        let mut ignored = false;

        for entry in &self.entries {
            if entry.matcher.matches(relative_path) {
                ignored = entry.ignored;
            }
        }

        ignored
    }
}

impl Matcher {
    fn new(pattern: &str) -> Option<Self> {
        let pattern = pattern.trim_end_matches('/');
        let anchored = pattern.contains('/');
        let pattern = pattern.trim_start_matches('/');

        if pattern.is_empty() {
            return None;
        }

        if anchored {
            // `fnmatch(..., FNM_PATHNAME)`: `*` doesn't cross a slash.
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .ok()?;

            Some(Matcher::Path(glob.compile_matcher()))
        } else {
            Some(Matcher::Segment(Glob::new(pattern).ok()?.compile_matcher()))
        }
    }

    fn matches(&self, relative_path: &str) -> bool {
        match self {
            Matcher::Segment(glob) => relative_path
                .split('/')
                .any(|segment| glob.is_match(segment)),

            Matcher::Path(glob) => {
                if glob.is_match(relative_path) {
                    return true;
                }

                // Any directory above the file.
                let mut ancestor = String::new();
                let segments: Vec<&str> = relative_path.split('/').collect();

                for segment in &segments[..segments.len().saturating_sub(1)] {
                    if !ancestor.is_empty() {
                        ancestor.push('/');
                    }

                    ancestor.push_str(segment);

                    if glob.is_match(&ancestor) {
                        return true;
                    }
                }

                false
            }
        }
    }
}
