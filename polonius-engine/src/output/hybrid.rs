// Copyright 2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A hybrid version combining the optimized Datafrog model with the
//! location-insensitive version.

use crate::output::datafrog_opt;
use crate::output::location_insensitive;
use crate::output::Output;
use facts::{AllFacts, Atom};

pub(super) fn compute<Region: Atom, Loan: Atom, Point: Atom, Variable: Atom, MovePath: Atom>(
    dump_enabled: bool,
    all_facts: AllFacts<Region, Loan, Point, Variable, MovePath>,
) -> Output<Region, Loan, Point, Variable, MovePath> {
    let lins_output = location_insensitive::compute(dump_enabled, &all_facts);
    if lins_output.errors.is_empty() {
        lins_output
    } else {
        datafrog_opt::compute(dump_enabled, all_facts)
    }
}
