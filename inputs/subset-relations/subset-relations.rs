#![crate_type = "lib"]
#![feature(nll)]

fn missing_subset<'a, 'b>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y //~ ERROR
}

fn implied_bounds_subset<'a, 'b>(x: &'a &'b mut u32) -> &'a u32 {
    x
}

fn valid_subset<'a, 'b: 'a>(x: &'a u32, y: &'b u32) -> &'a u32 {
    y
}
