use std::fmt::Debug;
use std::hash::Hash;

/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone, Debug)]
pub struct AllFacts<R: Atom, L: Atom, P: Atom, V: Atom> {
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

    /// `var_used(V, P) when the variable V is used for anything but a drop at point P`
    pub var_used: Vec<(V, P)>,

    /// `var_defined(V, P) when the variable V is overwritten by the point P`
    pub var_defined: Vec<(V, P)>,

    /// `var_used(V, P) when the variable V is used in a drop at point P`
    pub var_drop_used: Vec<(V, P)>,

    /// `var_uses_region(V, R) when the type of V includes the region R`
    pub var_uses_region: Vec<(V, R)>,

    /// `var_drops_region(V, R) when the type of V includes the region R and uses
    /// it when dropping`
    pub var_drops_region: Vec<(V, R)>,

    /// `var_initialized_on_exit(V, P) when the variable `V` is initialized on
    /// exit from point `P` in the program flow.
    pub var_initialized_on_exit: Vec<(V, P)>,
}

impl<R: Atom, L: Atom, P: Atom, V: Atom> Default for AllFacts<R, L, P, V> {
    fn default() -> Self {
        AllFacts {
            borrow_region: Vec::default(),
            universal_region: Vec::default(),
            cfg_edge: Vec::default(),
            killed: Vec::default(),
            outlives: Vec::default(),
            region_live_at: Vec::default(),
            invalidates: Vec::default(),
            var_used: Vec::default(),
            var_defined: Vec::default(),
            var_drop_used: Vec::default(),
            var_uses_region: Vec::default(),
            var_drops_region: Vec::default(),
            var_initialized_on_exit: Vec::default(),
        }
    }
}

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + Debug + Eq + Ord + Hash + 'static
{
    fn index(self) -> usize;
}
