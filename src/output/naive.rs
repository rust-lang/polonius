// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A version of the Naive datalog analysis using Datafrog.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::facts::{AllFacts, Loan, Point, Region};
use crate::output::Output;

use datafrog::{Iteration, Relation};

// NOTE: The implementation could be simplified and optimized:
// - some indices could be shared between Iterations
// - having more than 1 Iteration is not absolutely necessary
pub(super) fn compute(dump_enabled: bool, mut all_facts: AllFacts) -> Output {
    let all_points: BTreeSet<Point> = all_facts
        .cfg_edge
        .iter()
        .map(|&(p, _)| p)
        .chain(all_facts.cfg_edge.iter().map(|&(_, q)| q))
        .collect();

    for &r in &all_facts.universal_region {
        for &p in &all_points {
            all_facts.region_live_at.push((r, p));
        }
    }

    let mut result = Output::new(dump_enabled);

    let subset_start = Instant::now();

    // compute the subsets rules, but indexed ((Region, Point), Region) for the next iteration
    let subset = {
        // Create a new iteration context, ...
        let mut iteration1 = Iteration::new();

        // .. some variables, ..
        let subset = iteration1.variable::<(Region, Region, Point)>("subset");

        // different indices for `subset`.
        let subset_r1p = iteration1.variable::<((Region, Point), Region)>("subset_r1p");
        let subset_r2p = iteration1.variable::<((Region, Point), Region)>("subset_r2p");
        let subset_p = iteration1.variable::<(Point, (Region, Region))>("subset_p");

        // temporaries as we perform a multi-way join.
        let subset_1 = iteration1.variable::<((Region, Point), Region)>("subset_1");
        let subset_2 = iteration1.variable::<((Region, Point), Region)>("subset_2");

        let region_live_at = iteration1.variable::<((Region, Point), ())>("region_live_at"); // redundantly computed index
        let cfg_edge_p = iteration1.variable::<(Point, Point)>("cfg_edge_p"); // redundantly computed index

        // load initial facts.
        subset.insert(all_facts.outlives.into());
        region_live_at.insert(Relation::from(
            all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())),
        ));
        cfg_edge_p.insert(all_facts.cfg_edge.clone().into());

        // .. and then start iterating rules!
        while iteration1.changed() {
            // remap fields to re-index by keys.
            subset_r1p.from_map(&subset, |&(r1, r2, p)| ((r1, p), r2));
            subset_r2p.from_map(&subset, |&(r1, r2, p)| ((r2, p), r1));
            subset_p.from_map(&subset, |&(r1, r2, p)| (p, (r1, r2)));

            // subset(R1, R2, P) :- outlives(R1, R2, P).
            // Already loaded; outlives is static.

            // subset(R1, R3, P) :-
            //   subset(R1, R2, P),
            //   subset(R2, R3, P).
            subset.from_join(&subset_r2p, &subset_r1p, |&(_r2, p), &r1, &r3| (r1, r3, p));

            // subset(R1, R2, Q) :-
            //   subset(R1, R2, P),
            //   cfg_edge(P, Q),
            //   region_live_at(R1, Q),
            //   region_live_at(R2, Q).

            subset_1.from_join(&subset_p, &cfg_edge_p, |&_p, &(r1, r2), &q| ((r1, q), r2));
            subset_2.from_join(&subset_1, &region_live_at, |&(r1, q), &r2, &()| {
                ((r2, q), r1)
            });
            subset.from_join(&subset_2, &region_live_at, |&(r2, q), &r1, &()| (r1, r2, q));
        }

        subset_r1p.complete()
    };

    if dump_enabled {
        println!(
            "subset is complete: {} tuples, {:?}",
            subset.len(),
            subset_start.elapsed()
        );

        for ((r1, location), r2) in &subset.elements {
            result
                .subset
                .entry(*location)
                .or_insert(BTreeMap::new())
                .entry(*r1)
                .or_insert(BTreeSet::new())
                .insert(*r2);
            result.region_degrees.update_degrees(*r1, *r2, *location);
        }
    }

    let requires_start = Instant::now();

    // compute the requires rules, but indexed ((Region, Point), Loan) for the next iteration
    let requires = {
        // Create a new iteration context, ...
        let mut iteration2 = Iteration::new();

        // .. some variables, ..
        let requires = iteration2.variable::<(Region, Loan, Point)>("requires");
        requires.insert(all_facts.borrow_region.into());

        // some indexes
        let requires_rp = iteration2.variable::<((Region, Point), Loan)>("requires_rp");
        let requires_bp = iteration2.variable::<((Loan, Point), Region)>("requires_bp");

        let requires_1 = iteration2.variable::<(Point, (Loan, Region))>("requires_1");
        let requires_2 = iteration2.variable::<((Region, Point), Loan)>("requires_2");

        // since we're using subset mapped ((r, p), r) we can use it directly out of iteration 1
        let subset_r1p = iteration2.variable::<((Region, Point), Region)>("subset_r1p");
        subset_r1p.insert(subset);

        let killed = all_facts.killed.into();
        let region_live_at = iteration2.variable::<((Region, Point), ())>("region_live_at"); // redundantly computed index
        let cfg_edge_p = iteration2.variable::<(Point, Point)>("cfg_edge_p"); // redundantly computed index

        // load initial facts.
        region_live_at.insert(Relation::from(
            all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())),
        ));
        cfg_edge_p.insert(all_facts.cfg_edge.into());

        // .. and then start iterating rules!
        while iteration2.changed() {
            requires_rp.from_map(&requires, |&(r, b, p)| ((r, p), b));
            requires_bp.from_map(&requires, |&(r, b, p)| ((b, p), r));

            // requires(R, B, P) :- borrow_region(R, B, P).
            // Already loaded; borrow_region is static.

            // requires(R2, B, P) :-
            //   requires(R1, B, P),
            //   subset(R1, R2, P).
            requires.from_join(&requires_rp, &subset_r1p, |&(_r1, p), &b, &r2| (r2, b, p));

            // requires(R, B, Q) :-
            //   requires(R, B, P),
            //   !killed(B, P),
            //   cfg_edge(P, Q),
            //   region_live_at(R, Q).
            requires_1.from_antijoin(&requires_bp, &killed, |&(b, p), &r| (p, (b, r)));
            requires_2.from_join(&requires_1, &cfg_edge_p, |&_p, &(b, r), &q| ((r, q), b));
            requires.from_join(&requires_2, &region_live_at, |&(r, q), &b, &()| (r, b, q));
        }

        requires_rp.complete()
    };

    if dump_enabled {
        println!(
            "requires is complete: {} tuples, {:?}",
            requires.len(),
            requires_start.elapsed()
        );

        for ((region, location), borrow) in &requires.elements {
            result
                .restricts
                .entry(*location)
                .or_insert(BTreeMap::new())
                .entry(*region)
                .or_insert(BTreeSet::new())
                .insert(*borrow);
        }
    }

    let borrow_live_at_start = Instant::now();

    let borrow_live_at = {
        // Create a new iteration context, ...
        let mut iteration3 = Iteration::new();

        // .. some variables, ..
        let borrow_live_at = iteration3.variable::<(Loan, Point)>("borrow_live_at");

        // since we're using requires mapped ((r, p), b) we can use it directly out of iteration 2
        let requires_rp = iteration3.variable::<((Region, Point), Loan)>("requires_rp");
        requires_rp.insert(requires.into());

        let region_live_at = iteration3.variable::<((Region, Point), ())>("region_live_at"); // redundantly computed index

        // load initial facts.
        region_live_at.insert(Relation::from(
            all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())),
        ));

        while iteration3.changed() {
            // borrow_live_at(B, P) :- requires(R, B, P), region_live_at(R, P)
            borrow_live_at.from_join(&requires_rp, &region_live_at, |&(_r, p), &b, &()| (b, p));
        }

        borrow_live_at.complete()
    };

    if dump_enabled {
        println!(
            "borrow_live_at is complete: {} tuples, {:?}",
            borrow_live_at.len(),
            borrow_live_at_start.elapsed()
        );
    }

    for (borrow, location) in &borrow_live_at.elements {
        result
            .borrow_live_at
            .entry(*location)
            .or_insert(Vec::new())
            .push(*borrow);
    }

    result
}
