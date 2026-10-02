use std::convert::TryInto;
use std::pin::Pin;

use log::warn;
use polonius_facts::{AllFacts, FactTypes};

use crate::ffi::{self, InsertIntoRelation};

fn insert_facts<T>(mut rel: Pin<&mut ffi::Relation>, name: &str, facts: &[T])
where
    T: Copy + InsertIntoRelation,
{
    debug_assert_eq!(std::mem::size_of::<T>() % std::mem::size_of::<u32>(), 0);
    let datafrog_arity = std::mem::size_of::<T>() / std::mem::size_of::<u32>();

    let souffle_arity: usize = rel.getArity().try_into().unwrap();

    if souffle_arity != datafrog_arity {
        panic!(
            r#"Arity mismatch for "{}". souffle={}, datafrog={}"#,
            name, souffle_arity, datafrog_arity
        );
    }

    for &fact in facts {
        fact.insert_into_relation(rel.as_mut());
    }
}

macro_rules! load_facts {
    ($prog:ident, $facts:ident; $( $f:ident ),* $(,)?) => {
        // Exhaustive matching, since new facts must be reflected below as well.
        let AllFacts {
            $( ref $f ),*
        } = $facts;
        $(
            let name = stringify!($f);
            let rel = $prog.as_mut().relation_mut(name);
            if let Some(rel) = rel {
                insert_facts(rel, name, $f);
            } else {
                warn!("Relation named `{}` not found. Skipping...", name);
            }
        )*
    }
}

pub fn insert_all_facts<T>(mut prog: Pin<&mut ffi::Program>, facts: &AllFacts<T>)
where
    T: FactTypes,
    T::Origin: Into<u32>,
    T::Loan: Into<u32>,
    T::Point: Into<u32>,
    T::Variable: Into<u32>,
    T::Path: Into<u32>,
{
    load_facts!(prog, facts;
        loan_issued_at,
        universal_region,
        cfg_edge,
        loan_killed_at,
        subset_base,
        loan_invalidated_at,
        var_used_at,
        var_defined_at,
        var_dropped_at,
        use_of_var_derefs_origin,
        drop_of_var_derefs_origin,
        child_path,
        path_is_var,
        path_assigned_at_base,
        path_moved_at_base,
        path_accessed_at_base,
        known_placeholder_subset,
        placeholder,
    );
}
