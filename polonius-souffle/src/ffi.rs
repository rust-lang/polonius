use core::fmt;
use std::pin::Pin;

use cxx::let_cxx_string;

pub use self::ffi::{
    load_all, print_all, DynTuples, Program, Relation, Tuple1, Tuple2, Tuple3, Tuple4,
};

#[cxx::bridge(namespace = "souffle")]
mod ffi {
    struct Tuple1 {
        a: u32,
    }

    struct Tuple2 {
        a: u32,
        b: u32,
    }

    struct Tuple3 {
        a: u32,
        b: u32,
        c: u32,
    }

    struct Tuple4 {
        a: u32,
        b: u32,
        c: u32,
        d: u32,
    }

    /// A list of tuples whose arity is only known at runtime.
    #[derive(Default)]
    struct DynTuples {
        data: Vec<u32>,
        arity: usize,
    }

    /// A pointer to a relation.
    ///
    /// Needed to work around the fact that does not allow for types like `Vec<*const T>` in
    /// signatures.
    struct RelationPtr {
        ptr: *const Relation,
    }

    unsafe extern "C++" {
        include!("souffle/SouffleInterface.h");
        include!("polonius-souffle/shims/shims.hpp");

        #[cxx_name = "SouffleProgram"]
        type Program;

        fn ProgramFactory_newInstance(s: &CxxString) -> UniquePtr<Program>;

        fn load_all(prog: Pin<&mut Program>, dir: &CxxString);
        fn print_all(prog: Pin<&mut Program>, dir: &CxxString);
        fn getRelation(self: &Program, relation: &CxxString) -> *mut Relation;
        fn run(self: Pin<&mut Program>);
        fn get_output_relations(prog: &Program, relations: &mut Vec<RelationPtr>);
        fn get_all_relations(prog: &Program, relations: &mut Vec<RelationPtr>);

        type Relation;

        fn size(self: &Relation) -> usize;
        // fn getSignature(self: &Relation) -> UniquePtr<CxxString>;
        fn getArity(self: &Relation) -> u32;
        fn get_name(rel: &Relation) -> UniquePtr<CxxString>;

        fn insert_tuple1(rel: Pin<&mut Relation>, t: Tuple1);
        fn insert_tuple2(rel: Pin<&mut Relation>, t: Tuple2);
        fn insert_tuple3(rel: Pin<&mut Relation>, t: Tuple3);
        fn insert_tuple4(rel: Pin<&mut Relation>, t: Tuple4);

        fn dump_tuples(rel: &Relation) -> DynTuples;
    }
}

impl fmt::Debug for DynTuples {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl Relation {
    pub fn name(&self) -> String {
        let s = ffi::get_name(self);
        s.as_ref()
            .expect("Relation must have a name")
            .to_string_lossy()
            .into_owned()
    }

    pub fn tuples(&self) -> DynTuples {
        ffi::dump_tuples(self)
    }
}

// Rust wrappers

impl Program {
    pub fn new(name: &str) -> cxx::UniquePtr<Self> {
        let_cxx_string!(name = name);
        ffi::ProgramFactory_newInstance(&*name)
    }

    pub fn relation(&self, name: &str) -> Option<&Relation> {
        let_cxx_string!(name = name);
        let rel = self.getRelation(&*name);

        // SAFETY: `getRelation` returns a valid pointer or NULL.
        unsafe { rel.as_ref() }
    }

    pub fn relation_mut(self: Pin<&mut Self>, name: &str) -> Option<Pin<&mut Relation>> {
        let_cxx_string!(name = name);
        let rel = self.getRelation(&*name);

        // SAFETY: `relation_mut` returns a valid pointer or NULL. The returned reference has the
        // same lifetime as `self`, so multiple mutable references to the same `Relation` are
        // impossible. The mutable reference is never made available outside the `Pin`, so it
        // cannot be moved by the caller.
        unsafe { rel.as_mut().map(|x| Pin::new_unchecked(x)) }
    }

    pub fn output_relations(&self) -> impl Iterator<Item = &'_ Relation> {
        let mut relations = vec![];
        ffi::get_output_relations(self, &mut relations);

        relations.into_iter().map(|ptr| unsafe { &*ptr.ptr })
    }

    pub fn relations(&self) -> impl Iterator<Item = &'_ Relation> {
        let mut relations = vec![];
        ffi::get_all_relations(self, &mut relations);

        relations.into_iter().map(|ptr| unsafe { &*ptr.ptr })
    }
}

impl DynTuples {
    pub fn iter(&self) -> std::slice::ChunksExact<'_, u32> {
        self.data.chunks_exact(self.arity)
    }
}

// Tuples

pub trait InsertIntoRelation {
    fn insert_into_relation(self, rel: Pin<&mut Relation>);
}

impl<A: Into<u32>> InsertIntoRelation for (A,) {
    fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple1(rel, self.into_tuple())
    }
}
impl<A: Into<u32>, B: Into<u32>> InsertIntoRelation for (A, B) {
    fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple2(rel, self.into_tuple())
    }
}

impl<A: Into<u32>, B: Into<u32>, C: Into<u32>> InsertIntoRelation for (A, B, C) {
    fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple3(rel, self.into_tuple())
    }
}

impl<A: Into<u32>, B: Into<u32>, C: Into<u32>, D: Into<u32>> InsertIntoRelation for (A, B, C, D) {
    fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple4(rel, self.into_tuple())
    }
}

impl Tuple1 {
    pub fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple1(rel, self)
    }
}

impl Tuple2 {
    pub fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple2(rel, self)
    }
}

impl Tuple3 {
    pub fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple3(rel, self)
    }
}

impl Tuple4 {
    pub fn insert_into_relation(self, rel: Pin<&mut Relation>) {
        ffi::insert_tuple4(rel, self)
    }
}

// Conversion method into FFI tuples.
//
// `From` or `Into` would be better, but this helps type deduction inside the fact loading macro.
pub trait IntoTuple<T> {
    fn into_tuple(self) -> T;
}

impl<A: Into<u32>> IntoTuple<Tuple1> for (A,) {
    fn into_tuple(self) -> Tuple1 {
        Tuple1 { a: self.0.into() }
    }
}

impl<A: Into<u32>, B: Into<u32>> IntoTuple<Tuple2> for (A, B) {
    fn into_tuple(self) -> Tuple2 {
        Tuple2 {
            a: self.0.into(),
            b: self.1.into(),
        }
    }
}

impl<A: Into<u32>, B: Into<u32>, C: Into<u32>> IntoTuple<Tuple3> for (A, B, C) {
    fn into_tuple(self) -> Tuple3 {
        Tuple3 {
            a: self.0.into(),
            b: self.1.into(),
            c: self.2.into(),
        }
    }
}

impl<A: Into<u32>, B: Into<u32>, C: Into<u32>, D: Into<u32>> IntoTuple<Tuple4> for (A, B, C, D) {
    fn into_tuple(self) -> Tuple4 {
        Tuple4 {
            a: self.0.into(),
            b: self.1.into(),
            c: self.2.into(),
            d: self.3.into(),
        }
    }
}

impl<A: From<u32>> From<Tuple1> for (A,) {
    fn from(t: Tuple1) -> Self {
        (t.a.into(),)
    }
}

impl<A: From<u32>, B: From<u32>> From<Tuple2> for (A, B) {
    fn from(t: Tuple2) -> Self {
        (t.a.into(), t.b.into())
    }
}

impl<A: From<u32>, B: From<u32>, C: From<u32>> From<Tuple3> for (A, B, C) {
    fn from(t: Tuple3) -> Self {
        (t.a.into(), t.b.into(), t.c.into())
    }
}

impl<A: From<u32>, B: From<u32>, C: From<u32>, D: From<u32>> From<Tuple4> for (A, B, C, D) {
    fn from(t: Tuple4) -> Self {
        (t.a.into(), t.b.into(), t.c.into(), t.d.into())
    }
}
