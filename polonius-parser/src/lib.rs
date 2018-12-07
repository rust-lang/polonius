#[macro_use]
extern crate lalrpop_util;

pub mod ir;

lalrpop_mod!(#[rustfmt::skip] parser); // synthetized by LALRPOP
mod tests;

pub fn parse_input(text: &str) -> Result<ir::Input, String> {
    parser::InputParser::new()
        .parse(text)
        .map_err(|e| format!("Polonius parse error: {:?}", e))
}
