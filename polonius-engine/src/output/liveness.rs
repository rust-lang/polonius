// Copyright 2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An implementation of the region liveness calculation logic

use std::collections::BTreeSet;
use std::time::Instant;

use crate::output::Output;
use facts::Atom;

use datafrog::{Iteration, Relation, RelationLeaper};

pub(super) fn compute_live_regions<Region: Atom, Loan: Atom, Point: Atom, Variable: Atom>(
    var_used: Vec<(Variable, Point)>,
    var_drop_used: Vec<(Variable, Point)>,
    var_defined: Vec<(Variable, Point)>,
    var_uses_region: Vec<(Variable, Region)>,
    var_drops_region: Vec<(Variable, Region)>,
    cfg_edge: &[(Point, Point)],
    var_initialized_on_exit: Vec<(Variable, Point)>,
    output: &mut Output<Region, Loan, Point, Variable>,
) -> Vec<(Region, Point)> {
    debug!("compute_liveness()");
    let computation_start = Instant::now();
    let mut iteration = Iteration::new();

    // Relations
    let var_defined_rel: Relation<(Variable, Point)> = var_defined.into();
    let cfg_edge_reverse_rel: Relation<(Point, Point)> =
        cfg_edge.iter().map(|(p, q)| (*q, *p)).collect();
    let var_uses_region_rel: Relation<(Variable, Region)> = var_uses_region.into();
    let var_drops_region_rel: Relation<(Variable, Region)> = var_drops_region.into();
    let var_initialized_on_exit_rel: Relation<(Variable, Point)> = var_initialized_on_exit.into();

    // Variables

    // `var_live`: variable V is live upon entry in point P
    let var_live_var = iteration.variable::<(Variable, Point)>("var_live_at");
    // `var_drop_live`: variable V is drop-live (will be used for a drop) upon entry in point P
    let var_drop_live_var = iteration.variable::<(Variable, Point)>("var_drop_live_at");

    // This is what we are actually calculating:
    let region_live_at_var = iteration.variable::<((Region, Point), ())>("region_live_at");

    // This propagates the relation `var_live(V, P) :- var_used(V, P)`:
    var_live_var.insert(var_used.into());

    // This propagates the relation `var_drop_live(V, P) :- var_drop_used(V, P)`:
    var_drop_live_var.insert(var_drop_used.into());

    while iteration.changed() {
        // region_live_at(R, P) :-
        //   var_drop_live(V, P),
        //   var_drops_region(V, R).
        region_live_at_var.from_join(&var_drop_live_var, &var_drops_region_rel, |_v, &p, &r| {
            ((r, p), ())
        });

        // region_live_at(R, P) :-
        //   var_live(V, P),
        //   var_uses_region(V, R).
        region_live_at_var.from_join(&var_live_var, &var_uses_region_rel, |_v, &p, &r| {
            ((r, p), ())
        });

        // var_live(V, P) :-
        //     var_live(V, Q),
        //     cfg_edge(P, Q),
        //     !var_defined(V, P).
        // extend p with v:s from q such that v is not in q, there is an edge from p to q
        var_live_var.from_leapjoin(
            &var_live_var,
            (
                var_defined_rel.extend_anti(|&(v, _q)| v),
                cfg_edge_reverse_rel.extend_with(|&(_v, q)| q),
            ),
            |&(v, _q), &p| (v, p),
        );

        // var_drop_live(V, P) :-
        //     var_drop_live(V, Q),
        //     cfg_edge(P, Q),
        //     !var_defined(V, P)
        //     var_initialized_on_exit(V, P).
        // extend p with v:s from q such that v is not in q, there is an edge from p to q
        var_drop_live_var.from_leapjoin(
            &var_drop_live_var,
            (
                var_defined_rel.extend_anti(|&(v, _q)| v),
                cfg_edge_reverse_rel.extend_with(|&(_v, q)| q),
                var_initialized_on_exit_rel.extend_with(|&(v, _q)| v),
            ),
            |&(v, _q), &p| (v, p),
        );
    }

    let region_live_at_rel = region_live_at_var.complete();

    info!(
        "compute_liveness() completed: {} tuples, {:?}",
        region_live_at_rel.len(),
        computation_start.elapsed()
    );

    if output.dump_enabled {
        let var_drop_live_at = var_drop_live_var.complete();
        for &(var, location) in &var_drop_live_at.elements {
            output
                .var_drop_live_at
                .entry(location)
                .or_insert_with(Vec::new)
                .push(var);
        }

        let var_live_at = var_live_var.complete();
        for &(var, location) in &var_live_at.elements {
            output
                .var_live_at
                .entry(location)
                .or_insert_with(Vec::new)
                .push(var);
        }
    }

    region_live_at_rel
        .iter()
        .map(|&((r, p), _)| (r, p))
        .collect()
}

pub(super) fn make_universal_region_live<Region: Atom, Point: Atom>(
    region_live_at: &mut Vec<(Region, Point)>,
    cfg_edge: &[(Point, Point)],
    universal_region: Vec<Region>,
) {
    debug!("make_universal_regions_live()");

    let all_points: BTreeSet<Point> = cfg_edge
        .iter()
        .map(|&(p, _)| p)
        .chain(cfg_edge.iter().map(|&(_, q)| q))
        .collect();

    region_live_at.reserve(universal_region.len() * all_points.len());
    for &r in &universal_region {
        for &p in &all_points {
            region_live_at.push((r, p));
        }
    }
}

pub(super) fn init_region_live_at<Region: Atom, Loan: Atom, Point: Atom, Variable: Atom>(
    var_used: Vec<(Variable, Point)>,
    var_drop_used: Vec<(Variable, Point)>,
    var_defined: Vec<(Variable, Point)>,
    var_uses_region: Vec<(Variable, Region)>,
    var_drops_region: Vec<(Variable, Region)>,
    var_initialized_on_exit: Vec<(Variable, Point)>,
    cfg_edge: &[(Point, Point)],
    region_live_at: Vec<(Region, Point)>,
    universal_region: Vec<Region>,
    output: &mut Output<Region, Loan, Point, Variable>,
) -> Vec<(Region, Point)> {
    debug!("init_region_live_at()");
    let mut region_live_at = if region_live_at.is_empty() {
        debug!("no region_live_at facts provided");
        compute_live_regions(
            var_used,
            var_drop_used,
            var_defined,
            var_uses_region,
            var_drops_region,
            cfg_edge,
            var_initialized_on_exit,
            output,
        )
    } else {
        debug!("using provided region_live_at facts");
        region_live_at
    };

    make_universal_region_live(&mut region_live_at, cfg_edge, universal_region);

    region_live_at
}
