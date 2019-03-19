// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::BTreeSet;
use std::time::Instant;

use crate::output::Output;

use datafrog::{Iteration, Relation, RelationLeaper};
use facts::{AllFacts, Atom};

pub(super) fn compute<Region: Atom, Loan: Atom, Point: Atom>(
    dump_enabled: bool,
    all_facts: &AllFacts<Region, Loan, Point>,
) -> Output<Region, Loan, Point> {
    let all_points: BTreeSet<Point> = all_facts
        .cfg_edge
        .iter()
        .map(|&(p, _)| p)
        .chain(all_facts.cfg_edge.iter().map(|&(_, q)| q))
        .collect();

    let mut region_live_at = all_facts.region_live_at.clone();
    region_live_at.reserve(all_facts.universal_region.len() * all_points.len());
    for &r in &all_facts.universal_region {
        for &p in &all_points {
            region_live_at.push((r, p));
        }
    }

    let mut result = Output::new(dump_enabled);

    let potential_errors_start = Instant::now();

    let potential_errors = {
        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // static inputs
        let region_live_at: Relation<(Region, Point)> = region_live_at.into();
        let invalidates = Relation::from_iter(all_facts.invalidates.iter().map(|&(b, p)| (p, b)));

        // .. some variables, ..
        let subset = iteration.variable::<(Region, Region)>("subset");
        let requires = iteration.variable::<(Region, Loan)>("requires");

        let potential_errors = iteration.variable::<(Loan, Point)>("potential_errors");

        // load initial facts.

        // subset(R1, R2) :- outlives(R1, R2, _P)
        subset.extend(all_facts.outlives.iter().map(|&(r1, r2, _p)| (r1, r2)));

        // requires(R, B) :- borrow_region(R, B, _P).
        requires.extend(all_facts.borrow_region.iter().map(|&(r, b, _p)| (r, b)));

        // .. and then start iterating rules!
        while iteration.changed() {
            // requires(R2, B) :- requires(R1, B), subset(R1, R2).
            //
            // Note: Since `subset` is effectively a static input, this join can be ported to
            // a leapjoin. Doing so, however, was 7% slower on `clap`.
            requires.from_join(&requires, &subset, |&_r1, &b, &r2| (r2, b));

            // borrow_live_at(B, P) :- requires(R, B), region_live_at(R, P)
            // potential_errors(B, P) :- invalidates(B, P), borrow_live_at(B, P).
            //
            // Note: we don't need to materialize `borrow_live_at` here
            // so we can inline it in the `potential_errors` relation.
            //
            potential_errors.from_leapjoin(
                &requires,
                (
                    region_live_at.extend_with(|&(r, _b)| r),
                    invalidates.extend_with(|&(_r, b)| b),
                ),
                |&(_r, b), &p| (b, p),
            );
        }

        if dump_enabled {
            let subset = subset.complete();
            for (r1, r2) in &subset.elements {
                result
                    .subset_anywhere
                    .entry(*r1)
                    .or_insert(BTreeSet::new())
                    .insert(*r2);
            }

            let requires = requires.complete();
            for (region, borrow) in &requires.elements {
                result
                    .restricts_anywhere
                    .entry(*region)
                    .or_insert(BTreeSet::new())
                    .insert(*borrow);
            }

            for (region, location) in &region_live_at.elements {
                result
                    .region_live_at
                    .entry(*location)
                    .or_insert(vec![])
                    .push(*region);
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

    for (borrow, location) in &potential_errors.elements {
        result
            .errors
            .entry(*location)
            .or_insert(Vec::new())
            .push(*borrow);
    }

    result
}
