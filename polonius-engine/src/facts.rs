use std::fmt::Debug;
use std::hash::Hash;

/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone, Debug)]
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

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + Debug + Eq + Ord + Hash + 'static
{
    fn index(self) -> usize;
}
