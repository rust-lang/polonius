use glob::glob;

fn main() {
    // See `polonius-souffle/build.rs` for background.
    //
    // FIXME: We should expose these symbols in a single location instead of globbing.
    for ruleset in glob("rules/*.dl").unwrap() {
        let stem = ruleset.unwrap();
        let stem = stem.file_stem().unwrap().to_str().unwrap();
        println!("cargo:rustc-link-arg=-u__factory_Sf_{}_instance", stem);
    }
}
