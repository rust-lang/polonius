pub mod facts;
mod ffi;

pub use ffi::{DynTuples, Program};

use std::collections::HashMap;
use std::path::Path;

use cxx::let_cxx_string;
use polonius_facts::{AllFacts, FactTypes};

pub fn run_from_dir(prog: &str, facts_dir: &Path) {
    let_cxx_string!(facts = facts_dir.to_string_lossy().as_bytes());
    let_cxx_string!(empty = "");

    let mut prog = Program::new(&prog);
    let mut prog = prog.as_mut().expect("Wrong program name");
    ffi::load_all(prog.as_mut(), &facts);
    prog.as_mut().run();
    ffi::print_all(prog.as_mut(), &empty);
}

pub fn run_from_facts<T>(prog: &str, facts: &AllFacts<T>) -> HashMap<String, DynTuples>
where
    T: FactTypes,
{
    let mut prog = Program::new(prog);
    let mut prog = prog.as_mut().expect("Wrong program name");

    facts::insert_all_facts(prog.as_mut(), facts);

    prog.as_mut().run();

    let output_relations: HashMap<_, _> = prog
        .relations()
        .map(|rel| {
            let s = rel.name();
            let tuples = rel.tuples();
            (s, tuples)
        })
        .collect();

    output_relations
}
