// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::cli::Algorithm;
use fxhash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

mod datafrog_opt;
mod location_insensitive;
mod naive;
use polonius_engine::{AllFacts, Atom};

#[derive(Clone, Debug)]
crate struct Output<Region: Atom, Loan: Atom, Point: Atom> {
    crate borrow_live_at: FxHashMap<Point, Vec<Loan>>,

    crate dump_enabled: bool,

    // these are just for debugging
    crate restricts: FxHashMap<Point, BTreeMap<Region, BTreeSet<Loan>>>,
    crate restricts_anywhere: FxHashMap<Region, BTreeSet<Loan>>,
    crate region_live_at: FxHashMap<Point, Vec<Region>>,
    crate invalidates: FxHashMap<Point, Vec<Loan>>,
    crate potential_errors: FxHashMap<Point, Vec<Loan>>,
    crate subset: FxHashMap<Point, BTreeMap<Region, BTreeSet<Region>>>,
    crate subset_anywhere: FxHashMap<Region, BTreeSet<Region>>,
}

impl<Region, Loan, Point> Output<Region, Loan, Point>
where
    Region: Atom,
    Loan: Atom,
    Point: Atom,
{
    crate fn compute(
        all_facts: &AllFacts<Region, Loan, Point>,
        algorithm: Algorithm,
        dump_enabled: bool,
    ) -> Self {
        match algorithm {
            Algorithm::Naive => naive::compute(dump_enabled, all_facts.clone()),
            Algorithm::DatafrogOpt => datafrog_opt::compute(dump_enabled, all_facts.clone()),
            Algorithm::LocationInsensitive => {
                location_insensitive::compute(dump_enabled, all_facts.clone())
            }
        }
    }

    fn new(dump_enabled: bool) -> Self {
        Output {
            borrow_live_at: FxHashMap::default(),
            restricts: FxHashMap::default(),
            restricts_anywhere: FxHashMap::default(),
            region_live_at: FxHashMap::default(),
            invalidates: FxHashMap::default(),
            potential_errors: FxHashMap::default(),
            subset: FxHashMap::default(),
            subset_anywhere: FxHashMap::default(),
            dump_enabled,
        }
    }

    crate fn borrows_in_scope_at(&self, location: Point) -> &[Loan] {
        match self.borrow_live_at.get(&location) {
            Some(p) => p,
            None => &[],
        }
    }

    crate fn restricts_at(&self, location: Point) -> Cow<'_, BTreeMap<Region, BTreeSet<Loan>>> {
        assert!(self.dump_enabled);
        match self.restricts.get(&location) {
            Some(map) => Cow::Borrowed(map),
            None => Cow::Owned(BTreeMap::default()),
        }
    }

    crate fn regions_live_at(&self, location: Point) -> &[Region] {
        assert!(self.dump_enabled);
        match self.region_live_at.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    crate fn subsets_at(&self, location: Point) -> Cow<'_, BTreeMap<Region, BTreeSet<Region>>> {
        assert!(self.dump_enabled);
        match self.subset.get(&location) {
            Some(v) => Cow::Borrowed(v),
            None => Cow::Owned(BTreeMap::default()),
        }
    }
}
