/// The "facts" which are the basis of the NLL borrow analysis.
#[derive(Clone, Default)]
crate struct AllFacts {
    /// `borrow_region(R, B, P)` -- the region R may refer to data
    /// from borrow B starting at the point P (this is usually the
    /// point *after* a borrow rvalue)
    crate borrow_region: Vec<(Region, Loan, Point)>,

    /// `universal_region(R)` -- this is a "free region" within fn body
    crate universal_region: Vec<Region>,

    /// `cfg_edge(P,Q)` for each edge P -> Q in the control flow
    crate cfg_edge: Vec<(Point, Point)>,

    /// `killed(B,P)` when some prefix of the path borrowed at B is assigned at point P
    crate killed: Vec<(Loan, Point)>,

    /// `outlives(R1, R2, P)` when we require `R1@P: R2@P`
    crate outlives: Vec<(Region, Region, Point)>,

    /// `region_live_at(R, P)` when the region R appears in a live variable at P
    crate region_live_at: Vec<(Region, Point)>,
}

macro_rules! index_type {
    ($t:ident) => {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Debug, Hash)]
        pub(crate) struct $t {
            index: u32,
        }

        impl From<usize> for $t {
            fn from(index: usize) -> $t {
                $t {
                    index: index as u32,
                }
            }
        }

        impl Into<usize> for $t {
            fn into(self) -> usize {
                self.index as usize
            }
        }

        impl $t {
            pub(crate) fn index(self) -> usize {
                self.into()
            }
        }
    };
}

index_type!(Region);
index_type!(Loan);
index_type!(Point);
