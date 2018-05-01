#![feature(catch_expr)]
#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(dyn_trait)]
#![feature(in_band_lifetimes)]
#![feature(match_default_bindings)]
#![feature(termination_trait_test)]

#![allow(dead_code)]

extern crate abomonation;
#[macro_use]
extern crate abomonation_derive;
extern crate differential_dataflow;
extern crate timely;

mod facts;
mod intern;
mod tab_delim;
