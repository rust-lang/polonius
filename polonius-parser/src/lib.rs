pub mod ir;

#[rustfmt::skip]
mod parser;
mod tests;

pub fn parse_input(text: &str) -> Result<ir::Input, String> {
    parser::InputParser::new()
        .parse(text)
        .map_err(|e| format!("Polonius parse error: {:?}", e))
}
