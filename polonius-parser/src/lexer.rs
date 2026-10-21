//! Defines the [`Lexer`].

use crate::{
    token::{Span, Token},
    T,
};

/// Input tokenizer.
///
/// The primary way to use the lexer is through its implementation of [`Iterator`], which produces
/// [`Token`]s lazily.
/// A single [end-of-file token](crate::token::TokenKind::Eof) will be created at the end of the input.
/// Erroneous inputs will result in [`T![error]`](crate::token::TokenKind::Error) tokens.
pub struct Lexer<'input> {
    input: &'input str,
    position: u32,
    eof: bool,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            input,
            position: 0,
            eof: false,
        }
    }

    fn next_token(&mut self, input: &str) -> Token {
        self.valid_token(input)
            .unwrap_or_else(|| self.invalid_token(input))
    }

    /// Returns `None` if the lexer cannot find a token at the start of `input`.
    fn valid_token(&mut self, input: &str) -> Option<Token> {
        let (len, kind) = match input.as_bytes() {
            [c, ..] if (*c as char).is_whitespace() => (
                input
                    .char_indices()
                    .take_while(|(_, c)| c.is_whitespace())
                    .last()
                    .unwrap() // we know there is at least one whitespace character
                    .0 as u32
                    + 1,
                T![ws],
            ),
            [b',', ..] => (1, T![,]),
            [b':', ..] => (1, T![:]),
            [b';', ..] => (1, T![;]),
            [b'/', next, ..] if *next != b'/' => (1, T![/]), // clash with comments
            [b'(', ..] => (1, T!['(']),
            [b')', ..] => (1, T![')']),
            [b'{', ..] => (1, T!['{']),
            [b'}', ..] => (1, T!['}']),
            // parameters
            [c @ b'\'' | c @ b'B' | c @ b'L' | c @ b'V', ..] => (
                input
                    .char_indices()
                    .skip(1)
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
                    .last()
                    .map(|(pos, _c)| pos)
                    .unwrap_or(0) as u32
                    + 1,
                match c {
                    b'\'' => T![origin],
                    b'B' => T![Block],
                    b'L' => T![loan],
                    b'V' => T![variable],
                    _ => unreachable!(),
                },
            ),
            [b'/', b'/', ..] => (
                input
                    .char_indices()
                    .take_while(|(_, c)| *c != '\n')
                    .last()
                    .map(|(pos, _c)| pos)
                    .unwrap_or(input.len()) as u32
                    + 1,
                T![comment],
            ),
            // relation keywords
            kw if kw.starts_with(b"use_of_var_derefs_origin") => (
                "use_of_var_derefs_origin".len() as u32,
                T![use_of_var_derefs_origin],
            ),
            kw if kw.starts_with(b"drop_of_var_derefs_origin") => (
                "drop_of_var_derefs_origin".len() as u32,
                T![drop_of_var_derefs_origin],
            ),
            kw if kw.starts_with(b"placeholders") => {
                ("placeholders".len() as u32, T![placeholders])
            }
            kw if kw.starts_with(b"known_subsets") => {
                ("known_subsets".len() as u32, T![known subsets])
            }
            // CFG keywords
            kw if kw.starts_with(b"block") => ("block".len() as u32, T![block]),
            kw if kw.starts_with(b"goto") => ("goto".len() as u32, T![goto]),
            // effect keywords - facts
            kw if kw.starts_with(b"outlives") => ("outlives".len() as u32, T![outlives]),
            kw if kw.starts_with(b"loan_issued_at") => {
                ("loan_issued_at".len() as u32, T![loan_issued_at])
            }
            kw if kw.starts_with(b"loan_invalidated_at") => {
                ("loan_invalidated_at".len() as u32, T![loan_invalidated_at])
            }
            kw if kw.starts_with(b"loan_killed_at") => {
                ("loan_killed_at".len() as u32, T![loan_killed_at])
            }
            kw if kw.starts_with(b"var_used_at") => ("var_used_at".len() as u32, T![var_used_at]),
            kw if kw.starts_with(b"var_defined_at") => {
                ("var_defined_at".len() as u32, T![var_defined_at])
            }
            kw if kw.starts_with(b"origin_live_on_entry") => (
                "origin_live_on_entry".len() as u32,
                T![origin_live_on_entry],
            ),
            kw if kw.starts_with(b"var_dropped_at") => {
                ("var_dropped_at".len() as u32, T![var_dropped_at])
            }
            // effect keywords - use
            kw if kw.starts_with(b"use") => ("use".len() as u32, T![use]),
            _ => return None,
        };

        let start = self.position;
        self.position += len;
        Some(Token {
            kind,
            span: Span {
                start,
                end: start + len,
            },
        })
    }

    /// Always "succeeds", because it creates an error `Token`.
    fn invalid_token(&mut self, input: &str) -> Token {
        let start = self.position;
        let len = input
            .char_indices()
            .find(|(pos, _)| self.valid_token(&input[*pos..]).is_some())
            .map(|(pos, _)| pos)
            .unwrap_or_else(|| input.len());
        debug_assert!(len <= input.len());

        // Because `valid_token` advances our position,
        // we need to reset it to after the errornous token.
        let len = len as u32;
        self.position = start + len;
        Token {
            kind: T![error],
            span: Span {
                start,
                end: start + len,
            },
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position as usize >= self.input.len() {
            if self.eof {
                return None;
            }
            self.eof = true;
            Some(Token {
                kind: T![eof],
                span: Span {
                    start: self.position,
                    end: self.position,
                },
            })
        } else {
            Some(self.next_token(&self.input[self.position as usize..]))
        }
    }
}
