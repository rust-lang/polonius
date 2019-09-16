use std::fmt::Debug;
use std::hash::Hash;

/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone, Debug)]
pub struct AllFacts<Origin: Atom, Loan: Atom, Point: Atom, V: Atom, M: Atom> {
    /// `borrow_region(O, L, P)` -- the origin `O` may refer to data
    /// from loan `L` starting at the point `P` (this is usually the
    /// point *after* a borrow rvalue)
    pub borrow_region: Vec<(Origin, Loan, Point)>,

    /// `universal_region(O)` -- this is a "free region" within fn body
    pub universal_region: Vec<Origin>,

    /// `cfg_edge(P,Q)` for each edge `P -> Q` in the control flow
    pub cfg_edge: Vec<(Point, Point)>,

    /// `killed(L,P)` when some prefix of the path borrowed at `L` is assigned at point `P`
    pub killed: Vec<(Loan, Point)>,

    /// `outlives(O1, P2, P)` when we require `O1@P: O2@P`
    pub outlives: Vec<(Origin, Origin, Point)>,

    ///  `invalidates(P, L)` when the loan `L` is invalidated at point `P`
    pub invalidates: Vec<(Point, Loan)>,

    /// `var_used(V, P) when the variable V is used for anything but a drop at point P`
    pub var_used: Vec<(V, Point)>,

    /// `var_defined(V, P) when the variable V is overwritten by the point P`
    pub var_defined: Vec<(V, Point)>,

    /// `var_used(V, P) when the variable V is used in a drop at point P`
    pub var_drop_used: Vec<(V, Point)>,

    /// `var_uses_region(V, O)` when the type of V includes the origin `O`
    pub var_uses_region: Vec<(V, Origin)>,

    /// `var_drops_region(V, O)` when the type of V includes the origin `O` and uses
    /// it when dropping
    pub var_drops_region: Vec<(V, Origin)>,

    /// `child(M1, M2) when the move path `M1` is the direct or transitive child
    /// of `M2`, e.g. `child(x.y, x)`, `child(x.y.z, x.y)`, `child(x.y.z, x)`
    /// would all be true if there was a path like `x.y.z`.
    pub child: Vec<(M, M)>,

    /// `path_belongs_to_var(M, V) the root path `M` starting in variable `V`.
    pub path_belongs_to_var: Vec<(M, V)>,

    /// `initialized_at(M, P) when the move path `M` was initialized at point
    /// `P`. This fact is only emitted for a prefix `M`, and not for the
    /// implicit initialization of all of `M`'s children. E.g. a statement like
    /// `x.y = 3` at point `P` would give the fact `initialized_at(x.y, P)` (but
    /// neither `initialized_at(x.y.z, P)` nor `initialized_at(x, P)`).
    pub initialized_at: Vec<(M, Point)>,

    /// `moved_out_at(M, P) when the move path `M` was moved at point `P`. The
    /// same logic is applied as for `initialized_at` above.
    pub moved_out_at: Vec<(M, Point)>,

    /// `path_accessed_at(M, P) when the move path `M` was accessed at point
    /// `P`. The same logic as for `initialized_at` and `moved_out_at` applies.
    pub path_accessed_at: Vec<(M, Point)>,
}

impl<Origin: Atom, Loan: Atom, Point: Atom, V: Atom, M: Atom> Default for AllFacts<Origin, Loan, Point, V, M> {
    fn default() -> Self {
        AllFacts {
            borrow_region: Vec::default(),
            universal_region: Vec::default(),
            cfg_edge: Vec::default(),
            killed: Vec::default(),
            outlives: Vec::default(),
            invalidates: Vec::default(),
            var_used: Vec::default(),
            var_defined: Vec::default(),
            var_drop_used: Vec::default(),
            var_uses_region: Vec::default(),
            var_drops_region: Vec::default(),
            child: Vec::default(),
            path_belongs_to_var: Vec::default(),
            initialized_at: Vec::default(),
            moved_out_at: Vec::default(),
            path_accessed_at: Vec::default(),
        }
    }
}

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + Debug + Eq + Ord + Hash + 'static
{
    fn index(self) -> usize;
}
