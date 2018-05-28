#[derive(Debug)]
pub struct Input {
    pub universal_regions: Vec<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Block {
    pub name: String,
    pub statements: Vec<Statement>,
    pub goto: Vec<String>,
}

#[derive(Debug)]
pub struct Statement {
    /// Effects destined to be emitted at the Statement's Start point
    pub effects_start: Vec<Effect>,

    /// Effects destined to be emitted at the Statement's Mid point
    pub effects: Vec<Effect>,
}

#[derive(Debug, PartialEq)]
pub enum Effect {
    Use { regions: Vec<String> },
    Fact(Fact),
}

#[derive(Debug, PartialEq)]
pub enum Fact {
    Outlives { a: String, b: String },
    BorrowRegionAt { region: String, loan: String },
    Invalidates { loan: String },
    Kill { loan: String },
}

impl Statement {
    crate fn new(effects: Vec<Effect>) -> Self {
        Self {
            effects_start: Vec::new(),
            effects,
        }
    }

    crate fn with_start_effects(effects_start: Vec<Effect>, effects: Vec<Effect>) -> Self {
        Self {
            effects_start,
            effects,
        }
    }
}
