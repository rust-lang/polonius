use super::{Computation, Dump};
use crate::FactTypes;

use datafrog::{Iteration, Relation, RelationLeaper};

#[derive(Clone, Copy)]
pub struct LiveOrigins;

input! {
    LiveOriginsInput {
        cfg_edge,
        cfg_node,
        var_dropped_while_init_at,
        var_used_at,
        var_defined_at,
        use_of_var_derefs_origin,
        drop_of_var_derefs_origin,
        universal_region,
    }
}

output!(origin_live_on_entry);

impl<T: FactTypes> Computation<T> for LiveOrigins {
    type Input<'db> = LiveOriginsInput<'db, T>;
    type Output = OriginLiveOnEntry<T>;

    fn compute(&self, input: Self::Input<'_>, dump: &mut Dump<'_>) -> Self::Output {
        let LiveOriginsInput {
            cfg_edge,
            cfg_node,
            var_dropped_while_init_at,
            var_used_at,
            var_defined_at,
            use_of_var_derefs_origin,
            drop_of_var_derefs_origin,
            universal_region,
        } = input;

        let cfg_edge_reverse: Relation<_> = cfg_edge
            .iter()
            .map(|&(point1, point2)| (point2, point1))
            .collect();

        let mut iteration = Iteration::new();

        // Variables

        // `var_live_on_entry`: variable `var` is live upon entry at `point`
        let var_live_on_entry = iteration.variable::<(T::Variable, T::Point)>("var_live_on_entry");
        // `var_drop_live_on_entry`: variable `var` is drop-live (will be used for a drop) upon entry in `point`
        let var_drop_live_on_entry =
            iteration.variable::<(T::Variable, T::Point)>("var_drop_live_on_entry");

        // This is what we are actually calculating:
        let origin_live_on_entry =
            iteration.variable::<(T::Origin, T::Point)>("origin_live_on_entry");

        // var_live_on_entry(var, point) :- var_used_at(var, point).
        var_live_on_entry.insert(var_used_at.clone());

        // var_drop_live_on_entry(var, point) :- var_dropped_while_init_at(var, point).
        var_drop_live_on_entry.insert(var_dropped_while_init_at.clone());

        while iteration.changed() {
            // origin_live_on_entry(origin, point) :-
            //   var_drop_live_on_entry(var, point),
            //   drop_of_var_derefs_origin(var, origin).
            origin_live_on_entry.from_join(
                &var_drop_live_on_entry,
                drop_of_var_derefs_origin,
                |_var, &point, &origin| (origin, point),
            );

            // origin_live_on_entry(origin, point) :-
            //   var_live_on_entry(var, point),
            //   use_of_var_derefs_origin(var, origin).
            origin_live_on_entry.from_join(
                &var_live_on_entry,
                use_of_var_derefs_origin,
                |_var, &point, &origin| (origin, point),
            );

            // var_live_on_entry(var, point1) :-
            //     var_live_on_entry(var, point2),
            //     cfg_edge(point1, point2),
            //     !var_defined(var, point1).
            var_live_on_entry.from_leapjoin(
                &var_live_on_entry,
                (
                    var_defined_at.extend_anti(|&(var, _point2)| var),
                    cfg_edge_reverse.extend_with(|&(_var, point2)| point2),
                ),
                |&(var, _point2), &point1| (var, point1),
            );

            // var_drop_live_on_entry(Var, SourceNode) :-
            //   var_drop_live_on_entry(Var, TargetNode),
            //   cfg_edge(SourceNode, TargetNode),
            //   !var_defined_at(Var, SourceNode).
            //   // var_maybe_partly_initialized_on_exit(Var, SourceNode).
            var_drop_live_on_entry.from_leapjoin(
                &var_drop_live_on_entry,
                (
                    var_defined_at.extend_anti(|&(var, _target_node)| var),
                    cfg_edge_reverse.extend_with(|&(_var, target_node)| target_node),
                ),
                |&(var, _targetnode), &source_node| (var, source_node),
            );
        }

        // Universal regions are live at all points
        let mut origin_live_on_entry = origin_live_on_entry.complete().elements;
        origin_live_on_entry.reserve(cfg_node.len() * universal_region.len());
        for &(o,) in universal_region.iter() {
            for &(n,) in cfg_node.iter() {
                origin_live_on_entry.push((o, n));
            }
        }

        dump.var(&var_live_on_entry);

        Relation::from_vec(origin_live_on_entry).into()
    }
}
