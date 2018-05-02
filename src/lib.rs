#![feature(catch_expr)]
#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(proc_macro)]
#![feature(in_band_lifetimes)]
#![feature(termination_trait_test)]

#![allow(dead_code)]

extern crate abomonation;
extern crate abomonation_derive;
extern crate differential_dataflow;
extern crate failure;
extern crate fxhash;
extern crate timely;
extern crate structopt;

#[macro_use]
extern crate clap;

mod facts;
mod intern;
mod output;
mod tab_delim;
mod test;

pub mod cli;
