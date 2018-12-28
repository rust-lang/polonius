#![cfg(test)]

use crate::facts::{AllFacts, Loan, Point, Region};
use crate::intern;
use crate::program::parse_from_program;
use crate::tab_delim;
use crate::test_util::assert_equal;
use failure::Error;
use polonius_engine::{Algorithm, Output};
use rustc_hash::FxHashMap;
use std::path::Path;

fn test_facts(all_facts: &AllFacts, algorithms: &[Algorithm]) {
    let naive = Output::compute(all_facts, Algorithm::Naive, true);

    // If the insensitive analysis concludes no errors, then naive
    // should also.
    let insensitive = Output::compute(all_facts, Algorithm::LocationInsensitive, false);
    for (naive_point, naive_loans) in &naive.errors {
        match insensitive.errors.get(&naive_point) {
            Some(insensitive_loans) => {
                for naive_loan in naive_loans {
                    if !insensitive_loans.contains(naive_loan) {
                        panic!(
                            "naive analysis had error for `{:?}` at `{:?}` \
                             but insensitive analysis did not \
                             (loans = {:#?})",
                            naive_point, naive_loan, insensitive_loans,
                        );
                    }
                }
            }

            None => {
                panic!(
                    "naive analysis had errors at `{:?}` but insensitive analysis did not \
                     (loans = {:#?})",
                    naive_point, naive_loans,
                );
            }
        }
    }

    // The optimized checks should behave exactly the same as the naive check.
    for &optimized_algorithm in algorithms {
        println!("Algorithm {:?}", optimized_algorithm);
        let opt = Output::compute(all_facts, optimized_algorithm, true);
        assert_equal(&naive.borrow_live_at, &opt.borrow_live_at);
        assert_equal(&naive.errors, &opt.errors);
    }
}

fn test_fn(dir_name: &str, fn_name: &str, algorithm: Algorithm) -> Result<(), Error> {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join(dir_name)
        .join("nll-facts")
        .join(fn_name);
    println!("facts_dir = {:?}", facts_dir);
    let tables = &mut intern::InternerTables::new();
    let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
    Ok(test_facts(&all_facts, &[algorithm]))
}

macro_rules! tests {
    ($($name:ident($dir:expr, $fn:expr),)*) => {
        $(
            mod $name {
                use super::*;

                #[test]
                fn datafrog_opt() -> Result<(), Error> {
                    test_fn($dir, $fn, Algorithm::DatafrogOpt)
                }
            }
        )*
    }
}

tests! {
    issue_47680("issue-47680", "main"),
    vec_push_ref_foo1("vec-push-ref", "foo1"),
    vec_push_ref_foo2("vec-push-ref", "foo2"),
    vec_push_ref_foo3("vec-push-ref", "foo3"),
}

#[test]
fn test_insensitive_errors() -> Result<(), Error> {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("issue-47680")
        .join("nll-facts")
        .join("main");
    println!("facts_dir = {:?}", facts_dir);
    let tables = &mut intern::InternerTables::new();
    let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
    let insensitive = Output::compute(&all_facts, Algorithm::LocationInsensitive, false);

    let mut expected = FxHashMap::default();
    expected.insert(Point::from(1), vec![Loan::from(1)]);
    expected.insert(Point::from(2), vec![Loan::from(2)]);

    assert_equal(&insensitive.errors, &expected);
    Ok(())
}

#[test]
fn test_sensitive_passes_issue_47680() -> Result<(), Error> {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("issue-47680")
        .join("nll-facts")
        .join("main");
    let tables = &mut intern::InternerTables::new();
    let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
    let sensitive = Output::compute(&all_facts, Algorithm::DatafrogOpt, false);

    assert!(sensitive.errors.is_empty());
    Ok(())
}

#[test]
fn no_subset_symmetries_exist() -> Result<(), Error> {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("issue-47680")
        .join("nll-facts")
        .join("main");
    let tables = &mut intern::InternerTables::new();
    let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;

    let subset_symmetries_exist = |output: &Output<Region, Loan, Point>| {
        for (_, subsets) in &output.subset {
            for (r1, rs) in subsets {
                if rs.contains(&r1) {
                    return true;
                }
            }
        }
        false
    };

    let naive = Output::compute(&all_facts, Algorithm::Naive, true);
    assert!(!subset_symmetries_exist(&naive));

    // FIXME: the issue-47680 dataset is suboptimal here as DatafrogOpt does not
    // produce subset symmetries for it. It does for clap, and it was used to manually verify
    // that the assert in verbose  mode didn't trigger. Therefore, switch to this dataset
    // whenever it's fast enough to be enabled in tests, or somehow create a test facts program
    // or reduce it from clap.
    let opt = Output::compute(&all_facts, Algorithm::DatafrogOpt, true);
    assert!(!subset_symmetries_exist(&opt));
    Ok(())
}

// The following 3 tests, `send_is_not_static_std_sync`, `escape_upvar_nested`, and `issue_31567`
// are extracted from rustc's test suite, and fail because of differences between the Naive
// and DatafrogOpt variants, on the computation of the transitive closure.
// They are part of the same pattern that the optimized variant misses, and only differ in
// the length of the `outlives` chain reaching a live region at a specific point.

#[test]
fn send_is_not_static_std_sync() {
    // Reduced from rustc test: ui/span/send-is-not-static-std-sync.rs
    // (in the functions: `mutex` and `rwlock`)
    let program = r"
        universal_regions { }
        block B0 {
            borrow_region_at('a, L0), outlives('a: 'b), region_live_at('b);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");
    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
fn escape_upvar_nested() {
    // Reduced from rustc test: ui/nll/closure-requirements/escape-upvar-nested.rs
    // (in the function: `test-\{\{closure\}\}-\{\{closure\}\}/`)
    // This reduction is also present in other tests:
    // - ui/nll/closure-requirements/escape-upvar-ref.rs, in the `test-\{\{closure\}\}/` function
    let program = r"
        universal_regions { }
        block B0 {
            borrow_region_at('a, L0), outlives('a: 'b), outlives('b: 'c), region_live_at('c);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");
    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
fn issue_31567() {
    // Reduced from rustc test: ui/nll/issue-31567.rs
    // This is one of two tuples present in the Naive results and missing from the Opt results,
    // the second tuple having the same pattern as the one in this test.
    // This reduction is also present in other tests:
    // - ui/issue-48803.rs, in the `flatten` function
    let program = r"
        universal_regions { }
        block B0 {
            borrow_region_at('a, L0),
            outlives('a: 'b),
            outlives('b: 'c),
            outlives('c: 'd),
            region_live_at('d);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");
    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
fn borrowed_local_error() {
    // This test is related to the previous 3: there is still a borrow_region outliving a live region,
    // through a chain of `outlives` at a single point, but this time there are also 2 points
    // and an edge.

    // Reduced from rustc test: ui/nll/borrowed-local-error.rs
    // (in the function: `gimme`)
    // This reduction is also present in other tests:
    // - ui/nll/borrowed-temporary-error.rs, in the `gimme` function
    // - ui/nll/borrowed-referent-issue-38899.rs, in the `bump` function
    // - ui/nll/return-ref-mut-issue-46557.rs, in the `gimme_static_mut` function
    // - ui/span/dropck_direct_cycle_with_drop.rs, in the `{{impl}}[1]-drop-{{closure}}` function
    // - ui/span/wf-method-late-bound-regions.rs, in the `{{impl}}-xmute` function
    let program = r"
        universal_regions { 'c }
        block B0 {
            borrow_region_at('a, L0), outlives('a: 'b), outlives('b: 'c);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");
    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
fn smoke_test_errors() {
    let failures = [
        "return_ref_to_local",
        "use_while_mut",
        "use_while_mut_fr",
        "well_formed_function_inputs",
    ];

    for test_fn in &failures {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join("smoke-test")
            .join("nll-facts")
            .join(test_fn);
        println!("facts_dir = {:?}", facts_dir);
        let tables = &mut intern::InternerTables::new();
        let facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir).expect("facts");

        let location_insensitive = Output::compute(&facts, Algorithm::LocationInsensitive, true);
        assert!(
            !location_insensitive.errors.is_empty(),
            format!("LocationInsensitive didn't find errors for '{}'", test_fn)
        );

        let naive = Output::compute(&facts, Algorithm::Naive, true);
        assert!(
            !naive.errors.is_empty(),
            format!("Naive didn't find errors for '{}'", test_fn)
        );

        let opt = Output::compute(&facts, Algorithm::DatafrogOpt, true);
        assert!(
            !opt.errors.is_empty(),
            format!("DatafrogOpt didn't find errors for '{}'", test_fn)
        );
    }
}

#[test]
fn smoke_test_success_1() {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("smoke-test")
        .join("nll-facts")
        .join("position_dependent_outlives");
    println!("facts_dir = {:?}", facts_dir);
    let tables = &mut intern::InternerTables::new();
    let facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir).expect("facts");

    let location_insensitive = Output::compute(&facts, Algorithm::LocationInsensitive, true);
    assert!(!location_insensitive.errors.is_empty());

    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
fn smoke_test_success_2() {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("smoke-test")
        .join("nll-facts")
        .join("foo");
    println!("facts_dir = {:?}", facts_dir);
    let tables = &mut intern::InternerTables::new();
    let facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir).expect("facts");

    let location_insensitive = Output::compute(&facts, Algorithm::LocationInsensitive, true);
    assert!(location_insensitive.errors.is_empty());

    test_facts(&facts, Algorithm::OPTIMIZED);
}
