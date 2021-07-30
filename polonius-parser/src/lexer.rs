use std::{fmt, ops::Deref};

use logos::{Logos, Span};

#[derive(Logos, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u16)]
pub enum Token {
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,
    #[token("/")]
    Slash,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LCurly,
    #[token("}")]
    RCurly,
    // relation keywords
    #[token("use_of_var_derefs_origin")]
    KwUseOfVarDerefsOrigin,
    #[token("drop_of_var_derefs_origin")]
    KwDropOfVarDerefsOrigin,
    #[token("placeholders")]
    KwPlaceholders,
    #[token("known_subsets")]
    KwKnownSubsets,
    // CFG keywords
    #[token("block")]
    KwBlock,
    #[token("goto")]
    KwGoto,
    // effect keywords - facts
    #[token("outlives")]
    KwOutlives,
    #[token("loan_issued_at")]
    KwLoanIssuedAt,
    #[token("loan_invalidated_at")]
    KwLoanInvalidatedAt,
    #[token("loan_killed_at")]
    KwLoanKilledAt,
    #[token("var_used_at")]
    KwVarUsedAt,
    #[token("var_defined_at")]
    KwVarDefinedAt,
    #[token("origin_live_on_entry")]
    KwOriginLiveOnEntry,
    #[token("var_dropped_at")]
    KwVarDroppedAt,
    // effect keywords - use
    #[token("use")]
    KwUse,
    // parameters
    #[regex(r"'\w+")]
    Origin,
    #[regex(r"B\w+")]
    Block,
    #[regex(r"L\w+")]
    Loan,
    #[regex(r"V\w+")]
    Variable,
    #[regex(r"//.*", logos::skip)]
    Comment,
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Whitespace,
    #[error]
    Error,
    Eof,
}

#[macro_export]
macro_rules! T {
    [,] => { $crate::lexer::Token::Comma};
    [:] => { $crate::lexer::Token::Colon};
    [;] => { $crate::lexer::Token::Semi};
    [/] => { $crate::lexer::Token::Slash};
    ['('] => { $crate::lexer::Token::LParen};
    [')'] => { $crate::lexer::Token::RParen};
    ['{'] => { $crate::lexer::Token::LCurly};
    ['}'] => { $crate::lexer::Token::RCurly};
    // relation keywords
    [use of var derefs origin] => { $crate::lexer::Token::KwUseOfVarDerefsOrigin};
    [drop of var derefs origin] => { $crate::lexer::Token::KwDropOfVarDerefsOrigin};
    [placeholders] => { $crate::lexer::Token::KwPlaceholders};
    [known subsets] => { $crate::lexer::Token::KwKnownSubsets};
    // CFG keywords
    [block] => { $crate::lexer::Token::KwBlock};
    [goto] => { $crate::lexer::Token::KwGoto};
    // effect keywords - facts
    [outlives] => { $crate::lexer::Token::KwOutlives};
    [loan issued at] => { $crate::lexer::Token::KwLoanIssuedAt};
    [loan invalidated at] => { $crate::lexer::Token::KwLoanInvalidatedAt};
    [loan killed at] => { $crate::lexer::Token::KwLoanKilledAt};
    [var used at] => { $crate::lexer::Token::KwVarUsedAt};
    [var defined at] => { $crate::lexer::Token::KwVarDefinedAt};
    [origin live on entry] => { $crate::lexer::Token::KwOriginLiveOnEntry};
    [var dropped at] => { $crate::lexer::Token::KwVarDroppedAt};
    // effect keywords - use
    [use] => { $crate::lexer::Token::KwUse};
    // parameters
    [origin] => { $crate::lexer::Token::Origin};
    [Block] => { $crate::lexer::Token::Block};
    [loan] => { $crate::lexer::Token::Loan};
    [variable] => { $crate::lexer::Token::Variable};
    [comment] => { $crate::lexer::Token::Comment};
    [ws] => { $crate::lexer::Token::Whitespace};
    [error] => { $crate::lexer::Token::Error};
    [eof] => { $crate::lexer::Token::Eof};
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub t: T,
    pub span: Span,
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.t
    }
}

pub fn lex(input: &str) -> impl Iterator<Item = Spanned<Token>> + '_ {
    Token::lexer(input)
        .spanned()
        .map(|(t, span)| Spanned { t, span })
}

impl fmt::Display for Token {
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
