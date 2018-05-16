// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn populate_args_for_differential_dataflow(workers: u32) -> Vec<String> {
    let mut dataflow_arg = Vec::new();
    if workers > 1 {
        dataflow_arg.push(format!("-w"));
        dataflow_arg.push(format!("{}", workers));
    }
    return dataflow_arg;
}
