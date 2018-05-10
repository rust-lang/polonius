extern crate borrow_check;
extern crate failure;
extern crate structopt;

use structopt::StructOpt;

pub fn main() -> Result<(), failure::Error> {
    let opt = borrow_check::cli::Opt::from_args();
    borrow_check::cli::main(opt)
}
