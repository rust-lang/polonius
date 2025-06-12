//! test input data model

#[derive(Debug)]
pub struct Input {
    pub placeholders: Vec<Placeholder>,
    pub known_subsets: Vec<KnownSubset>,
    pub blocks: Vec<Block>,
    pub use_of_var_derefs_origin: Vec<(String, String)>,
    pub drop_of_var_derefs_origin: Vec<(String, String)>,
    pub child_path: Vec<(String, String)>,
    pub path_is_var: Vec<(String, String)>,
}

impl Input {
    pub fn new(
        placeholders: Vec<String>,
        known_subsets: Vec<KnownSubset>,
        use_of_var_derefs_origin: Vec<(String, String)>,
        drop_of_var_derefs_origin: Vec<(String, String)>,
        child_path: Vec<(String, String)>,
        path_is_var: Vec<(String, String)>,
        blocks: Vec<Block>,
    ) -> Input {
        // set-up placeholders as origins with a placeholder loan of the same name
        let placeholders: Vec<_> = placeholders
            .into_iter()
            .map(|origin| Placeholder {
                loan: origin.clone(),
                origin,
            })
            .collect();

        Input {
            placeholders,
            known_subsets,
            use_of_var_derefs_origin,
            drop_of_var_derefs_origin,
            child_path,
            path_is_var,
            blocks,
        }
    }
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

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Use { origins: Vec<String> },
    Fact(Fact),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fact {
    Outlives { a: String, b: String },
    LoanIssuedAt { origin: String, loan: String },
    LoanInvalidatedAt { loan: String },
    LoanKilledAt { loan: String },
    OriginLiveOnEntry { origin: String },
    DefineVariable { variable: String },
    UseVariable { variable: String },
    PathMovedAtBase { path: String },
    PathAssignedAtBase { path: String },
    PathAccessedAtBase { path: String },
}

#[derive(Debug, PartialEq)]
pub struct KnownSubset {
    pub a: String,
    pub b: String,
}

#[derive(Debug, PartialEq)]
pub struct Placeholder {
    pub origin: String,
    pub loan: String,
}

impl Statement {
    pub(crate) fn new(effects: Vec<Effect>) -> Self {
        // Anything live on entry to the "mid point" is also live on
        // entry to the start point.
        let effects_start = effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Fact(Fact::OriginLiveOnEntry { .. })))
            .cloned()
            .collect();

        Self {
            effects_start,
            effects,
        }
    }
}
