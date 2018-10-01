use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;

use datafrog::{Iteration, Relation};
use rustc_hash::{FxHashMap, FxHashSet};

/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone)]
pub struct AllFacts<R: Atom, L: Atom, P: Atom> {
    /// `borrow_region(R, B, P)` -- the region R may refer to data
    /// from borrow B starting at the point P (this is usually the
    /// point *after* a borrow rvalue)
    pub borrow_region: Vec<(R, L, P)>,

    /// `universal_region(R)` -- this is a "free region" within fn body
    pub universal_region: Vec<R>,

    /// `cfg_edge(P,Q)` for each edge P -> Q in the control flow
    pub cfg_edge: Vec<(P, P)>,

    /// `killed(B,P)` when some prefix of the path borrowed at B is assigned at point P
    pub killed: Vec<(L, P)>,

    /// `outlives(R1, R2, P)` when we require `R1@P: R2@P`
    pub outlives: Vec<(R, R, P)>,

    /// `region_live_at(R, P)` when the region R appears in a live variable at P
    pub region_live_at: Vec<(R, P)>,

    ///  `invalidates(P, L)` when the loan L is invalidated at point P
    pub invalidates: Vec<(P, L)>,
}

impl<R: Atom, L: Atom, P: Atom> Default for AllFacts<R, L, P> {
    fn default() -> Self {
        AllFacts {
            borrow_region: Vec::default(),
            universal_region: Vec::default(),
            cfg_edge: Vec::default(),
            killed: Vec::default(),
            outlives: Vec::default(),
            region_live_at: Vec::default(),
            invalidates: Vec::default(),
        }
    }
}

impl<R: Atom, L: Atom, P: Atom> AllFacts<R, L, P> {
    /// For the analysis, the critical path of the CFG is along the edges
    /// where liveness changes happen. The most expensive of those being the `subset` transitive
    /// closure, at each point, and between points along the CFG edges.
    /// Only a subset of the CFG is interesting for the `subset` relation, and we can compress
    /// the other points away. First: compute the set of important edges where subsets change.
    /// Then: compress the other edges by contracting the superfluous vertices.
    /// If in an edge e1(p, q) where there is no possible subset change:
    /// - `q` has only one parent: `p`
    /// - `p` has only one child: `q`
    /// - `q` and `p` have the same set of live regions
    /// Then `p` and `q` are "equivalent" with regards to liveness: all borrows live at `q` will be live at `p`.
    /// If another edge e2(q, r) exists, then the vertex `q` is superflous and can be contracted away,
    /// by merging `r` into e1, leaving a final edge e1(p, r).
    /// Thus, we can compute the analysis on such a compressed graph of edges like e1(p, r)
    /// but when there's a `borrow_live_at` `r`, also emit a `borrow_live_at` for the
    /// compressed point `q` (itself absent from the CFG).
    /// Finally: to output these facts at compressed points, we need to record that these `q -> r`
    /// contractions happened, while taking care of chains of contractions:
    /// as contracting p -> q -> r -> s to p -> s will need to emit `borrow_live_at` facts for
    /// the compressed points `q` and `r` when emitting them for `s`.
    pub fn compress(&mut self) -> FxHashMap<P, P> {
        // Step 1 - Index the CFG to find the eligible points matching the requirements,
        // and make the sets of live regions at each point easily comparable.
        let mut successors = FxHashMap::default();
        let mut predecessors = FxHashMap::default();

        for &(p, q) in &self.cfg_edge {
            match successors.entry(p) {
                Entry::Vacant(entry) => {
                    entry.insert(Some(q));
                }
                Entry::Occupied(mut entry) => {
                    let successor = entry.get_mut();
                    if successor.is_some() {
                        // There is already a successor: this point can't be compressed
                        *successor = None;
                    }
                }
            }

            match predecessors.entry(q) {
                Entry::Vacant(entry) => {
                    entry.insert(Some(p));
                }

                Entry::Occupied(mut entry) => {
                    let predecessor = entry.get_mut();
                    if predecessor.is_some() {
                        // There is already a predecessor: this point can't be compressed
                        *predecessor = None;
                    }
                }
            }
        }

        let mut region_live_at = FxHashMap::default();
        for &(r, p) in &self.region_live_at {
            region_live_at
                .entry(p)
                .or_insert_with(FxHashSet::default)
                .insert(r);
        }

        // Step 2 - Locate eligible points
        let mut compressed_points = FxHashSet::default();
        let mut compressible_edges = Vec::new();

        for (p, q) in successors.into_iter() {
            // Compressible points have only one outgoing edge.
            //
            // Note: it could be interesting to investigate the impact of contracting acceptable
            // children edges into `p`'s parent : in the graph `(a, b), (b, c), (b, d)`
            // contracting `b` away would require additional constraints between `c`, `d` and `a`.
            // However, in early tests of our datasets, the number of cases where all children edges
            // had the same set of live regions as their parent seemed small (e.g for clap, for 49K total
            // points: 3275 points had more than one child node, with 231 having the same live regions).
            // There might be rules specific to multiple-children cases but they probably aren't the same as the
            // single-child node cases implemented here.

            let q = match q {
                None => continue, // more than one successor
                Some(successor) => successor,
            };

            // Compressible points have only one incoming edge, and as the the point `p` is merged
            // into its parent's edge, the root edge can't be contracted.
            match predecessors.get(&q) {
                None => continue,       // root node
                Some(None) => continue, // more than one predecessor
                Some(Some(_)) => {}
            }

            // Compressible points have the same live regions.
            if region_live_at.get(&p) != region_live_at.get(&q) {
                continue;
            }

            // Note: as only `invalidates` points can be the source of errors, more investigation
            // could be done to see whether there is more compression and filtering opportunities
            // there: the important points will contribute to liveness while not being error
            // sources.

            compressible_edges.push((p, q));
            compressed_points.insert(p);
        }

        // Step 3 - Compute the CFG compression
        // - Propagate the previously located points along compressed edges until reaching the
        //   important point at the end of the chain.
        // - Record these vertex contractions in an equivalence table: the edges from compressed
        //   points to non-compressed points necessary to decompress the live borrows which could
        //   be computed at these important points.
        let equivalence_table = {
            let mut iteration = Iteration::new();

            let compression_table = iteration.variable("compression_table");
            let compressed_edge = iteration.variable_indistinct("compressed_edge");

            let compressed_point = Relation::from(compressed_points.iter().cloned());

            let propagated_edge_q = iteration.variable("propagated_edge_q");
            propagated_edge_q.insert(Relation::from(
                compressible_edges.iter().map(|&(p, q)| (q, p)),
            ));

            compressed_edge.insert(compressible_edges.into());

            while iteration.changed() {
                // propagated_edge(P, Q) :- compressed_edge(P, Q);
                // -> already done at loading time for a static input

                // propagated_edge(P, R) :-
                //     propagated_edge(P, Q),
                //     compressed_edge(Q, R).
                propagated_edge_q
                    .from_join(&propagated_edge_q, &compressed_edge, |&_q, &p, &r| (r, p));

                // compression_table(P, Q) :-
                //     propagated_edge(P, Q),
                //     !compressed_point(Q).
                compression_table
                    .from_antijoin(&propagated_edge_q, &compressed_point, |&q, &p| (p, q));
            }

            let mut equivalence_table = FxHashMap::default();
            for &(p, q) in compression_table.complete().iter() {
                equivalence_table.insert(p, q);
            }

            equivalence_table
        };

        // Step 4 - Contract the CFG according to the edges' contribution to `subset`s:
        // 1) edges between important points: keep them as-is, they contribute to `subset`s
        // 2) edges from compressible points: contract them away, they don't contribute to
        //   `subset`s.
        // 3) edges from an important point to a compressible point: propagated from the source to
        //    its eventual destination (an important point), potentially through a chain of compressible
        //    points.
        //    For example, e1: (p, q), e2 (q, r), e3 (r, s) where `q` and `r` are compressible, and
        //    `q`, `r`, and `s` have equivalent contributions to `subset`s.
        //    - e2 and e3 will be removed by rule #2.
        //    - we keep the same liveness differences by connecting the source and destination
        //      of the chain: (p, s)
        //
        for i in (0..self.cfg_edge.len()).rev() {
            let (p, q) = self.cfg_edge[i];
            if compressed_points.contains(&p) {
                self.cfg_edge.swap_remove(i);
            } else {
                if compressed_points.contains(&q) {
                    self.cfg_edge[i].1 = equivalence_table[&q];
                }
            }
        }

        // Step 5 - Prune facts which are now unneeded: they refer to points which are absent
        // from the CFG, there is no need to take them into account in the analysis.
        self.borrow_region
            .retain(|(_, _, p)| !compressed_points.contains(p));

        self.killed.retain(|(_, p)| !compressed_points.contains(p));

        self.outlives
            .retain(|(_, _, p)| !compressed_points.contains(p));

        self.region_live_at
            .retain(|(_, p)| !compressed_points.contains(p));

        // We're done, and we can do the regular borrow checking now
        equivalence_table
    }
}

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + Debug + Eq + Ord + Hash + 'static
{
    fn index(self) -> usize;
}
