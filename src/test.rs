#![cfg(test)]

use crate::dump::Output;
use crate::facts::{AllFacts, Loan, Origin, Point};
use crate::intern;
use crate::program::parse_from_program;
use crate::tab_delim;
use crate::test_util::{assert_equal, check_program};
use polonius_engine::Algorithm;
use rustc_hash::FxHashMap;
use std::error::Error;
use std::path::Path;

fn test_facts(all_facts: &AllFacts, algorithms: &[Algorithm]) {
    let naive = Output::compute(all_facts, Algorithm::Naive, true);

    // Check that the "naive errors" are a subset of the "insensitive
    // ones".
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
                            naive_loan, naive_point, insensitive_loans,
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
        // TMP: until we reach our correctness goals, deactivate some comparisons between variants
        // assert_equal(&naive.borrow_live_at, &opt.borrow_live_at);
        assert_equal(&naive.errors, &opt.errors);
    }

    // The hybrid algorithm gets the same errors as the naive version
    let opt = Output::compute(all_facts, Algorithm::Hybrid, true);
    assert_equal(&naive.errors, &opt.errors);
}

fn test_fn(dir_name: &str, fn_name: &str, algorithm: Algorithm) -> Result<(), Box<dyn Error>> {
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
                fn datafrog_opt() -> Result<(), Box<dyn Error>> {
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
fn test_insensitive_errors() -> Result<(), Box<dyn Error>> {
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
    expected.insert(Point::from(24), vec![Loan::from(1)]);
    expected.insert(Point::from(48), vec![Loan::from(2)]);

    assert_equal(&insensitive.errors, &expected);
    Ok(())
}

#[test]
fn test_sensitive_passes_issue_47680() -> Result<(), Box<dyn Error>> {
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
fn no_subset_symmetries_exist() -> Result<(), Box<dyn Error>> {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("issue-47680")
        .join("nll-facts")
        .join("main");
    let tables = &mut intern::InternerTables::new();
    let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;

    let subset_symmetries_exist = |output: &Output| {
        for (_, subsets) in &output.subset {
            for (origin, origins) in subsets {
                if origins.contains(&origin) {
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
// the length of the `outlives` chain reaching a live origin at a specific point.

#[test]
fn send_is_not_static_std_sync() {
    // Reduced from rustc test: ui/span/send-is-not-static-std-sync.rs
    // (in the functions: `mutex` and `rwlock`)
    let program = r"
        placeholders { }
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
        placeholders { }
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
        placeholders { }
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
    // This test is related to the previous 3: there is still a borrow_region outliving a live origin,
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
        placeholders { 'c }
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

#[test]
// `var` used in `point` => `var` live upon entry into `point`
fn var_live_in_single_block() {
    let program = r"
        placeholders { }

        block B0 {
            var_used(V1);
            goto B1;
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");

    let liveness = Output::compute(&facts, Algorithm::Naive, true).var_live_at;
    println!("Registered liveness data: {:?}", liveness);
    for (point, variables) in liveness.iter() {
        println!("{:?} has live variables: {:?}", point, variables);
        assert_eq!(variables.len(), 1);
    }
    assert_eq!(liveness.len(), 2);
}

#[test]
// `point1` GOTO `point2`, `var` used in `point2` => `var` live in `point1`
fn var_live_in_successor_propagates_to_predecessor() {
    let program = r"
        placeholders { }

        block B0 {
            invalidates(L0); // generate a point
            goto B1;
        }

        block B1 {
            invalidates(L0);
            goto B2;
        }

        block B2 {
            invalidates(L0);
            var_used(V1);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");

    let liveness = Output::compute(&facts, Algorithm::Naive, true).var_live_at;
    println!("Registered liveness data: {:?}", liveness);
    println!("CFG: {:?}", facts.cfg_edge);
    for (point, variables) in liveness.iter() {
        println!("{:?} has live variables: {:?}", point, variables);
        assert_eq!(variables.len(), 1);
    }

    assert!(!liveness.get(&0.into()).unwrap().is_empty());
}

#[test]
// `point1` GOTO `point2`, `var` used in `point2`, `var` defined in `point1` => `var` not live in `point1`
fn var_live_in_successor_killed_by_reassignment() {
    let program = r"
        placeholders { }

        block B0 {
            invalidates(L0); // generate a point
            goto B1;
        }

        block B1 {
            var_defined(V1); // V1 dies
            invalidates(L0);
            goto B2;
        }

        block B2 {
            invalidates(L0);
            var_used(V1);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");

    let result = Output::compute(&facts, Algorithm::Naive, true);
    println!("result: {:#?}", result);
    let liveness = result.var_live_at;
    println!("CFG: {:#?}", facts.cfg_edge);

    let first_defined: Point = 3.into(); // Mid(B1[0])

    for (&point, variables) in liveness.iter() {
        println!(
            "{} ({:?}) has live variables: {:?}",
            tables.points.untern(point),
            point,
            tables.variables.untern_vec(variables)
        );
    }

    let live_at_start = liveness.get(&0.into());

    assert_eq!(
        liveness.get(&0.into()),
        None,
        "{:?} were live at start!",
        live_at_start.and_then(|var| Some(tables.variables.untern_vec(var))),
    );

    let live_at_defined = liveness.get(&first_defined);

    assert_eq!(
        live_at_defined,
        None,
        "{:?} were alive at {}",
        live_at_defined.and_then(|var| Some(tables.variables.untern_vec(var))),
        tables.points.untern(first_defined)
    );
}

#[test]
fn var_drop_used_simple() {
    let program = r"
        placeholders { }

        block B0 {
            invalidates(L0); // generate a point
            goto B1;
        }

        block B1 {
            var_defined(V1); // V1 dies
            invalidates(L0);
            goto B2;
        }

        block B2 {
            invalidates(L0);
            var_drop_used(V1);
        }
    ";

    let mut tables = intern::InternerTables::new();
    let facts = parse_from_program(program, &mut tables).expect("Parsing failure");

    let result = Output::compute(&facts, Algorithm::Naive, true);
    println!("result: {:#?}", result);
    let liveness = result.var_drop_live_at;
    println!("CFG: {:#?}", facts.cfg_edge);
    let first_defined: Point = 3.into(); // Mid(B1[0])

    for (&point, variables) in liveness.iter() {
        println!(
            "{} ({:?}) has live variables: {:?}",
            tables.points.untern(point),
            point,
            tables.variables.untern_vec(variables)
        );
    }

    let live_at_start = liveness.get(&0.into());

    assert_eq!(
        liveness.get(&0.into()),
        None,
        "{:?} were live at start!",
        live_at_start.and_then(|var| Some(tables.variables.untern_vec(var))),
    );

    let live_at_defined = liveness.get(&first_defined);

    assert_eq!(
        live_at_defined,
        None,
        "{:?} were alive at {}",
        live_at_defined.and_then(|var| Some(tables.variables.untern_vec(var))),
        tables.points.untern(first_defined)
    );
}

/// This test ensures one of the two placeholder origins will flow into the
/// other, but without declaring this subset as a `known_subset`, which is
/// an illegal subset relation error.
#[test]
fn illegal_subset_error() {
    let program = r"
        placeholders { 'a, 'b }
        
        block B0 {
            // creates a transitive `'b: 'a` subset
            borrow_region_at('x, L0),
              outlives('b: 'x),
              outlives('x: 'a);
        }
    ";

    let mut checker = check_program(program, Algorithm::Naive, true);

    assert_eq!(checker.facts.universal_region.len(), 2);
    assert_eq!(checker.facts.placeholder.len(), 2);

    // no known subsets are defined in the program...
    assert_eq!(checker.facts.known_subset.len(), 0);

    // ...so there should be an error here about the missing `'b: 'a` subset
    assert_eq!(checker.subset_errors_count(), 1);
    assert!(checker.subset_error_exists("'b", "'a", "\"Mid(B0[0])\""));
}

/// This is the same test as the `illegal_subset_error` one, but specifies the `'b: 'a` subset
/// relation as being "known", making this program valid.
#[test]
fn known_placeholder_origin_subset() {
    let program = r"
        placeholders { 'a, 'b }
        known_subsets { 'b: 'a }

        block B0 {
            borrow_region_at('x, L0),
              outlives('b: 'x),
              outlives('x: 'a);
        }
    ";

    let checker = check_program(program, Algorithm::Naive, true);

    assert_eq!(checker.facts.universal_region.len(), 2);
    assert_eq!(checker.facts.placeholder.len(), 2);
    assert_eq!(checker.facts.known_subset.len(), 1);

    assert_eq!(checker.subset_errors_count(), 0);
}

/// This test ensures `known_subset`s are handled transitively: a known subset `'a: 'c` should be
/// known via transitivity, making this program valid.
#[test]
fn transitive_known_subset() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        known_subsets { 'a: 'b, 'b: 'c }
        
        block B0 {
            borrow_region_at('x, L0),
              outlives('a: 'x),
              outlives('x: 'c);
        }
    ";

    let checker = check_program(program, Algorithm::Naive, true);

    assert_eq!(checker.facts.universal_region.len(), 3);
    assert_eq!(checker.facts.placeholder.len(), 3);

    // the 2 `known_subset`s here mean 3 `known_contains`, transitively
    assert_eq!(checker.facts.known_subset.len(), 2);
    assert_eq!(checker.output.known_contains.len(), 3);

    assert_eq!(checker.subset_errors_count(), 0);
}

/// Even if `'a: 'b` is known, `'a`'s placeholder loan can flow into `'b''s supersets,
/// and this relation must be known for the program to be valid.
#[test]
fn transitive_illegal_subset_error() {
    let program = r"
        placeholders { 'a, 'b, 'c }
        known_subsets { 'a: 'b }
        
        block B0 {
            // this transitive `'a: 'b` subset is already known
            borrow_region_at('x, L0),
              outlives('a: 'x),
              outlives('x: 'b);
            // creates an unknown transitive `'a: 'c` subset
            borrow_region_at('y, L1),
              outlives('b: 'y),
              outlives('y: 'c);
        }
    ";

    let mut checker = check_program(program, Algorithm::Naive, true);

    assert_eq!(checker.facts.universal_region.len(), 3);
    assert_eq!(checker.facts.placeholder.len(), 3);
    assert_eq!(checker.facts.known_subset.len(), 1);

    // there should be an error here about the missing `'a: 'c` subset
    assert_eq!(checker.subset_errors_count(), 1);
    assert!(checker.subset_error_exists("'a", "'c", "\"Mid(B0[1])\""));
}

#[test]
fn successes_in_subset_relations_dataset() {
    let successes = ["valid_subset", "implied_bounds_subset"];

    // these tests have no illegal access errors or subset errors
    for test_fn in &successes {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join("subset-relations")
            .join("nll-facts")
            .join(test_fn);
        let tables = &mut intern::InternerTables::new();
        let facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir).expect("facts");

        let result = Output::compute(&facts, Algorithm::Naive, true);
        assert!(result.errors.is_empty());
        assert!(result.subset_errors.is_empty());
    }
}

#[test]
fn errors_in_subset_relations_dataset() {
    let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("inputs")
        .join("subset-relations")
        .join("nll-facts")
        .join("missing_subset");
    let tables = &mut intern::InternerTables::new();
    let facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir).expect("facts");

    // this function has no illegal access errors, but one subset error, over 3 points
    let result = Output::compute(&facts, Algorithm::Naive, true);
    assert!(result.errors.is_empty());
    assert_eq!(result.subset_errors.len(), 3);

    let points = ["\"Mid(bb0[0])\"", "\"Start(bb0[1])\"", "\"Mid(bb0[1])\""];
    for point in &points {
        let point = tables.points.intern(point);
        let subset_error = result.subset_errors.get(&point).unwrap();

        // in this dataset, `'a` is interned as `'1`
        let origin_a = Origin::from(1);

        // `'b` is interned as `'2`
        let origin_b = Origin::from(2);

        // and there should be a `'b: 'a` known subset to make the function valid, so
        // that is the subset error we should find
        assert!(subset_error.contains(&(origin_b, origin_a)));
    }
}
