#![feature(catch_expr)]
#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(proc_macro)]
#![feature(in_band_lifetimes)]
#![feature(termination_trait_test)]
#![allow(dead_code)]

extern crate datafrog;
extern crate failure;
extern crate fxhash;
extern crate histo;
extern crate structopt;

#[macro_use]
extern crate clap;

extern crate polonius_parser;

mod facts;
mod intern;
mod output;
mod program;
mod tab_delim;
mod test;

pub mod cli;
