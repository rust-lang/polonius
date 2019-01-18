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

use crate::output::Output;
use facts::{AllFacts, Atom};

use datafrog::{Iteration, Relation, RelationLeaper};

pub(super) fn compute<Region: Atom, Loan: Atom, Point: Atom>(
    dump_enabled: bool,
    mut all_facts: AllFacts<Region, Loan, Point>,
) -> Output<Region, Loan, Point> {
    let all_points: BTreeSet<Point> = all_facts
        .cfg_edge
        .iter()
        .map(|&(p, _)| p)
        .chain(all_facts.cfg_edge.iter().map(|&(_, q)| q))
        .collect();

    all_facts
        .region_live_at
        .reserve(all_facts.universal_region.len() * all_points.len());
    for &r in &all_facts.universal_region {
        for &p in &all_points {
            all_facts.region_live_at.push((r, p));
        }
    }

    let mut result = Output::new(dump_enabled);

    let computation_start = Instant::now();

    // Step 0 - Compute the full transitive closure of placeholder regions subsets.
    // This will be used in the "main" computation to check errors in relations between
    // named lifetimes.
    //
    // NOTE: this is done in a separate datafrog computation, because right now datafrog only
    // supports the antijoins we need to generate errors on static `Relation`s instead of dynamic
    // `Variable`s.
    // Whenever datafrog supports regular and leapjoin antijoins, this Step 0 may be entirely
    // folded into the main computation of the analysis, if needed.
    let known_subset = {
        let mut iteration = Iteration::new();

        // .decl known_base_subset(R1: region, R2: region)
        //
        // Indicates that the placeholder region `R1` is a known "base subset" of the
        // placeholder region `R2`: either specified manually by the user or via implied bounds.
        //
        // Input relation: stored ready for joins, keyed by `R2`.
        let known_base_subset_r2 = iteration.variable::<(Region, Region)>("known_base_subset_r2");

        // .decl known_subset(R1: region, R2: region)
        //
        // Output relation: the complete set of placeholder regions subsets `R1: R2`, containing
        // - the input "user facing" subsets
        // - the subsets derived by transitivity
        let known_subset = iteration.variable::<(Region, Region)>("known_subset");

        known_base_subset_r2.insert(Relation::from(
            all_facts.known_subset.iter().map(|&(r1, r2)| (r2, r1)),
        ));

        // known_subset(R1, R2) :-
        //   known_base_subset(R1, R2).
        known_subset.insert(all_facts.known_subset.into());

        while iteration.changed() {
            // known_subset(R1, R3) :-
            //   known_base_subset(R1, R2),
            //   known_subset(R2, R3).
            known_subset.from_join(&known_base_subset_r2, &known_subset, |&_r2, &r1, &r3| {
                (r1, r3)
            });
        }

        known_subset.complete()
    };

    // main computation
    let (errors, subset_errors) = {
        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // static inputs
        let cfg_edge_rel: Relation<(Point, Point)> = all_facts.cfg_edge.into();
        let killed_rel: Relation<(Loan, Point)> = all_facts.killed.into();

        // .decl placeholder_region(R: region)
        //
        // Input relation: universally quantified regions.
        let placeholder_region = iteration.variable::<((Region), ())>("placeholder_region");
        placeholder_region.insert(Relation::from(
            all_facts.universal_region.iter().map(|&r| ((r), ())),
        ));

        // .decl known_subset(R1: region, R2: region)
        //
        // Input relation: complete set of placeholder regions subsets `R1: R2`.
        let known_subset: Relation<(Region, Region)> = known_subset.into();

        // .. some variables, ..
        let subset = iteration.variable::<(Region, Region, Point)>("subset");
        let requires = iteration.variable::<(Region, Loan, Point)>("requires");
        let borrow_live_at = iteration.variable::<((Loan, Point), ())>("borrow_live_at");

        // `invalidates` facts, stored ready for joins
        let invalidates = iteration.variable::<((Loan, Point), ())>("invalidates");

        // different indices for `subset`.
        let subset_r1p = iteration.variable_indistinct("subset_r1p");
        let subset_r2p = iteration.variable_indistinct("subset_r2p");
        let subset_r1 = iteration.variable_indistinct("subset_r1");

        // different index for `requires`.
        let requires_rp = iteration.variable_indistinct("requires_rp");

        // we need `region_live_at` in both variable and relation forms.
        // (respectively, for the regular join and the leapjoin).
        let region_live_at_var = iteration.variable::<((Region, Point), ())>("region_live_at");
        let region_live_at_rel = Relation::from_iter(all_facts.region_live_at.iter().cloned());

        // output
        let errors = iteration.variable("errors");

        // .decl subset_error(R1: region, R2: region, P:point)
        //
        // Output relation: illegal subset relations, subset requirements which are missing
        // from the inputs.
        //
        // FIXME: uses intermediary variables for the multi-way join. A longer comment explaining
        // why can be found below, where the relation is computed.
        let subset_error = iteration.variable::<(Region, Region, Point)>("subset_error");
        let subset_error_1 = iteration.variable_indistinct("subset_error_1");
        let subset_error_2 = iteration.variable_indistinct("subset_error_2");

        // load initial facts.
        subset.insert(all_facts.outlives.into());
        requires.insert(all_facts.borrow_region.into());
        invalidates.extend(all_facts.invalidates.iter().map(|&(p, b)| ((b, p), ())));
        region_live_at_var.extend(all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())));

        // .. and then start iterating rules!
        while iteration.changed() {
            // Cleanup step: remove symmetries
            // - remove regions which are `subset`s of themselves
            //
            // FIXME: investigate whether is there a better way to do that without complicating
            // the rules too much, because it would also require temporary variables and
            // impact performance. Until then, the big reduction in tuples improves performance
            // a lot, even if we're potentially adding a small number of tuples
            // per round just to remove them in the next round.
            subset
                .recent
                .borrow_mut()
                .elements
                .retain(|&(r1, r2, _)| r1 != r2);

            // remap fields to re-index by keys.
            subset_r1p.from_map(&subset, |&(r1, r2, p)| ((r1, p), r2));
            subset_r2p.from_map(&subset, |&(r1, r2, p)| ((r2, p), r1));
            subset_r1.from_map(&subset, |&(r1, r2, p)| (r1, (r2, p)));

            requires_rp.from_map(&requires, |&(r, b, p)| ((r, p), b));

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
            subset.from_leapjoin(
                &subset,
                (
                    cfg_edge_rel.extend_with(|&(_r1, _r2, p)| p),
                    region_live_at_rel.extend_with(|&(r1, _r2, _p)| r1),
                    region_live_at_rel.extend_with(|&(_r1, r2, _p)| r2),
                ),
                |&(r1, r2, _p), &q| (r1, r2, q),
            );

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
            requires.from_leapjoin(
                &requires,
                (
                    killed_rel.filter_anti(|&(_r, b, p)| (b, p)),
                    cfg_edge_rel.extend_with(|&(_r, _b, p)| p),
                    region_live_at_rel.extend_with(|&(r, _b, _p)| r),
                ),
                |&(r, b, _p), &q| (r, b, q),
            );

            // borrow_live_at(B, P) :-
            //   requires(R, B, P),
            //   region_live_at(R, P).
            borrow_live_at.from_join(&requires_rp, &region_live_at_var, |&(_r, p), &b, &()| {
                ((b, p), ())
            });

            // .decl errors(B, P) :- invalidates(B, P), borrow_live_at(B, P).
            errors.from_join(&invalidates, &borrow_live_at, |&(b, p), &(), &()| (b, p));

            // subset_error(R1, R2, P) :-
            //   subset(R1, R2, P),
            //   placeholder_region(R1),
            //   placeholder_region(R2),
            //   !known_subset(R1, R2).
            //
            // FIXME: As mentioned above, this join requires intermediary variables and indices.
            // Since it's _only filtering data_ depending on multiple relations, it would not be
            // considered a well-formed leapjoin and would panic.
            // When we relax these well-formedness constraints in datafrog, we'll be able to use a
            // leapjoin here to remove the intermediary scaffolding.
            subset_error_1.from_join(&subset_r1, &placeholder_region, |&r1, &(r2, p), _| {
                (r2, (r1, p))
            });
            subset_error_2.from_join(&subset_error_1, &placeholder_region, |&r2, &(r1, p), _| {
                ((r1, r2), p)
            });
            subset_error.from_antijoin(&subset_error_2, &known_subset, |&(r1, r2), &p| (r1, r2, p));
        }

        if dump_enabled {
            let subset = subset.complete();
            assert!(
                subset.iter().filter(|&(r1, r2, _)| r1 == r2).count() == 0,
                "unwanted subset symmetries"
            );
            for (r1, r2, location) in &subset.elements {
                result
                    .subset
                    .entry(*location)
                    .or_insert(BTreeMap::new())
                    .entry(*r1)
                    .or_insert(BTreeSet::new())
                    .insert(*r2);
            }

            for (r1, r2) in known_subset.iter() {
                result
                    .known_subset
                    .entry(*r1)
                    .or_insert(BTreeSet::new())
                    .insert(*r2);
            }

            let requires = requires.complete();
            for (region, borrow, location) in &requires.elements {
                result
                    .restricts
                    .entry(*location)
                    .or_insert(BTreeMap::new())
                    .entry(*region)
                    .or_insert(BTreeSet::new())
                    .insert(*borrow);
            }

            for (region, location) in &region_live_at_rel.elements {
                result
                    .region_live_at
                    .entry(*location)
                    .or_insert(vec![])
                    .push(*region);
            }

            let borrow_live_at = borrow_live_at.complete();
            for &((loan, location), ()) in &borrow_live_at.elements {
                result
                    .borrow_live_at
                    .entry(location)
                    .or_insert(Vec::new())
                    .push(loan);
            }
        }

        (errors.complete(), subset_error.complete())
    };

    if dump_enabled {
        info!(
            "analysis is complete: {} `errors` tuples, {} `subset_errors` tuples, {:?}",
            errors.len(),
            subset_errors.len(),
            computation_start.elapsed()
        );
    }

    for (borrow, location) in &errors.elements {
        result
            .errors
            .entry(*location)
            .or_insert(Vec::new())
            .push(*borrow);
    }

    // TMP: ignore the location of these errors for now
    for (r1, r2, _) in subset_errors.iter() {
        result
            .subset_errors
            .insert((*r1, *r2));
    }

    result
}
