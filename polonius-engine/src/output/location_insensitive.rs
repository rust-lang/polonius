// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use datafrog::{Iteration, Relation, RelationLeaper};
use std::time::Instant;

use crate::facts::FactTypes;
use crate::output::{Context, Output};

pub(super) fn compute<T: FactTypes>(
    ctx: &Context<'_, T>,
    result: &mut Output<T>,
) -> Relation<(T::Loan, T::Point)> {
    let timer = Instant::now();

    let potential_errors = {
        // Static inputs
        let origin_live_on_entry = &ctx.origin_live_on_entry;
        let invalidates = &ctx.invalidates;

        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // .. some variables, ..
        let subset = iteration.variable::<(T::Origin, T::Origin)>("subset");
        let requires = iteration.variable::<(T::Origin, T::Loan)>("requires");

        let potential_errors = iteration.variable::<(T::Loan, T::Point)>("potential_errors");

        // load initial facts.

        // subset(origin1, origin2) :- outlives(origin1, origin2, _point)
        subset.extend(
            ctx.outlives
                .iter()
                .map(|&(origin1, origin2, _point)| (origin1, origin2)),
        );

        // requires(origin, loan) :- borrow_region(origin, loan, _point).
        requires.extend(
            ctx.borrow_region
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
            //   origin_live_on_entry(origin, point)
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
                    origin_live_on_entry.extend_with(|&(origin, _loan)| origin),
                    invalidates.extend_with(|&(_origin, loan)| loan),
                ),
                |&(_origin, loan), &point| (loan, point),
            );
        }

        if result.dump_enabled {
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
        }

        potential_errors.complete()
    };

    info!(
        "potential_errors is complete: {} tuples, {:?}",
        potential_errors.len(),
        timer.elapsed()
    );

    potential_errors
}
