// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::facts::{AllFacts, Loan, Point, Region};
use crate::intern::InternerTables;
use fxhash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

mod dump;
mod timely;

#[derive(Debug)]
crate enum Algorithm {
    Naive,
}

#[derive(Clone, Debug)]
crate struct Output {
    borrow_live_at: FxHashMap<Point, Vec<Loan>>,

    dump_enabled: bool,

    // these are just for debugging
    restricts: FxHashMap<Point, BTreeMap<Region, BTreeSet<Loan>>>,
    region_live_at: FxHashMap<Point, Vec<Region>>,
    subset: FxHashMap<Point, BTreeMap<Region, BTreeSet<Region>>>,
}

impl Output {
    crate fn compute(all_facts: AllFacts, algorithm: Algorithm, dump_enabled: bool) -> Self {
        match algorithm {
            Algorithm::Naive => timely::timely_dataflow(dump_enabled, all_facts),
        }
    }

    fn new(dump_enabled: bool) -> Self {
        Output {
            borrow_live_at: FxHashMap::default(),
            restricts: FxHashMap::default(),
            region_live_at: FxHashMap::default(),
            subset: FxHashMap::default(),
            dump_enabled,
        }
    }

    crate fn dump(&self, intern: &InternerTables) {
        dump::dump_rows("borrow_live_at", intern, &self.borrow_live_at);

        if self.dump_enabled {
            dump::dump_rows("restricts", intern, &self.restricts);
            dump::dump_rows("region_live_at", intern, &self.region_live_at);
            dump::dump_rows("subset", intern, &self.subset);
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
