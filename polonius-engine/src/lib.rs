/// Contains the core of the Polonius borrow checking engine.
/// Input is fed in via AllFacts, and outputs are returned via Output

mod facts;

// Reexports of facts
pub use facts::Atom;
pub use facts::AllFacts;
