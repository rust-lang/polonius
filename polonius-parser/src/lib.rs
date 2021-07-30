mod error;
pub mod ir;
mod lexer;
mod parser;
pub type Result<T> = std::result::Result<T, error::ParseError>;
mod tests;

pub fn parse_input(input: &str) -> Result<ir::Input> {
    let mut parser = parser::Parser::new(input, lexer::lex(input));
    parser.parse_input()
}
