#![cfg(test)]

use crate::facts::{Loan, Point};
use crate::intern;
use crate::tab_delim;
use failure::Error;
use polonius_engine::{Algorithm, Output};
use rustc_hash::FxHashMap;
use std::path::Path;

fn test_fn(dir_name: &str, fn_name: &str) -> Result<(), Error> {
    do catch {
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
fn test_insensitive_potential_error() -> Result<(), Error> {
    do catch {
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

        assert_eq!(insensitive.potential_errors, expected);
    }
}

#[test]
fn test_sensitive_passes_issue_47680() -> Result<(), Error> {
    do catch {
        let facts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("inputs")
            .join("issue-47680")
            .join("nll-facts")
            .join("main");
        let tables = &mut intern::InternerTables::new();
        let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
        let sensitive = Output::compute(&all_facts, Algorithm::DatafrogOpt, false);

        assert!(sensitive.potential_errors.is_empty());
    }
}
