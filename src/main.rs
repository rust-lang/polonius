extern crate failure;
extern crate polonius;
extern crate structopt;
extern crate env_logger;

use structopt::StructOpt;

pub fn main() -> Result<(), failure::Error> {
    env_logger::init();
    let opt = polonius::cli::Opt::from_args();
    polonius::cli::main(opt)
}
