extern crate polonius;
extern crate failure;
extern crate structopt;

use structopt::StructOpt;

pub fn main() -> Result<(), failure::Error> {
    let opt = polonius::cli::Opt::from_args();
    polonius::cli::main(opt)
}
