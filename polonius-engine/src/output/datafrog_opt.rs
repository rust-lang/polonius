// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::output::Output;

use datafrog::{Iteration, Relation};
use facts::{AllFacts, Atom};

pub(super) fn compute<Region: Atom, Loan: Atom, Point: Atom>(
    dump_enabled: bool,
    mut all_facts: AllFacts<Region, Loan, Point>,
) -> Output<Region, Loan, Point> {
    // Declare that each universal region is live at every point.
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

    let timer = Instant::now();

    let mut result = Output::new(dump_enabled);

    let errors = {
        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // static inputs
        let cfg_edge = iteration.variable::<(Point, Point)>("cfg_edge");
        let killed = all_facts.killed.into();

        // `invalidates` facts, stored ready for joins
        let invalidates = iteration.variable::<((Loan, Point), ())>("invalidates");

        // we need `region_live_at` in both variable and relation forms.
        // (respectively, for join and antijoin).
        let region_live_at_rel =
            Relation::from(all_facts.region_live_at.iter().map(|&(r, p)| (r, p)));
        let region_live_at_var = iteration.variable::<((Region, Point), ())>("region_live_at");

        // variables, indices for the computation rules, and temporaries for the multi-way joins
        let subset = iteration.variable::<(Region, Region, Point)>("subset");
        let subset_1 = iteration.variable_indistinct("subset_1");
        let subset_2 = iteration.variable_indistinct("subset_2");
        let subset_r1p = iteration.variable_indistinct("subset_r1p");
        let subset_p = iteration.variable_indistinct("subset_p");

        let requires = iteration.variable::<(Region, Loan, Point)>("requires");
        let requires_1 = iteration.variable_indistinct("requires_1");
        let requires_2 = iteration.variable_indistinct("requires_2");
        let requires_bp = iteration.variable_indistinct("requires_bp");
        let requires_rp = iteration.variable_indistinct("requires_rp");

        let borrow_live_at = iteration.variable::<((Loan, Point), ())>("borrow_live_at");

        let live_to_dead_regions =
            iteration.variable::<(Region, Region, Point, Point)>("live_to_dead_regions");
        let live_to_dead_regions_1 = iteration.variable_indistinct("live_to_dead_regions_1");
        let live_to_dead_regions_2 = iteration.variable_indistinct("live_to_dead_regions_2");
        let live_to_dead_regions_r2pq = iteration.variable_indistinct("live_to_dead_regions_r2pq");

        let dead_region_requires =
            iteration.variable::<((Region, Point, Point), Loan)>("dead_region_requires");
        let dead_region_requires_1 = iteration.variable_indistinct("dead_region_requires_1");
        let dead_region_requires_2 = iteration.variable_indistinct("dead_region_requires_2");

        let dead_can_reach_origins =
            iteration.variable::<((Region, Point), Point)>("dead_can_reach_origins");
        let dead_can_reach = iteration.variable::<(Region, Region, Point, Point)>("dead_can_reach");
        let dead_can_reach_1 = iteration.variable_indistinct("dead_can_reach_1");
        let dead_can_reach_r2q = iteration.variable_indistinct("dead_can_reach_r2q");
        // nmatsakis: I tried to merge `dead_can_reach_r2q` and
        // `dead_can_reach`, but the result was ever so slightly slower, at least on clap.

        let dead_can_reach_live =
            iteration.variable::<((Region, Point, Point), Region)>("dead_can_reach_live");
        let dead_can_reach_live_r1pq = iteration.variable_indistinct("dead_can_reach_live_r1pq");

        // output
        let errors = iteration.variable("errors");

        // load initial facts.
        cfg_edge.insert(all_facts.cfg_edge.into());
        invalidates.insert(Relation::from(
            all_facts.invalidates.iter().map(|&(p, b)| ((b, p), ())),
        ));
        region_live_at_var.insert(Relation::from(
            all_facts.region_live_at.iter().map(|&(r, p)| ((r, p), ())),
        ));
        subset.insert(all_facts.outlives.into());
        requires.insert(all_facts.borrow_region.into());

        // .. and then start iterating rules!
        while iteration.changed() {
            // remap fields to re-index by the different keys
            subset_r1p.from_map(&subset, |&(r1, r2, p)| ((r1, p), r2));
            subset_p.from_map(&subset, |&(r1, r2, p)| (p, (r1, r2)));

            requires_bp.from_map(&requires, |&(r, b, p)| ((b, p), r));
            requires_rp.from_map(&requires, |&(r, b, p)| ((r, p), b));

            live_to_dead_regions_r2pq
                .from_map(&live_to_dead_regions, |&(r1, r2, p, q)| ((r2, p, q), r1));

            dead_can_reach_r2q.from_map(&dead_can_reach, |&(r1, r2, p, q)| ((r2, q), (r1, p)));
            dead_can_reach_live_r1pq
                .from_map(&dead_can_reach_live, |&((r1, p, q), r2)| ((r1, p, q), r2));

            // it's now time ... to datafrog:

            // .decl subset(R1, R2, P)
            //
            // At the point P, R1 <= R2.
            //
            // subset(R1, R2, P) :- outlives(R1, R2, P).
            // -> already loaded; outlives is a static input.

            // .decl requires(R, B, P) -- at the point, things with region R
            // may depend on data from borrow B
            //
            // requires(R, B, P) :- borrow_region(R, B, P).
            // -> already loaded; borrow_region is a static input.

            // .decl live_to_dead_regions(R1, R2, P, Q)
            //
            // The regions `R1` and `R2` are "live to dead"
            // on the edge `P -> Q` if:
            //
            // - In P, `R1` <= `R2`
            // - In Q, `R1` is live but `R2` is dead.
            //
            // In that case, `Q` would like to add all the
            // live things reachable from `R2` to `R1`.
            //
            // live_to_dead_regions(R1, R2, P, Q) :-
            //   subset(R1, R2, P),
            //   cfg_edge(P, Q),
            //   region_live_at(R1, Q),
            //   !region_live_at(R2, Q).
            live_to_dead_regions_1
                .from_join(&subset_p, &cfg_edge, |&p, &(r1, r2), &q| ((r1, q), (r2, p)));
            live_to_dead_regions_2.from_join(
                &live_to_dead_regions_1,
                &region_live_at_var,
                |&(r1, q), &(r2, p), &()| ((r2, q), (r1, p)),
            );
            live_to_dead_regions.from_antijoin(
                &live_to_dead_regions_2,
                &region_live_at_rel,
                |&(r2, q), &(r1, p)| (r1, r2, p, q),
            );

            // .decl dead_region_requires((R, P, Q), B)
            //
            // The region `R` requires the borrow `B`, but the
            // region `R` goes dead along the edge `P -> Q`
            //
            // dead_region_requires((R, P, Q), B) :-
            //   requires(R, B, P),
            //   !killed(B, P),
            //   cfg_edge(P, Q),
            //   !region_live_at(R, Q).
            dead_region_requires_1.from_antijoin(&requires_bp, &killed, |&(b, p), &r| (p, (b, r)));
            dead_region_requires_2.from_join(
                &dead_region_requires_1,
                &cfg_edge,
                |&p, &(b, r), &q| ((r, q), (b, p)),
            );
            dead_region_requires.from_antijoin(
                &dead_region_requires_2,
                &region_live_at_rel,
                |&(r, q), &(b, p)| ((r, p, q), b),
            );

            // .decl dead_can_reach_origins(R, P, Q)
            //
            // Contains dead regions where we are interested
            // in computing the transitive closure of things they
            // can reach.
            dead_can_reach_origins.from_map(&live_to_dead_regions, |&(_r1, r2, p, q)| ((r2, p), q));
            dead_can_reach_origins.from_map(&dead_region_requires, |&((r, p, q), _b)| ((r, p), q));

            // .decl dead_can_reach(R1, R2, P, Q)
            //
            // Indicates that the region `R1`, which is dead
            // in `Q`, can reach the region `R2` in P.
            //
            // This is effectively the transitive subset
            // relation, but we try to limit it to regions
            // that are dying on the edge P -> Q.
            //
            // dead_can_reach(R1, R2, P, Q) :-
            //   dead_can_reach_origins(R1, P, Q),
            //   subset(R1, R2, P).
            dead_can_reach.from_join(&dead_can_reach_origins, &subset_r1p, |&(r1, p), &q, &r2| {
                (r1, r2, p, q)
            });

            // dead_can_reach(R1, R3, P, Q) :-
            //   dead_can_reach(R1, R2, P, Q),
            //   !region_live_at(R2, Q),
            //   subset(R2, R3, P).
            //
            // This is the "transitive closure" rule, but
            // note that we only apply it with the
            // "intermediate" region R2 is dead at Q.
            dead_can_reach_1.from_antijoin(
                &dead_can_reach_r2q,
                &region_live_at_rel,
                |&(r2, q), &(r1, p)| ((r2, p), (r1, q)),
            );
            dead_can_reach.from_join(
                &dead_can_reach_1,
                &subset_r1p,
                |&(_r2, p), &(r1, q), &r3| (r1, r3, p, q),
            );

            // .decl dead_can_reach_live(R1, R2, P, Q)
            //
            // Indicates that, along the edge `P -> Q`, the
            // dead (in Q) region R1 can reach the live (in Q)
            // region R2 via a subset relation. This is a
            // subset of the full `dead_can_reach` relation
            // where we filter down to those cases where R2 is
            // live in Q.
            dead_can_reach_live.from_join(
                &dead_can_reach_r2q,
                &region_live_at_var,
                |&(r2, q), &(r1, p), &()| ((r1, p, q), r2),
            );

            // subset(R1, R2, Q) :-
            //   subset(R1, R2, P),
            //   cfg_edge(P, Q),
            //   region_live_at(R1, Q),
            //   region_live_at(R2, Q).
            //
            // Carry `R1 <= R2` from P into Q if both `R1` and
            // `R2` are live in Q.
            subset_1.from_join(&subset_p, &cfg_edge, |&_p, &(r1, r2), &q| ((r1, q), r2));
            subset_2.from_join(&subset_1, &region_live_at_var, |&(r1, q), &r2, &()| {
                ((r2, q), r1)
            });
            subset.from_join(&subset_2, &region_live_at_var, |&(r2, q), &r1, &()| {
                (r1, r2, q)
            });

            // subset(R1, R3, Q) :-
            //   live_to_dead_regions(R1, R2, P, Q),
            //   dead_can_reach_live(R2, R3, P, Q).
            subset.from_join(
                &live_to_dead_regions_r2pq,
                &dead_can_reach_live_r1pq,
                |&(_r2, _p, q), &r1, &r3| (r1, r3, q),
            );

            // requires(R2, B, Q) :-
            //   dead_region_requires(R1, B, P, Q),
            //   dead_can_reach_live(R1, R2, P, Q).
            //
            // Communicate a `R1 requires B` relation across
            // an edge `P -> Q` where `R1` is dead in Q; in
            // that case, for each region `R2` live in `Q`
            // where `R1 <= R2` in P, we add `R2 requires B`
            // to `Q`.
            requires.from_join(
                &dead_region_requires,
                &dead_can_reach_live_r1pq,
                |&(_r1, _p, q), &b, &r2| (r2, b, q),
            );

            // requires(R, B, Q) :-
            //   requires(R, B, P),
            //   !killed(B, P),
            //   cfg_edge(P, Q),
            //   region_live_at(R, Q).
            requires_1.from_antijoin(&requires_bp, &killed, |&(b, p), &r| (p, (r, b)));
            requires_2.from_join(&requires_1, &cfg_edge, |&_p, &(r, b), &q| ((r, q), b));
            requires.from_join(&requires_2, &region_live_at_var, |&(r, q), &b, &()| {
                (r, b, q)
            });

            // .decl borrow_live_at(B, P) -- true if the restrictions of the borrow B
            // need to be enforced at the point P
            //
            // borrow_live_at(B, P) :- requires(R, B, P), region_live_at(R, P)
            borrow_live_at.from_join(&requires_rp, &region_live_at_var, |&(_r, p), &b, &()| {
                ((b, p), ())
            });

            // .decl errors(B, P) :- invalidates(B, P), borrow_live_at(B, P).
            errors.from_join(&invalidates, &borrow_live_at, |&(b, p), &(), &()| (b, p));
        }

        if dump_enabled {
            for (region, location) in &region_live_at_rel.elements {
                result
                    .region_live_at
                    .entry(*location)
                    .or_insert(vec![])
                    .push(*region);
            }

            let subset = subset.complete();
            for (r1, r2, location) in &subset.elements {
                result
                    .subset
                    .entry(*location)
                    .or_insert(BTreeMap::new())
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

            let borrow_live_at = borrow_live_at.complete();
            for ((borrow, location), ()) in &borrow_live_at.elements {
                result
                    .borrow_live_at
                    .entry(*location)
                    .or_insert(Vec::new())
                    .push(*borrow);
            }
        }

        errors.complete()
    };

    if dump_enabled {
        println!(
            "errors is complete: {} tuples, {:?}",
            errors.len(),
            timer.elapsed()
        );
    }

    for (borrow, location) in &errors.elements {
        result
            .errors
            .entry(*location)
            .or_insert(Vec::new())
            .push(*borrow);
    }

    result
}
