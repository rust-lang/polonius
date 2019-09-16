#![cfg(test)]

use std::collections::BTreeSet;

use polonius_parser::{
    ir::{Effect, Fact},
    parse_input,
};

use crate::facts::{AllFacts, Loan, MovePath, Point, Origin, Variable};
use crate::intern::InternerTables;

/// A structure to hold and deduplicate facts
#[derive(Default)]
struct Facts {
    borrow_region: BTreeSet<(Origin, Loan, Point)>,
    universal_region: BTreeSet<Origin>,
    cfg_edge: BTreeSet<(Point, Point)>,
    killed: BTreeSet<(Loan, Point)>,
    outlives: BTreeSet<(Origin, Origin, Point)>,
    invalidates: BTreeSet<(Point, Loan)>,
    var_defined: BTreeSet<(Variable, Point)>,
    var_used: BTreeSet<(Variable, Point)>,
    var_drop_used: BTreeSet<(Variable, Point)>,
    var_uses_region: BTreeSet<(Variable, Origin)>,
    var_drops_region: BTreeSet<(Variable, Origin)>,
    child: BTreeSet<(MovePath, MovePath)>,
    path_belongs_to_var: BTreeSet<(MovePath, Variable)>,
    initialized_at: BTreeSet<(MovePath, Point)>,
    moved_out_at: BTreeSet<(MovePath, Point)>,
    path_accessed_at: BTreeSet<(MovePath, Point)>,
}

impl From<Facts> for AllFacts {
    fn from(facts: Facts) -> Self {
        Self {
            borrow_region: facts.borrow_region.into_iter().collect(),
            universal_region: facts.universal_region.into_iter().collect(),
            cfg_edge: facts.cfg_edge.into_iter().collect(),
            killed: facts.killed.into_iter().collect(),
            outlives: facts.outlives.into_iter().collect(),
            invalidates: facts.invalidates.into_iter().collect(),
            var_defined: facts.var_defined.into_iter().collect(),
            var_used: facts.var_used.into_iter().collect(),
            var_drop_used: facts.var_drop_used.into_iter().collect(),
            var_uses_region: facts.var_uses_region.into_iter().collect(),
            var_drops_region: facts.var_drops_region.into_iter().collect(),
            child: facts.child.into_iter().collect(),
            path_belongs_to_var: facts.path_belongs_to_var.into_iter().collect(),
            initialized_at: facts.initialized_at.into_iter().collect(),
            moved_out_at: facts.moved_out_at.into_iter().collect(),
            path_accessed_at: facts.path_accessed_at.into_iter().collect(),
        }
    }
}

/// Parses an input program into a set of its facts, into the same format `rustc` outputs.
pub(crate) fn parse_from_program(
    program: &str,
    tables: &mut InternerTables,
) -> Result<AllFacts, String> {
    let input = parse_input(program)?;

    let mut facts: Facts = Default::default();

    // facts: universal_region(Origin)
    facts.universal_region.extend(
        input
            .universal_regions
            .iter()
            .map(|region| tables.origins.intern(region)),
    );

    facts
        .var_drops_region
        .extend(input.var_drops_region.iter().map(|(variable, region)| {
            (
                tables.variables.intern(variable),
                tables.origins.intern(region),
            )
        }));

    facts
        .var_uses_region
        .extend(input.var_uses_region.iter().map(|(variable, region)| {
            (
                tables.variables.intern(variable),
                tables.origins.intern(region),
            )
        }));

    for block in &input.blocks {
        let block_name = &block.name;

        for (statement_idx, statement) in block.statements.iter().enumerate() {
            let start = format!(
                "\"Start({block}[{statement}])\"",
                block = block_name,
                statement = statement_idx
            );
            let mid = format!(
                "\"Mid({block}[{statement}])\"",
                block = block_name,
                statement = statement_idx
            );

            let start = tables.points.intern(&start);
            let mid = tables.points.intern(&mid);

            // facts: cfg_edge(Point, Point)
            {
                if statement_idx > 0 {
                    // edge: Previous Mid point to this Start point
                    let previous_mid = format!(
                        "\"Mid({block}[{previous_statement}])\"",
                        block = block_name,
                        previous_statement = statement_idx - 1
                    );
                    let previous_mid = tables.points.intern(&previous_mid);

                    facts.cfg_edge.insert((previous_mid, start));
                }

                // edge: Start to Mid point
                facts.cfg_edge.insert((start, mid));

                // goto edges
                let terminator_idx = block.statements.len() - 1;
                facts.cfg_edge.extend(block.goto.iter().map(|goto| {
                    // edge: last Mid point to Start of remote block
                    let from = format!(
                        "\"Mid({block}[{statement}])\"",
                        block = block_name,
                        statement = terminator_idx
                    );
                    let to = format!("\"Start({next_block}[0])\"", next_block = goto);

                    let from = tables.points.intern(&from);
                    let to = tables.points.intern(&to);

                    (from, to)
                }));
            }

            // the most common statement effects: mid point effects
            for effect in &statement.effects {
                match effect {
                    // TODO: once the parser is revamped for liveness etc, make
                    // sure to catch the new inputs here!
                    Effect::Fact(ref fact) => {
                        // Manually specified facts
                        emit_fact(&mut facts, fact, mid, tables)
                    }
                    _ => {}
                };
            }

            // commonly used to emit manual `invalidates` at Start points, like some rustc features do
            for effect in &statement.effects_start {
                if let Effect::Fact(ref fact) = effect {
                    emit_fact(&mut facts, fact, start, tables);
                }
            }
        }
    }

    Ok(facts.into())
}

fn emit_fact(facts: &mut Facts, fact: &Fact, point: Point, tables: &mut InternerTables) {
    match fact {
        // facts: borrow_region(Origin, Loan, Point)
        Fact::BorrowRegionAt {
            ref region,
            ref loan,
        } => {
            // borrow_region: a `borrow_region_at` occurs on the Mid point
            let region = tables.origins.intern(region);
            let loan = tables.loans.intern(loan);

            facts.borrow_region.insert((region, loan, point));
        }

        // facts: outlives(Origin, Origin, Point)
        Fact::Outlives { ref a, ref b } => {
            // outlives: a `outlives` occurs on Mid points
            let region_a = tables.origins.intern(a);
            let region_b = tables.origins.intern(b);

            facts.outlives.insert((region_a, region_b, point));
        }

        // facts: killed(Loan, Point)
        Fact::Kill { ref loan } => {
            // killed: a loan is killed on Mid points
            let loan = tables.loans.intern(loan);
            facts.killed.insert((loan, point));
        }

        // facts: invalidates(Point, Loan)
        Fact::Invalidates { ref loan } => {
            let loan = tables.loans.intern(loan);
            // invalidates: a loan can be invalidated on both Start and Mid points
            facts.invalidates.insert((point, loan));
        }

        // facts: var_defined(V, P)
        Fact::DefineVariable { ref variable } => {
            // var_defined: a variable is overwritten here
            let variable = tables.variables.intern(variable);
            facts.var_defined.insert((variable, point));
        }

        // facts: var_used(V, P)
        Fact::UseVariable { ref variable } => {
            // var_used: a variable is used here
            let variable = tables.variables.intern(variable);
            facts.var_used.insert((variable, point));
        }

        _ => {}
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::InternerTables;

    #[test]
    fn complete_program() {
        let program = r"
            // program description
            universal_regions { 'a, 'b, 'c }

            // block description
            block B0 {
                // 0:
                invalidates(L0);

                // 1:
                invalidates(L1), region_live_at('d) / kill(L2);

                // another comment
                goto B1;
            }

            block B1 {
                // O:
                use('a, 'b), outlives('a: 'b), borrow_region_at('b, L1);
            }
        ";

        let mut tables = InternerTables::new();

        let facts = parse_from_program(program, &mut tables);
        assert!(facts.is_ok());

        let facts = facts.unwrap();

        // facts: universal_region
        let universal_regions: Vec<_> = facts
            .universal_region
            .iter()
            .map(|r| tables.origins.untern(*r).to_string())
            .collect();
        assert_eq!(universal_regions, ["'a", "'b", "'c"]);

        // facts: invalidates
        assert_eq!(facts.invalidates.len(), 2);
        {
            // regular mid point `invalidates`
            let point = tables.points.untern(facts.invalidates[0].0);
            let loan = tables.loans.untern(facts.invalidates[0].1);

            assert_eq!(point, "\"Mid(B0[0])\"");
            assert_eq!(loan, "L0");
        }

        {
            // uncommon start point `invalidates`
            let point = tables.points.untern(facts.invalidates[1].0);
            let loan = tables.loans.untern(facts.invalidates[1].1);

            assert_eq!(point, "\"Start(B0[1])\"");
            assert_eq!(loan, "L1");
        }

        assert_eq!(facts.outlives.len(), 1);
        {
            let region_a = tables.origins.untern(facts.outlives[0].0);
            let region_b = tables.origins.untern(facts.outlives[0].1);
            let point = tables.points.untern(facts.outlives[0].2);

            assert_eq!(region_a, "'a");
            assert_eq!(region_b, "'b");
            assert_eq!(point, "\"Mid(B1[0])\"");
        }

        assert_eq!(facts.borrow_region.len(), 1);
        {
            let region = tables.origins.untern(facts.borrow_region[0].0);
            let loan = tables.loans.untern(facts.borrow_region[0].1);
            let point = tables.points.untern(facts.borrow_region[0].2);

            assert_eq!(region, "'b");
            assert_eq!(loan, "L1");
            assert_eq!(point, "\"Mid(B1[0])\"");
        }

        assert_eq!(facts.killed.len(), 1);
        {
            let loan = tables.loans.untern(facts.killed[0].0);
            let point = tables.points.untern(facts.killed[0].1);

            assert_eq!(loan, "L2");
            assert_eq!(point, "\"Mid(B0[1])\"");
        }

        // 6 points (3 statements * 2 points) => 5 edges, including the 1 goto edge
        let points: BTreeSet<Point> = facts
            .cfg_edge
            .iter()
            .map(|&(p, _)| p)
            .chain(facts.cfg_edge.iter().map(|&(_, q)| q))
            .collect();
        assert_eq!(points.len(), 6);
        assert_eq!(facts.cfg_edge.len(), 5);

        let mut make_edge = |a, b| (tables.points.intern(a), tables.points.intern(b));

        // Start to Mid edge, per statement
        assert!(facts
            .cfg_edge
            .contains(&make_edge("\"Start(B0[0])\"", "\"Mid(B0[0])\"")));
        assert!(facts
            .cfg_edge
            .contains(&make_edge("\"Start(B0[1])\"", "\"Mid(B0[1])\"")));
        assert!(facts
            .cfg_edge
            .contains(&make_edge("\"Start(B1[0])\"", "\"Mid(B1[0])\"")));

        // Mid to Start edge, per statement pair for each block
        assert!(facts
            .cfg_edge
            .contains(&make_edge("\"Mid(B0[0])\"", "\"Start(B0[1])\"")));

        // 1 goto edge from B0 to B1
        assert!(facts
            .cfg_edge
            .contains(&make_edge("\"Mid(B0[1])\"", "\"Start(B1[0])\"")));
    }
}
