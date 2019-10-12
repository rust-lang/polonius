// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use datafrog::Relation;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use crate::facts::{AllFacts, Atom, FactTypes};

mod datafrog_opt;
mod initialization;
mod liveness;
mod location_insensitive;
mod naive;

#[derive(Debug, Clone, Copy)]
pub enum Algorithm {
    /// Simple rules, but slower to execute
    Naive,

    /// Optimized variant of the rules
    DatafrogOpt,

    /// Fast to compute, but imprecise: there can be false-positives
    /// but no false-negatives. Tailored for quick "early return" situations.
    LocationInsensitive,

    /// Compares the `Naive` and `DatafrogOpt` variants to ensure they indeed
    /// compute the same errors.
    Compare,

    /// Combination of the fast `LocationInsensitive` pre-pass, followed by
    /// the more expensive `DatafrogOpt` variant.
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
pub struct Output<T: FactTypes> {
    pub errors: FxHashMap<T::Point, Vec<T::Loan>>,

    pub dump_enabled: bool,

    // these are just for debugging
    pub borrow_live_at: FxHashMap<T::Point, Vec<T::Loan>>,
    pub restricts: FxHashMap<T::Point, BTreeMap<T::Origin, BTreeSet<T::Loan>>>,
    pub restricts_anywhere: FxHashMap<T::Origin, BTreeSet<T::Loan>>,
    pub region_live_at: FxHashMap<T::Point, Vec<T::Origin>>,
    pub invalidates: FxHashMap<T::Point, Vec<T::Loan>>,
    pub subset: FxHashMap<T::Point, BTreeMap<T::Origin, BTreeSet<T::Origin>>>,
    pub subset_anywhere: FxHashMap<T::Origin, BTreeSet<T::Origin>>,
    pub var_live_at: FxHashMap<T::Point, Vec<T::Variable>>,
    pub var_drop_live_at: FxHashMap<T::Point, Vec<T::Variable>>,
    pub path_maybe_initialized_at: FxHashMap<T::Point, Vec<T::Path>>,
    pub var_maybe_initialized_on_exit: FxHashMap<T::Point, Vec<T::Variable>>,
}

struct Context<T: FactTypes> {
    all_facts: AllFacts<T>,

    // `Relation`s used by multiple variants as static inputs
    region_live_at: Relation<(T::Origin, T::Point)>,
    invalidates: Relation<(T::Loan, T::Point)>,
    cfg_edge: Relation<(T::Point, T::Point)>,
    killed: Relation<(T::Loan, T::Point)>,

    // Partial results possibly used by other variants as input
    potential_errors: FxHashSet<T::Loan>,
}

impl<T: FactTypes> Output<T> {
    pub fn compute(all_facts: &AllFacts<T>, algorithm: Algorithm, dump_enabled: bool) -> Self {
        // All variants require the same initial preparations, done in multiple
        // successive steps:
        // - compute initialization data
        // - compute liveness
        // - prepare static inputs as shared `Relation`s
        // - in cases where `LocationInsensitive` variant is ran as a filtering pre-pass,
        //   partial results can also be stored in the context, so that the following
        //   variant can use it to prune its own input data

        // TODO: remove the need for this clone in concert here and in rustc
        let mut all_facts = all_facts.clone();

        let mut result = Output::new(dump_enabled);

        let cfg_edge = mem::replace(&mut all_facts.cfg_edge, Vec::new()).into();

        // Initialization
        let var_maybe_initialized_on_exit = initialization::init_var_maybe_initialized_on_exit(
            mem::replace(&mut all_facts.child, Vec::new()),
            mem::replace(&mut all_facts.path_belongs_to_var, Vec::new()),
            mem::replace(&mut all_facts.initialized_at, Vec::new()),
            mem::replace(&mut all_facts.moved_out_at, Vec::new()),
            mem::replace(&mut all_facts.path_accessed_at, Vec::new()),
            &cfg_edge,
            &mut result,
        );

        // Liveness
        let region_live_at = liveness::init_region_live_at(
            mem::replace(&mut all_facts.var_used, Vec::new()),
            mem::replace(&mut all_facts.var_drop_used, Vec::new()),
            mem::replace(&mut all_facts.var_defined, Vec::new()),
            mem::replace(&mut all_facts.var_uses_region, Vec::new()),
            mem::replace(&mut all_facts.var_drops_region, Vec::new()),
            var_maybe_initialized_on_exit,
            &cfg_edge,
            mem::replace(&mut all_facts.universal_region, Vec::new()),
            &mut result,
        );

        // Prepare data as datafrog relations, ready to join.
        //
        // Note: if rustc and polonius had more interaction, we could also delay or avoid
        // generating some of the facts that are now always present here. For example,
        // the `LocationInsensitive` variant doesn't use the `killed` or `invalidates`
        // relations, so we could technically delay passing them from rustc, when
        // using this or the `Hybrid` variant, to after the pre-pass has made sure
        // we actually need to compute the full analysis. If these facts happened to
        // be recorded in separate MIR walks, we might also avoid generating those facts.

        let region_live_at = region_live_at.into();
        let killed = mem::replace(&mut all_facts.killed, Vec::new()).into();

        // TODO: flip the order of this relation's arguments in rustc
        // from `invalidates(loan, point)` to `invalidates(point, loan)`.
        // to avoid this allocation.
        let invalidates = Relation::from_iter(
            all_facts
                .invalidates
                .iter()
                .map(|&(loan, point)| (point, loan)),
        );

        // Ask the variants to compute errors in their own way
        let mut ctx = Context {
            all_facts,
            region_live_at,
            cfg_edge,
            invalidates,
            killed,
            potential_errors: FxHashSet::default(),
        };

        let errors = match algorithm {
            Algorithm::LocationInsensitive => location_insensitive::compute(&ctx, &mut result),
            Algorithm::Naive => naive::compute(&ctx, &mut result),
            Algorithm::DatafrogOpt => datafrog_opt::compute(&ctx, &mut result),
            Algorithm::Hybrid => {
                // Execute the fast `LocationInsensitive` computation as a pre-pass:
                // if it finds no possible errors, we don't need to do the more complex
                // computations as they won't find errors either, and we can return early.
                let potential_errors = location_insensitive::compute(&ctx, &mut result);
                if potential_errors.is_empty() {
                    potential_errors
                } else {
                    // Record these potential errors as they can be used to limit the next
                    // variant's work to only these loans.
                    ctx.potential_errors
                        .extend(potential_errors.iter().map(|&(loan, _)| loan));

                    datafrog_opt::compute(&ctx, &mut result)
                }
            }
            Algorithm::Compare => {
                // Ensure the `Naive` and `DatafrogOpt` errors are the same
                let naive_errors = naive::compute(&ctx, &mut result);
                let opt_errors = datafrog_opt::compute(&ctx, &mut result);

                let mut naive_errors_by_point = FxHashMap::default();
                for &(loan, point) in naive_errors.iter() {
                    naive_errors_by_point
                        .entry(point)
                        .or_insert(Vec::new())
                        .push(loan);
                }

                let mut opt_errors_by_point = FxHashMap::default();
                for &(loan, point) in opt_errors.iter() {
                    opt_errors_by_point
                        .entry(point)
                        .or_insert(Vec::new())
                        .push(loan);
                }

                if compare_errors(&naive_errors_by_point, &opt_errors_by_point) {
                    panic!(concat!(
                        "The errors reported by the naive algorithm differ from ",
                        "the errors reported by the optimized algorithm. ",
                        "See the error log for details."
                    ));
                } else {
                    debug!("Naive and optimized algorithms reported the same errors.");
                }

                naive_errors
            }
        };

        for &(loan, location) in errors.iter() {
            result.errors.entry(location).or_default().push(loan);
        }

        // Record more debugging info when necessary
        if dump_enabled {
            for &(origin, location) in ctx.region_live_at.iter() {
                result
                    .region_live_at
                    .entry(location)
                    .or_default()
                    .push(origin);
            }
        }

        result
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
            path_maybe_initialized_at: FxHashMap::default(),
            var_maybe_initialized_on_exit: FxHashMap::default(),
            dump_enabled,
        }
    }

    pub fn errors_at(&self, location: T::Point) -> &[T::Loan] {
        match self.errors.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn borrows_in_scope_at(&self, location: T::Point) -> &[T::Loan] {
        match self.borrow_live_at.get(&location) {
            Some(p) => p,
            None => &[],
        }
    }

    pub fn restricts_at(
        &self,
        location: T::Point,
    ) -> Cow<'_, BTreeMap<T::Origin, BTreeSet<T::Loan>>> {
        assert!(self.dump_enabled);
        match self.restricts.get(&location) {
            Some(map) => Cow::Borrowed(map),
            None => Cow::Owned(BTreeMap::default()),
        }
    }

    pub fn regions_live_at(&self, location: T::Point) -> &[T::Origin] {
        assert!(self.dump_enabled);
        match self.region_live_at.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn subsets_at(
        &self,
        location: T::Point,
    ) -> Cow<'_, BTreeMap<T::Origin, BTreeSet<T::Origin>>> {
        assert!(self.dump_enabled);
        match self.subset.get(&location) {
            Some(v) => Cow::Borrowed(v),
            None => Cow::Owned(BTreeMap::default()),
        }
    }
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
        let mut naive_errors = all_naive_errors.get(&point).cloned().unwrap_or_default();
        naive_errors.sort();

        let mut opt_errors = all_opt_errors.get(&point).cloned().unwrap_or_default();
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
