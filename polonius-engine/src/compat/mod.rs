use crate::{FactTypes, Pipeline};

mod all_facts;
mod output;

pub use self::all_facts::AllFacts;
pub use self::output::Output;

#[derive(Debug, Clone, Copy)]
pub enum Algorithm {
    /// Simple rules, but slower to execute
    Naive,

    /// Optimized variant of the rules
    DatafrogOpt,

    /// Fast to compute, but imprecise: there can be false-positives
    /// but no false-negatives. Tailored for quick "early return" situations.
    LocationInsensitive,

    /// Compares the `Naive` and `DatafrogOpt` variants to ensure they indeed
    /// compute the same errors.
    Compare,

    /// Combination of the fast `LocationInsensitive` pre-pass, followed by
    /// the more expensive `DatafrogOpt` variant.
    Hybrid,
}

impl Algorithm {
    /// Optimized variants that ought to be equivalent to "naive"
    pub const OPTIMIZED: &'static [Algorithm] = &[Algorithm::DatafrogOpt];

    pub fn variants() -> [&'static str; 5] {
        [
            "Naive",
            "DatafrogOpt",
            "LocationInsensitive",
            "Compare",
            "Hybrid",
        ]
    }

    fn pipeline<T: FactTypes>(&self) -> Pipeline<T> {
        match self {
            Algorithm::Naive => Pipeline::naive(),
            Algorithm::DatafrogOpt => Pipeline::opt(),
            Algorithm::LocationInsensitive => Pipeline::location_insensitive(),
            Algorithm::Compare => Pipeline::compare(),
            Algorithm::Hybrid => Pipeline::hybrid(),
        }
    }
}

impl ::std::str::FromStr for Algorithm {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "naive" => Ok(Algorithm::Naive),
            "datafrogopt" => Ok(Algorithm::DatafrogOpt),
            "locationinsensitive" => Ok(Algorithm::LocationInsensitive),
            "compare" => Ok(Algorithm::Compare),
            "hybrid" => Ok(Algorithm::Hybrid),
            _ => Err(String::from(
                "valid values: Naive, DatafrogOpt, LocationInsensitive, Compare, Hybrid",
            )),
        }
    }
}
