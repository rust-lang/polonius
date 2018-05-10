// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::facts::{Point, Region};
use fxhash::FxHashMap;
use histo::Histogram;

#[derive(Clone, Debug)]
crate struct RegionDegrees {
    in_degree: FxHashMap<(Region, Point), usize>,
    out_degree: FxHashMap<(Region, Point), usize>,
}

impl RegionDegrees {
    crate fn new() -> Self {
        Self {
            in_degree: Default::default(),
            out_degree: Default::default(),
        }
    }

    crate fn update_degrees(&mut self, r1: Region, r2: Region, p: Point) {
        *self.in_degree.entry((r2, p)).or_insert(0) += 1;
        *self.out_degree.entry((r1, p)).or_insert(0) += 1;
    }

    crate fn max_out_degree(&self) -> usize {
        *self.out_degree.values().max().unwrap_or(&0)
    }

    crate fn max_in_degree(&self) -> usize {
        *self.in_degree.values().max().unwrap_or(&0)
    }

    crate fn has_multidegree(&self) -> bool {
        for (region_point, in_count) in &self.in_degree {
            match self.out_degree.get(region_point) {
                Some(out_count) => if *out_count > 1 && *in_count > 1 {
                    return true;
                }
                None => {}
            }
        }
        return false;
    }

    crate fn histogram(&self) -> (Histogram,Histogram) {
        let mut histo_in = Histogram::with_buckets(10);
        let mut histo_out= Histogram::with_buckets(10);
        for v in self.in_degree.values() {
            histo_in.add(*v as u64);
        }
        for v in self.in_degree.values() {
            histo_out.add(*v as u64);
        }
        (histo_in, histo_out)
    }
}
