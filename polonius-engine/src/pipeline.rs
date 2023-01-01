use std::time::Instant;

use rustc_hash::FxHashSet;

use crate::dump::{Dump, Dumper};
use crate::{compute, Computation, Db, FactTypes, LoadFrom, Rels, StoreTo};

pub struct Pipeline<T: FactTypes>(&'static [&'static dyn ComputationDyn<T>]);

macro_rules! pipeline {
    ( $($unit:expr),* $(,)? ) => {
        $crate::pipeline::Pipeline::new(&[
            $( &$unit as _, )*
        ])
    };
}

impl<T: FactTypes> Pipeline<T> {
    pub fn new(x: &'static [&'static dyn ComputationDyn<T>]) -> Self {
        Pipeline(x)
    }

    pub(crate) fn naive() -> Self {
        pipeline![
            compute::Cfg,
            compute::Paths,
            compute::MaybeInit,
            compute::VarDroppedWhileInit,
            compute::MaybeUninit,
            compute::MoveError,
            compute::KnownPlaceholder,
            compute::LiveOrigins,
            compute::BorrowckNaive,
        ]
    }

    pub(crate) fn opt() -> Self {
        pipeline![
            compute::Cfg,
            compute::Paths,
            compute::MaybeInit,
            compute::VarDroppedWhileInit,
            compute::MaybeUninit,
            compute::MoveError,
            compute::KnownPlaceholder,
            compute::LiveOrigins,
            compute::BorrowckOptimized,
        ]
    }

    pub(crate) fn location_insensitive() -> Self {
        pipeline![
            compute::Cfg,
            compute::Paths,
            compute::MaybeInit,
            compute::VarDroppedWhileInit,
            compute::MaybeUninit,
            compute::MoveError,
            compute::KnownPlaceholder,
            compute::KnownPlaceholderLoans,
            compute::LiveOrigins,
            compute::BorrowckLocationInsensitive,
            compute::BorrowckLocationInsensitiveAsSensitive,
        ]
    }

    pub(crate) fn compare() -> Self {
        pipeline![
            compute::Cfg,
            compute::Paths,
            compute::MaybeInit,
            compute::VarDroppedWhileInit,
            compute::MaybeUninit,
            compute::MoveError,
            compute::KnownPlaceholder,
            compute::LiveOrigins,
            compute::BorrowckNaive,
            compute::BorrowckOptimized,
        ]
    }

    pub(crate) fn hybrid() -> Self {
        pipeline![
            compute::Cfg,
            compute::Paths,
            compute::MaybeInit,
            compute::VarDroppedWhileInit,
            compute::MaybeUninit,
            compute::MoveError,
            compute::KnownPlaceholder,
            compute::KnownPlaceholderLoans,
            compute::LiveOrigins,
            compute::BorrowckLocationInsensitive,
            compute::BorrowckOptimized,
        ]
    }

    pub fn compute<I, O>(&self, input: I, dumpers: Vec<&mut dyn Dumper>) -> O
    where
        I: StoreTo<T>,
        O: for<'db> LoadFrom<'db, T>,
    {
        self.validate(I::RELATIONS, O::RELATIONS);

        let mut cx = Dump::new(dumpers);

        let mut facts = Db::default();
        input.store_to_db(&mut facts, &mut cx);

        // FIXME: clean up relations that aren't needed for the final output in-between units.
        for unit in self.0 {
            unit.compute(&mut facts, &mut cx);
        }

        O::load_from_db(&facts)
    }

    /// Check that this pipeline is able to compute the specified outputs if given the specificied
    /// inputs.
    ///
    /// Panics if this requirement is not met.
    fn validate(&self, inputs: Rels, outputs: Rels) {
        let mut available: FxHashSet<&str> = Default::default();
        available.extend(inputs.iter());

        for unit in self.0 {
            // Ensure that the required inputs have all been computed
            for input in unit.inputs() {
                if !available.contains(input) {
                    panic!(
                        "`{}` required by {} but not provided by input or preceding computation",
                        input,
                        unit.name()
                    )
                }
            }
            available.extend(unit.outputs());
        }

        for output in outputs {
            if !available.contains(output) {
                panic!(
                    "Required output `{}` not computed by any computation",
                    output
                )
            }
        }
    }
}

/// An object-safe wrapper around a [`Computation`].
pub trait ComputationDyn<T: FactTypes> {
    /// A human-readable name for this computation.
    fn name(&self) -> &'static str;

    /// The required inputs.
    fn inputs(&self) -> Rels;

    /// The outputs.
    fn outputs(&self) -> Rels;

    /// Loads the input for a [`Computation`], runs it, and stores the result.
    fn compute(&self, db: &mut Db<T>, dump: &mut Dump<'_>);
}

// `#![feature(associated_type_bounds)]` is required for this impl. Otherwise the type parameters
// we create to represent for `Input` and `Output` are unbound.
//
// FIXME: Can we write this impl without the feature gate?
impl<C, T> ComputationDyn<T> for C
where
    C: Computation<T>,
    for<'db> C::Input<'db>: LoadFrom<'db, T>,
    C::Output: StoreTo<T>,
    T: FactTypes,
{
    fn name(&self) -> &'static str {
        readable_typename::<C>()
    }

    fn inputs(&self) -> Rels {
        <C::Input<'_>>::RELATIONS
    }

    fn outputs(&self) -> Rels {
        <C::Output>::RELATIONS
    }

    fn compute(&self, db: &mut Db<T>, dump: &mut Dump<'_>) {
        compute_(self, db, dump)
    }
}

/// Loads the input for a [`Computation`], runs it, and stores the result.
fn compute_<C, T>(computation: &C, db: &mut Db<T>, dump: &mut Dump<'_>)
where
    C: Computation<T>,
    for<'db> C::Input<'db>: LoadFrom<'db, T>,
    C::Output: StoreTo<T>,
    T: FactTypes,
{
    let name = readable_typename::<C>();
    db.curr_unit = name;
    info!("Running computation `{}`...", name);

    let input = <C::Input<'_>>::load_from_db(db);
    dump.unit_start(name);
    let start_time = Instant::now();
    let output = computation.compute(input, dump);
    let end_time = Instant::now();
    dump.unit_end(name);

    output.store_to_db(db, dump);

    let elapsed_time = end_time - start_time;
    info!(
        "Finished computation `{}` in {:.5}s",
        name,
        elapsed_time.as_secs_f64(),
    );
}

fn readable_typename<T>() -> &'static str {
    std::any::type_name::<T>().split(':').last().unwrap()
}
