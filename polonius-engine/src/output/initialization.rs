use std::time::Instant;

use crate::facts::FactTypes;
use crate::output::{InitializationContext, Output};

use datafrog::{Iteration, Relation, RelationLeaper};

// This represents the output of an intermediate elaboration step (step 1).
struct TransitivePaths<T: FactTypes> {
    moved_out_at: Relation<(T::Path, T::Point)>,
    initialized_at: Relation<(T::Path, T::Point)>,
    accessed_at: Relation<(T::Path, T::Point)>,
}

// This is the output of the intermediate partial initialization computation. It
// over-approximates (computes an upper bound on) the initialization status of
// paths and variables. Additionally, it also does the same for if a variable
// may have been moved.
//
// For example:
// ```rust
// let x = (1, 2); // x, x.1, x.2 maybe initialized
// if random() { move x.0 };
// // x.0 maybe moved, x, x.1, x.2 maybe initialized.
// ````
// Note that var_maybe... only tracks moves of *entire variables*, i.e. root paths.
struct InitializationStatus<T: FactTypes> {
    var_maybe_initialized_on_exit: Relation<(T::Variable, T::Point)>,
    path_maybe_initialized_on_exit: Relation<(T::Path, T::Point)>,
    path_maybe_moved_at: Relation<(T::Path, T::Point)>,
}

// Step 1: compute transitive closures of path operations. This would elaborate,
// for example, an access to x into an access to x.f, x.f.0, etc. We do this for:
// - access to a path
// - initialization of a path
// - moves of a path
// Note that this step may not be entirely necessary!
fn compute_transitive_paths<T: FactTypes>(
    child: Vec<(T::Path, T::Path)>,
    initialized_at: Vec<(T::Path, T::Point)>,
    moved_out_at: Vec<(T::Path, T::Point)>,
    path_accessed_at: Vec<(T::Path, T::Point)>,
) -> TransitivePaths<T> {
    let mut iteration = Iteration::new();
    let child: Relation<(T::Path, T::Path)> = child.into();

    let ancestor_var = iteration.variable::<(T::Path, T::Path)>("ancestor");

    // These are the actual targets:
    let moved_out_at_var = iteration.variable::<(T::Path, T::Point)>("moved_out_at");
    let initialized_at_var = iteration.variable::<(T::Path, T::Point)>("initialized_at");
    let accessed_at_var = iteration.variable::<(T::Path, T::Point)>("accessed_at");

    // ancestor(Mother, Daughter) :- child(Daughter, Mother).
    ancestor_var.insert(
        child
            .iter()
            .map(|&(child_path, parent_path)| (parent_path, child_path))
            .collect(),
    );

    // moved_out_at(Path, Point) :- moved_out_at(path, point).
    moved_out_at_var.insert(moved_out_at.into());

    // initialized_at(Path, Point) :- initialized_at(path, Point).
    initialized_at_var.insert(initialized_at.into());

    // accessed_at(Path, Point) :- path_accessed_at(path, Point).
    accessed_at_var.insert(path_accessed_at.into());

    while iteration.changed() {
        // ancestor(Grandmother, Daughter) :-
        //    ancestor(Mother, Daughter),
        //    child(Mother, Grandmother).
        ancestor_var.from_join(
            &ancestor_var,
            &child,
            |&_mother, &daughter, &grandmother| (grandmother, daughter),
        );

        // moving a path moves its children
        // moved_out_at(Child, Point) :-
        //     moved_out_at(Parent, Point),
        //     ancestor(Parent, Child).
        moved_out_at_var.from_join(&moved_out_at_var, &ancestor_var, |&_parent, &p, &child| {
            (child, p)
        });

        // initialising x at p initialises all x:s children
        // initialized_at(Child, point) :-
        //     initialized_at(Parent, point),
        //     ancestor(Parent, Child).
        initialized_at_var.from_join(
            &initialized_at_var,
            &ancestor_var,
            |&_parent, &p, &child| (child, p),
        );

        // accessing x at p accesses all x:s children at p (actually,
        // accesses should be maximally precise and this shouldn't happen?)
        // accessed_at(Child, point) :-
        //   accessed_at(Parent, point),
        //   ancestor(Parent, Child).
        accessed_at_var.from_join(&accessed_at_var, &ancestor_var, |&_parent, &p, &child| {
            (child, p)
        });
    }

    TransitivePaths {
        initialized_at: initialized_at_var.complete(),
        moved_out_at: moved_out_at_var.complete(),
        accessed_at: accessed_at_var.complete(),
    }
}

// Step 2: Compute path initialization and deinitialization across the CFG.
fn compute_initialization_status<T: FactTypes>(
    path_belongs_to_var: Vec<(T::Path, T::Variable)>,
    moved_out_at: Relation<(T::Path, T::Point)>,
    initialized_at: Relation<(T::Path, T::Point)>,
    cfg_edge: &Relation<(T::Point, T::Point)>,
) -> InitializationStatus<T> {
    let mut iteration = Iteration::new();

    // Relations
    let path_belongs_to_var: Relation<(T::Path, T::Variable)> = path_belongs_to_var.into();

    // Variables

    // var_maybe_initialized_on_exit(var, point): Upon leaving `point`, `var` is
    // initialized for some path through the CFG, that is there has been an
    // initialization of var, and var has not been moved in all paths through
    // the CFG.
    let var_maybe_initialized_on_exit_var =
        iteration.variable::<(T::Variable, T::Point)>("var_maybe_initialized_on_exit");

    // path_maybe_initialized_on_exit(path, point): Upon leaving `point`, the
    // move path `path` is initialized for some path through the CFG.
    let path_maybe_initialized_on_exit_var =
        iteration.variable::<(T::Path, T::Point)>("path_maybe_initialized_on_exit");

    // path_maybe_moved_at(Path, Point): There exists at least one path through
    // the CFG to Point such that `Path` has been moved out by the time we
    // arrive at `Point` without it being re-initialized for sure.
    let path_maybe_moved_at_var = iteration.variable::<(T::Path, T::Point)>("path_maybe_moved_at");

    // Initial propagation of static relations

    // path_maybe_initialized_on_exit(path, point) :- initialized_at(path, point).
    path_maybe_initialized_on_exit_var.insert(initialized_at.clone());

    // path_maybe_moved_at(path, point) :- moved_out_at(path, point).
    path_maybe_moved_at_var.insert(
        moved_out_at
            .iter()
            .map(|&(path, point)| (path, point))
            .collect(),
    );

    while iteration.changed() {
        // path_maybe_initialized_on_exit(path, point2) :-
        //     path_maybe_initialized_on_exit(path, point1),
        //     cfg_edge(point1, point2),
        //     !moved_out_at(path, point2).
        path_maybe_initialized_on_exit_var.from_leapjoin(
            &path_maybe_initialized_on_exit_var,
            (
                cfg_edge.extend_with(|&(_path, point1)| point1),
                moved_out_at.extend_anti(|&(path, _point1)| path),
            ),
            |&(path, _point1), &point2| (path, point2),
        );

        // path_maybe_moved_at(path, point2) :-
        //     path_maybe_moved_at(path, point1),
        //     cfg_edge_(point1, point2)
        //     !initialized_at(point1, point2).
        path_maybe_moved_at_var.from_leapjoin(
            &path_maybe_moved_at_var,
            (
                cfg_edge.extend_with(|&(_path, point1)| point1),
                initialized_at.extend_anti(|&(path, _point1)| path),
            ),
            |&(path, _point1), &point2| (path, point2),
        );

        // var_maybe_initialized_on_exit(var, point) :-
        //     path_belongs_to_var(path, var),
        //     path_maybe_initialized_at(path, point).
        var_maybe_initialized_on_exit_var.from_leapjoin(
            &path_maybe_initialized_on_exit_var,
            path_belongs_to_var.extend_with(|&(path, _point)| path),
            |&(_path, point), &var| (var, point),
        );
    }

    InitializationStatus {
        var_maybe_initialized_on_exit: var_maybe_initialized_on_exit_var.complete(),
        path_maybe_initialized_on_exit: path_maybe_initialized_on_exit_var.complete(),
        path_maybe_moved_at: path_maybe_moved_at_var.complete(),
    }
}

// Step 3: Calculate provably initialised paths. This computes the following
// relation:
//
// path_definitely_initialized_at(Path, Point): Any path through the CFG to
// `Point` has `Path` initialized.
fn compute_known_initialized_paths<T: FactTypes>(
    path_maybe_initialized_on_exit: &Relation<(T::Path, T::Point)>,
    path_maybe_moved_at: Relation<(T::Path, T::Point)>,
) -> Relation<(T::Path, T::Point)> {
    // FIXME: these variables are artificial and requires no iteration. They
    // are just here due to Datafrog limitations.
    let mut iteration = Iteration::new();
    let path_definitely_initialized_at_var =
        iteration.variable::<(T::Path, T::Point)>("path_definitely_initialized_at");
    let path_maybe_initialized_on_exit_var =
        iteration.variable::<((T::Path, T::Point), ())>("path_maybe_initialized_on_exit");

    path_maybe_initialized_on_exit_var.insert(
        path_maybe_initialized_on_exit
            .elements
            .iter()
            .map(|&(path, point)| ((path, point), ()))
            .collect(),
    );

    while iteration.changed() {
        // path_definitely_initialized_at(Path, Point) :-
        //   path_maybe_initialized_on_exit(Path, Point),
        //   !path_maybe_moved_at(Path, Point).
        path_definitely_initialized_at_var.from_antijoin(
            &path_maybe_initialized_on_exit_var,
            &path_maybe_moved_at,
            |&(path, point), &()| (path, point),
        );
    }

    path_definitely_initialized_at_var.complete()
}

// Step 4: Compute erroneous path accesses
fn compute_move_errors<T: FactTypes>(
    accessed_at: Relation<(T::Path, T::Point)>,
    path_definitely_initialized_at: Relation<(T::Path, T::Point)>,
) -> Relation<(T::Path, T::Point)> {
    // FIXME: these variables are artificial and requires no iteration. They
    // are just here due to Datafrog limitations.
    let mut iteration = Iteration::new();

    // move_error(Path, Point): There is an access to `Path` at `Point`, but
    // `Path` is potentially moved (or never initialised).
    let move_error_var = iteration.variable::<(T::Path, T::Point)>("move_error");
    let accessed_at_var = iteration.variable::<((T::Path, T::Point), ())>("accessed_at");

    accessed_at_var.insert(
        accessed_at
            .elements
            .iter()
            .map(|&(path, point)| ((path, point), ()))
            .collect(),
    );

    while iteration.changed() {
        // NOTE: Double join!
        // move_error(path, point) :-
        //     path_accessed_at(path, point),
        //     !path_definitely_initialized_at(path, point).
        move_error_var.from_antijoin(
            &accessed_at_var,
            &path_definitely_initialized_at,
            |&(path, point), &()| (path, point),
        );
    }
    move_error_var.complete()
}

// Compute two things:
//
// - an over-approximation of the initialization of variables. This is used in
//   the region_live_at computations to determine when a drop may happen; a
//   definitely moved variable would not be actually dropped.
// - move errors.
//
// The process is split into four stages:
//
// 1. Compute the transitive closure of path accesses. That is, accessing `f.a`
//   would access `f.a.b`, etc.
// 2. Use this to compute both a lower and an upper bound on the paths that may
//   have been moved at any given point.
// 3. Use those to derive a set of paths that are known to be initialized:
//   `definitely_initialized = maybe_initialized - maybe_moved`.
// 4. Use *those* to determine move errors; i.e. accesses - known initialized
//   paths.
pub(super) fn compute_initialization<T: FactTypes>(
    ctx: InitializationContext<T>,
    cfg_edge: &Relation<(T::Point, T::Point)>,
    output: &mut Output<T>,
) -> (Relation<(T::Variable, T::Point)>, Relation<(T::Path, T::Point)>) {
    let timer = Instant::now();

    let TransitivePaths {
        moved_out_at,
        initialized_at,
        accessed_at,
    } = compute_transitive_paths::<T>(
        ctx.child,
        ctx.initialized_at,
        ctx.moved_out_at,
        ctx.path_accessed_at,
    );
    info!("initialization phase 1 completed: {:?}", timer.elapsed());

    let InitializationStatus {
        var_maybe_initialized_on_exit,
        path_maybe_initialized_on_exit,
        path_maybe_moved_at,
    } = compute_initialization_status::<T>(
        ctx.path_belongs_to_var,
        moved_out_at,
        initialized_at,
        cfg_edge,
    );
    info!("initialization phase 2 completed: {:?}", timer.elapsed());

    let path_definitely_initialized_at =
        compute_known_initialized_paths::<T>(&path_maybe_initialized_on_exit, path_maybe_moved_at);
    info!(
        "initialization phase 3 completed: {} tuples in {:?}",
        path_definitely_initialized_at.elements.len(),
        timer.elapsed()
    );

    let move_errors = compute_move_errors::<T>(accessed_at, path_definitely_initialized_at);
    info!(
        "initialization phase 4 completed: {} move errors in {:?}",
        move_errors.elements.len(),
        timer.elapsed()
    );

    if output.dump_enabled {
        for &(path, location) in path_maybe_initialized_on_exit.iter() {
            output
                .path_maybe_initialized_at
                .entry(location)
                .or_default()
                .push(path);
        }

        for &(var, location) in var_maybe_initialized_on_exit.iter() {
            output
                .var_maybe_initialized_on_exit
                .entry(location)
                .or_default()
                .push(var);
        }
    }

    (var_maybe_initialized_on_exit, move_errors)
}
