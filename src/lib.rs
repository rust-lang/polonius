#![feature(catch_expr)]
#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(extern_absolute_paths)]
#![feature(extern_prelude)]
#![feature(proc_macro)]
#![feature(in_band_lifetimes)]
#![feature(termination_trait_test)]

#![allow(dead_code)]

mod facts;
mod intern;
mod output;
mod tab_delim;
mod test;

pub mod cli;
