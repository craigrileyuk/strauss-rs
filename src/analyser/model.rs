//! The result of analysing a file.

use php_ast::Span;

#[derive(Debug)]
pub struct Reference {
    pub name: String,
    /// Global name PHP falls back to at runtime when `name` isn't defined.
    /// Only set for unqualified function and constant references inside
    /// a namespace with no matching import.
    pub fallback: Option<String>,
    pub span: Span,
    pub kind: ReferenceKind,
}

#[derive(Debug)]
pub enum ReferenceKind {
    ClassName,
    FunctionName,
    ConstantName,
    ClassConstantName,
}

#[derive(Debug)]
pub struct FileAnalysis {
    pub namespaces: Vec<NamespaceAnalysis>,
}

#[derive(Debug)]
pub struct NamespaceAnalysis {
    pub namespace: Option<String>,

    /// Where the name is written in `namespace Foo;`. `None` for the global
    /// namespace, which is not written.
    pub name_span: Option<Span>,

    /// Byte range of the source this namespace covers. A comment belongs to
    /// the namespace that is current where it appears, so a docblock above
    /// `namespace Foo;` belongs to the preceding (global) segment.
    pub span: Span,
    pub imports: Vec<Import>,

    /// The `use A\{B, C}` statements, which are edited as a whole.
    pub group_uses: Vec<GroupUse>,
    pub symbols: Vec<DeclaredSymbol>,
    pub references: Vec<Reference>,
    /// Strings and comments, in source order. Which of them name a symbol
    /// that needs rewriting is decided later, once the symbols are known.
    pub regions: Vec<CodeRegion>,
}

/// A span of source text that may mention a symbol without being code the
/// parser resolves: a string literal, a comment, or a string argument to a
/// function such as `define()` or `function_exists()`.
#[derive(Debug)]
pub struct CodeRegion {
    pub kind: RegionKind,
    pub span: Span,
    /// The raw source text of `span`, escapes and all (`'Foo\\Bar'`).
    pub text: String,
    /// The unescaped value (`Foo\Bar`) for string arguments.
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// Any string literal, including quotes: single, double, heredoc,
    /// nowdoc and backtick.
    String,
    /// `//`, `#` and `/* */` comments.
    Comment,
    /// `/** */` comments.
    DocComment,
    /// The argument of a function that takes a function name, e.g.
    /// `function_exists('foo')` or the callback of `add_action()`. Excludes
    /// the quotes.
    CallableStringArg,
    /// First argument of `define()` or `defined()`. Excludes the quotes.
    DefineStringArg,
}

#[derive(Debug)]
pub struct Import {
    /// The full name, without a leading backslash. For an item in a group
    /// `use`, the prefix is included.
    pub name: String,
    pub alias: Option<String>,
    pub kind: ImportKind,

    /// Where the name is written, without a leading backslash. For an item in
    /// a group `use`, only the part inside the braces. `None` if it could not
    /// be found.
    pub span: Option<Span>,

    /// Where the alias after `as` is written.
    pub alias_span: Option<Span>,

    /// The `use` statement it is in, if it is a group `use`.
    pub group: Option<usize>,
}

/// `use A\B\{C, D as E};`
#[derive(Debug)]
pub struct GroupUse {
    /// The whole statement, from `use` to `;`.
    pub span: Span,

    /// The `A\B`.
    pub prefix_span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum ImportKind {
    Class,
    Function,
    Constant,
}

#[derive(Debug)]
pub struct DeclaredSymbol {
    /// The full name, without a leading backslash.
    pub name: String,
    pub kind: SymbolKind,

    /// Where the name is written in the declaration (`Foo` in `class Foo`).
    /// `None` for a constant made with `define()`, whose name is a string, and
    /// if it could not be found.
    pub name_span: Option<Span>,

    /// `abstract class`.
    pub is_abstract: bool,

    /// What a class extends (at most one name) or an interface extends (any
    /// number), as full names.
    pub extends: Vec<String>,

    /// What a class or enum implements, as full names.
    pub implements: Vec<String>,

    /// The type of a backed enum: `string` or `int`.
    pub backing_type: Option<String>,
}

impl DeclaredSymbol {
    pub fn new(name: String, kind: SymbolKind) -> Self {
        Self {
            name,
            kind,
            name_span: None,
            is_abstract: false,
            extends: Vec::new(),
            implements: Vec::new(),
            backing_type: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Constant,
}
