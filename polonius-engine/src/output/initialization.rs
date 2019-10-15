use std::time::Instant;

use crate::facts::FactTypes;
use crate::output::{InitializationContext, Output};

use datafrog::{Iteration, Relation, RelationLeaper};

pub(super) fn init_var_maybe_initialized_on_exit<T: FactTypes>(
    ctx: InitializationContext<T>,
    cfg_edge: &Relation<(T::Point, T::Point)>,
    output: &mut Output<T>,
) -> Relation<(T::Variable, T::Point)> {
    let timer = Instant::now();

    // Relations used by all steps:
    let cfg_edge: Relation<(T::Point, T::Point)> = cfg_edge.iter().cloned().collect();

    // Step 1: compute transitive closures of path operations
    let (initialized_at, moved_out_at, accessed_at) = {
        let mut iteration = Iteration::new();
        let child: Relation<(T::Path, T::Path)> = ctx.child.into();

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
        moved_out_at_var.insert(ctx.moved_out_at.into());

        // initialized_at(Path, Point) :- initialized_at(path, Point).
        initialized_at_var.insert(ctx.initialized_at.into());

        // accessed_at(Path, Point) :- path_accessed_at(path, Point).
        accessed_at_var.insert(ctx.path_accessed_at.into());

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

        let result = (
            initialized_at_var.complete(),
            moved_out_at_var.complete(),
            accessed_at_var.complete(),
        );

        info!("initialization phase 1 completed: {:?}", timer.elapsed());

        result
    };

    // Step 2: Compute path initialization and deinitialization across the CFG.
    let (var_maybe_initialized_on_exit, path_maybe_initialized_on_exit, path_maybe_moved_at) = {
        let mut iteration = Iteration::new();

        // Relations
        let path_belongs_to_var: Relation<(T::Path, T::Variable)> = ctx.path_belongs_to_var.into();

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
        let path_maybe_moved_at_var =
            iteration.variable::<(T::Path, T::Point)>("path_maybe_moved_at");

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

        let results = (
            var_maybe_initialized_on_exit_var.complete(),
            path_maybe_initialized_on_exit_var.complete(),
            path_maybe_moved_at_var.complete(),
        );

        info!("initialization phase 2 completed: {:?}", timer.elapsed());

        results
    };

    // Step 3: Calculate provably initialised paths:
    // path_definitely_initialized_at(Path, Point): Any path through the CFG to
    // `Point` has `Path` initialized.
    let path_definitely_initialized_at: Relation<(T::Path, T::Point)> = {
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

        let path_definitely_initialized = path_definitely_initialized_at_var.complete();

        info!(
            "initialization phase 3 completed: {} tuples in {:?}",
            path_definitely_initialized.elements.len(),
            timer.elapsed()
        );

        path_definitely_initialized
    };

    // Step 4: Compute erroneous path accesses:
    let move_errors = {
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
    };

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

    var_maybe_initialized_on_exit
}
