use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use glob::glob;
use which::which;

const RULES_DIR: &str = "../rules";
const CXX_BRIDGE: &str = "src/ffi.rs";

type Result<T> = ::std::result::Result<T, Box<dyn Error>>;

/// Gets the filename for each "top-level" rulest
fn get_rulesets() -> Vec<PathBuf> {
    let result: std::result::Result<Vec<_>, _> =
        glob(&format!("{}/*.dl", RULES_DIR)).unwrap().collect();
    result.unwrap()
}
fn print_rerun_if_changed() {
    // Rerun if any C++ file changes
    for file in glob("shims/*").unwrap() {
        println!("cargo:rerun-if-changed={}", file.unwrap().to_string_lossy());
    }

    // Rerun if any datalog file changes.
    for file in glob(&format!("{}/**/*.dl", RULES_DIR)).unwrap() {
        println!("cargo:rerun-if-changed={}", file.unwrap().to_string_lossy());
    }

    // Rerun if our CXX bindings change.
    println!("cargo:rerun-if-changed={}", CXX_BRIDGE);
}

fn main() -> Result<()> {
    print_rerun_if_changed();

    if which("souffle").is_err() {
        eprintln!("`souffle` not in PATH. Is it installed?");
        return Err("missing `souffle`".into());
    }

    let mut cpp_filenames = vec![];
    let mut known_stems = HashSet::new();
    for ruleset in get_rulesets() {
        // Get the common name for this ruleset.
        let stem = ruleset.file_stem().unwrap().to_str().unwrap();

        // Check that name for duplicates
        //
        // Souffle uses a single, global registry for datalog programs, indexed by string.
        if !known_stems.insert(stem.to_owned()) {
            eprintln!("Multiple datalog files named `{}`", stem);
            return Err("Duplicate filenames".into());
        }

        let cpp_filename = souffle_generate(&ruleset)?;
        cpp_filenames.push(cpp_filename);
    }

    for stem in known_stems {
        // HACK: Souffle adds datalog programs to the registry in the initializer of a global
        // variable (whose name begins with `__factory_Sf`). Since that global variable is never used
        // by the Rust program, it is occasionally removed by the linker, its initializer is never
        // run (!!!), and the program is never registered.
        //
        // `-u` marks the symbol as undefined, so that it will not be optimized out.
        let prog_symbol = format!("__factory_Sf_{}_instance", stem);
        println!("cargo:rustc-link-arg=-u{}", prog_symbol);
    }

    let mut cc = cxx_build::bridge(CXX_BRIDGE);

    for file in cpp_filenames {
        cc.file(&file);
    }

    cc.cpp(true)
        .define("__EMBEDDED_SOUFFLE__", None)
        .include("./shims")
        .flag("-std=c++17")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-parentheses")
        .try_compile("souffle")?;

    Ok(())
}

/// Uses Souffle to generate a C++ file for evaluating the given datalog program.
fn souffle_generate(datalog_filename: &Path) -> Result<PathBuf> {
    let mut cpp_filename = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    cpp_filename.push(datalog_filename.with_extension("cpp").file_name().unwrap());

    eprintln!("Generating code for {:?}...", &datalog_filename);

    let result = Command::new("souffle")
        .arg("--generate")
        .arg(&cpp_filename)
        .arg(&datalog_filename)
        .status()?;

    if !result.success() {
        return Err("Invalid datalog".into());
    }

    Ok(cpp_filename)
}
