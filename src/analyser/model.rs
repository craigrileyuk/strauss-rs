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
    /// Byte range of the source this namespace covers. A comment belongs to
    /// the namespace that is current where it appears, so a docblock above
    /// `namespace Foo;` belongs to the preceding (global) segment.
    pub span: Span,
    pub imports: Vec<Import>,
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
    /// First argument of a function that takes a function name, e.g.
    /// `function_exists('foo')`. Excludes the quotes.
    CallableStringArg,
    /// First argument of `define()` or `defined()`. Excludes the quotes.
    DefineStringArg,
}

#[derive(Debug)]
pub struct Import {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, Copy)]
pub enum ImportKind {
    Class,
    Function,
    Constant,
}

#[derive(Debug)]
pub struct DeclaredSymbol {
    pub name: String,
    pub kind: SymbolKind,
}

#[derive(Debug)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Constant,
}
