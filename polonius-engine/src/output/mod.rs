// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

mod datafrog_opt;
mod hybrid;
mod liveness;
mod location_insensitive;
mod naive;
use facts::{AllFacts, Atom};

#[derive(Debug, Clone, Copy)]
pub enum Algorithm {
    Naive,
    DatafrogOpt,
    LocationInsensitive,
    /// Compare Naive and DatafrogOpt.
    Compare,
    Hybrid,
}

impl Algorithm {
    /// Optimized variants that ought to be equivalent to "naive"
    pub const OPTIMIZED: &'static [Algorithm] = &[Algorithm::DatafrogOpt];

    pub fn variants() -> [&'static str; 5] {
        [
            "Naive",
            "DatafrogOpt",
            "LocationInsensitive",
            "Compare",
            "Hybrid",
        ]
    }
}

impl ::std::str::FromStr for Algorithm {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "naive" => Ok(Algorithm::Naive),
            "datafrogopt" => Ok(Algorithm::DatafrogOpt),
            "locationinsensitive" => Ok(Algorithm::LocationInsensitive),
            "compare" => Ok(Algorithm::Compare),
            "hybrid" => Ok(Algorithm::Hybrid),
            _ => Err(String::from(
                "valid values: Naive, DatafrogOpt, LocationInsensitive, Compare, Hybrid",
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Output<Region: Atom, Loan: Atom, Point: Atom, Variable: Atom> {
    pub errors: FxHashMap<Point, Vec<Loan>>,

    pub dump_enabled: bool,

    // these are just for debugging
    pub borrow_live_at: FxHashMap<Point, Vec<Loan>>,
    pub restricts: FxHashMap<Point, BTreeMap<Region, BTreeSet<Loan>>>,
    pub restricts_anywhere: FxHashMap<Region, BTreeSet<Loan>>,
    pub region_live_at: FxHashMap<Point, Vec<Region>>,
    pub invalidates: FxHashMap<Point, Vec<Loan>>,
    pub subset: FxHashMap<Point, BTreeMap<Region, BTreeSet<Region>>>,
    pub subset_anywhere: FxHashMap<Region, BTreeSet<Region>>,
    pub var_live_at: FxHashMap<Point, Vec<Variable>>,
    pub var_drop_live_at: FxHashMap<Point, Vec<Variable>>,
}

/// Compares errors reported by Naive implementation with the errors
/// reported by the optimized implementation.
fn compare_errors<Loan: Atom, Point: Atom>(
    all_naive_errors: &FxHashMap<Point, Vec<Loan>>,
    all_opt_errors: &FxHashMap<Point, Vec<Loan>>,
) -> bool {
    let points = all_naive_errors.keys().chain(all_opt_errors.keys());

    let mut differ = false;
    for point in points {
        let mut naive_errors = all_naive_errors.get(&point).cloned().unwrap_or(Vec::new());
        naive_errors.sort();
        let mut opt_errors = all_opt_errors.get(&point).cloned().unwrap_or(Vec::new());
        opt_errors.sort();
        for err in naive_errors.iter() {
            if !opt_errors.contains(err) {
                error!(
                    "Error {0:?} at {1:?} reported by naive, but not opt.",
                    err, point
                );
                differ = true;
            }
        }
        for err in opt_errors.iter() {
            if !naive_errors.contains(err) {
                error!(
                    "Error {0:?} at {1:?} reported by opt, but not naive.",
                    err, point
                );
                differ = true;
            }
        }
    }
    differ
}

impl<Region, Loan, Point, Variable> Output<Region, Loan, Point, Variable>
where
    Region: Atom,
    Loan: Atom,
    Point: Atom,
    Variable: Atom,
{
    pub fn compute(
        all_facts: &AllFacts<Region, Loan, Point, Variable>,
        algorithm: Algorithm,
        dump_enabled: bool,
    ) -> Self {
        match algorithm {
            Algorithm::Naive => naive::compute(dump_enabled, all_facts.clone()),
            Algorithm::DatafrogOpt => datafrog_opt::compute(dump_enabled, all_facts.clone()),
            Algorithm::LocationInsensitive => {
                location_insensitive::compute(dump_enabled, &all_facts)
            }
            Algorithm::Compare => {
                let naive_output = naive::compute(dump_enabled, all_facts.clone());
                let opt_output = datafrog_opt::compute(dump_enabled, all_facts.clone());
                if compare_errors(&naive_output.errors, &opt_output.errors) {
                    panic!(concat!(
                        "The errors reported by the naive algorithm differ from ",
                        "the errors reported by the optimized algorithm. ",
                        "See the error log for details."
                    ));
                } else {
                    debug!("Naive and optimized algorithms reported the same errors.");
                }
                opt_output
            }
            Algorithm::Hybrid => hybrid::compute(dump_enabled, all_facts.clone()),
        }
    }

    fn new(dump_enabled: bool) -> Self {
        Output {
            borrow_live_at: FxHashMap::default(),
            restricts: FxHashMap::default(),
            restricts_anywhere: FxHashMap::default(),
            region_live_at: FxHashMap::default(),
            invalidates: FxHashMap::default(),
            errors: FxHashMap::default(),
            subset: FxHashMap::default(),
            subset_anywhere: FxHashMap::default(),
            var_live_at: FxHashMap::default(),
            var_drop_live_at: FxHashMap::default(),
            dump_enabled,
        }
    }

    pub fn errors_at(&self, location: Point) -> &[Loan] {
        match self.errors.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn borrows_in_scope_at(&self, location: Point) -> &[Loan] {
        match self.borrow_live_at.get(&location) {
            Some(p) => p,
            None => &[],
        }
    }

    pub fn restricts_at(&self, location: Point) -> Cow<'_, BTreeMap<Region, BTreeSet<Loan>>> {
        assert!(self.dump_enabled);
        match self.restricts.get(&location) {
            Some(map) => Cow::Borrowed(map),
            None => Cow::Owned(BTreeMap::default()),
        }
    }

    pub fn regions_live_at(&self, location: Point) -> &[Region] {
        assert!(self.dump_enabled);
        match self.region_live_at.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn subsets_at(&self, location: Point) -> Cow<'_, BTreeMap<Region, BTreeSet<Region>>> {
        assert!(self.dump_enabled);
        match self.subset.get(&location) {
            Some(v) => Cow::Borrowed(v),
            None => Cow::Owned(BTreeMap::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Atom for usize {
        fn index(self) -> usize {
            self
        }
    }

    fn compare(
        errors1: &FxHashMap<usize, Vec<usize>>,
        errors2: &FxHashMap<usize, Vec<usize>>,
    ) -> bool {
        let diff1 = compare_errors(errors1, errors2);
        let diff2 = compare_errors(errors2, errors1);
        assert_eq!(diff1, diff2);
        diff1
    }

    #[test]
    fn test_compare_errors() {
        let empty = FxHashMap::default();
        assert_eq!(false, compare(&empty, &empty));
        let mut empty_vec = FxHashMap::default();
        empty_vec.insert(1, vec![]);
        empty_vec.insert(2, vec![]);
        assert_eq!(false, compare(&empty, &empty_vec));

        let mut singleton1 = FxHashMap::default();
        singleton1.insert(1, vec![10]);
        assert_eq!(false, compare(&singleton1, &singleton1));
        let mut singleton2 = FxHashMap::default();
        singleton2.insert(1, vec![11]);
        assert_eq!(false, compare(&singleton2, &singleton2));
        let mut singleton3 = FxHashMap::default();
        singleton3.insert(2, vec![10]);
        assert_eq!(false, compare(&singleton3, &singleton3));

        assert_eq!(true, compare(&singleton1, &singleton2));
        assert_eq!(true, compare(&singleton2, &singleton3));
        assert_eq!(true, compare(&singleton1, &singleton3));

        assert_eq!(true, compare(&empty, &singleton1));
        assert_eq!(true, compare(&empty, &singleton2));
        assert_eq!(true, compare(&empty, &singleton3));

        let mut errors1 = FxHashMap::default();
        errors1.insert(1, vec![11]);
        errors1.insert(2, vec![10]);
        assert_eq!(false, compare(&errors1, &errors1));
        assert_eq!(true, compare(&errors1, &singleton1));
        assert_eq!(true, compare(&errors1, &singleton2));
        assert_eq!(true, compare(&errors1, &singleton3));
    }
}
