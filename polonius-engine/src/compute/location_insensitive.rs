use super::BorrowckErrors;
use crate::{Computation, Dump, FactTypes};
use datafrog::{Iteration, Relation, RelationLeaper};

input! {
    BorrowckLocationInsensitiveInput {
        origin_live_on_entry,
        loan_invalidated_at,
        known_placeholder_requires,
        placeholder,
        loan_issued_at,
        subset_base,
    }
}

output! {
    BorrowckLocationInsensitiveErrors {
        potential_errors,
        potential_subset_errors,
    }
}

#[derive(Clone, Copy)]
pub struct BorrowckLocationInsensitive;

impl<T: FactTypes> Computation<T> for BorrowckLocationInsensitive {
    type Input<'db> = BorrowckLocationInsensitiveInput<'db, T>;
    type Output = BorrowckLocationInsensitiveErrors<T>;

    fn compute(&self, input: Self::Input<'_>, dump: &mut Dump) -> Self::Output {
        let BorrowckLocationInsensitiveInput {
            origin_live_on_entry,
            loan_invalidated_at,
            loan_issued_at,
            placeholder: placeholder_loan,
            known_placeholder_requires: known_contains,
            subset_base,
        } = input;

        let placeholder_loan_lo: Relation<_> =
            placeholder_loan.iter().map(|&(o, l)| (l, o)).collect();
        let placeholder_origin: Relation<_> =
            placeholder_loan.iter().map(|&(o, _l)| (o, ())).collect();

        // subset(Origin1, Origin2) :-
        //   subset_base(Origin1, Origin2, _).
        let subset = Relation::from_iter(
            subset_base
                .iter()
                .map(|&(origin1, origin2, _point)| (origin1, origin2)),
        );

        // Create a new iteration context, ...
        let mut iteration = Iteration::new();

        // .. some variables, ..
        let origin_contains_loan_on_entry =
            iteration.variable::<(T::Origin, T::Loan)>("origin_contains_loan_on_entry");

        let potential_errors = iteration.variable::<(T::Loan, T::Point)>("potential_errors");
        let potential_subset_errors =
            iteration.variable::<(T::Origin, T::Origin)>("potential_subset_errors");

        // load initial facts.

        // origin_contains_loan_on_entry(Origin, Loan) :-
        //   loan_issued_at(Origin, Loan, _).
        origin_contains_loan_on_entry.extend(
            loan_issued_at
                .iter()
                .map(|&(origin, loan, _point)| (origin, loan)),
        );

        // origin_contains_loan_on_entry(Origin, Loan) :-
        //   placeholder_loan(Origin, Loan).
        origin_contains_loan_on_entry.extend(placeholder_loan.iter().copied());

        // .. and then start iterating rules!
        while iteration.changed() {
            // origin_contains_loan_on_entry(Origin2, Loan) :-
            //   origin_contains_loan_on_entry(Origin1, Loan),
            //   subset(Origin1, Origin2).
            //
            // Note: Since `subset` is effectively a static input, this join can be ported to
            // a leapjoin. Doing so, however, was 7% slower on `clap`.
            origin_contains_loan_on_entry.from_join(
                &origin_contains_loan_on_entry,
                &subset,
                |&_origin1, &loan, &origin2| (origin2, loan),
            );

            // loan_live_at(Loan, Point) :-
            //   origin_contains_loan_on_entry(Origin, Loan),
            //   origin_live_on_entry(Origin, Point)
            //
            // potential_errors(Loan, Point) :-
            //   loan_invalidated_at(Loan, Point),
            //   loan_live_at(Loan, Point).
            //
            // Note: we don't need to materialize `loan_live_at` here
            // so we can inline it in the `potential_errors` relation.
            //
            potential_errors.from_leapjoin(
                &origin_contains_loan_on_entry,
                (
                    origin_live_on_entry.extend_with(|&(origin, _loan)| origin),
                    loan_invalidated_at.extend_with(|&(_origin, loan)| loan),
                ),
                |&(_origin, loan), &point| (loan, point),
            );

            // potential_subset_errors(Origin1, Origin2) :-
            //   placeholder(Origin1, Loan1),
            //   placeholder(Origin2, _),
            //   origin_contains_loan_on_entry(Origin2, Loan1),
            //   !known_contains(Origin2, Loan1).
            potential_subset_errors.from_leapjoin(
                &origin_contains_loan_on_entry,
                (
                    known_contains.filter_anti(|&(origin2, loan1)| (origin2, loan1)),
                    placeholder_origin.filter_with(|&(origin2, _loan1)| (origin2, ())),
                    placeholder_loan_lo.extend_with(|&(_origin2, loan1)| loan1),
                    // remove symmetries:
                    datafrog::ValueFilter::from(|&(origin2, _loan1), &origin1| origin2 != origin1),
                ),
                |&(origin2, _loan1), &origin1| (origin1, origin2),
            );
        }

        dump.var(&origin_contains_loan_on_entry);
        dump.rel("subset", subset);

        Self::Output {
            potential_errors: potential_errors.complete(),
            potential_subset_errors: potential_subset_errors.complete(),
        }
    }
}

/// Copies `potential_errors` and `potential_subset_errors` into `errors` and `subset_errors`
/// respectively.
///
/// This is a hack to conform to the old `Output` interface. It will cause a panic if run
/// alongside any other location-sensitive borrow-checking one, since the results may not match.
#[derive(Clone, Copy)]
pub struct BorrowckLocationInsensitiveAsSensitive;

input! {
    BorrowckLocationInsensitiveErrorsRef {
        potential_errors,
        potential_subset_errors,
    }
}

impl<T: FactTypes> Computation<T> for BorrowckLocationInsensitiveAsSensitive {
    type Input<'db> = BorrowckLocationInsensitiveErrorsRef<'db, T>;
    type Output = BorrowckErrors<T>;

    fn compute(&self, input: Self::Input<'_>, _dump: &mut Dump<'_>) -> Self::Output {
        BorrowckErrors {
            errors: input.potential_errors.clone(),
            subset_errors: input
                .potential_subset_errors
                .iter()
                .map(|&(o1, o2)| (o1, o2, 0.into()))
                .collect(),
        }
    }
}
