#![feature(catch_expr)]
#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(extern_absolute_paths)]
#![feature(extern_prelude)]
#![feature(proc_macro)]
#![feature(in_band_lifetimes)]
#![feature(termination_trait_test)]

use structopt::StructOpt;

pub fn main() -> Result<(), failure::Error> {
    let opt = borrow_check::cli::Opt::from_args();
    borrow_check::cli::main(opt)
}
