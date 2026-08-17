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

        let cpp_filename = souffle_generate(&ruleset, stem)?;
        cpp_filenames.push(cpp_filename);
    }

    odr_use_generate(&known_stems)?;

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

fn odr_use_func_name(stem: &str) -> String {
    format!("odr_use_{}_global", stem)
}

/// Uses Souffle to generate a C++ file for evaluating the given datalog program.
///
/// Returns the filename for the generated C code, as well as the name of a generated function that
/// will trigger the global initializers in that translation unit.
fn souffle_generate(datalog_filename: &Path, stem: &str) -> Result<PathBuf> {
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

    let mut generated_cpp = fs::OpenOptions::new().append(true).open(&cpp_filename)?;
    writeln!(
        generated_cpp,
        r#"
        extern "C"
        void {}() {{}}"#,
        odr_use_func_name(stem)
    )?;

    Ok(cpp_filename)
}

// HACK: Souffle adds datalog programs to the registry in the initializer of a global
// variable (whose name begins with `__factory_Sf`). That global variable is eligible for
// deferred initialization, so we need to force its initializer to run before we do a lookup in
// the registry (which happens in a different translation unit from the generated code).
//
// We accomplish this by defining a single, no-op function in each generated C++ file, and calling
// it on the Rust side before doing any meaningful work. By the C++ standard, this forces global
// initializers for anything in the that translation unit to run, since calling the function is an
// ODR-use of something in the same translation unit. We also define a helper function,
// `odr_use_all`, which calls the no-op function in every known module.
fn odr_use_generate(known_stems: &HashSet<String>) -> Result<()> {
    let mut odr_use_filename = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    odr_use_filename.push("odr_use.rs");

    let mut odr_use = BufWriter::new(fs::File::create(odr_use_filename)?);
    writeln!(odr_use, r#"extern "C" {{"#)?;
    for stem in known_stems {
        writeln!(odr_use, "fn {}();", odr_use_func_name(stem))?;
    }
    writeln!(odr_use, r#"}}"#)?;

    writeln!(odr_use, "fn odr_use_all() {{")?;
    for stem in known_stems {
        writeln!(odr_use, "unsafe {{ {}(); }}", odr_use_func_name(stem))?;
    }
    writeln!(odr_use, "}}")?;
    Ok(())
}
