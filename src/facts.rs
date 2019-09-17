use polonius_engine::{self, FactTypes};

#[derive(Copy, Clone, Debug)]
pub(crate) struct LocalFacts;

pub(crate) type AllFacts = polonius_engine::AllFacts<LocalFacts>;

macro_rules! index_type {
    ($t:ident) => {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Debug, Hash)]
        pub(crate) struct $t {
            index: u32,
        }

        impl From<usize> for $t {
            fn from(index: usize) -> $t {
                $t {
                    index: index as u32,
                }
            }
        }

        impl Into<usize> for $t {
            fn into(self) -> usize {
                self.index as usize
            }
        }

        impl polonius_engine::Atom for $t {
            fn index(self) -> usize {
                self.into()
            }
        }
    };
}

index_type!(Origin);
index_type!(Loan);
index_type!(Point);
index_type!(Variable);
index_type!(Path);

impl FactTypes for LocalFacts {
    type Origin = Origin;
    type Loan = Loan;
    type Point = Point;
    type Variable = Variable;
    type Path = Path;
}
