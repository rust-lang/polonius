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
    for &optimized_algorithm in algorithms {
        println!("Algorithm {:?}", optimized_algorithm);
        let opt = Output::compute(all_facts, optimized_algorithm, true);
        assert_equal(&naive.borrow_live_at, &opt.borrow_live_at);
    }
}

fn test_fn(dir_name: &str, fn_name: &str, algorithm: Algorithm) -> Result<(), Error> {
    try {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join(dir_name)
            .join("nll-facts")
            .join(fn_name);
        println!("facts_dir = {:?}", facts_dir);
        let tables = &mut intern::InternerTables::new();
        let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
        test_facts(&all_facts, &[algorithm])
    }
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
}

#[test]
fn test_insensitive_errors() -> Result<(), Error> {
    try {
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
    }
}

#[test]
fn test_sensitive_passes_issue_47680() -> Result<(), Error> {
    try {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join("issue-47680")
            .join("nll-facts")
            .join("main");
        let tables = &mut intern::InternerTables::new();
        let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
        let sensitive = Output::compute(&all_facts, Algorithm::DatafrogOpt, false);

        assert!(sensitive.errors.is_empty());
    }
}

#[test]
fn no_subset_symmetries_exist() -> Result<(), Error> {
    try {
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
    }
}

// The following 3 tests, `send_is_not_static_std_sync`, `escape_upvar_nested`, and `issue_31567`
// are extracted from rustc's test suite, and fail because of differences between the Naive
// and DatafrogOpt variants, on the computation of the transitive closure.
// They are part of the same pattern that the optimized variant misses, and only differ in
// the length of the `outlives` chain reaching a live region at a specific point.

#[test]
#[should_panic]
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
#[should_panic]
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
#[should_panic]
fn issue_31567() {
    // Reduced from rustc test: ui/nll/issue-31567.rs
    // This is one of two tuples present in the Naive results and missing from the Opt results,
    // the second tuple having the same pattern as the one in this test.
    // This reduction is also present in other tests:
    // - ui/issue-48803.rs, in the `flatten` function
    let program = r"
        universal_regions { }
        block B0 {
            borrow_region_at('a, L0), outlives('a: 'b), outlives('b: 'c), outlives('c: 'd), region_live_at('d);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");
    test_facts(&facts, Algorithm::OPTIMIZED);
}

#[test]
#[should_panic]
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
