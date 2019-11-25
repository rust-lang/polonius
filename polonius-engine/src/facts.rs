use std::fmt::Debug;
use std::hash::Hash;

/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone, Debug)]
pub struct AllFacts<T: FactTypes> {
    /// `borrow_region(origin, loan, point)` -- the `origin` may refer to data
    /// from `loan` starting at `point` (this is usually the
    /// point *after* a borrow rvalue)
    pub borrow_region: Vec<(T::Origin, T::Loan, T::Point)>,

    /// `universal_region(origin)` -- this is a "free region" within fn body
    pub universal_region: Vec<T::Origin>,

    /// `cfg_edge(point1, point2)` for each edge `point1 -> point2` in the control flow
    pub cfg_edge: Vec<(T::Point, T::Point)>,

    /// `killed(loan, point)` when some prefix of the path borrowed at `loan` is assigned at `point`
    pub killed: Vec<(T::Loan, T::Point)>,

    /// `outlives(origin1, origin2, point)` when we require `origin1@point: origin2@point`
    pub outlives: Vec<(T::Origin, T::Origin, T::Point)>,

    /// `invalidates(point, loan)` when the `loan` is invalidated at `point`
    pub invalidates: Vec<(T::Point, T::Loan)>,

    /// `var_used(var, point)` when the variable `var` is used for anything but a drop at `point`
    pub var_used: Vec<(T::Variable, T::Point)>,

    /// `var_defined(var, point)` when the variable `var` is overwritten at `point`
    pub var_defined: Vec<(T::Variable, T::Point)>,

    /// `var_used(var, point)` when the variable `var` is used in a drop at `point`
    pub var_drop_used: Vec<(T::Variable, T::Point)>,

    /// `var_uses_region(var, origin)` when the type of `var` includes the `origin`
    pub var_uses_region: Vec<(T::Variable, T::Origin)>,

    /// `var_drops_region(var, origin)` when the type of `var` includes the `origin` and uses
    /// it when dropping
    pub var_drops_region: Vec<(T::Variable, T::Origin)>,

    /// `child(path1, path2)` when the path `path1` is the direct or transitive child
    /// of `path2`, e.g. `child(x.y, x)`, `child(x.y.z, x.y)`, `child(x.y.z, x)`
    /// would all be true if there was a path like `x.y.z`.
    pub child: Vec<(T::Path, T::Path)>,

    /// `path_belongs_to_var(path, var)` the root path `path` starting in variable `var`.
    pub path_belongs_to_var: Vec<(T::Path, T::Variable)>,

    /// `initialized_at(path, point)` when the `path` was initialized at point
    /// `point`. This fact is only emitted for a prefix `path`, and not for the
    /// implicit initialization of all of `path`'s children. E.g. a statement like
    /// `x.y = 3` at `point` would give the fact `initialized_at(x.y, point)` (but
    /// neither `initialized_at(x.y.z, point)` nor `initialized_at(x, point)`).
    pub initialized_at: Vec<(T::Path, T::Point)>,

    /// `moved_out_at(path, point)` when the `path` was moved at `point`. The
    /// same logic is applied as for `initialized_at` above.
    pub moved_out_at: Vec<(T::Path, T::Point)>,

    /// `path_accessed_at(path, point)` when the `path` was accessed at point
    /// `point`. The same logic as for `initialized_at` and `moved_out_at` applies.
    pub path_accessed_at: Vec<(T::Path, T::Point)>,

    /// These reflect the `'a: 'b` relations that are either declared by the user on function
    /// declarations or which are inferred via implied bounds.
    /// For example: `fn foo<'a, 'b: 'a, 'c>(x: &'c &'a u32)` would have two entries:
    /// - one for the user-supplied subset `'b: 'a`
    /// - and one for the `'a: 'c` implied bound from the `x` parameter,
    /// (note that the transitive relation `'b: 'c` is not necessarily included
    /// explicitly, but rather inferred by polonius).
    pub known_subset: Vec<(T::Origin, T::Origin)>,

    /// `placeholder(origin, loan)` describes a placeholder `origin`, with its associated
    ///  placeholder `loan`.
    pub placeholder: Vec<(T::Origin, T::Loan)>,
}

impl<T: FactTypes> Default for AllFacts<T> {
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
            known_subset: Vec::default(),
            placeholder: Vec::default(),
        }
    }
}

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + Debug + Eq + Ord + Hash + 'static
{
    fn index(self) -> usize;
}

pub trait FactTypes: Copy + Clone + Debug {
    type Origin: Atom;
    type Loan: Atom;
    type Point: Atom;
    type Variable: Atom;
    type Path: Atom;
}
