//! Working out the changes to one file.
//!
//! A renamed namespace is rewritten where it is declared and imported, and
//! most names in code then follow it without being touched: a name relative to
//! the file's own namespace, or to an import, still means the same thing. So
//! each name in code is checked against what it *will* mean in the rewritten
//! file, and changed only if that isn't what it meant before (allowing for
//! the renaming).

use php_ast::Span;
use php_ast::ast::UseKind;
use php_ast::resolve::{NameContext, NameResolver, ResolvedName};

use crate::analyser::{
    FileAnalysis, GroupUse, Import, ImportKind, NamespaceAnalysis, Reference, ReferenceKind,
    RegionKind, resolve_written_name,
};
use crate::edits::context::{NewContext, NewImport};
use crate::edits::model::{Edit, EditKind};
use crate::edits::names::{Group, NameMap};
use crate::edits::text::{comment_names, doc_type_names, string_edits};
use crate::symbols::SymbolKind;

pub(super) struct FileInput<'a> {
    pub file: &'a str,
    pub source: &'a str,
    pub analysis: &'a FileAnalysis,

    /// The namespaces the file declares are left as they are: its package isn't
    /// being prefixed, or the file is excluded from it. What it refers to is
    /// still updated.
    pub keeps_namespaces: bool,
}

#[derive(Debug, Default)]
pub(super) struct Planned {
    pub edits: Vec<Edit>,
    pub warnings: Vec<String>,
}

pub(super) fn plan_file(names: &NameMap, input: &FileInput) -> Planned {
    let mut planned = Planned::default();

    for segment in &input.analysis.namespaces {
        Segment {
            names,
            input,
            segment,
            planned: &mut planned,
        }
        .plan();
    }

    tidy(&mut planned, input.file);

    planned
}

/// Orders the edits, and drops any that repeat or clash with an earlier one.
pub(super) fn tidy(planned: &mut Planned, file: &str) {
    planned.edits.sort_by_key(|edit| (edit.start, edit.end));

    let mut kept: Vec<Edit> = Vec::with_capacity(planned.edits.len());

    for edit in std::mem::take(&mut planned.edits) {
        match kept.last() {
            Some(previous)
                if previous.start == edit.start
                    && previous.end == edit.end
                    && previous.replacement == edit.replacement => {}
            Some(previous) if edit.start < previous.end => planned.warnings.push(format!(
                "{file}: the changes at {}..{} and {}..{} clash; only the first is made",
                previous.start, previous.end, edit.start, edit.end
            )),
            _ => kept.push(edit),
        }
    }

    planned.edits = kept;
}

struct Segment<'a, 'p> {
    names: &'a NameMap,
    input: &'a FileInput<'a>,
    segment: &'a NamespaceAnalysis,
    planned: &'p mut Planned,
}

impl Segment<'_, '_> {
    fn plan(&mut self) {
        // The namespace this part of the file will be in.
        let new_namespace = match &self.segment.namespace {
            Some(namespace) if !self.input.keeps_namespaces => {
                Some(self.names.namespace(namespace))
            }
            other => other.clone(),
        };

        self.plan_namespace(new_namespace.as_deref());

        let imports: Vec<NewImport> = self
            .segment
            .imports
            .iter()
            .map(|import| self.new_import(import))
            .collect();

        self.plan_imports(&imports);
        self.plan_group_uses(&imports);

        let context = NewContext::new(self.names, new_namespace.as_deref(), &imports);

        self.plan_declarations();
        self.plan_strings();
        self.plan_quoted_strings();
        self.warn_of_dynamic_names();
        self.plan_references(&context);
        self.plan_docblocks(&context);
        self.plan_comments();
    }

    fn warn(&mut self, message: String) {
        self.planned
            .warnings
            .push(format!("{}: {message}", self.input.file));
    }

    fn push(&mut self, kind: EditKind, span: Span, replacement: String) {
        self.planned.edits.push(Edit {
            start: span.start,
            end: span.end,
            replacement,
            kind,
        });
    }

    fn text(&self, span: Span) -> Option<&str> {
        self.input
            .source
            .get(span.start as usize..span.end as usize)
    }

    // ---- namespace ----

    fn plan_namespace(&mut self, new_namespace: Option<&str>) {
        let (Some(old), Some(new)) = (&self.segment.namespace, new_namespace) else {
            return;
        };

        if old == new {
            return;
        }

        match self.segment.name_span {
            Some(span) => self.push(EditKind::Namespace, span, new.to_string()),
            None => self.warn(format!(
                "could not find where `namespace {old};` is written"
            )),
        }
    }

    // ---- use ----

    fn new_import(&self, import: &Import) -> NewImport {
        let group = import_group(import.kind);
        let name = self.names.new_import_name(group, &import.name);

        // `use A\B;` makes `B` mean it. If the end of the name changes, the code
        // that says `B` would stop working, so it is imported `as B`.
        let (old_last, new_last) = (last_segment(&import.name), last_segment(&name));

        let added_alias =
            (import.alias.is_none() && old_last != new_last).then(|| old_last.to_string());

        NewImport {
            kind: import.kind,
            alias: import.alias.clone().or_else(|| added_alias.clone()),
            added_alias,
            name,
        }
    }

    fn plan_imports(&mut self, imports: &[NewImport]) {
        for (import, new) in self.segment.imports.iter().zip(imports) {
            if import.group.is_some() || is_unchanged(import, new) {
                continue;
            }

            let Some(span) = import.span else {
                self.warn(format!(
                    "could not find where `use {};` is written",
                    import.name
                ));
                continue;
            };

            let mut replacement = new.name.clone();

            if let Some(alias) = &new.added_alias {
                replacement.push_str(&format!(" as {alias}"));
            }

            self.push(EditKind::Import, span, replacement);
        }
    }

    fn plan_group_uses(&mut self, imports: &[NewImport]) {
        for (index, group) in self.segment.group_uses.iter().enumerate() {
            let members: Vec<usize> = (0..self.segment.imports.len())
                .filter(|member| self.segment.imports[*member].group == Some(index))
                .collect();

            if members
                .iter()
                .all(|member| is_unchanged(&self.segment.imports[*member], &imports[*member]))
            {
                continue;
            }

            if members
                .iter()
                .any(|member| self.segment.imports[*member].span.is_none())
            {
                self.warn(
                    "could not find where the items of a group `use` are written".to_string(),
                );
                continue;
            }

            match self.new_group_prefix(group, &members, imports) {
                Some(prefix) => {
                    if self.text(group.prefix_span) != Some(prefix.as_str()) {
                        self.push(EditKind::GroupUse, group.prefix_span, prefix);
                    }
                }
                None => {
                    let replacement = self.expanded_group(group, &members, imports);

                    self.push(EditKind::GroupUse, group.span, replacement);
                }
            }
        }
    }

    /// The new prefix of `use A\{B, C};`, if every item moves to the same one.
    fn new_group_prefix(
        &self,
        group: &GroupUse,
        members: &[usize],
        imports: &[NewImport],
    ) -> Option<String> {
        let _ = group;
        let mut prefix: Option<String> = None;

        for member in members {
            let item = self.text(self.segment.imports[*member].span?)?;
            let new = &imports[*member];

            if new.added_alias.is_some() {
                return None;
            }

            let member_prefix = new.name.strip_suffix(&format!("\\{item}"))?;

            match &prefix {
                None => prefix = Some(member_prefix.to_string()),
                Some(existing) if existing == member_prefix => {}
                Some(_) => return None,
            }
        }

        prefix
    }

    /// `use A\{B, C};` as one `use` statement for each item. That is the only way to
    /// send items to different places.
    fn expanded_group(&self, group: &GroupUse, members: &[usize], imports: &[NewImport]) -> String {
        let line_start = self.input.source[..group.span.start as usize]
            .rfind('\n')
            .map_or(0, |at| at + 1);
        let before = &self.input.source[line_start..group.span.start as usize];
        let indent = if before.trim().is_empty() { before } else { "" };

        members
            .iter()
            .map(|member| {
                let import = &self.segment.imports[*member];
                let new = &imports[*member];

                let keyword = match import.kind {
                    ImportKind::Class => "",
                    ImportKind::Function => "function ",
                    ImportKind::Constant => "const ",
                };

                let alias = match &new.alias {
                    Some(alias) => format!(" as {alias}"),
                    None => String::new(),
                };

                format!("use {keyword}{}{alias};", new.name)
            })
            .collect::<Vec<_>>()
            .join(&format!("\n{indent}"))
    }

    // ---- declarations ----

    /// A global symbol that is renamed is renamed where it is declared. A symbol
    /// in a namespace keeps its name: the namespace is what changes.
    fn plan_declarations(&mut self) {
        if self.input.keeps_namespaces {
            return;
        }

        for symbol in &self.segment.symbols {
            let Some(group) = Group::of(symbol_kind(symbol.kind)) else {
                continue;
            };

            if symbol.name.contains('\\') {
                continue;
            }

            let Some(replacement) = self.names.replacement(group, &symbol.name) else {
                continue;
            };

            match symbol.name_span {
                Some(span) => {
                    let replacement = replacement.to_string();

                    self.push(EditKind::Declaration, span, replacement);
                }

                // A constant made with `define()` is changed through its string.
                None if symbol.kind == crate::analyser::SymbolKind::Constant => {
                    let defined = self.segment.regions.iter().any(|region| {
                        region.kind == RegionKind::DefineStringArg
                            && region.value.as_deref() == Some(&symbol.name)
                    });

                    if !defined {
                        let message = format!(
                            "could not find where the constant {} is defined",
                            symbol.name
                        );

                        self.warn(message);
                    }
                }

                None => {
                    let message = format!("could not find where {} is declared", symbol.name);

                    self.warn(message);
                }
            }
        }
    }

    // ---- strings that name a symbol ----

    /// `define('FOO', …)`, `defined('FOO')` and `function_exists('foo')`: a
    /// string that is a symbol's name, wherever the call is.
    fn plan_strings(&mut self) {
        for region in &self.segment.regions {
            let (group, kind) = match region.kind {
                RegionKind::DefineStringArg => (Group::Constant, EditKind::DefineString),
                RegionKind::CallableStringArg => (Group::Function, EditKind::CallableString),
                _ => continue,
            };

            let Some(value) = &region.value else {
                continue;
            };

            let target = value.trim_start_matches('\\');

            // A function passed as a callback is a string for
            // `exclude_from_string_rename` to leave alone.
            let replacement = match region.kind {
                RegionKind::CallableStringArg => self.names.string_replacement(group, target),
                _ => self.names.replacement(group, target),
            };

            let Some(replacement) = replacement else {
                continue;
            };

            let leading = "\\".repeat(value.len() - target.len());
            let new = format!("{leading}{replacement}");

            self.planned.edits.push(Edit {
                start: region.span.start,
                end: region.span.end,
                replacement: written_like(&region.text, &new),
                kind,
            });
        }
    }

    /// Any other quoted string: names with a backslash in them, and a global
    /// function that is the whole string.
    fn plan_quoted_strings(&mut self) {
        // The arguments of `define()` and the like are handled as such.
        let handled: Vec<Span> = self
            .segment
            .regions
            .iter()
            .filter(|region| {
                matches!(
                    region.kind,
                    RegionKind::DefineStringArg | RegionKind::CallableStringArg
                )
            })
            .map(|region| region.span)
            .collect();

        for region in &self.segment.regions {
            if region.kind != RegionKind::String
                || handled
                    .iter()
                    .any(|span| span.start >= region.span.start && span.end <= region.span.end)
            {
                continue;
            }

            for edit in string_edits(self.names, &region.text) {
                self.planned.edits.push(Edit {
                    start: region.span.start + edit.start as u32,
                    end: region.span.start + edit.end as u32,
                    replacement: edit.replacement,
                    kind: EditKind::String,
                });
            }
        }
    }

    /// A name put together while the code runs can't be found, so a string that
    /// starts one is reported when it begins a renamed namespace: `'Acme\\' . $name`
    /// or `"Acme\\$name"` where only `Acme\Core` is renamed.
    fn warn_of_dynamic_names(&mut self) {
        let mut found = Vec::new();

        for region in &self.segment.regions {
            if region.kind != RegionKind::String {
                continue;
            }

            let Some(fragment) = dynamic_fragment(self.input.source, region.span, &region.text)
            else {
                continue;
            };

            let Some(namespace) = self.names.renamed_namespace_starting_with(&fragment) else {
                continue;
            };

            let line = self.input.source[..region.span.start as usize]
                .matches('\n')
                .count()
                + 1;

            found.push(format!(
                "line {line}: a name is built at run time from `{fragment}`, the start of renamed namespaces such as `{namespace}`; it is not rewritten, so write the prefixed namespace in by hand"
            ));
        }

        for message in found {
            self.warn(message);
        }
    }

    // ---- docblocks ----

    /// The classes named in the types of `@param`, `@return` and the like,
    /// which are read as they are in code.
    fn plan_docblocks(&mut self, context: &NewContext) {
        let mut resolver = NameResolver::new();

        if let Some(namespace) = &self.segment.namespace {
            resolver.enter_namespace(Some(namespace));
        }

        for import in &self.segment.imports {
            let kind = match import.kind {
                ImportKind::Class => UseKind::Normal,
                ImportKind::Function => UseKind::Function,
                ImportKind::Constant => UseKind::Const,
            };

            resolver.add_import(kind, &import.name, import.alias.as_deref());
        }

        for region in &self.segment.regions {
            if region.kind != RegionKind::DocComment {
                continue;
            }

            for (start, end) in doc_type_names(&region.text) {
                let span = Span::new(
                    region.span.start + start as u32,
                    region.span.start + end as u32,
                );

                let ResolvedName::Fqn(name) =
                    resolve_written_name(&resolver, NameContext::Class, &region.text[start..end])
                else {
                    continue;
                };

                let reference = Reference {
                    name,
                    fallback: None,
                    span,
                    kind: ReferenceKind::ClassName,
                };

                if let Some(mut edit) = self.reference_edit(context, &reference) {
                    edit.kind = EditKind::Docblock;
                    self.planned.edits.push(edit);
                }
            }
        }
    }

    // ---- comments ----

    /// Names with a backslash in them in the text of comments and docblocks:
    /// prose (`@deprecated Use \Acme\Core\Client instead`), examples, and the
    /// lines of a tag's description. The types of tags are planned as code
    /// (`plan_docblocks`), so a name that edit was made for is left to it. One
    /// it found nothing to change in, such as `@see Acme\Core\Client` which is
    /// relative to the namespace as written but meant in full, is read here.
    fn plan_comments(&mut self) {
        let planned: Vec<(u32, u32)> = self
            .planned
            .edits
            .iter()
            .map(|edit| (edit.start, edit.end))
            .collect();

        for region in &self.segment.regions {
            if !matches!(region.kind, RegionKind::Comment | RegionKind::DocComment) {
                continue;
            }

            for (start, end) in comment_names(&region.text) {
                let span_start = region.span.start + start as u32;
                let span_end = region.span.start + end as u32;

                if planned.iter().any(|(planned_start, planned_end)| {
                    span_start < *planned_end && *planned_start < span_end
                }) {
                    continue;
                }

                let written = &region.text[start..end];
                let name = written.trim_start_matches('\\');
                let new = self.names.new_text_name(name);

                if new == name {
                    continue;
                }

                let leading = &written[..written.len() - name.len()];

                self.planned.edits.push(Edit {
                    start: region.span.start + start as u32,
                    end: region.span.start + end as u32,
                    replacement: format!("{leading}{new}"),
                    kind: EditKind::Comment,
                });
            }
        }
    }

    // ---- references in code ----

    fn plan_references(&mut self, context: &NewContext) {
        for reference in &self.segment.references {
            if let Some(edit) = self.reference_edit(context, reference) {
                self.planned.edits.push(edit);
            }
        }
    }

    fn reference_edit(&self, context: &NewContext, reference: &Reference) -> Option<Edit> {
        let group = match reference.kind {
            ReferenceKind::ClassName => Group::ClassLike,
            ReferenceKind::FunctionName => Group::Function,
            ReferenceKind::ConstantName => Group::Constant,
            // A class constant isn't renamed; its class is a reference of its own.
            ReferenceKind::ClassConstantName => return None,
        };

        let written = self.text(reference.span)?;

        // What the name meant before. An unqualified function or constant in a
        // namespace meant the one in the namespace if there is one, else the
        // global one; which of the two it was is whichever was found.
        let target = match &reference.fallback {
            Some(_) if self.names.known(group, &reference.name) => reference.name.clone(),
            Some(global) if self.names.known(group, global) => global.clone(),
            // Neither was found: a function or constant PHP provides.
            Some(_) => return None,
            None => reference.name.clone(),
        };

        let new_target = self.names.new_name(group, &target);
        let style = Style::of(written);

        // A fully qualified name that isn't changing is already right.
        if style == Style::FullyQualified && new_target == target {
            return None;
        }

        let chosen = candidates(written, style, &new_target)
            .into_iter()
            .find(|candidate| context.means(group, candidate, &new_target))
            .unwrap_or_else(|| format!("\\{new_target}"));

        (chosen != written).then(|| Edit {
            start: reference.span.start,
            end: reference.span.end,
            replacement: chosen,
            kind: EditKind::Reference,
        })
    }
}

fn is_unchanged(import: &Import, new: &NewImport) -> bool {
    new.name == import.name && new.added_alias.is_none()
}

fn import_group(kind: ImportKind) -> Group {
    match kind {
        ImportKind::Class => Group::ClassLike,
        ImportKind::Function => Group::Function,
        ImportKind::Constant => Group::Constant,
    }
}

/// The analyser and the symbol table each have a kind of symbol.
fn symbol_kind(kind: crate::analyser::SymbolKind) -> SymbolKind {
    use crate::analyser::SymbolKind as Declared;

    match kind {
        Declared::Class => SymbolKind::Class,
        Declared::Interface => SymbolKind::Interface,
        Declared::Trait => SymbolKind::Trait,
        Declared::Enum => SymbolKind::Enum,
        Declared::Function => SymbolKind::Function,
        Declared::Constant => SymbolKind::Constant,
    }
}

fn last_segment(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// How a name is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// `\A\B`
    FullyQualified,
    /// `namespace\A`
    Relative,
    /// `A\B`
    Qualified,
    /// `A`
    Unqualified,
}

impl Style {
    fn of(written: &str) -> Style {
        if written.starts_with('\\') {
            Style::FullyQualified
        } else if written.len() > 10 && written[..10].eq_ignore_ascii_case("namespace\\") {
            Style::Relative
        } else if written.contains('\\') {
            Style::Qualified
        } else {
            Style::Unqualified
        }
    }
}

/// The ways to write the new name, from the least to change to the most. The
/// last of them always works.
fn candidates(written: &str, style: Style, new_target: &str) -> Vec<String> {
    let fully_qualified = format!("\\{new_target}");

    match style {
        Style::FullyQualified => vec![fully_qualified],
        Style::Relative => vec![written.to_string(), fully_qualified],
        Style::Qualified | Style::Unqualified => {
            let mut candidates = vec![written.to_string()];

            // In the global namespace, or where PHP falls back to it.
            if !new_target.contains('\\') {
                candidates.push(new_target.to_string());
            }

            candidates.push(fully_qualified);
            candidates
        }
    }
}

/// `name` written as `raw` is: with `\\` for each backslash if `raw` does that.
fn written_like(raw: &str, name: &str) -> String {
    if raw.contains("\\\\") {
        name.replace('\\', "\\\\")
    } else {
        name.to_string()
    }
}

/// The namespace-like text a string ends with when something dynamic follows
/// it: a variable inside the string, or a `.` after it. `Acme\` for
/// `'Acme\\' . $name`. `None` if nothing dynamic follows or the string doesn't
/// end like a namespace.
fn dynamic_fragment(source: &str, span: Span, raw: &str) -> Option<String> {
    let quote = *raw.as_bytes().first()?;

    if !matches!(quote, b'\'' | b'"') || raw.len() < 2 || !raw.ends_with(quote as char) {
        return None;
    }

    let content = &raw[1..raw.len() - 1];

    let before = if quote == b'"' {
        match interpolation_start(content) {
            Some(at) => &content[..at],
            None if concatenated(source, span) => content,
            None => return None,
        }
    } else if concatenated(source, span) {
        content
    } else {
        return None;
    };

    let start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\\' || !c.is_ascii()))
        .map_or(0, |at| {
            at + before[at..].chars().next().map_or(1, char::len_utf8)
        });

    let fragment = before[start..].replace("\\\\", "\\");

    fragment.ends_with('\\').then_some(fragment)
}

/// Where a variable is written inside a double quoted string.
fn interpolation_start(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut at = 0;

    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'{' if bytes.get(at + 1) == Some(&b'$') => return Some(at),
            b'$' if bytes
                .get(at + 1)
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_' || *b == b'{') =>
            {
                return Some(at);
            }
            _ => at += 1,
        }
    }

    None
}

/// Whether the string is joined to something with `.`.
fn concatenated(source: &str, span: Span) -> bool {
    let rest = source[span.end as usize..].trim_start().as_bytes();

    rest.first() == Some(&b'.') && !matches!(rest.get(1), Some(b'=' | b'.') | Some(b'0'..=b'9'))
}
