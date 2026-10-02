use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use rustc_hash::FxHashMap;

use super::{Algorithm, AllFacts};
use crate::{dump, Db, FactTypes, LoadFrom};

#[derive(Clone, Debug)]
pub struct Output<T: FactTypes> {
    pub errors: FxHashMap<T::Point, Vec<T::Loan>>,
    pub subset_errors: FxHashMap<T::Point, BTreeSet<(T::Origin, T::Origin)>>,
    pub move_errors: FxHashMap<T::Point, Vec<T::Path>>,

    pub dump_enabled: bool,

    // these are just for debugging
    pub loan_live_at: FxHashMap<T::Point, Vec<T::Loan>>,
    pub origin_contains_loan_at: FxHashMap<T::Point, BTreeMap<T::Origin, BTreeSet<T::Loan>>>,
    pub origin_contains_loan_anywhere: FxHashMap<T::Origin, BTreeSet<T::Loan>>,
    pub origin_live_on_entry: FxHashMap<T::Point, Vec<T::Origin>>,
    pub loan_invalidated_at: FxHashMap<T::Point, Vec<T::Loan>>,
    pub subset: FxHashMap<T::Point, BTreeMap<T::Origin, BTreeSet<T::Origin>>>,
    pub subset_anywhere: FxHashMap<T::Origin, BTreeSet<T::Origin>>,
    pub var_live_on_entry: FxHashMap<T::Point, Vec<T::Variable>>,
    pub var_drop_live_on_entry: FxHashMap<T::Point, Vec<T::Variable>>,
    pub path_maybe_initialized_on_exit: FxHashMap<T::Point, Vec<T::Path>>,
    pub path_maybe_uninitialized_on_exit: FxHashMap<T::Point, Vec<T::Path>>,
    pub known_contains: FxHashMap<T::Origin, BTreeSet<T::Loan>>,
    pub var_maybe_partly_initialized_on_exit: FxHashMap<T::Point, Vec<T::Variable>>,
}

struct OutputErrors<T: FactTypes> {
    errors: FxHashMap<T::Point, Vec<T::Loan>>,
    subset_errors: FxHashMap<T::Point, BTreeSet<(T::Origin, T::Origin)>>,
    move_errors: FxHashMap<T::Point, Vec<T::Path>>,
}

impl<'db, T: FactTypes> LoadFrom<'db, T> for OutputErrors<T> {
    const RELATIONS: crate::Rels = &["errors", "subset_errors", "move_errors"];

    fn load_from_db(facts: &'db Db<T>) -> Self {
        let mut ret = OutputErrors {
            errors: Default::default(),
            subset_errors: Default::default(),
            move_errors: Default::default(),
        };

        for &(l, p) in facts.errors.as_ref().unwrap().iter() {
            ret.errors.entry(p).or_default().push(l);
        }

        for &(o1, o2, p) in facts.subset_errors.as_ref().unwrap().iter() {
            ret.subset_errors.entry(p).or_default().insert((o1, o2));
        }

        for &(l, p) in facts.move_errors.as_ref().unwrap().iter() {
            ret.move_errors.entry(p).or_default().push(l);
        }

        ret
    }
}

impl<T: FactTypes> Output<T> {
    fn new(dump_enabled: bool) -> Self {
        Self {
            errors: Default::default(),
            subset_errors: Default::default(),
            move_errors: Default::default(),

            dump_enabled,

            loan_live_at: Default::default(),
            origin_contains_loan_at: Default::default(),
            origin_contains_loan_anywhere: Default::default(),
            origin_live_on_entry: Default::default(),
            loan_invalidated_at: Default::default(),
            subset: Default::default(),
            subset_anywhere: Default::default(),
            var_live_on_entry: Default::default(),
            var_drop_live_on_entry: Default::default(),
            path_maybe_initialized_on_exit: Default::default(),
            path_maybe_uninitialized_on_exit: Default::default(),
            known_contains: Default::default(),
            var_maybe_partly_initialized_on_exit: Default::default(),
        }
    }

    pub fn compute(input: &AllFacts<T>, algorithm: Algorithm, dump_enabled: bool) -> Self {
        let pipeline = algorithm.pipeline();
        let mut ret = Output::new(dump_enabled);
        let ref mut counts = dump::Counts;

        let dumpers = if dump_enabled {
            vec![counts as _, &mut ret as _]
        } else {
            vec![counts as _]
        };

        let out_errors: OutputErrors<_> = pipeline.compute(input.clone(), dumpers);
        ret.errors = out_errors.errors;
        ret.subset_errors = out_errors.subset_errors;
        ret.move_errors = out_errors.move_errors;

        for &(p, l) in &input.loan_invalidated_at {
            ret.loan_invalidated_at.entry(p).or_default().push(l);
        }

        ret
    }

    pub fn errors_at(&self, location: T::Point) -> &[T::Loan] {
        match self.errors.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn loans_in_scope_at(&self, location: T::Point) -> &[T::Loan] {
        match self.loan_live_at.get(&location) {
            Some(p) => p,
            None => &[],
        }
    }

    pub fn origin_contains_loan_at(
        &self,
        location: T::Point,
    ) -> Cow<'_, BTreeMap<T::Origin, BTreeSet<T::Loan>>> {
        assert!(self.dump_enabled);
        match self.origin_contains_loan_at.get(&location) {
            Some(map) => Cow::Borrowed(map),
            None => Cow::Owned(BTreeMap::default()),
        }
    }

    pub fn origins_live_at(&self, location: T::Point) -> &[T::Origin] {
        assert!(self.dump_enabled);
        match self.origin_live_on_entry.get(&location) {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn subsets_at(
        &self,
        location: T::Point,
    ) -> Cow<'_, BTreeMap<T::Origin, BTreeSet<T::Origin>>> {
        assert!(self.dump_enabled);
        match self.subset.get(&location) {
            Some(v) => Cow::Borrowed(v),
            None => Cow::Owned(BTreeMap::default()),
        }
    }
}

impl<T: FactTypes> dump::Dumper for Output<T> {
    fn dump_iter(&mut self, id: &dump::RelationId, tuples: Box<dyn crate::TupleIter<'_> + '_>) {
        use crate::tuples::downcast_iter;

        if !self.dump_enabled {
            return;
        }

        match (id.relation_name(), id.unit_name()) {
            ("loan_live_at", _) => {
                for (l, p) in downcast_iter(tuples).unwrap() {
                    self.loan_live_at.entry(p).or_default().push(l);
                }
            }

            ("origin_contains_loan_at", "BorrowckLocationInsensitive") => {
                for (o, l) in downcast_iter(tuples).unwrap() {
                    self.origin_contains_loan_anywhere.entry(o).or_default().insert(l);
                }
            }

            ("origin_contains_loan_at", _) => {
                for (o, l, p) in downcast_iter(tuples).unwrap() {
                    self.origin_contains_loan_at
                        .entry(p)
                        .or_default()
                        .entry(o)
                        .or_default()
                        .insert(l);
                }
            }

            ("origin_live_on_entry", _) => {
                for (o, p) in downcast_iter(tuples).unwrap() {
                    self.origin_live_on_entry.entry(p).or_default().push(o);
                }
            }

            // loan_invalidated_at

            ("subset", "BorrowckLocationInsensitive") => {
                for (o1, o2) in downcast_iter(tuples).unwrap() {
                    self.subset_anywhere.entry(o1).or_default().insert(o2);
                }
            }

            ("subset", _) => {
                for (o1, o2, p) in downcast_iter(tuples).unwrap() {
                    self.subset
                        .entry(p)
                        .or_default()
                        .entry(o1)
                        .or_default()
                        .insert(o2);
                }
            }

            ("var_live_on_entry", _) => {
                for (v, p) in downcast_iter(tuples).unwrap() {
                    self.var_live_on_entry.entry(p).or_default().push(v);
                }
            }

            ("var_drop_live_on_entry", _) => {
                for (pt, p) in downcast_iter(tuples).unwrap() {
                    self.var_drop_live_on_entry.entry(p).or_default().push(pt);
                }
            }

            ("path_maybe_initialized_on_exit", _) => {
                for (pt, p) in downcast_iter(tuples).unwrap() {
                    self.path_maybe_initialized_on_exit.entry(p).or_default().push(pt);
                }
            }

            ("path_maybe_uninitialized_on_exit", _) => {
                for (pt, p) in downcast_iter(tuples).unwrap() {
                    self.path_maybe_uninitialized_on_exit.entry(p).or_default().push(pt);
                }
            }

            ("known_placeholder_requires", _) => {
                for (o, l) in downcast_iter(tuples).unwrap() {
                    self.known_contains.entry(o).or_default().insert(l);
                }
            }

            ("var_maybe_partly_initialized_on_exit", _) => {
                for (v, p) in downcast_iter(tuples).unwrap() {
                    self.var_maybe_partly_initialized_on_exit.entry(p).or_default().push(v);
                }
            }

            _ => {}
        }
    }
}
