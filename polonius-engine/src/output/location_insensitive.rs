// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::time::Instant;

use crate::output::initialization;
use crate::output::liveness;
use crate::output::Output;

use datafrog::{Iteration, Relation, RelationLeaper};
use facts::{AllFacts, FactTypes};

pub(super) fn compute<T: FactTypes>(dump_enabled: bool, all_facts: &AllFacts<T>) -> Output<T> {
    let mut result = Output::new(dump_enabled);
    let var_maybe_initialized_on_exit = initialization::init_var_maybe_initialized_on_exit(
        all_facts.child.clone(),
        all_facts.path_belongs_to_var.clone(),
        all_facts.initialized_at.clone(),
        all_facts.moved_out_at.clone(),
        all_facts.path_accessed_at.clone(),
        &all_facts.cfg_edge,
        &mut result,
    );
    let region_live_at = liveness::init_region_live_at(
        all_facts.var_used.clone(),
        all_facts.var_drop_used.clone(),
        all_facts.var_defined.clone(),
        all_facts.var_uses_region.clone(),
        all_facts.var_drops_region.clone(),
        var_maybe_initialized_on_exit.clone(),
        &all_facts.cfg_edge,
        all_facts.universal_region.clone(),
        &mut result,
    );

    let potential_errors_start = Instant::now();

    let potential_errors = {
        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // static inputs
        let region_live_at: Relation<(T::Origin, T::Point)> = region_live_at.into();
        let invalidates = Relation::from_iter(
            all_facts
                .invalidates
                .iter()
                .map(|&(loan, point)| (point, loan)),
        );

        // .. some variables, ..
        let subset = iteration.variable::<(T::Origin, T::Origin)>("subset");
        let requires = iteration.variable::<(T::Origin, T::Loan)>("requires");

        let potential_errors = iteration.variable::<(T::Loan, T::Point)>("potential_errors");

        // load initial facts.

        // subset(origin1, origin2) :- outlives(origin1, origin2, _point)
        subset.extend(
            all_facts
                .outlives
                .iter()
                .map(|&(origin1, origin2, _point)| (origin1, origin2)),
        );

        // requires(origin, loan) :- borrow_region(origin, loan, _point).
        requires.extend(
            all_facts
                .borrow_region
                .iter()
                .map(|&(origin, loan, _point)| (origin, loan)),
        );

        // .. and then start iterating rules!
        while iteration.changed() {
            // requires(origin2, loan) :-
            //   requires(origin1, loan),
            //   subset(origin1, origin2).
            //
            // Note: Since `subset` is effectively a static input, this join can be ported to
            // a leapjoin. Doing so, however, was 7% slower on `clap`.
            requires.from_join(&requires, &subset, |&_origin1, &loan, &origin2| {
                (origin2, loan)
            });

            // borrow_live_at(loan, point) :-
            //   requires(origin, loan),
            //   region_live_at(origin, point)
            //
            // potential_errors(loan, point) :-
            //   invalidates(loan, point),
            //   borrow_live_at(loan, point).
            //
            // Note: we don't need to materialize `borrow_live_at` here
            // so we can inline it in the `potential_errors` relation.
            //
            potential_errors.from_leapjoin(
                &requires,
                (
                    region_live_at.extend_with(|&(origin, _loan)| origin),
                    invalidates.extend_with(|&(_origin, loan)| loan),
                ),
                |&(_origin, loan), &point| (loan, point),
            );
        }

        if dump_enabled {
            let subset = subset.complete();
            for &(origin1, origin2) in subset.iter() {
                result
                    .subset_anywhere
                    .entry(origin1)
                    .or_default()
                    .insert(origin2);
            }

            let requires = requires.complete();
            for &(origin, loan) in requires.iter() {
                result
                    .restricts_anywhere
                    .entry(origin)
                    .or_default()
                    .insert(loan);
            }

            for &(origin, location) in region_live_at.iter() {
                result
                    .region_live_at
                    .entry(location)
                    .or_default()
                    .push(origin);
            }
        }

        potential_errors.complete()
    };

    if dump_enabled {
        info!(
            "potential_errors is complete: {} tuples, {:?}",
            potential_errors.len(),
            potential_errors_start.elapsed()
        );
    }

    for &(loan, location) in potential_errors.iter() {
        result.errors.entry(location).or_default().push(loan);
    }

    result
}
