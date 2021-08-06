//! Defines the output of the [`Lexer`](crate::lexer::Lexer).

use std::{
    fmt,
    ops::{Index, Range},
};

/// [`Token`]s produced by the lexer.
///
/// The primary information inside each token is its [`kind`](TokenKind), which stores which
/// syntactical element the token represents.
/// Instead of storing a token's source string, which would involve either lifetimes or allocation,
/// we store its position (in bytes) inside the source. The input string can be indexed with this
/// [`span`](Span) to obtain the token text.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Represents what input was lexed into a [`Token`].
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u16)]
pub enum TokenKind {
    Comma,
    Colon,
    Semi,
    Slash,
    LParen,
    RParen,
    LCurly,
    RCurly,
    // relation keywords
    KwUseOfVarDerefsOrigin,
    KwDropOfVarDerefsOrigin,
    KwPlaceholders,
    KwKnownSubsets,
    // CFG keywords
    KwBlock,
    KwGoto,
    // effect keywords - facts
    KwOutlives,
    KwLoanIssuedAt,
    KwLoanInvalidatedAt,
    KwLoanKilledAt,
    KwVarUsedAt,
    KwVarDefinedAt,
    KwOriginLiveOnEntry,
    KwVarDroppedAt,
    // effect keywords - use
    KwUse,
    // parameters
    Origin,
    Block,
    Loan,
    Variable,
    Comment,
    Whitespace,
    Error,
    Eof,
}

/// A source range in bytes.
///
/// A [`Span`] is essentially a [`Range<u32>`] that is [`Copy`].
/// Spans implement [`Index`], so they can be used directly to index the source string.
///
/// Can be converted [`Into`] and [`From`] a [`Range<usize>`].
#[derive(Eq, PartialEq, Clone, Copy, Hash, Default, Debug)]
pub struct Span {
    /// inclusive
    pub start: u32,
    /// exclusive
    pub end: u32,
}

impl Index<Span> for str {
    type Output = str;

    fn index(&self, index: Span) -> &Self::Output {
        &self[Range::<usize>::from(index)]
    }
}

/// Returns the [`TokenKind`] of a character, or that of keywords and parameters by a short name.
///
/// This is mostly a convenience to avoid typing and reading `TokenKind::Comma` and
/// `TokenKind::KwDropOfVarDerefsOrigin` everywhere, and instead be able to write `T![,]` and
/// `T![drop_of_var_derefs_origin]`.
#[macro_export]
macro_rules! T {
    [,] => { $crate::token::TokenKind::Comma};
    [:] => { $crate::token::TokenKind::Colon};
    [;] => { $crate::token::TokenKind::Semi};
    [/] => { $crate::token::TokenKind::Slash};
    ['('] => { $crate::token::TokenKind::LParen};
    [')'] => { $crate::token::TokenKind::RParen};
    ['{'] => { $crate::token::TokenKind::LCurly};
    ['}'] => { $crate::token::TokenKind::RCurly};
    // relation keywords
    [use of var derefs origin] => { $crate::token::TokenKind::KwUseOfVarDerefsOrigin};
    [drop of var derefs origin] => { $crate::token::TokenKind::KwDropOfVarDerefsOrigin};
    [placeholders] => { $crate::token::TokenKind::KwPlaceholders};
    [known subsets] => { $crate::token::TokenKind::KwKnownSubsets};
    // CFG keywords
    [block] => { $crate::token::TokenKind::KwBlock};
    [goto] => { $crate::token::TokenKind::KwGoto};
    // effect keywords - facts
    [outlives] => { $crate::token::TokenKind::KwOutlives};
    [loan issued at] => { $crate::token::TokenKind::KwLoanIssuedAt};
    [loan invalidated at] => { $crate::token::TokenKind::KwLoanInvalidatedAt};
    [loan killed at] => { $crate::token::TokenKind::KwLoanKilledAt};
    [var used at] => { $crate::token::TokenKind::KwVarUsedAt};
    [var defined at] => { $crate::token::TokenKind::KwVarDefinedAt};
    [origin live on entry] => { $crate::token::TokenKind::KwOriginLiveOnEntry};
    [var dropped at] => { $crate::token::TokenKind::KwVarDroppedAt};
    // effect keywords - use
    [use] => { $crate::token::TokenKind::KwUse};
    // parameters
    [origin] => { $crate::token::TokenKind::Origin};
    [Block] => { $crate::token::TokenKind::Block};
    [loan] => { $crate::token::TokenKind::Loan};
    [variable] => { $crate::token::TokenKind::Variable};
    [comment] => { $crate::token::TokenKind::Comment};
    [ws] => { $crate::token::TokenKind::Whitespace};
    [error] => { $crate::token::TokenKind::Error};
    [eof] => { $crate::token::TokenKind::Eof};
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} - <{}, {}>",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            T![,] => write!(f, ","),
            T![:] => write!(f, ":"),
            T![;] => write!(f, ";"),
            T![/] => write!(f, "/"),
            T!['('] => write!(f, "("),
            T![')'] => write!(f, ")"),
            T!['{'] => write!(f, "{{"),
            T!['}'] => write!(f, "}}"),
            T![use of var derefs origin] => write!(f, "use_of_var_derefs_origin"),
            T![drop of var derefs origin] => write!(f, "drop_of_var_derefs_origin"),
            T![placeholders] => write!(f, "placeholders"),
            T![known subsets] => write!(f, "known_subsets"),
            T![block] => write!(f, "block"),
            T![goto] => write!(f, "goto"),
            T![outlives] => write!(f, "outlives"),
            T![loan issued at] => write!(f, "loan_issued_at"),
            T![loan invalidated at] => write!(f, "loan_invalidated_at"),
            T![loan killed at] => write!(f, "loan_killed_at"),
            T![var used at] => write!(f, "var_used_at"),
            T![var defined at] => write!(f, "var_defined_at"),
            T![origin live on entry] => write!(f, "origin_live_on_entry"),
            T![var dropped at] => write!(f, "var_dropped_at"),
            T![use] => write!(f, "use"),
            T![origin] => write!(f, "Origin"),
            T![Block] => write!(f, "Block"),
            T![loan] => write!(f, "Loan"),
            T![variable] => write!(f, "Variable"),
            T![comment] => write!(f, "// Comment"),
            T![ws] => write!(f, "<ws>"),
            T![error] => write!(f, "<?>"),
            T![eof] => write!(f, "EOF"),
        }
    }
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.start as usize..span.end as usize
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start as u32,
            end: range.end as u32,
        }
    }
}
