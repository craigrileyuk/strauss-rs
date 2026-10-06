//! Where names are written in the source, found from the token stream.
//!
//! The parser gives a position for most names, but not for the name of a
//! declaration (`class Foo`), and the position it gives for an item in a group
//! `use A\{B, C}` runs from the start of the statement. The tokens give both
//! exactly.

use php_ast::Span;
use php_lexer::{Token, TokenKind};

/// The file's tokens, with the source they came from.
pub(super) struct SourceTokens<'a> {
    source: &'a str,
    tokens: &'a [Token],
}

/// Where the parts of a `use` statement are written.
#[derive(Debug)]
pub(super) struct UseSpans {
    /// The `A\B` of `use A\B\{C, D};`, without the backslash before the brace.
    pub group_prefix: Option<Span>,

    /// One for each name imported, in order.
    pub items: Vec<ItemSpans>,
}

#[derive(Debug)]
pub(super) struct ItemSpans {
    /// The name as written, without a leading backslash. In a group it is just
    /// the part inside the braces (`D\E`).
    pub name: Span,

    /// The alias after `as`.
    pub alias: Option<Span>,
}

impl<'a> SourceTokens<'a> {
    pub fn new(source: &'a str, tokens: &'a [Token]) -> Self {
        Self { source, tokens }
    }

    fn text(&self, token: &Token) -> &'a str {
        &self.source[token.span.start as usize..token.span.end as usize]
    }

    /// The tokens that start within `span`.
    fn within(&self, span: Span) -> &'a [Token] {
        let start = self
            .tokens
            .partition_point(|token| token.span.start < span.start);
        let end = self
            .tokens
            .partition_point(|token| token.span.start < span.end);

        &self.tokens[start..end]
    }

    /// The position of the name in the declaration `statement`, which starts
    /// with `keyword` (`class`, `function`…) somewhere inside it.
    ///
    /// The name is matched by its text, not its kind: `class Enum` and
    /// `function match()` have names that lex as keywords.
    pub fn declaration_name(
        &self,
        statement: Span,
        keyword: TokenKind,
        name: &str,
    ) -> Option<Span> {
        let tokens = self.within(statement);

        for (index, token) in tokens.iter().enumerate() {
            // `Foo::class` is not a declaration.
            if token.kind != keyword
                || (index > 0 && tokens[index - 1].kind == TokenKind::DoubleColon)
            {
                continue;
            }

            let mut next = index + 1;

            // `function &foo()`
            if tokens
                .get(next)
                .is_some_and(|token| token.kind == TokenKind::Ampersand)
            {
                next += 1;
            }

            if let Some(candidate) = tokens.get(next) {
                if self.text(candidate) == name {
                    return Some(candidate.span);
                }
            }
        }

        None
    }

    /// The span of `name` if it is written at `start`.
    pub fn name_at(&self, start: u32, name: &str) -> Option<Span> {
        let end = start as usize + name.len();

        (self.source.get(start as usize..end)? == name).then(|| Span::new(start, end as u32))
    }

    /// The positions of the names in a `use` statement.
    pub fn use_statement(&self, statement: Span) -> Option<UseSpans> {
        let tokens = self.within(statement);

        if tokens.first()?.kind != TokenKind::Use {
            return None;
        }

        let mut index = 1;

        // `use function …` and `use const …`.
        if tokens
            .get(index)
            .is_some_and(|token| matches!(token.kind, TokenKind::Function | TokenKind::Const))
        {
            index += 1;
        }

        let (first, after_first) = self.read_name(tokens, index)?;
        index = after_first;

        // `use A\B\{…}`: the name just read is the prefix.
        if tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Backslash)
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::LeftBrace)
        {
            let mut items = Vec::new();
            index += 2;

            loop {
                // `function` or `const` for this item alone.
                if tokens.get(index).is_some_and(|token| {
                    matches!(token.kind, TokenKind::Function | TokenKind::Const)
                }) {
                    index += 1;
                }

                let (name, after_name) = self.read_name(tokens, index)?;
                let (alias, after_item) = self.read_alias(tokens, after_name);

                items.push(ItemSpans { name, alias });
                index = after_item;

                match tokens.get(index)?.kind {
                    TokenKind::Comma => {
                        index += 1;

                        // A trailing comma.
                        if tokens
                            .get(index)
                            .is_some_and(|token| token.kind == TokenKind::RightBrace)
                        {
                            break;
                        }
                    }
                    TokenKind::RightBrace => break,
                    _ => return None,
                }
            }

            return Some(UseSpans {
                group_prefix: Some(first),
                items,
            });
        }

        // `use A\B, C\D as E;`
        let mut items = Vec::new();
        let mut name = first;

        loop {
            let (alias, after_item) = self.read_alias(tokens, index);

            items.push(ItemSpans { name, alias });
            index = after_item;

            match tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Comma) => {
                    let (next, after_next) = self.read_name(tokens, index + 1)?;

                    name = next;
                    index = after_next;
                }
                _ => break,
            }
        }

        Some(UseSpans {
            group_prefix: None,
            items,
        })
    }

    /// Reads `A\B\C`, with or without a leading backslash, returning the span
    /// from `A` to `C` and the index after it. It stops before a backslash
    /// that is followed by `{`.
    fn read_name(&self, tokens: &[Token], mut index: usize) -> Option<(Span, usize)> {
        if tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Backslash)
        {
            index += 1;
        }

        let first = tokens.get(index)?;

        if !self.is_name_part(first) {
            return None;
        }

        let mut end = first.span.end;
        index += 1;

        while tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Backslash)
        {
            let Some(part) = tokens
                .get(index + 1)
                .filter(|token| self.is_name_part(token))
            else {
                break;
            };

            end = part.span.end;
            index += 2;
        }

        Some((Span::new(first.span.start, end), index))
    }

    fn read_alias(&self, tokens: &[Token], index: usize) -> (Option<Span>, usize) {
        match (tokens.get(index), tokens.get(index + 1)) {
            (Some(keyword), Some(alias))
                if keyword.kind == TokenKind::As && self.is_name_part(alias) =>
            {
                (Some(alias.span), index + 2)
            }
            _ => (None, index),
        }
    }

    /// A word that can be part of a name, including a soft keyword.
    fn is_name_part(&self, token: &Token) -> bool {
        let text = self.text(token);

        !text.is_empty()
            && text
                .chars()
                .all(|c| c == '_' || c.is_alphanumeric() || !c.is_ascii())
            && !text.starts_with(|c: char| c.is_ascii_digit())
    }
}
