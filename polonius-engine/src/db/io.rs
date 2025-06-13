use std::fmt::Debug;

use crate::tuples::Tuple;
use crate::{Db, Dump, FactTypes, Rels};

/// Types that can be loaded from a [`Db`].
pub trait LoadFrom<'db, T: FactTypes> {
    /// The names of all relations contained in this type.
    const RELATIONS: Rels;

    fn load_from_db(facts: &'db Db<T>) -> Self;
}

/// Types that can be stored to a [`Db`].
pub trait StoreTo<T: FactTypes> {
    /// The names of all relations contained in this type.
    const RELATIONS: Rels;

    fn store_to_db(self, facts: &mut Db<T>, dump: &mut Dump<'_>);
}

/// Define a `struct` that references a subset of the fields in `Db`.
#[macro_export]
macro_rules! input {
    ($Name:ident { $( $r:ident ),* $(,)* }) => {paste::paste!{
        pub struct $Name<'db, T: $crate::FactTypes> {
            $( pub $r: &'db datafrog::Relation<$crate::db::[<$r:camel>]<T>> ),*
        }

        impl<'db, T: $crate::FactTypes> $crate::LoadFrom<'db, T> for $Name<'db, T> {
            const RELATIONS: $crate::Rels = &[$(stringify!($r)),*];

            fn load_from_db(db: &'db $crate::Db<T>) -> Self {
                Self {
                    $( $r: &db.$r.as_ref().expect("Missing input"), )*
                }
            }
        }
    }};
}

/// Define a `struct` that owns a subset of the fields in `Db`.
#[macro_export]
macro_rules! output {
    // Single relation outputs are common
    ($r:ident) => {paste::paste! {
        output!([<$r:camel>] { $r });

        impl<T: $crate::FactTypes> From<datafrog::Relation<$crate::db::[<$r:camel>]<T>>> for [<$r:camel>]<T> {
            fn from(r: datafrog::Relation<$crate::db::[<$r:camel>]<T>>) -> Self {
                Self {
                    $r: r,
                }
            }
        }
    }};

    ($Name:ident {
        $( $r:ident ),* $(,)?
    }) => {paste::paste!{
        pub struct $Name<T: $crate::FactTypes> {
            $( pub $r: datafrog::Relation<$crate::db::[<$r:camel>]<T>>, )*
        }

        impl<T: $crate::FactTypes> $crate::StoreTo<T> for $Name<T> {
            const RELATIONS: $crate::Rels = &[$(stringify!($r)),*];

            fn store_to_db(self, db: &mut $crate::Db<T>, dump: &mut $crate::Dump<'_>) {
                use crate::internal::store_to_db_field;
                let curr_unit = db.curr_unit;
                $( store_to_db_field(stringify!($r), curr_unit, dump, &mut db.$r, self.$r); )*
            }
        }
    }};
}

/// Saves a computed relation to the `Db`.
///
/// This is publicly exported because it is an implementation detail of the `output` macro.
/// It is not subject to stability guarantees.
#[doc(hidden)]
pub fn store_to_db_field<T: 'static + Eq + Debug + Tuple>(
    name: &'static str,
    curr_unit: &'static str,
    dump: &mut Dump<'_>,
    opt: &mut Option<datafrog::Relation<T>>,
    val: datafrog::Relation<T>,
) {
    match opt {
        Some(old) => {
            pretty_assertions::assert_eq!(
                old,
                &val,
                "`{}` computed by `{}` differed from the existing",
                name,
                curr_unit
            );
        }
        None => {
            dump.rel_ref(name, &val);
            *opt = Some(val);
        }
    }
}
