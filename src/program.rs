#![cfg(test)]

use std::collections::BTreeSet;

use polonius_parser::{
    ir::{Effect, Fact, KnownSubset, Placeholder},
    parse_input,
};

use crate::facts::{AllFacts, Loan, Origin, Path, Point, Variable};
use crate::intern::InternerTables;

/// A structure to hold and deduplicate facts
#[derive(Default)]
struct Facts {
    loan_issued_at: BTreeSet<(Origin, Loan, Point)>,
    universal_region: BTreeSet<Origin>,
    cfg_edge: BTreeSet<(Point, Point)>,
    killed: BTreeSet<(Loan, Point)>,
    outlives: BTreeSet<(Origin, Origin, Point)>,
    invalidates: BTreeSet<(Point, Loan)>,
    known_subset: BTreeSet<(Origin, Origin)>,
    placeholder: BTreeSet<(Origin, Loan)>,
    var_defined_at: BTreeSet<(Variable, Point)>,
    var_used_at: BTreeSet<(Variable, Point)>,
    var_dropped_at: BTreeSet<(Variable, Point)>,
    use_of_var_derefs_origin: BTreeSet<(Variable, Origin)>,
    drop_of_var_derefs_origin: BTreeSet<(Variable, Origin)>,
    child_path: BTreeSet<(Path, Path)>,
    path_is_var: BTreeSet<(Path, Variable)>,
    path_assigned_at_base: BTreeSet<(Path, Point)>,
    path_moved_at_base: BTreeSet<(Path, Point)>,
    path_accessed_at_base: BTreeSet<(Path, Point)>,
}

impl From<Facts> for AllFacts {
    fn from(facts: Facts) -> Self {
        Self {
            loan_issued_at: facts.loan_issued_at.into_iter().collect(),
            universal_region: facts.universal_region.into_iter().collect(),
            cfg_edge: facts.cfg_edge.into_iter().collect(),
            killed: facts.killed.into_iter().collect(),
            outlives: facts.outlives.into_iter().collect(),
            invalidates: facts.invalidates.into_iter().collect(),
            var_defined_at: facts.var_defined_at.into_iter().collect(),
            var_used_at: facts.var_used_at.into_iter().collect(),
            var_dropped_at: facts.var_dropped_at.into_iter().collect(),
            use_of_var_derefs_origin: facts.use_of_var_derefs_origin.into_iter().collect(),
            drop_of_var_derefs_origin: facts.drop_of_var_derefs_origin.into_iter().collect(),
            child_path: facts.child_path.into_iter().collect(),
            path_is_var: facts.path_is_var.into_iter().collect(),
            path_assigned_at_base: facts.path_assigned_at_base.into_iter().collect(),
            path_moved_at_base: facts.path_moved_at_base.into_iter().collect(),
            path_accessed_at_base: facts.path_accessed_at_base.into_iter().collect(),
            known_subset: facts.known_subset.into_iter().collect(),
            placeholder: facts.placeholder.into_iter().collect(),
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
            .placeholders
            .iter()
            .map(|placeholder| tables.origins.intern(&placeholder.origin)),
    );

    // facts: placeholder(Origin, Loan)
    facts
        .placeholder
        .extend(input.placeholders.iter().map(|placeholder| {
            (
                tables.origins.intern(&placeholder.origin),
                tables.loans.intern(&placeholder.loan),
            )
        }));

    facts
        .drop_of_var_derefs_origin
        .extend(
            input
                .drop_of_var_derefs_origin
                .iter()
                .map(|(variable, origin)| {
                    (
                        tables.variables.intern(variable),
                        tables.origins.intern(origin),
                    )
                }),
        );

    facts
        .use_of_var_derefs_origin
        .extend(
            input
                .use_of_var_derefs_origin
                .iter()
                .map(|(variable, origin)| {
                    (
                        tables.variables.intern(variable),
                        tables.origins.intern(origin),
                    )
                }),
        );

    // facts: known_subset(Origin, Origin)
    facts.known_subset.extend(
        input
            .known_subsets
            .iter()
            .map(|KnownSubset { ref a, ref b }| {
                (tables.origins.intern(a), tables.origins.intern(b))
            }),
    );

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
        // facts: loan_issued_at(Origin, Loan, Point)
        Fact::LoanIssuedAt {
            ref origin,
            ref loan,
        } => {
            // loan_issued_at: a `loan_issued_at` occurs on the Mid point
            let origin = tables.origins.intern(origin);
            let loan = tables.loans.intern(loan);

            facts.loan_issued_at.insert((origin, loan, point));
        }

        // facts: outlives(Origin, Origin, Point)
        Fact::Outlives { ref a, ref b } => {
            // outlives: a `outlives` occurs on Mid points
            let origin_a = tables.origins.intern(a);
            let origin_b = tables.origins.intern(b);

            facts.outlives.insert((origin_a, origin_b, point));
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

        // facts: var_defined_at(Variable, Point)
        Fact::DefineVariable { ref variable } => {
            // var_defined_at: a variable is overwritten here
            let variable = tables.variables.intern(variable);
            facts.var_defined_at.insert((variable, point));
        }

        // facts: var_used_at(Variable, Point)
        Fact::UseVariable { ref variable } => {
            // var_used_at: a variable is used here
            let variable = tables.variables.intern(variable);
            facts.var_used_at.insert((variable, point));
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
            placeholders { 'a, 'b, 'c }

            // block description
            block B0 {
                // 0:
                invalidates(L0);

                // 1:
                invalidates(L1), origin_live_on_entry('d) / kill(L2);

                // another comment
                goto B1;
            }

            block B1 {
                // O:
                use('a, 'b), outlives('a: 'b), loan_issued_at('b, L1);
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
            .map(|origin| tables.origins.untern(*origin))
            .collect();
        assert_eq!(universal_regions, ["'a", "'b", "'c"]);

        // facts: placeholder
        let placeholders: Vec<_> = facts
            .placeholder
            .iter()
            .map(|&(origin, loan)| Placeholder {
                origin: tables.origins.untern(origin).to_string(),
                loan: tables.loans.untern(loan).to_string(),
            })
            .collect();

        assert_eq!(
            placeholders,
            vec![
                Placeholder {
                    origin: "'a".to_string(),
                    loan: "'a".to_string(),
                },
                Placeholder {
                    origin: "'b".to_string(),
                    loan: "'b".to_string(),
                },
                Placeholder {
                    origin: "'c".to_string(),
                    loan: "'c".to_string(),
                },
            ]
        );

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
            let origin_a = tables.origins.untern(facts.outlives[0].0);
            let origin_b = tables.origins.untern(facts.outlives[0].1);
            let point = tables.points.untern(facts.outlives[0].2);

            assert_eq!(origin_a, "'a");
            assert_eq!(origin_b, "'b");
            assert_eq!(point, "\"Mid(B1[0])\"");
        }

        assert_eq!(facts.loan_issued_at.len(), 1);
        {
            let origin = tables.origins.untern(facts.loan_issued_at[0].0);
            let loan = tables.loans.untern(facts.loan_issued_at[0].1);
            let point = tables.points.untern(facts.loan_issued_at[0].2);

            assert_eq!(origin, "'b");
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
            .map(|&(point1, _)| point1)
            .chain(facts.cfg_edge.iter().map(|&(_, point2)| point2))
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
