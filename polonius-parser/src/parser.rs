//! Defines the [`Parser`].

use std::iter::Peekable;
use std::vec;

use crate::error::ParseError;
use crate::ir::*;
use crate::token::Span;
use crate::token::Token;
use crate::token::TokenKind;
use crate::Result;
use crate::T;

/// Input parser.
///
/// Construct with an iterator that produces [`Token`]s and choose any of the `parse_` methods as an entrypoint.
/// The main entrypoint for full programs is [`parse_input`](Parser::parse_input).
pub struct Parser<'input, I>
where
    I: Iterator,
{
    input: &'input str,
    lexer: Peekable<I>,
}

impl<'input, I> Parser<'input, I>
where
    I: Iterator,
{
    pub fn new(input: &'input str, lexer: I) -> Self {
        Self {
            input,
            lexer: lexer.peekable(),
        }
    }
}

impl<'input, I> Parser<'input, I>
where
    I: Iterator<Item = Token>,
{
    /// Returns the [`TokenKind`] of the next token, or `T![eof]` if at the end of input.
    pub(crate) fn peek(&mut self) -> TokenKind {
        self.lexer.peek().map(|token| token.kind).unwrap_or(T![eof])
    }

    /// Returns the string text of the next token, or an empty string if at the end of input.
    pub(crate) fn text(&mut self) -> &str {
        &self.input[self.position()]
    }

    /// Returns the [`Span`] of the next token, or an empty span at byte 0 if at the end of input.
    pub(crate) fn position(&mut self) -> Span {
        let peek = self.lexer.peek().map(|token| token.span);
        peek.unwrap_or_else(|| (0..0).into())
    }

    /// If the next token is of kind `expected`, advances past it and returns `true`.
    /// Otherwise, does not advance and returns `false`.
    pub(crate) fn try_consume(&mut self, expected: TokenKind) -> bool {
        if !self.at(expected) {
            return false;
        }
        self.bump();
        true
    }

    /// Tries to advance past a token of kind `expected`.
    /// If the next token is of a different kind, returns [`ParseError::UnexpectedToken`].
    pub(crate) fn consume(&mut self, expected: TokenKind) -> Result<TokenKind> {
        if self.try_consume(expected) {
            return Ok(expected);
        }
        Err(ParseError::UnexpectedToken {
            found: self.peek(),
            expected: vec![expected],
            position: self.position(),
        })
    }

    /// Returns `true` if the next token is of kind `kind`.
    pub(crate) fn at(&mut self, kind: TokenKind) -> bool {
        self.peek() == kind
    }

    /// Unconditionally advances the lexer by one token.
    pub(crate) fn bump(&mut self) {
        self.lexer.next();
    }
}

impl<'input, I> Parser<'input, I>
where
    I: Iterator<Item = Token>,
{
    pub fn parse_input(&mut self) -> Result<Input> {
        let placeholders = self.parse_placeholders()?;
        let known_subsets = self.parse_known_subsets().unwrap_or_default();
        let use_of_var_derefs_origin = self.parse_use_of_var_derefs_origin().unwrap_or_default();
        let drop_of_var_derefs_origin = self.parse_drop_of_var_derefs_origin().unwrap_or_default();
        let blocks = self.parse_blocks()?;
        Ok(Input::new(
            placeholders,
            known_subsets,
            use_of_var_derefs_origin,
            drop_of_var_derefs_origin,
            blocks,
        ))
    }

    pub fn parse_placeholders(&mut self) -> Result<Vec<String>> {
        self.consume(T![placeholders])?;
        self.consume(T!['{'])?;
        let origins = self.delimited(T![origin], T![,])?;
        self.consume(T!['}'])?;
        Ok(origins)
    }

    pub fn parse_known_subsets(&mut self) -> Result<Vec<KnownSubset>> {
        self.consume(T![known subsets])?;
        self.consume(T!['{'])?;
        let mut known_subsets = Vec::new();
        while self.at(T![origin]) {
            let a = self.text().to_string();
            self.consume(T![origin])?;
            self.consume(T![:])?;
            let b = self.text().to_string();
            self.consume(T![origin])?;
            known_subsets.push(KnownSubset { a, b });
            if !self.try_consume(T![,]) {
                break;
            }
        }
        self.consume(T!['}'])?;
        Ok(known_subsets)
    }

    pub fn parse_use_of_var_derefs_origin(&mut self) -> Result<Vec<(String, String)>> {
        self.consume(T![use of var derefs origin])?;
        self.consume(T!['{'])?;
        let var_region_mappings = self.parse_var_region_mappings()?;
        self.consume(T!['}'])?;
        Ok(var_region_mappings)
    }

    pub fn parse_drop_of_var_derefs_origin(&mut self) -> Result<Vec<(String, String)>> {
        self.consume(T![drop of var derefs origin])?;
        self.consume(T!['{'])?;
        let var_region_mappings = self.parse_var_region_mappings()?;
        self.consume(T!['}'])?;
        Ok(var_region_mappings)
    }

    pub fn parse_var_region_mappings(&mut self) -> Result<Vec<(String, String)>> {
        let mut var_region_mappings = Vec::new();
        while self.try_consume(T!['(']) {
            let variable = self.text().to_string();
            self.consume(T![variable])?;
            self.consume(T![,])?;
            let origin = self.text().to_string();
            self.consume(T![origin])?;
            self.consume(T![')'])?;
            var_region_mappings.push((variable, origin));
            if !self.try_consume(T![,]) {
                break;
            }
        }
        Ok(var_region_mappings)
    }

    pub fn parse_blocks(&mut self) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();
        while self.try_consume(T![block]) {
            let name = self.text().to_string();
            self.consume(T![Block])?;
            self.consume(T!['{'])?;
            let statements = self.parse_statements()?;
            let goto = self.parse_goto()?;
            self.consume(T!['}'])?;
            blocks.push(Block {
                name,
                statements,
                goto,
            });
        }
        Ok(blocks)
    }

    pub fn parse_statements(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();
        loop {
            if matches!(self.peek(), T![goto] | T!['}']) {
                // end of block
                return Ok(statements);
            }
            let effects_start = self.parse_effects()?;
            match self.peek() {
                T![;] => {
                    self.consume(T![;])?;
                    statements.push(Statement::new(effects_start));
                }
                T![/] => {
                    self.consume(T![/])?;
                    let effects = self.parse_effects()?;
                    self.consume(T![;])?;
                    statements.push(Statement {
                        effects_start,
                        effects,
                    });
                }
                found => {
                    return Err(ParseError::UnexpectedToken {
                        found,
                        expected: vec![T![;], T![/]],
                        position: self.position(),
                    })
                }
            }
        }
    }

    pub fn parse_effects(&mut self) -> Result<Vec<Effect>> {
        let mut effects = Vec::new();
        loop {
            match self.peek() {
                T![use] => effects.push(self.parse_use()?),
                _ => match self.parse_fact() {
                    Ok(fact) => effects.push(Effect::Fact(fact)),
                    _ => break, // not an error, just the end of the enumeration
                },
            }
            if !self.try_consume(T![,]) {
                break;
            }
        }
        Ok(effects)
    }

    pub fn parse_fact(&mut self) -> Result<Fact> {
        match self.peek() {
            T![outlives] => {
                self.consume(T![outlives])?;
                self.consume(T!['('])?;
                let a = self.text().to_string();
                self.consume(T![origin])?;
                self.consume(T![:])?;
                let b = self.text().to_string();
                self.consume(T![origin])?;
                self.consume(T![')'])?;
                Ok(Fact::Outlives { a, b })
            }
            T![loan issued at] => {
                self.consume(T![loan issued at])?;
                self.consume(T!['('])?;
                let origin = self.text().to_string();
                self.consume(T![origin])?;
                self.consume(T![,])?;
                let loan = self.text().to_string();
                self.consume(T![loan])?;
                self.consume(T![')'])?;
                Ok(Fact::LoanIssuedAt { origin, loan })
            }
            T![loan invalidated at] => {
                self.consume(T![loan invalidated at])?;
                self.consume(T!['('])?;
                let loan = self.text().to_string();
                self.consume(T![loan])?;
                self.consume(T![')'])?;
                Ok(Fact::LoanInvalidatedAt { loan })
            }
            T![loan killed at] => {
                self.consume(T![loan killed at])?;
                self.consume(T!['('])?;
                let loan = self.text().to_string();
                self.consume(T![loan])?;
                self.consume(T![')'])?;
                Ok(Fact::LoanKilledAt { loan })
            }
            T![var used at] => {
                self.consume(T![var used at])?;
                self.consume(T!['('])?;
                let variable = self.text().to_string();
                self.consume(T![variable])?;
                self.consume(T![')'])?;
                Ok(Fact::UseVariable { variable })
            }
            T![var defined at] => {
                self.consume(T![var defined at])?;
                self.consume(T!['('])?;
                let variable = self.text().to_string();
                self.consume(T![variable])?;
                self.consume(T![')'])?;
                Ok(Fact::DefineVariable { variable })
            }
            T![origin live on entry] => {
                self.consume(T![origin live on entry])?;
                self.consume(T!['('])?;
                let origin = self.text().to_string();
                self.consume(T![origin])?;
                self.consume(T![')'])?;
                Ok(Fact::OriginLiveOnEntry { origin })
            }
            T![var dropped at] => {
                self.consume(T![var dropped at])?;
                self.consume(T!['('])?;
                let variable = self.text().to_string();
                self.consume(T![variable])?;
                self.consume(T![')'])?;
                Ok(Fact::UseVariable { variable })
            }
            found => Err(ParseError::UnexpectedToken {
                found,
                expected: vec![
                    T![outlives],
                    T![loan issued at],
                    T![loan invalidated at],
                    T![loan killed at],
                    T![var used at],
                    T![var defined at],
                    T![origin live on entry],
                    T![var dropped at],
                ],
                position: self.position(),
            }),
        }
    }

    pub fn parse_use(&mut self) -> Result<Effect> {
        self.consume(T![use])?;
        self.consume(T!['('])?;
        let origins = self.delimited(T![origin], T![,])?;
        self.consume(T![')'])?;
        Ok(Effect::Use { origins })
    }

    pub fn parse_goto(&mut self) -> Result<Vec<String>> {
        if self.try_consume(T![goto]) {
            let gotos = self.delimited(T![Block], T![,])?;
            self.consume(T![;])?;
            Ok(gotos)
        } else {
            Ok(vec![])
        }
    }

    /// Parses a sequence of tokens of kind `kind` that are delimited by tokens of kind `delimiter`
    /// into a [`Vec`] of their string contents.
    pub(crate) fn delimited(
        &mut self,
        kind: TokenKind,
        delimiter: TokenKind,
    ) -> Result<Vec<String>> {
        let mut result = Vec::new();
        while self.at(kind) {
            result.push(self.text().to_string());
            self.consume(kind)?;
            if !self.try_consume(delimiter) {
                break;
            }
        }
        Ok(result)
    }
}
