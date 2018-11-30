#![feature(crate_visibility_modifier)]
#![feature(in_band_lifetimes)]
#![feature(try_blocks)]
#![allow(dead_code)]

extern crate clap;
extern crate datafrog;
extern crate failure;
extern crate histo;
extern crate polonius_engine;
extern crate polonius_parser;
extern crate rustc_hash;
extern crate structopt;

mod dump;
mod facts;
mod intern;
mod program;
mod tab_delim;
mod test;
mod test_util;

pub mod cli;
