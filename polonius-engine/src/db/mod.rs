#[macro_use]
pub(crate) mod io;

pub use self::io::{LoadFrom, StoreTo};

use datafrog::Relation;
use paste::paste;

use crate::FactTypes;

macro_rules! relations {
    ( $( $(#[$meta:meta])* $name:ident : [$($Ty:ident),+ $(,)?] ),* $(,)? ) => {
        /// The "facts" which are the basis of the NLL borrow analysis.
        #[derive(Clone)]
        #[non_exhaustive]
        pub struct Db<T: FactTypes> {
            /// The name of the computation unit that is currently executing.
            ///
            /// Used to print better messages results differ for a single relation.
            pub(crate) curr_unit: &'static str,

            $( $(#[$meta])* pub $name: Option<Relation<($(T::$Ty,)+)>>, )*
        }

        impl<T: FactTypes> Default for Db<T> {
            fn default() -> Self {
                Self {
                    curr_unit: Default::default(),
                    $( $name: Default::default(), )*
                }
            }
        }

        paste!{ $(
            #[allow(unused)]
            pub type [<$name:camel>]<T> = ($(<T as FactTypes>::$Ty,)+);
        )* }
    }
}

relations! {
    /// `loan_issued_at(origin, loan, point)` indicates that the `loan` was "issued"
    /// at the given `point`, creating a reference with the `origin`.
    /// Effectively, `origin` may refer to data from `loan` starting at `point` (this is usually
    /// the point *after* a borrow rvalue).
    loan_issued_at: [Origin, Loan, Point],

    /// `universal_region(origin)` -- this is a "free region" within fn body
    universal_region: [Origin],

    /// `cfg_edge(point1, point2)` for each edge `point1 -> point2` in the control flow
    cfg_edge: [Point, Point],

    /// `cfg_node(point)` for each node that appears as the source *or* target of an edge in the
    /// control flow graph.
    cfg_node: [Point],

    /// `loan_killed_at(loan, point)` when some prefix of the path borrowed at `loan`
    /// is assigned at `point`.
    /// Indicates that the path borrowed by the `loan` has changed in some way that the loan no
    /// longer needs to be tracked. (In particular, mutations to the path that was borrowed
    /// no longer invalidate the loan)
    loan_killed_at: [Loan, Point],

    /// `subset_base(origin1, origin2, point)` when we require `origin1@point: origin2@point`.
    /// Indicates that `origin1 <= origin2` -- i.e., the set of loans in `origin1` are a subset
    /// of those in `origin2`.
    subset_base: [Origin, Origin, Point],

    /// `loan_invalidated_at(loan, point)` indicates that the `loan` is invalidated by some action
    /// taking place at `point`; if any origin that references this loan is live, this is an error.
    loan_invalidated_at: [Loan, Point],

    /// `var_used_at(var, point)` when the variable `var` is used for anything
    /// but a drop at `point`
    var_used_at: [Variable, Point],

    /// `var_defined_at(var, point)` when the variable `var` is overwritten at `point`
    var_defined_at: [Variable, Point],

    /// `var_dropped_at(var, point)` when the variable `var` is used in a drop at `point`
    var_dropped_at: [Variable, Point],

    /// `var_dropped_while_init_at(var, point)` when the variable `var` is used in a drop at
    /// `point` *while it is (maybe) initialized*.
    ///
    /// Drops of variables that are known to be uninit are no-ops, and are ignored by borrowck.
    var_dropped_while_init_at: [Variable, Point],

    /// `use_of_var_derefs_origin(variable, origin)`: References with the given
    /// `origin` may be dereferenced when the `variable` is used.
    ///
    /// In rustc, we generate this whenever the type of the variable includes the
    /// given origin.
    use_of_var_derefs_origin: [Variable, Origin],

    /// `drop_of_var_derefs_origin(var, origin)` when the type of `var` includes
    /// the `origin` and uses it when dropping
    drop_of_var_derefs_origin: [Variable, Origin],

    /// `child_path(child, parent)` when the path `child` is the direct child of
    /// `parent`, e.g. `child_path(x.y, x)`, but not `child_path(x.y.z, x)`.
    child_path: [Path, Path],

    /// `path_is_var(path, var)` the root path `path` starting in variable `var`.
    path_is_var: [Path, Variable],

    /// `path_assigned_at_base(path, point)` when the `path` was initialized at point
    /// `point`. This fact is only emitted for a prefix `path`, and not for the
    /// implicit initialization of all of `path`'s children. E.g. a statement like
    /// `x.y = 3` at `point` would give the fact `path_assigned_at_base(x.y, point)` (but
    /// neither `path_assigned_at_base(x.y.z, point)` nor `path_assigned_at_base(x, point)`).
    path_assigned_at_base: [Path, Point],

    /// `path_moved_at_base(path, point)` when the `path` was moved at `point`. The
    /// same logic is applied as for `path_assigned_at_base` above.
    path_moved_at_base: [Path, Point],

    /// `path_accessed_at_base(path, point)` when the `path` was accessed at point
    /// `point`. The same logic as for `path_assigned_at_base` and `path_moved_at_base` applies.
    path_accessed_at_base: [Path, Point],

    /// These reflect the `'a: 'b` relations that are either declared by the user on function
    /// declarations or which are inferred via implied bounds.
    /// For example: `fn foo<'a, 'b: 'a, 'c>(x: &'c &'a u32)` would have two entries:
    /// - one for the user-supplied subset `'b: 'a`
    /// - and one for the `'a: 'c` implied bound from the `x` parameter,
    /// (note that the transitive relation `'b: 'c` is not necessarily included
    /// explicitly, but rather inferred by polonius).
    known_placeholder_subset: [Origin, Origin],
    known_placeholder_subset_base: [Origin, Origin],

    /// `placeholder(origin, loan)` describes a placeholder `origin`, with its associated
    ///  placeholder `loan`.
    placeholder: [Origin, Loan],

    path_assigned_at: [Path, Point],
    path_moved_at: [Path, Point],
    path_accessed_at: [Path, Point],
    path_begins_with_var: [Path, Variable],

    origin_live_on_entry: [Origin, Point],
    path_maybe_initialized_on_exit: [Path, Point],
    path_maybe_uninitialized_on_exit: [Path, Point],

    errors: [Loan, Point],
    subset_errors: [Origin, Origin, Point],
    move_errors: [Path, Point],

    known_placeholder_requires: [Origin, Loan],

    potential_errors: [Loan, Point],
    potential_subset_errors: [Origin, Origin],
}
