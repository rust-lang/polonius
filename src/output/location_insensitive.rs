// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Timely dataflow runs on its own thread.

use crate::facts::{AllFacts, Point};
use crate::output::Output;
use crate::output::timely_util::populate_args_for_differential_dataflow;
use differential_dataflow::collection::Collection;
use differential_dataflow::operators::*;
use std::collections::BTreeSet;
use std::mem;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use timely;
use timely::dataflow::operators::*;

pub(super) fn compute(dump_enabled: bool, mut all_facts: AllFacts, workers: u32) -> Output {
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

    let result = Arc::new(Mutex::new(Output::new(dump_enabled)));

    // Use a channel to send `all_facts` to one worker (and only one)
    let (tx, rx) = mpsc::channel();
    tx.send(all_facts).unwrap();
    mem::drop(tx);
    let rx = Mutex::new(rx);
    let dataflow_arg = populate_args_for_differential_dataflow(workers);
    timely::execute_from_args(dataflow_arg.into_iter(), {
        let result = result.clone();
        move |worker| {
            // First come, first serve: one worker gets all the facts;
            // the others get empty vectors.
            let my_facts = rx.lock()
                .unwrap()
                .recv()
                .unwrap_or_else(|_| AllFacts::default());

            worker.dataflow::<(), _, _>(|scope| {
                macro_rules! let_collections {
                    (let ($($facts:ident,)*) = ..$base:expr;) => {
                        let ($($facts),*) = (
                            $(Collection::<_, _, isize>::new(
                                $base.$facts
                                    .to_stream(scope)
                                    .map(|datum| (datum, Default::default(), 1)),
                            ),)*
                        );
                    }
                }

                let_collections! {
                    let (
                        borrow_region,
                        outlives,
                        region_live_at,
                    ) = ..my_facts;
                }

                // .decl subset(Ra, Rb) -- `R1 <= R2` holds
                //
                // subset(Ra, Rb) :- outlives(Ra, Rb, _P)
                let subset = outlives
                    .map(|(r_a, r_b, _p)| (r_a, r_b))
                    .distinct_total();


                // requires(R, L) :- borrow_region(R, L, _P).
                let requires_base = borrow_region
                    .map(|(r, l, _p)| (r, l))
                    .distinct_total();

                // requires(R2, L) :- requires(R1, L), subset(R1, R2).
                let requires = requires_base.iterate(|requires| {
                    let subset = subset.enter(&requires.scope());
                    let requires_base = requires_base.enter(&requires.scope());

                    let requires1 = requires_base.clone();

                    let requires2 = requires
                        .join(&subset)
                        .map(|(_r1, l, r2)| (r2, l));

                    requires1.concat(&requires2).distinct_total()
                });

                // borrow_live_at(L, P) :-
                //     requires(R, L), region_live_at(R, P)
                let borrow_live_at = requires
                    .join(&region_live_at)
                    .map(|(_r, l, p)| (l, p))
                    .distinct_total();

                if dump_enabled {
                    region_live_at.inspect_batch({
                        let result = result.clone();
                        move |_timestamp, facts| {
                            let mut result = result.lock().unwrap();
                            for ((region, location), _timestamp, multiplicity) in facts {
                                assert_eq!(*multiplicity, 1);
                                result
                                    .region_live_at
                                    .entry(*location)
                                    .or_insert(vec![])
                                    .push(*region);
                            }
                        }
                    });

                    subset.inspect_batch({
                        let result = result.clone();
                        move |_timestamp, facts| {
                            let mut result = result.lock().unwrap();
                            for ((r1, r2), _timestamp, multiplicity) in facts {
                                assert_eq!(*multiplicity, 1);
                                result
                                    .subset_anywhere
                                    .entry(*r1)
                                    .or_insert(BTreeSet::new())
                                    .insert(*r2);
                            }
                        }
                    });

                    requires.inspect_batch({
                        let result = result.clone();
                        move |_timestamp, facts| {
                            let mut result = result.lock().unwrap();
                            for ((region, borrow), _timestamp, multiplicity) in facts {
                                assert_eq!(*multiplicity, 1);
                                result
                                    .restricts_anywhere
                                    .entry(*region)
                                    .or_insert(BTreeSet::new())
                                    .insert(*borrow);
                            }
                        }
                    });
                }

                borrow_live_at.inspect_batch({
                    let result = result.clone();
                    move |_timestamp, facts| {
                        let mut result = result.lock().unwrap();
                        for ((borrow, location), _timestamp, multiplicity) in facts {
                            assert_eq!(*multiplicity, 1);
                            result
                                .borrow_live_at
                                .entry(*location)
                                .or_insert(Vec::new())
                                .push(*borrow);
                        }
                    }
                });
            });
        }
    }).unwrap();

    // Remove from the Arc and Mutex, since it is fully populated now.
    Arc::try_unwrap(result)
        .unwrap_or_else(|_| panic!("somebody still has a handle to this arc"))
        .into_inner()
        .unwrap()
}
