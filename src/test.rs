#![cfg(test)]

use crate::facts::{Loan, Point, Region};
use crate::intern;
use crate::tab_delim;
use failure::Error;
use polonius_engine::{Algorithm, AllFacts, Output};
use rustc_hash::FxHashMap;
use std::path::Path;

fn test_fn(dir_name: &str, fn_name: &str) -> Result<(), Error> {
    try {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join(dir_name)
            .join("nll-facts")
            .join(fn_name);
        println!("facts_dir = {:?}", facts_dir);
        let tables = &mut intern::InternerTables::new();
        let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
        let naive = Output::compute(&all_facts, Algorithm::Naive, false);
        let opt = Output::compute(&all_facts, Algorithm::DatafrogOpt, true);
        assert_eq!(naive.borrow_live_at, opt.borrow_live_at);
    }
}

macro_rules! tests {
    ($($name:ident($dir:expr, $fn:expr),)*) => {
        $(
            #[test]
            fn $name() -> Result<(), Error> {
                test_fn($dir, $fn)
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

        assert_eq!(insensitive.errors, expected);
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
    let mut facts = AllFacts::default();

    let p0 = Point::from(0);
    let l0 = Loan::from(0);
    let (r0, r1) = (
        Region::from(0),
        Region::from(1),
    );

    facts.borrow_region.push((r0, l0, p0));
    facts.outlives.push((r0, r1, p0));
    facts.region_live_at.push((r1, p0));

    let naive = Output::compute(&facts, Algorithm::Naive, true);
    let opt = Output::compute(&facts, Algorithm::DatafrogOpt, true);
    assert_eq!(naive.borrow_live_at, opt.borrow_live_at);
}

#[test]
#[should_panic]
fn escape_upvar_nested() {
    // Reduced from rustc test: ui/nll/closure-requirements/escape-upvar-nested.rs
    // (in the function: `test-\{\{closure\}\}-\{\{closure\}\}/`)
    // This reduction is also present in other tests:
    // - ui/nll/closure-requirements/escape-upvar-ref.rs, in the `test-\{\{closure\}\}/` function
    let mut facts = AllFacts::default();

    let p0 = Point::from(0);
    let l0 = Loan::from(0);
    let (r0, r1, r2) = (
        Region::from(0),
        Region::from(1),
        Region::from(2),
    );

    facts.borrow_region.push((r0, l0, p0));
    facts.outlives.push((r0, r1, p0));
    facts.outlives.push((r1, r2, p0));
    facts.region_live_at.push((r2, p0));

    let naive = Output::compute(&facts, Algorithm::Naive, true);
    let opt = Output::compute(&facts, Algorithm::DatafrogOpt, true);
    assert_eq!(naive.borrow_live_at, opt.borrow_live_at);
}

#[test]
#[should_panic]
fn issue_31567() {
    // Reduced from rustc test: ui/nll/issue-31567.rs
    // This is one of two tuples present in the Naive results and missing from the Opt results,
    // the second tuple having the same pattern as the one in this test.
    // This reduction is also present in other tests:
    // - ui/issue-48803.rs, in the `flatten` function
    let mut facts = AllFacts::default();

    let p0 = Point::from(0);
    let l0 = Loan::from(0);
    let (r0, r1, r2, r3) = (
        Region::from(0),
        Region::from(1),
        Region::from(2),
        Region::from(3),
    );

    facts.borrow_region.push((r0, l0, p0));
    facts.outlives.push((r0, r1, p0));
    facts.outlives.push((r1, r2, p0));
    facts.outlives.push((r2, r3, p0));
    facts.region_live_at.push((r3, p0));

    let naive = Output::compute(&facts, Algorithm::Naive, true);
    let opt = Output::compute(&facts, Algorithm::DatafrogOpt, true);
    assert_eq!(naive.borrow_live_at, opt.borrow_live_at);
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
    let mut facts = AllFacts::default();

    let (p0, p1) = (
        Point::from(0),
        Point::from(1),
    );
    let l0 = Loan::from(0);
    let (r0, r1, r2) = (
        Region::from(0),
        Region::from(1),
        Region::from(2),
    );

    facts.cfg_edge.push((p0, p1));
    facts.borrow_region.push((r0, l0, p1));
    facts.outlives.push((r0, r1, p1));
    facts.outlives.push((r1, r2, p1));
    facts.universal_region.push(r2);

    let naive = Output::compute(&facts, Algorithm::Naive, true);
    let opt = Output::compute(&facts, Algorithm::DatafrogOpt, true);
    assert_eq!(naive.borrow_live_at, opt.borrow_live_at);
}
