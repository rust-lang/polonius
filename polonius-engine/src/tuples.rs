//! Ob
//!
//! Allows us to represent relations as trait objects.

use std::any::{Any, TypeId};

use dyn_clone::DynClone;
use smallvec::{smallvec, SmallVec};

use crate::Atom;

/// A series of `TypeId`s representing the type of a tuple (e.g. `(u32, i32)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TupleSchema {
    tys: &'static [TypeId],
}

impl TupleSchema {
    pub const fn new(tys: &'static [TypeId]) -> Self {
        TupleSchema { tys }
    }

    pub const fn arity(&self) -> usize {
        self.tys.len()
    }
}

/// Types that have an associated `TupleSchema`.
///
/// This includes both iterators over `Tuple`s and `Vec`s containing them.
pub trait HasSchema {
    /// Returns the `TupleSchema` associated with this type.
    fn schema(&self) -> TupleSchema;
}

impl<I, T: Tuple> HasSchema for I
where
    I: IntoIterator<Item = T>,
{
    fn schema(&self) -> TupleSchema {
        T::SCHEMA
    }
}

pub type RawTuple = SmallVec<[usize; 6]>;

/// A single datafrog fact. A tuple of newtyped indices.
pub trait Tuple: 'static + Copy + Sized {
    const SCHEMA: TupleSchema;

    fn into_raw(self) -> RawTuple;
    fn from_raw(raw: RawTuple) -> Self;
}

macro_rules! impl_raw_tuple {
    ($($T:ident)*) => {
        impl<$($T: Atom),*> Tuple for ($($T,)*) {
            const SCHEMA: TupleSchema = TupleSchema::new(&[$( TypeId::of::<$T>() ),*]);

            #[allow(non_snake_case)]
            fn into_raw(self) -> RawTuple {
                let ($($T,)*) = self;
                smallvec![$($T.into()),*]
            }

            #[allow(non_snake_case)]
            fn from_raw(raw: RawTuple) -> Self {
                assert_eq!(raw.len(), count_idents!($($T)*));

                match raw.as_slice() {
                    &[$($T),*] => ($($T.into(),)*),
                    _ => unreachable!(),
                }
            }
        }
    }
}

for_each_tuple!(impl_raw_tuple => [F E D C B A]);

/// An object-safe representation of `Vec<T> where T: Tuple`.
pub trait TupleVec: HasSchema + DynClone {
    /// Returns an iterator over the tuples in this vector.
    fn iter_tuples(&self) -> Box<dyn TupleIter<'_> + '_>;

    /// Converts to a [`std::any::Any`] for downcasting.
    fn as_any(&self) -> &dyn Any;
}

impl<T: Tuple> TupleVec for Vec<T> {
    fn iter_tuples(&self) -> Box<dyn TupleIter<'_> + '_> {
        Box::new(self.iter().copied())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// An object-safe representation of `Iterator<Item = T> where T: Tuple`.
///
/// You might think that `Box<dyn Iterator<Item = RawTuple>>` would be sufficient.
/// However, `TupleVec` needs the underlying type to be an actual `Vec<T>` for downcasting, and we
/// cannot create one from an iterator over [`RawTuple`]s, even if the [`TupleSchema`] is known.
/// [`TupleIter::collect_tuples`] serves this purpose.
pub trait TupleIter<'me>: HasSchema + DynClone {
    /// Consumes this iterator, returning the number of tuples it would yield.
    ///
    /// Analagous to [`std::iter::Iterator::count`].
    fn count(self: Box<Self>) -> usize;

    /// Maps this iterator to one that returns [`RawTuple`]s.
    fn map_raw(self: Box<Self>) -> Box<dyn Iterator<Item = RawTuple> + 'me>;

    /// Collects the tuples yielded by this iterator into a [`Box<dyn TupleVec>`](TupleVec).
    fn collect_tuples(self: Box<Self>) -> Box<dyn TupleVec>;
}

impl<'me, T, I> TupleIter<'me> for I
where
    I: 'me + Clone + Iterator<Item = T> + ?Sized,
    T: Tuple,
{
    fn count(self: Box<Self>) -> usize {
        Iterator::count(*self)
    }

    fn map_raw(self: Box<Self>) -> Box<dyn Iterator<Item = RawTuple> + 'me> {
        let iter = Box::new(self.map(T::into_raw));
        iter
    }

    fn collect_tuples(self: Box<Self>) -> Box<dyn TupleVec> {
        Box::new(self.collect::<Vec<T>>())
    }
}

impl<T: Tuple> From<Vec<T>> for Box<dyn TupleVec> {
    fn from(x: Vec<T>) -> Self {
        Box::new(x)
    }
}

/// Casts a [`TupleVec`] to its underlying `Vec`.
pub fn downcast_vec<T: Tuple>(x: &dyn TupleVec) -> Option<&Vec<T>> {
    (T::SCHEMA == x.schema()).then(|| x.as_any().downcast_ref::<Vec<T>>().unwrap())
}

/// Casts a [`TupleIter`] to its underlying `Iterator`.
pub fn downcast_iter<T: Tuple>(
    x: Box<dyn TupleIter<'_> + '_>,
) -> Option<impl Iterator<Item = T> + '_> {
    (T::SCHEMA == x.schema()).then(|| x.map_raw().map(|raw| T::from_raw(raw)))
}
