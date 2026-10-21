use super::{Computation, Dump};
use crate::FactTypes;

#[derive(Clone, Copy)]
pub struct Cfg;

input! {
    CfgEdge {
        cfg_edge,
    }
}

output!(cfg_node);

impl<T: FactTypes> Computation<T> for Cfg {
    type Input<'db> = CfgEdge<'db, T>;
    type Output = CfgNode<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let CfgEdge { cfg_edge } = input;
        let cfg_node = cfg_edge
            .iter()
            .map(|e| e.0)
            .chain(cfg_edge.iter().map(|e| e.1))
            .map(|n| (n,))
            .collect();
        CfgNode { cfg_node }
    }
}
