mod error;
pub mod ir;
mod lexer;
mod parser;
mod token;
pub type Result<T> = std::result::Result<T, error::ParseError>;
mod tests;

pub fn parse_input(input: &str) -> Result<ir::Input> {
    let mut parser = parser::Parser::new(
        input,
        lexer::Lexer::new(input)
            .into_iter()
            .filter(|token| !matches!(token.kind, T![ws] | T![comment])),
    );
    parser.parse_input()
}
