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
        let loan_invalidated_at = &ctx.loan_invalidated_at;

        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // .. some variables, ..
        let subset = iteration.variable::<(T::Origin, T::Origin)>("subset");
        let origin_contains_loan_on_entry =
            iteration.variable::<(T::Origin, T::Loan)>("origin_contains_loan_on_entry");

        let potential_errors = iteration.variable::<(T::Loan, T::Point)>("potential_errors");

        // load initial facts.

        // subset(origin1, origin2) :-
        //   subset_base(origin1, origin2, _point).
        subset.extend(
            ctx.subset_base
                .iter()
                .map(|&(origin1, origin2, _point)| (origin1, origin2)),
        );

        // origin_contains_loan_on_entry(origin, loan) :-
        //   loan_issued_at(origin, loan, _point).
        origin_contains_loan_on_entry.extend(
            ctx.loan_issued_at
                .iter()
                .map(|&(origin, loan, _point)| (origin, loan)),
        );

        // .. and then start iterating rules!
        while iteration.changed() {
            // origin_contains_loan_on_entry(origin2, loan) :-
            //   origin_contains_loan_on_entry(origin1, loan),
            //   subset(origin1, origin2).
            //
            // Note: Since `subset` is effectively a static input, this join can be ported to
            // a leapjoin. Doing so, however, was 7% slower on `clap`.
            origin_contains_loan_on_entry.from_join(
                &origin_contains_loan_on_entry,
                &subset,
                |&_origin1, &loan, &origin2| (origin2, loan),
            );

            // loan_live_at(loan, point) :-
            //   origin_contains_loan_on_entry(origin, loan),
            //   origin_live_on_entry(origin, point)
            //
            // potential_errors(loan, point) :-
            //   loan_invalidated_at(loan, point),
            //   loan_live_at(loan, point).
            //
            // Note: we don't need to materialize `loan_live_at` here
            // so we can inline it in the `potential_errors` relation.
            //
            potential_errors.from_leapjoin(
                &origin_contains_loan_on_entry,
                (
                    origin_live_on_entry.extend_with(|&(origin, _loan)| origin),
                    loan_invalidated_at.extend_with(|&(_origin, loan)| loan),
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

            let origin_contains_loan_on_entry = origin_contains_loan_on_entry.complete();
            for &(origin, loan) in origin_contains_loan_on_entry.iter() {
                result
                    .origin_contains_loan_anywhere
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
