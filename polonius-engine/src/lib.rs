#![feature(const_type_id)]
#![feature(generic_associated_types)]
#![feature(associated_type_bounds)]

#[macro_use]
mod util;
#[macro_use]
mod pipeline;
#[macro_use]
pub mod db;

mod compat;
pub mod compute;
pub mod dump;
mod tuples;

pub use self::compat::{Algorithm, AllFacts, Output};
#[doc(inline)]
pub use self::compute::Computation;
pub use self::db::{Db, LoadFrom, StoreTo};
pub use self::dump::{Dump, Dumper};
pub use self::pipeline::{ComputationDyn, Pipeline};
pub use self::tuples::{RawTuple, Tuple, TupleIter, TupleSchema, TupleVec};
pub use self::tuples::{downcast_vec, downcast_iter};

type Rel = &'static str;
type Rels = &'static [Rel];

pub trait Atom:
    From<usize> + Into<usize> + Copy + Clone + std::fmt::Debug + Eq + Ord + std::hash::Hash + 'static
{
    fn index(self) -> usize;
}

pub trait FactTypes: Copy + Clone + std::fmt::Debug + 'static {
    type Origin: Atom;
    type Loan: Atom;
    type Point: Atom;
    type Variable: Atom;
    type Path: Atom;
}

pub mod internal {
    pub use crate::db::io::store_to_db_field;
}
