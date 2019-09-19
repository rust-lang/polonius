extern crate env_logger;

use polonius::cli;
use std::error::Error;

pub fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let options = cli::Options::from_args()?;
    cli::main(options)
}
