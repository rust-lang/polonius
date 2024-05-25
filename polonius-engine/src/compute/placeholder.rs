use datafrog::{Iteration, RelationLeaper};

use super::{Computation, Dump};
use crate::FactTypes;

#[derive(Clone, Copy)]
pub struct KnownPlaceholder;

input! {
    KnownPlaceholderSubsetBase { known_placeholder_subset_base }
}

output!(known_placeholder_subset);

impl<T: FactTypes> Computation<T> for KnownPlaceholder {
    type Input<'db> = KnownPlaceholderSubsetBase<'db, T>;
    type Output = KnownPlaceholderSubset<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let KnownPlaceholderSubsetBase {
            known_placeholder_subset_base,
        } = input;

        let mut iteration = Iteration::new();

        let known_placeholder_subset = iteration.variable("known_placeholder_subset");

        // known_placeholder_subset(Origin1, Origin2) :-
        //   known_placeholder_subset_base(Origin1, Origin2).
        known_placeholder_subset.extend(known_placeholder_subset_base.iter());

        while iteration.changed() {
            // known_placeholder_subset(Origin1, Origin3) :-
            //   known_placeholder_subset(Origin1, Origin2),
            //   known_placeholder_subset_base(Origin2, Origin3).
            known_placeholder_subset.from_leapjoin(
                &known_placeholder_subset,
                known_placeholder_subset_base.extend_with(|&(_origin1, origin2)| origin2),
                |&(origin1, _origin2), &origin3| (origin1, origin3),
            );
        }

        known_placeholder_subset.complete().into()
    }
}

#[derive(Clone, Copy)]
pub struct KnownPlaceholderLoans;

input! {
    KnownPlaceholderRequiresInput {
        known_placeholder_subset,
        placeholder,
    }
}

output!(known_placeholder_requires);

impl<T: FactTypes> Computation<T> for KnownPlaceholderLoans {
    type Input<'db> = KnownPlaceholderRequiresInput<'db, T>;
    type Output = KnownPlaceholderRequires<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        let KnownPlaceholderRequiresInput {
            known_placeholder_subset,
            placeholder,
        } = input;

        let mut iteration = datafrog::Iteration::new();
        let known_contains = iteration.variable("known_contains");

        // known_contains(Origin1, Loan1) :-
        //   placeholder(Origin1, Loan1).
        known_contains.extend(placeholder.iter());

        while iteration.changed() {
            // known_contains(Origin2, Loan1) :-
            //   known_contains(Origin1, Loan1),
            //   known_placeholder_subset(Origin1, Origin2).
            known_contains.from_join(
                &known_contains,
                known_placeholder_subset,
                |&_origin1, &loan1, &origin2| (origin2, loan1),
            );
        }

        known_contains.complete().into()
    }
}
