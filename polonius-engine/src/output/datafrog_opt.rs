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

    let errors = {
        // Static inputs
        let origin_live_on_entry_rel = &ctx.origin_live_on_entry;
        let cfg_edge_rel = &ctx.cfg_edge;
        let killed_rel = &ctx.killed;

        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // `invalidates` facts, stored ready for joins
        let invalidates = iteration.variable::<((T::Loan, T::Point), ())>("invalidates");

        // we need `origin_live_on_entry` in both variable and relation forms,
        // (respectively, for join and antijoin).
        let origin_live_on_entry_var =
            iteration.variable::<((T::Origin, T::Point), ())>("origin_live_on_entry");

        // `borrow_region` input but organized for join
        let borrow_region_op =
            iteration.variable::<((T::Origin, T::Point), T::Loan)>("borrow_region_op");

        // .decl subset(origin1, origin2, point)
        //
        // Indicates that `origin1: origin2` at `point`.
        let subset_o1p = iteration.variable::<((T::Origin, T::Point), T::Origin)>("subset_o1p");

        // .decl requires(origin, loan, point)
        //
        // At `point`, things with `origin` may depend on data from `loan`.
        let requires_op = iteration.variable::<((T::Origin, T::Point), T::Loan)>("requires_op");

        // .decl borrow_live_at(loan, point)
        //
        // True if the restrictions of the `loan` need to be enforced at `point`.
        let borrow_live_at = iteration.variable::<((T::Loan, T::Point), ())>("borrow_live_at");

        // .decl live_to_dying_regions(origin1, origin2, point1, point2)
        //
        // The origins `origin1` and `origin2` are "live to dead"
        // on the edge `point1 -> point2` if:
        //
        // - In `point1`, `origin1` <= `origin2`
        // - In `point2`, `origin1` is live but `origin2` is dead.
        //
        // In that case, `point2` would like to add all the
        // live things reachable from `origin2` to `origin1`.
        //
        let live_to_dying_regions_o2pq = iteration
            .variable::<((T::Origin, T::Point, T::Point), T::Origin)>("live_to_dying_regions_o2pq");

        // .decl dying_region_requires((origin, point1, point2), loan)
        //
        // The `origin` requires `loan`, but the `origin` goes dead
        // along the edge `point1 -> point2`.
        let dying_region_requires = iteration
            .variable::<((T::Origin, T::Point, T::Point), T::Loan)>("dying_region_requires");

        // .decl dying_can_reach_origins(origin, point1, point2)
        //
        // Contains dead origins where we are interested
        // in computing the transitive closure of things they
        // can reach.
        //
        // FIXME: this relation was named before renaming the `regions` atoms to `origins`, and
        // will need to be renamed to change "_origins" to "_ascendants", "_roots", etc.
        let dying_can_reach_origins =
            iteration.variable::<((T::Origin, T::Point), T::Point)>("dying_can_reach_origins");

        // .decl dying_can_reach(origin1, origin2, point1, point2)
        //
        // Indicates that `origin1`, which is dead
        // in `point2`, can reach `origin2` in `point1`.
        //
        // This is effectively the transitive subset
        // relation, but we try to limit it to origins
        // that are dying on the edge `point1 -> point2`.
        let dying_can_reach_o2q =
            iteration.variable::<((T::Origin, T::Point), (T::Origin, T::Point))>("dying_can_reach");
        let dying_can_reach_1 = iteration.variable_indistinct("dying_can_reach_1");

        // .decl dying_can_reach_live(origin1, origin2, point1, point2)
        //
        // Indicates that, along the edge `point1 -> point2`, the dead (in `point2`)
        // `origin1` can reach the live (in `point2`) `origin2` via a subset
        // relation. This is a subset of the full `dying_can_reach`
        // relation where we filter down to those cases where `origin2` is
        // live in `point2`.
        let dying_can_reach_live = iteration
            .variable::<((T::Origin, T::Point, T::Point), T::Origin)>("dying_can_reach_live");

        // .decl dead_borrow_region_can_reach_root((origin, point), loan)
        //
        // Indicates a "borrow region" `origin` at `point` which is not live on
        // entry to `point`.
        let dead_borrow_region_can_reach_root = iteration
            .variable::<((T::Origin, T::Point), T::Loan)>("dead_borrow_region_can_reach_root");

        // .decl dead_borrow_region_can_reach_dead((origin2, point), loan)
        let dead_borrow_region_can_reach_dead = iteration
            .variable::<((T::Origin, T::Point), T::Loan)>("dead_borrow_region_can_reach_dead");
        let dead_borrow_region_can_reach_dead_1 =
            iteration.variable_indistinct("dead_borrow_region_can_reach_dead_1");

        // .decl errors(loan, point)
        let errors = iteration.variable("errors");

        // Make "variable" versions of the relations, needed for joins.
        borrow_region_op.extend(
            ctx.borrow_region
                .iter()
                .map(|&(origin, loan, point)| ((origin, point), loan)),
        );
        invalidates.extend(
            ctx.invalidates
                .iter()
                .map(|&(loan, point)| ((loan, point), ())),
        );
        origin_live_on_entry_var.extend(
            origin_live_on_entry_rel
                .iter()
                .map(|&(origin, point)| ((origin, point), ())),
        );

        // subset(origin1, origin2, point) :- outlives(origin1, origin2, point).
        subset_o1p.extend(
            ctx.outlives
                .iter()
                .map(|&(origin1, origin2, point)| ((origin1, point), origin2)),
        );

        // requires(origin, loan, point) :- borrow_region(origin, loan, point).
        requires_op.extend(
            ctx.borrow_region
                .iter()
                .map(|&(origin, loan, point)| ((origin, point), loan)),
        );

        // .. and then start iterating rules!
        while iteration.changed() {
            // Cleanup step: remove symmetries
            // - remove origins which are `subset`s of themselves
            //
            // FIXME: investigate whether is there a better way to do that without complicating
            // the rules too much, because it would also require temporary variables and
            // impact performance. Until then, the big reduction in tuples improves performance
            // a lot, even if we're potentially adding a small number of tuples
            // per round just to remove them in the next round.
            subset_o1p
                .recent
                .borrow_mut()
                .elements
                .retain(|&((origin1, _), origin2)| origin1 != origin2);

            // live_to_dying_regions(origin1, origin2, point1, point2) :-
            //   subset(origin1, origin2, point1),
            //   cfg_edge(point1, point2),
            //   origin_live_on_entry(origin1, point2),
            //   !origin_live_on_entry(origin2, point2).
            live_to_dying_regions_o2pq.from_leapjoin(
                &subset_o1p,
                (
                    cfg_edge_rel.extend_with(|&((_, point1), _)| point1),
                    origin_live_on_entry_rel.extend_with(|&((origin1, _), _)| origin1),
                    origin_live_on_entry_rel.extend_anti(|&((_, _), origin2)| origin2),
                ),
                |&((origin1, point1), origin2), &point2| ((origin2, point1, point2), origin1),
            );

            // dying_region_requires((origin, point1, point2), loan) :-
            //   requires(origin, loan, point1),
            //   !killed(loan, point1),
            //   cfg_edge(point1, point2),
            //   !origin_live_on_entry(origin, point2).
            dying_region_requires.from_leapjoin(
                &requires_op,
                (
                    killed_rel.filter_anti(|&((_, point1), loan)| (loan, point1)),
                    cfg_edge_rel.extend_with(|&((_, point1), _)| point1),
                    origin_live_on_entry_rel.extend_anti(|&((origin, _), _)| origin),
                ),
                |&((origin, point1), loan), &point2| ((origin, point1, point2), loan),
            );

            // dying_can_reach_origins(origin2, point1, point2) :-
            //   live_to_dying_regions(_, origin2, point1, point2).
            dying_can_reach_origins.from_map(
                &live_to_dying_regions_o2pq,
                |&((origin2, point1, point2), _origin1)| ((origin2, point1), point2),
            );

            // dying_can_reach_origins(origin, point1, point2) :-
            //   dying_region_requires(origin, point1, point2, _loan).
            dying_can_reach_origins.from_map(
                &dying_region_requires,
                |&((origin, point1, point2), _loan)| ((origin, point1), point2),
            );

            // dying_can_reach(origin1, origin2, point1, point2) :-
            //   dying_can_reach_origins(origin1, point1, point2),
            //   subset(origin1, origin2, point1).
            dying_can_reach_o2q.from_join(
                &dying_can_reach_origins,
                &subset_o1p,
                |&(origin1, point1), &point2, &origin2| ((origin2, point2), (origin1, point1)),
            );

            // dying_can_reach(origin1, origin3, point1, point2) :-
            //   dying_can_reach(origin1, origin2, point1, point2),
            //   !origin_live_on_entry(origin2, point2),
            //   subset(origin2, origin3, point1).
            //
            // This is the "transitive closure" rule, but
            // note that we only apply it with the
            // "intermediate" `origin2` is dead at `point2`.
            dying_can_reach_1.from_antijoin(
                &dying_can_reach_o2q,
                &origin_live_on_entry_rel,
                |&(origin2, point2), &(origin1, point1)| ((origin2, point1), (origin1, point2)),
            );
            dying_can_reach_o2q.from_join(
                &dying_can_reach_1,
                &subset_o1p,
                |&(_origin2, point1), &(origin1, point2), &origin3| {
                    ((origin3, point2), (origin1, point1))
                },
            );

            // dying_can_reach_live(origin1, origin2, point1, point2) :-
            //   dying_can_reach(origin1, origin2, point1, point2),
            //   origin_live_on_entry(origin2, point2).
            dying_can_reach_live.from_join(
                &dying_can_reach_o2q,
                &origin_live_on_entry_var,
                |&(origin2, point2), &(origin1, point1), _| ((origin1, point1, point2), origin2),
            );

            // subset(origin1, origin2, point2) :-
            //   subset(origin1, origin2, point1),
            //   cfg_edge(point1, point2),
            //   origin_live_on_entry(origin1, point2),
            //   origin_live_on_entry(origin2, point2).
            //
            // Carry `origin1 <= origin2` from `point1` into `point2` if both `origin1` and
            // `origin2` are live in `point2`.
            subset_o1p.from_leapjoin(
                &subset_o1p,
                (
                    cfg_edge_rel.extend_with(|&((_, point1), _)| point1),
                    origin_live_on_entry_rel.extend_with(|&((origin1, _), _)| origin1),
                    origin_live_on_entry_rel.extend_with(|&((_, _), origin2)| origin2),
                ),
                |&((origin1, _point1), origin2), &point2| ((origin1, point2), origin2),
            );

            // subset(origin1, origin3, point2) :-
            //   live_to_dying_regions(origin1, origin2, point1, point2),
            //   dying_can_reach_live(origin2, origin3, point1, point2).
            subset_o1p.from_join(
                &live_to_dying_regions_o2pq,
                &dying_can_reach_live,
                |&(_origin2, _point1, point2), &origin1, &origin3| ((origin1, point2), origin3),
            );

            // requires(origin2, loan, point2) :-
            //   dying_region_requires(origin1, loan, point1, point2),
            //   dying_can_reach_live(origin1, origin2, point1, point2).
            //
            // Communicate a `origin1 requires loan` relation across
            // an edge `point1 -> point2` where `origin1` is dead in `point2`; in
            // that case, for each origin `origin2` live in `point2`
            // where `origin1 <= origin2` in `point1`, we add `origin2 requires loan`
            // to `point2`.
            requires_op.from_join(
                &dying_region_requires,
                &dying_can_reach_live,
                |&(_origin1, _point1, point2), &loan, &origin2| ((origin2, point2), loan),
            );

            // requires(origin, loan, point2) :-
            //   requires(origin, loan, point1),
            //   !killed(loan, point1),
            //   cfg_edge(point1, point2),
            //   origin_live_on_entry(origin, point2).
            requires_op.from_leapjoin(
                &requires_op,
                (
                    killed_rel.filter_anti(|&((_, point1), loan)| (loan, point1)),
                    cfg_edge_rel.extend_with(|&((_, point1), _)| point1),
                    origin_live_on_entry_rel.extend_with(|&((origin, _), _)| origin),
                ),
                |&((origin, _), loan), &point2| ((origin, point2), loan),
            );

            // dead_borrow_region_can_reach_root((origin, point), loan) :-
            //   borrow_region(origin, loan, point),
            //   !origin_live_on_entry(origin, point).
            dead_borrow_region_can_reach_root.from_antijoin(
                &borrow_region_op,
                &origin_live_on_entry_rel,
                |&(origin, point), &loan| ((origin, point), loan),
            );

            // dead_borrow_region_can_reach_dead((origin, point), loan) :-
            //   dead_borrow_region_can_reach_root((origin, point), loan).
            dead_borrow_region_can_reach_dead
                .from_map(&dead_borrow_region_can_reach_root, |&tuple| tuple);

            // dead_borrow_region_can_reach_dead((origin2, point), loan) :-
            //   dead_borrow_region_can_reach_dead(origin1, loan, point),
            //   subset(origin1, origin2, point),
            //   !origin_live_on_entry(origin2, point).
            dead_borrow_region_can_reach_dead_1.from_join(
                &dead_borrow_region_can_reach_dead,
                &subset_o1p,
                |&(_origin1, point), &loan, &origin2| ((origin2, point), loan),
            );
            dead_borrow_region_can_reach_dead.from_antijoin(
                &dead_borrow_region_can_reach_dead_1,
                &origin_live_on_entry_rel,
                |&(origin2, point), &loan| ((origin2, point), loan),
            );

            // borrow_live_at(loan, point) :-
            //   requires(origin, loan, point),
            //   origin_live_on_entry(origin, point).
            borrow_live_at.from_join(
                &requires_op,
                &origin_live_on_entry_var,
                |&(_origin, point), &loan, _| ((loan, point), ()),
            );

            // borrow_live_at(loan, point) :-
            //   dead_borrow_region_can_reach_dead(origin1, loan, point),
            //   subset(origin1, origin2, point),
            //   origin_live_on_entry(origin2, point).
            //
            // NB: the datafrog code below uses
            // `dead_borrow_region_can_reach_dead_1`, which is equal
            // to `dead_borrow_region_can_reach_dead` and `subset`
            // joined together.
            borrow_live_at.from_join(
                &dead_borrow_region_can_reach_dead_1,
                &origin_live_on_entry_var,
                |&(_origin2, point), &loan, _| ((loan, point), ()),
            );

            // errors(loan, point) :-
            //   invalidates(loan, point),
            //   borrow_live_at(loan, point).
            errors.from_join(&invalidates, &borrow_live_at, |&(loan, point), _, _| {
                (loan, point)
            });
        }

        if result.dump_enabled {
            let subset_o1p = subset_o1p.complete();
            assert!(
                subset_o1p
                    .iter()
                    .filter(|&((origin1, _), origin2)| origin1 == origin2)
                    .count()
                    == 0,
                "unwanted subset symmetries"
            );
            for &((origin1, location), origin2) in subset_o1p.iter() {
                result
                    .subset
                    .entry(location)
                    .or_default()
                    .entry(origin1)
                    .or_default()
                    .insert(origin2);
            }

            let requires_op = requires_op.complete();
            for &((origin, location), loan) in requires_op.iter() {
                result
                    .restricts
                    .entry(location)
                    .or_default()
                    .entry(origin)
                    .or_default()
                    .insert(loan);
            }

            let borrow_live_at = borrow_live_at.complete();
            for &((loan, location), _) in borrow_live_at.iter() {
                result
                    .borrow_live_at
                    .entry(location)
                    .or_default()
                    .push(loan);
            }
        }

        errors.complete()
    };

    info!(
        "errors is complete: {} tuples, {:?}",
        errors.len(),
        timer.elapsed()
    );

    errors
}
