use super::{Computation, Dump};
use crate::FactTypes;

use datafrog::{Iteration, Relation, RelationLeaper};

// Step 1: compute transitive closures of path operations. This would elaborate,
// for example, an access to x into an access to x.f, x.f.0, etc. We do this for:
// - access to a path
// - initialization of a path
// - moves of a path
// FIXME: transitive rooting in a variable (path_begins_with_var)
// Note that this step may not be entirely necessary!
#[derive(Clone, Copy)]
pub struct Paths;

input! {
    BasePaths {
        child_path,
        path_is_var,
        path_moved_at_base,
        path_assigned_at_base,
        path_accessed_at_base,
    }
}

output! {
    TransitivePaths {
        path_moved_at,
        path_assigned_at,
        path_accessed_at,
        path_begins_with_var
    }
}

impl<T: FactTypes> Computation<T> for Paths {
    type Input<'db> = BasePaths<'db, T>;
    type Output = TransitivePaths<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let BasePaths {
            child_path,
            path_is_var,
            path_moved_at_base,
            path_assigned_at_base,
            path_accessed_at_base,
        } = input;

        let mut iteration = Iteration::new();

        let ancestor_path = iteration.variable::<(T::Path, T::Path)>("ancestor");

        // These are the actual targets:
        let path_moved_at = iteration.variable::<(T::Path, T::Point)>("path_moved_at");
        let path_assigned_at = iteration.variable::<(T::Path, T::Point)>("path_initialized_at");
        let path_accessed_at = iteration.variable::<(T::Path, T::Point)>("path_accessed_at");
        let path_begins_with_var =
            iteration.variable::<(T::Path, T::Variable)>("path_begins_with_var");

        // ancestor_path(Parent, Child) :- child_path(Child, Parent).
        ancestor_path.extend(child_path.iter().map(|&(child, parent)| (parent, child)));

        // path_moved_at(Path, Point) :- path_moved_at_base(Path, Point).
        path_moved_at.insert(path_moved_at_base.clone());

        // path_assigned_at(Path, Point) :- path_assigned_at_base(Path, Point).
        path_assigned_at.insert(path_assigned_at_base.clone());

        // path_accessed_at(Path, Point) :- path_accessed_at_base(Path, Point).
        path_accessed_at.insert(path_accessed_at_base.clone());

        // path_begins_with_var(Path, Var) :- path_is_var(Path, Var).
        path_begins_with_var.insert(path_is_var.clone());

        while iteration.changed() {
            // ancestor_path(Grandparent, Child) :-
            //    ancestor_path(Parent, Child),
            //    child_path(Parent, Grandparent).
            ancestor_path.from_join(
                &ancestor_path,
                child_path,
                |&_parent, &child, &grandparent| (grandparent, child),
            );

            // moving a path moves its children
            // path_moved_at(Child, Point) :-
            //     path_moved_at(Parent, Point),
            //     ancestor_path(Parent, Child).
            path_moved_at.from_join(&path_moved_at, &ancestor_path, |&_parent, &p, &child| {
                (child, p)
            });

            // initialising x at p initialises all x:s children
            // path_assigned_at(Child, point) :-
            //     path_assigned_at(Parent, point),
            //     ancestor_path(Parent, Child).
            path_assigned_at.from_join(
                &path_assigned_at,
                &ancestor_path,
                |&_parent, &p, &child| (child, p),
            );

            // accessing x at p accesses all x:s children at p (actually,
            // accesses should be maximally precise and this shouldn't happen?)
            // path_accessed_at(Child, point) :-
            //   path_accessed_at(Parent, point),
            //   ancestor_path(Parent, Child).
            path_accessed_at.from_join(
                &path_accessed_at,
                &ancestor_path,
                |&_parent, &p, &child| (child, p),
            );

            // path_begins_with_var(Child, Var) :-
            //   path_begins_with_var(Parent, Var)
            //   ancestor_path(Parent, Child).
            path_begins_with_var.from_join(
                &path_begins_with_var,
                &ancestor_path,
                |&_parent, &var, &child| (child, var),
            );
        }

        Self::Output {
            path_assigned_at: path_assigned_at.complete(),
            path_moved_at: path_moved_at.complete(),
            path_accessed_at: path_accessed_at.complete(),
            path_begins_with_var: path_begins_with_var.complete(),
        }
    }
}

input! {
    TransitivePathsAndCfg {
        cfg_edge,
        path_moved_at,
        path_assigned_at,
    }
}

#[derive(Clone, Copy)]
pub struct MaybeInit;

output!(path_maybe_initialized_on_exit);

impl<T: FactTypes> Computation<T> for MaybeInit {
    type Input<'db> = TransitivePathsAndCfg<'db, T>;
    type Output = PathMaybeInitializedOnExit<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let TransitivePathsAndCfg {
            cfg_edge,
            path_moved_at,
            path_assigned_at,
        } = input;

        let mut iteration = Iteration::new();

        // path_maybe_initialized_on_exit(path, point): Upon leaving `point`, the
        // move path `path` is initialized for some path through the CFG.
        let path_maybe_initialized_on_exit =
            iteration.variable::<(T::Path, T::Point)>("path_maybe_initialized_on_exit");

        // path_maybe_initialized_on_exit(path, point) :- path_assigned_at(path, point).
        path_maybe_initialized_on_exit.insert(path_assigned_at.clone());

        while iteration.changed() {
            // path_maybe_initialized_on_exit(path, point2) :-
            //     path_maybe_initialized_on_exit(path, point1),
            //     cfg_edge(point1, point2),
            //     !path_moved_at(path, point2).
            path_maybe_initialized_on_exit.from_leapjoin(
                &path_maybe_initialized_on_exit,
                (
                    cfg_edge.extend_with(|&(_path, point1)| point1),
                    path_moved_at.extend_anti(|&(path, _point1)| path),
                ),
                |&(path, _point1), &point2| (path, point2),
            );
        }

        path_maybe_initialized_on_exit.complete().into()
    }
}

#[derive(Clone, Copy)]
pub struct MaybeUninit;

output!(path_maybe_uninitialized_on_exit);

impl<T: FactTypes> Computation<T> for MaybeUninit {
    type Input<'db> = TransitivePathsAndCfg<'db, T>;
    type Output = PathMaybeUninitializedOnExit<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let TransitivePathsAndCfg {
            cfg_edge,
            path_moved_at,
            path_assigned_at,
        } = input;

        let mut iteration = Iteration::new();

        // path_maybe_uninitialized_on_exit(Path, Point): There exists at least one
        // path through the CFG to Point such that `Path` has been moved out by the
        // time we arrive at `Point` without it being re-initialized for sure.
        let path_maybe_uninitialized_on_exit =
            iteration.variable::<(T::Path, T::Point)>("path_maybe_uninitialized_on_exit");

        // path_maybe_uninitialized_on_exit(path, point) :- path_moved_at(path, point).
        path_maybe_uninitialized_on_exit.insert(path_moved_at.clone());

        while iteration.changed() {
            // path_maybe_uninitialized_on_exit(path, point2) :-
            //     path_maybe_uninitialized_on_exit(path, point1),
            //     cfg_edge(point1, point2)
            //     !path_assigned_at(path, point2).
            path_maybe_uninitialized_on_exit.from_leapjoin(
                &path_maybe_uninitialized_on_exit,
                (
                    cfg_edge.extend_with(|&(_path, point1)| point1),
                    path_assigned_at.extend_anti(|&(path, _point1)| path),
                ),
                |&(path, _point1), &point2| (path, point2),
            );
        }

        path_maybe_uninitialized_on_exit.complete().into()
    }
}

#[derive(Clone, Copy)]
pub struct VarDroppedWhileInit;

input! {
    VarDroppedWhileInitInput {
        var_dropped_at,
        path_maybe_initialized_on_exit,
        path_begins_with_var,
    }
}

output!(var_dropped_while_init_at);

impl<T: FactTypes> Computation<T> for VarDroppedWhileInit {
    type Input<'db> = VarDroppedWhileInitInput<'db, T>;
    type Output = VarDroppedWhileInitAt<T>;

    fn compute(&self, input: Self::Input<'_>, dump: &mut Dump<'_>) -> Self::Output {
        let VarDroppedWhileInitInput {
            path_begins_with_var,
            path_maybe_initialized_on_exit,
            var_dropped_at,
        } = input;

        // var_maybe_partly_initialized_on_exit(var, point): Upon leaving `point`,
        // `var` is partially initialized for some path through the CFG, that is
        // there has been an initialization of var, and var has not been moved in
        // all paths through the CFG.
        //
        // var_maybe_partly_initialized_on_exit(var, point) :-
        //     path_maybe_initialized_on_exit(path, point).
        //     path_begins_with_var(path, var).
        let var_maybe_partly_initialized_on_exit = Relation::from_join(
            path_maybe_initialized_on_exit,
            path_begins_with_var,
            |_path, &point, &var| (var, point),
        );

        let var_dropped_while_init_at = Relation::from_join(
            &var_maybe_partly_initialized_on_exit,
            var_dropped_at,
            |&var, &point, _point| (var, point),
        );

        dump.rel(
            "var_maybe_partly_initialized_on_exit",
            var_maybe_partly_initialized_on_exit,
        );

        var_dropped_while_init_at.into()
    }
}

#[derive(Clone, Copy)]
pub struct MoveError;

input! {
    MoveErrorInput {
        cfg_edge,
        path_maybe_uninitialized_on_exit,
        path_accessed_at,
    }
}

output!(move_errors);

impl<T: FactTypes> Computation<T> for MoveError {
    type Input<'db> = MoveErrorInput<'db, T>;
    type Output = MoveErrors<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let MoveErrorInput {
            cfg_edge,
            path_maybe_uninitialized_on_exit,
            path_accessed_at,
        } = input;

        // move_error(Path, Point): There is an access to `Path` at `Point`, but
        // `Path` is potentially moved (or never initialised).
        //
        // move_error(Path, TargetNode) :-
        //   path_maybe_uninitialized_on_exit(Path, SourceNode),
        //   cfg_edge(SourceNode, TargetNode),
        //   path_accessed_at(Path, TargetNode).
        let move_errors = Relation::from_leapjoin(
            path_maybe_uninitialized_on_exit,
            (
                cfg_edge.extend_with(|&(_path, source_node)| source_node),
                path_accessed_at.extend_with(|&(path, _source_node)| path),
            ),
            |&(path, _source_node), &target_node| (path, target_node),
        );

        move_errors.into()
    }
}
