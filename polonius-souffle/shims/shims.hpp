#pragma once

#include <cstdint>
#include <memory>
#include <string>

#include <souffle/SouffleInterface.h>

#include "polonius-souffle/src/ffi.rs.h"
#include "rust/cxx.h"

#ifndef __EMBEDDED_SOUFFLE__
#error "Build script must define __EMBEDDED_SOUFFLE__"
#endif

/*
 * Wrappers around generated souffle functions. From Rust, we cannot safely
 * call functions that accept or return things with move constructors by-value
 * (e.g.  `std::string`). The usual fix is to move them into a `unique_ptr`
 * first, or to pass a borrowed version (e.g. `cxx::Str`) from the Rust side
 * and copy it here.
 */

namespace souffle {

std::unique_ptr<SouffleProgram>
ProgramFactory_newInstance(const std::string &name) {
  std::unique_ptr<SouffleProgram> prog(ProgramFactory::newInstance(name));
  return prog;
}

void load_all(SouffleProgram &foo, const std::string &name) {
  foo.loadAll(name);
}

void print_all(SouffleProgram &foo, const std::string &name) {
  foo.printAll(name);
}

void get_output_relations(const SouffleProgram &prog, rust::Vec<RelationPtr> &out) {
    auto relations = prog.getOutputRelations();
    for (auto rel : relations) {
        RelationPtr ptr{};
        ptr.ptr = rel;
        out.push_back(ptr);
    }
}

void get_all_relations(const SouffleProgram &prog, rust::Vec<RelationPtr> &out) {
    auto relations = prog.getAllRelations();
    for (auto rel : relations) {
        RelationPtr ptr{};
        ptr.ptr = rel;
        out.push_back(ptr);
    }
}

std::unique_ptr<std::string> get_name(const Relation& rel) {
    return std::make_unique<std::string>(rel.getName());
}

// Fact loading

// This function is copied from the member function on `Program`.
//
// We cannot use the original because it requires a reference to both a
// `Program` and a `Relation`, which have the same lifetime on the Rust side.
template <typename... Args>
void insert(const std::tuple<Args...> &t, Relation *rel) {
    souffle::tuple t1(rel);
    SouffleProgram::tuple_insert<decltype(t), sizeof...(Args)>::add(t, t1);
    rel->insert(t1);
}

void insert_tuple1(Relation &rel, Tuple1 r) {
  insert(std::make_tuple(r.a), &rel);
}

void insert_tuple2(Relation &rel, Tuple2 r) {
  insert(std::make_tuple(r.a, r.b), &rel);
}

void insert_tuple3(Relation &rel, Tuple3 r) {
  insert(std::make_tuple(r.a, r.b, r.c), &rel);
}

void insert_tuple4(Relation &rel, Tuple4 r) {
  insert(std::make_tuple(r.a, r.b, r.c, r.d), &rel);
}

DynTuples dump_tuples(const Relation &rel) {
    DynTuples out{};
    out.arity = rel.getArity();

    for (auto tuple : rel) {
        for (unsigned i = 0; i < out.arity; ++i) {
            out.data.push_back(tuple[i]);
        }
    }

    return out;
}

} // namespace bridge
