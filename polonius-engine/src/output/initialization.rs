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
    let mut iteration = Iteration::new();

    // Relations
    //let parent: Relation<(Path, Path)> = child.iter().map(|&(child_path, parent_path)| (parent_path, child_path)).collect();
    let child: Relation<(T::Path, T::Path)> = ctx.child.into();
    let path_belongs_to_var: Relation<(T::Path, T::Variable)> = ctx.path_belongs_to_var.into();
    let initialized_at: Relation<(T::Path, T::Point)> = ctx.initialized_at.into();
    let moved_out_at: Relation<(T::Path, T::Point)> = ctx.moved_out_at.into();
    let _path_accessed_at: Relation<(T::Path, T::Point)> = ctx.path_accessed_at.into();

    // Variables

    // var_maybe_initialized_on_exit(var, point): Upon leaving `point`, at least one part of the
    // variable `var` might be initialized for some path through the CFG.
    let var_maybe_initialized_on_exit =
        iteration.variable::<(T::Variable, T::Point)>("var_maybe_initialized_on_exit");

    // path_maybe_initialized_on_exit(path, point): Upon leaving `point`, the move path `path`
    // might be *partially* initialized for some path through the CFG.
    let path_maybe_initialized_on_exit =
        iteration.variable::<(T::Path, T::Point)>("path_maybe_initialized_on_exit");

    // Initial propagation of static relations

    // path_maybe_initialized_on_exit(path, point) :- initialized_at(path, point).
    path_maybe_initialized_on_exit.insert(initialized_at);

    while iteration.changed() {
        // path_maybe_initialized_on_exit(path, point2) :-
        //     path_maybe_initialized_on_exit(path, point1),
        //     cfg_edge(point1, point2),
        //     !moved_out_at(path, point2).
        path_maybe_initialized_on_exit.from_leapjoin(
            &path_maybe_initialized_on_exit,
            (
                cfg_edge.extend_with(|&(_path, point1)| point1),
                moved_out_at.extend_anti(|&(path, _point1)| path),
            ),
            |&(path, _point1), &point2| (path, point2),
        );

        // path_maybe_initialized_on_exit(Mother, point) :-
        //     path_maybe_initialized_on_exit(Daughter, point),
        //     child(Daughter, Mother).
        path_maybe_initialized_on_exit.from_join(
            &path_maybe_initialized_on_exit,
            &child,
            |&_daughter, &point, &mother| (mother, point),
        );

        // TODO: the following lines contain things left to implement for move
        // tracking:

        // path_accessed :- path_accessed(path, point).
        //
        // -- transitive access of all children
        // path_accessed(Child, point) :-
        //     path_accessed(Mother, point),
        //     parent(Mother, Child).

        // Propagate across CFG edges:
        // path_maybe_uninit(path, point2) :-
        //     path_maybe_uninit(path, point1),
        //     cfg_edge_(point1, point2)
        //     !initialized_at(point1, point2).

        // Initial value (static).
        // path_maybe_uninit(path, point) :- moved_out_at(path, point).

        // NOTE: Double join!
        // errors(path, point) :-
        //     path_maybe_uninit(path, point),
        //     path_accessed(path, point).

        // END TODO

        // var_maybe_initialized_on_exit(var, point) :-
        //     path_maybe_initialized_on_exit(path, point),
        //     path_belongs_to_var(path, var).
        var_maybe_initialized_on_exit.from_join(
            &path_maybe_initialized_on_exit,
            &path_belongs_to_var,
            |&_path, &point, &var| (var, point),
        );
    }

    let var_maybe_initialized_on_exit = var_maybe_initialized_on_exit.complete();

    info!(
        "init_var_maybe_initialized_on_exit() completed: {} tuples, {:?}",
        var_maybe_initialized_on_exit.len(),
        timer.elapsed()
    );

    if output.dump_enabled {
        let path_maybe_initialized_on_exit = path_maybe_initialized_on_exit.complete();
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
