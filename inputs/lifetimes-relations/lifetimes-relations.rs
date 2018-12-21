#![crate_type = "lib"]
#![feature(nll)]

fn missing_subset<'a, 'b>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y //~ ERROR
}

fn implied_bound_subset<'a, 'b>(x: &'b &'a mut u32) -> &'b u32 {
    x
}

fn valid_subset<'a, 'b: 'a>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y
}
