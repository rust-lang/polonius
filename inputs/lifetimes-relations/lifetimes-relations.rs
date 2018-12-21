#![crate_type = "lib"]
#![feature(nll)]

fn missing_subset<'a, 'b>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y //~ ERROR
}

fn valid_subset<'a, 'b: 'a>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y
}
