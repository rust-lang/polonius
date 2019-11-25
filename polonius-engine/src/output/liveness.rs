// Copyright 2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An implementation of the origin liveness calculation logic

use std::collections::BTreeSet;
use std::time::Instant;

use crate::facts::FactTypes;
use crate::output::{LivenessContext, Output};

use datafrog::{Iteration, Relation, RelationLeaper};

pub(super) fn compute_live_regions<T: FactTypes>(
    ctx: LivenessContext<T>,
    cfg_edge_rel: &Relation<(T::Point, T::Point)>,
    var_maybe_initialized_on_exit_rel: Relation<(T::Variable, T::Point)>,
    output: &mut Output<T>,
) -> Vec<(T::Origin, T::Point)> {
    let timer = Instant::now();
    let mut iteration = Iteration::new();

    // Relations
    let var_defined_rel: Relation<(T::Variable, T::Point)> = ctx.var_defined.into();
    let cfg_edge_reverse_rel: Relation<(T::Point, T::Point)> = cfg_edge_rel
        .iter()
        .map(|&(point1, point2)| (point2, point1))
        .collect();
    let var_uses_region_rel: Relation<(T::Variable, T::Origin)> = ctx.var_uses_region.into();
    let var_drops_region_rel: Relation<(T::Variable, T::Origin)> = ctx.var_drops_region.into();
    let var_drop_used_rel: Relation<((T::Variable, T::Point), ())> = ctx
        .var_drop_used
        .into_iter()
        .map(|(var, point)| ((var, point), ()))
        .collect();

    // Variables

    // `var_live`: variable `var` is live upon entry at `point`
    let var_live_var = iteration.variable::<(T::Variable, T::Point)>("var_live_at");
    // `var_drop_live`: variable `var` is drop-live (will be used for a drop) upon entry in `point`
    let var_drop_live_var = iteration.variable::<(T::Variable, T::Point)>("var_drop_live_at");

    // This is what we are actually calculating:
    let region_live_at_var = iteration.variable::<(T::Origin, T::Point)>("region_live_at");

    // This propagates the relation `var_live(var, point) :- var_used(var, point)`:
    var_live_var.insert(ctx.var_used.into());

    // var_maybe_initialized_on_entry(var, point2) :-
    //     var_maybe_initialized_on_exit(var, point1),
    //     cfg_edge(point1, point2).
    let var_maybe_initialized_on_entry = Relation::from_leapjoin(
        &var_maybe_initialized_on_exit_rel,
        cfg_edge_rel.extend_with(|&(_var, point1)| point1),
        |&(var, _point1), &point2| ((var, point2), ()),
    );

    // var_drop_live(var, point) :-
    //     var_drop_used(var, point),
    //     var_maybe_initialzed_on_entry(var, point).
    var_drop_live_var.insert(Relation::from_join(
        &var_drop_used_rel,
        &var_maybe_initialized_on_entry,
        |&(var, point), _, _| (var, point),
    ));

    while iteration.changed() {
        // region_live_at(origin, point) :-
        //   var_drop_live(var, point),
        //   var_drops_region(var, origin).
        region_live_at_var.from_join(
            &var_drop_live_var,
            &var_drops_region_rel,
            |_var, &point, &origin| (origin, point),
        );

        // region_live_at(origin, point) :-
        //   var_live(var, point),
        //   var_uses_region(var, origin).
        region_live_at_var.from_join(
            &var_live_var,
            &var_uses_region_rel,
            |_var, &point, &origin| (origin, point),
        );

        // var_live(var, point1) :-
        //     var_live(var, point2),
        //     cfg_edge(point1, point2),
        //     !var_defined(var, point1).
        var_live_var.from_leapjoin(
            &var_live_var,
            (
                var_defined_rel.extend_anti(|&(var, _point2)| var),
                cfg_edge_reverse_rel.extend_with(|&(_var, point2)| point2),
            ),
            |&(var, _point2), &point1| (var, point1),
        );

        // var_drop_live(var, point1) :-
        //     var_drop_live(var, point2),
        //     cfg_edge(point1, point2),
        //     !var_defined(var, point1)
        //     var_maybe_initialized_on_exit(var, point1).
        //
        // Extend `point1` with `var:s` from `point2` such that `var` is not in `point2`,
        // there is an edge from `point1` to `point2`
        var_drop_live_var.from_leapjoin(
            &var_drop_live_var,
            (
                var_defined_rel.extend_anti(|&(var, _point2)| var),
                cfg_edge_reverse_rel.extend_with(|&(_var, point2)| point2),
                var_maybe_initialized_on_exit_rel.extend_with(|&(var, _point2)| var),
            ),
            |&(var, _point2), &point1| (var, point1),
        );
    }

    let region_live_at = region_live_at_var.complete();

    info!(
        "compute_live_regions() completed: {} tuples, {:?}",
        region_live_at.len(),
        timer.elapsed(),
    );

    if output.dump_enabled {
        let var_drop_live_at = var_drop_live_var.complete();
        for &(var, location) in var_drop_live_at.iter() {
            output
                .var_drop_live_at
                .entry(location)
                .or_default()
                .push(var);
        }

        let var_live_at = var_live_var.complete();
        for &(var, location) in var_live_at.iter() {
            output.var_live_at.entry(location).or_default().push(var);
        }
    }

    region_live_at.elements
}

pub(super) fn make_universal_regions_live<T: FactTypes>(
    region_live_at: &mut Vec<(T::Origin, T::Point)>,
    cfg_node: &BTreeSet<T::Point>,
    universal_regions: &[T::Origin],
) {
    debug!("make_universal_regions_live()");

    region_live_at.reserve(universal_regions.len() * cfg_node.len());
    for &origin in universal_regions.iter() {
        for &point in cfg_node.iter() {
            region_live_at.push((origin, point));
        }
    }
}
