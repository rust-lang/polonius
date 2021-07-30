use std::fmt;

use logos::Span;

use crate::lexer::Token;

#[derive(Debug, Clone)]
pub enum ParseError {
    UnexpectedToken {
        found: Token,
        expected: Vec<Token>,
        position: Span,
    },
}

impl From<&ParseError> for String {
    fn from(error: &ParseError) -> Self {
        match error {
            ParseError::UnexpectedToken {
                found,
                expected,
                position,
            } => {
                format!(
                    "Unexpected token at {}-{}: found '{}', but expected {}",
                    position.start,
                    position.end,
                    found,
                    token_list_to_string(expected)
                )
            }
        }
    }
}

impl From<ParseError> for String {
    fn from(error: ParseError) -> Self {
        String::from(&error)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

fn token_list_to_string(tokens: &[Token]) -> String {
    let res: Vec<String> = tokens.iter().map(|token| format!("'{}'", token)).collect();
    let mut res = res.join(", ");
    if let Some(pos) = res.rfind(", ") {
        res.replace_range(0..0, "one of ");
        res.replace_range(pos..=pos + 1, " or ");
    }
    res
}
