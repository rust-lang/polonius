use std::time::Instant;

use crate::output::Output;
use facts::Atom;

use datafrog::{Iteration, Relation, RelationLeaper};

pub(super) fn init_var_maybe_initialized_on_exit<Region, Loan, Point, Variable, MovePath>(
    child: Vec<(MovePath, MovePath)>,
    path_belongs_to_var: Vec<(MovePath, Variable)>,
    initialized_at: Vec<(MovePath, Point)>,
    moved_out_at: Vec<(MovePath, Point)>,
    path_accessed_at: Vec<(MovePath, Point)>,
    cfg_edge: &[(Point, Point)],
    output: &mut Output<Region, Loan, Point, Variable, MovePath>,
) -> Vec<(Variable, Point)>
where
    Region: Atom,
    Loan: Atom,
    Point: Atom,
    Variable: Atom,
    MovePath: Atom,
{
    debug!("compute_initialization()");
    let computation_start = Instant::now();
    let mut iteration = Iteration::new();

    // Relations
    //let parent: Relation<(MovePath, MovePath)> = child.iter().map(|&(c, p)| (p, c)).collect();
    let child: Relation<(MovePath, MovePath)> = child.into();
    let path_belongs_to_var: Relation<(MovePath, Variable)> = path_belongs_to_var.into();
    let initialized_at: Relation<(MovePath, Point)> = initialized_at.into();
    let moved_out_at: Relation<(MovePath, Point)> = moved_out_at.into();
    let cfg_edge: Relation<(Point, Point)> = cfg_edge.iter().cloned().collect();
    let _path_accessed_at: Relation<(MovePath, Point)> = path_accessed_at.into();

    // Variables

    // var_maybe_initialized_on_exit(V, P): Upon leaving `P`, at least one part of the
    // variable `V` might be initialized for some path through the CFG.
    let var_maybe_initialized_on_exit =
        iteration.variable::<(Variable, Point)>("var_maybe_initialized_on_exit");

    // path_maybe_initialized_on_exit(M, P): Upon leaving `P`, the move path `M`
    // might be *partially* initialized for some path through the CFG.
    let path_maybe_initialized_on_exit =
        iteration.variable::<(MovePath, Point)>("path_maybe_initialized_on_exit");

    // Initial propagation of static relations

    // path_maybe_initialized_on_exit(Path, Point) :- initialized_at(Path,
    // Point).
    path_maybe_initialized_on_exit.insert(initialized_at);

    while iteration.changed() {
        // path_maybe_initialized_on_exit(M, Q) :-
        //     path_maybe_initialized_on_exit(M, P),
        //     cfg_edge(P, Q),
        //     !moved_out_at(M, Q).
        path_maybe_initialized_on_exit.from_leapjoin(
            &path_maybe_initialized_on_exit,
            (
                cfg_edge.extend_with(|&(_m, p)| p),
                moved_out_at.extend_anti(|&(m, _p)| m),
            ),
            |&(m, _p), &q| (m, q),
        );

        // path_maybe_initialized_on_exit(Mother, P) :-
        //     path_maybe_initialized_on_exit(Daughter, P),
        //     child(Daughter, Mother).
        path_maybe_initialized_on_exit.from_leapjoin(
            &path_maybe_initialized_on_exit,
            child.extend_with(|&(daughter, _p)| daughter),
            |&(_daughter, p), &mother| (mother, p),
        );

        // TODO: the following lines contain things left to implement for move
        // tracking:

        // path_accessed :- path_accessed(M, P).
        //
        // -- transitive access of all children
        // path_accessed(Child, P) :-
        //     path_accessed(Mother, P),
        //     parent(Mother, Child).

        // Propagate across CFG edges:
        // path_maybe_uninit(M, Q) :-
        //     path_maybe_uninit(M, P),
        //     cfg_edge_(P, Q)
        //     !initialized_at(P, Q).

        // Initial value (static).
        // path_maybe_uninit(M, P) :- moved_out_at(M, P).

        // NOTE: Double join!
        // errors(M, P) :-
        //     path_maybe_uninit(M, P),
        //     path_accessed(M, P).

        // END TODO

        // var_maybe_initialized_on_exit(V, P) :-
        //     path_belongs_to_var(M, V),
        //     path_maybe_initialized_at(M, P).
        var_maybe_initialized_on_exit.from_leapjoin(
            &path_maybe_initialized_on_exit,
            path_belongs_to_var.extend_with(|&(m, _p)| m),
            |&(_m, p), &v| (v, p),
        );
    }

    let var_maybe_initialized_on_exit = var_maybe_initialized_on_exit.complete();

    info!(
        "compute_initialization() completed: {} tuples, {:?}",
        var_maybe_initialized_on_exit.len(),
        computation_start.elapsed()
    );

    if output.dump_enabled {
        let path_maybe_initialized_on_exit = path_maybe_initialized_on_exit.complete();
        for &(path, location) in &path_maybe_initialized_on_exit.elements {
            output
                .path_maybe_initialized_at
                .entry(location)
                .or_insert_with(Vec::new)
                .push(path);
        }

        for &(var, location) in &var_maybe_initialized_on_exit.elements {
            output
                .var_maybe_initialized_on_exit
                .entry(location)
                .or_insert_with(Vec::new)
                .push(var);
        }
    }

    var_maybe_initialized_on_exit
        .iter()
        .map(|&(v, p)| (v, p))
        .collect()
}
