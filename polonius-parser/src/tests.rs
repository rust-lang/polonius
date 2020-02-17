#![cfg(test)]

use crate::ir::{Effect, Fact, KnownSubset, Placeholder};
use crate::parse_input;

#[test]
fn placeholders() {
    let program = r"
        placeholders { 'a, 'b, 'c }
    ";
    let input = parse_input(program).expect("Placeholders");
    assert_eq!(
        input.placeholders,
        vec![
            Placeholder {
                origin: "'a".to_string(),
                loan: "'a".to_string()
            },
            Placeholder {
                origin: "'b".to_string(),
                loan: "'b".to_string()
            },
            Placeholder {
                origin: "'c".to_string(),
                loan: "'c".to_string()
            }
        ]
    );
}

#[test]
fn blocks() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        block B0 {
        }
        block B1 {
        }
    ";
    let input = parse_input(program).expect("Parse Error");
    assert_eq!(
        input.blocks.iter().map(|b| &b.name).collect::<Vec<_>>(),
        ["B0", "B1"]
    );
}

#[test]
fn goto() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        block B0 {
            goto B1;
        }
        block B1 {
        }
    ";
    let input = parse_input(program).expect("Parse Error");
    assert_eq!(input.blocks[0].goto, ["B1"]);
}

#[test]
fn effects() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        block B0 {
            use('a), outlives('a: 'b), borrow_region_at('b, L1);
            kill(L2);
            invalidates(L0);
        }
    ";
    let input = parse_input(program).expect("Parse Error");
    let block = &input.blocks[0];
    assert_eq!(block.statements.len(), 3);

    let statements = &block.statements;
    assert_eq!(statements[0].effects.len(), 3);
    assert_eq!(statements[1].effects.len(), 1);
    assert_eq!(statements[2].effects.len(), 1);

    let effects = &statements[0].effects;
    assert_eq!(
        effects[0],
        Effect::Use {
            origins: vec!["'a".to_string()]
        }
    );
    assert_eq!(
        effects[1],
        Effect::Fact(Fact::Outlives {
            a: "'a".to_string(),
            b: "'b".to_string()
        })
    );
    assert_eq!(
        effects[2],
        Effect::Fact(Fact::BorrowRegionAt {
            origin: "'b".to_string(),
            loan: "L1".to_string()
        })
    );

    let effects = &statements[1].effects;
    assert_eq!(
        effects[0],
        Effect::Fact(Fact::Kill {
            loan: "L2".to_string()
        })
    );

    let effects = &statements[2].effects;
    assert_eq!(
        effects[0],
        Effect::Fact(Fact::Invalidates {
            loan: "L0".to_string()
        })
    );
}

#[test]
fn effects_start() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        block B0 {
            invalidates(L0), origin_live_on_entry('a) / use('a);
            invalidates(L1);
            invalidates(L0), invalidates(L1) / use('c);
        }
    ";
    let input = parse_input(program).expect("Effects start");
    let block = &input.blocks[0];
    assert_eq!(block.statements.len(), 3);

    let statements = &block.statements[0];
    assert_eq!(
        statements.effects_start,
        [
            Effect::Fact(Fact::Invalidates {
                loan: "L0".to_string()
            }),
            Effect::Fact(Fact::OriginLiveOnEntry {
                origin: "'a".to_string()
            })
        ]
    );
    assert_eq!(
        statements.effects,
        [Effect::Use {
            origins: vec!["'a".to_string()]
        }]
    );

    let statements = &block.statements[1];
    assert!(statements.effects_start.is_empty());
    assert_eq!(
        statements.effects,
        [Effect::Fact(Fact::Invalidates {
            loan: "L1".to_string()
        })]
    );

    let statements = &block.statements[2];
    assert_eq!(
        statements.effects_start,
        [
            Effect::Fact(Fact::Invalidates {
                loan: "L0".to_string()
            }),
            Effect::Fact(Fact::Invalidates {
                loan: "L1".to_string()
            })
        ]
    );
    assert_eq!(
        statements.effects,
        [Effect::Use {
            origins: vec!["'c".to_string()]
        }]
    );
}

#[test]
fn complete_example() {
    let program = r"
        // program description
        placeholders { 'a, 'b, 'c }

        // block description
        block B0 {
            // 0:
            invalidates(L0);

            // 1:
            kill(L2);

            invalidates(L1) / use('a, 'b);

            // another comment
            goto B1, B2;
        }

        block B1 {
            use('a), outlives('a: 'b), borrow_region_at('b, L1);
        }
    ";
    assert!(parse_input(program).is_ok());
}

#[test]
fn variable_used() {
    let program = r"
        placeholders { 'a, 'b, 'c }

        block B0 {
            var_used_at(V0);
        }
    ";
    let input = parse_input(program).expect("Variable used");
    let block = &input.blocks[0];
    assert_eq!(block.statements.len(), 1);

    let statement = &block.statements[0];
    assert_eq!(
        statement.effects,
        [Effect::Fact(Fact::UseVariable {
            variable: "V0".to_string()
        })]
    );
}

#[test]
fn variable_defined() {
    let program = r"
        placeholders { 'a, 'b, 'c }

        block B0 {
            var_defined_at(V1);
        }
    ";
    let input = parse_input(program).expect("Variable defined");
    let block = &input.blocks[0];
    assert_eq!(block.statements.len(), 1);

    let statement = &block.statements[0];
    assert_eq!(
        statement.effects,
        [Effect::Fact(Fact::DefineVariable {
            variable: "V1".to_string()
        })]
    );
}

#[test]
fn use_of_var_derefs_origin() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        use_of_var_derefs_origin { (V1, 'a), (V2, 'b) }
        drop_of_var_derefs_origin {  }

        block B0 {
            var_defined_at(V1);
        }
    ";
    let input = parse_input(program).expect("Use of var derefs origin");
    assert_eq!(
        input.use_of_var_derefs_origin,
        [
            ("V1".to_string(), "'a".to_string()),
            ("V2".to_string(), "'b".to_string())
        ]
    );
}

#[test]
fn drop_of_var_derefs_origin() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        use_of_var_derefs_origin {  }
        drop_of_var_derefs_origin { (V1, 'a) }

        block B0 {
            var_defined_at(V1);
        }
    ";
    let input = parse_input(program).expect("Drop of var derefs origin");
    assert_eq!(
        input.drop_of_var_derefs_origin,
        [("V1".to_string(), "'a".to_string())]
    );
}

#[test]
fn known_subsets() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        known_subsets { 'a: 'b, 'b: 'c }
    ";
    let input = parse_input(program).expect("Known subsets");
    assert_eq!(
        input.placeholders,
        vec![
            Placeholder {
                origin: "'a".to_string(),
                loan: "'a".to_string()
            },
            Placeholder {
                origin: "'b".to_string(),
                loan: "'b".to_string()
            },
            Placeholder {
                origin: "'c".to_string(),
                loan: "'c".to_string()
            }
        ]
    );
    assert_eq!(
        input.known_subsets,
        vec![
            KnownSubset {
                a: "'a".to_string(),
                b: "'b".to_string()
            },
            KnownSubset {
                a: "'b".to_string(),
                b: "'c".to_string()
            }
        ]
    );
}
