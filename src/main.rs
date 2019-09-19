extern crate env_logger;
extern crate polonius;
extern crate structopt;

use structopt::StructOpt;
use std::error::Error;

pub fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let opt = polonius::cli::Opt::from_args();
    polonius::cli::main(opt)
}
