# Input relations

Polonius computes its analyses starting from "input facts", which can be seen as a little database of information about a piece of Rust code (most often: a function).

In this analogy, the database is the [`AllFacts` struct](https://github.com/rust-lang/polonius/blob/2cf8336f7ff9932270160a392ca5be3c804b7f41/polonius-engine/src/facts.rs#L6-L81), which contains all the data in tables (or relations), here as a handful of `Vec`s of rows. The table rows are these "facts": this terminology comes from Datalog, which Polonius uses to do its computations (and the reason for the `rustc` flag [outputting this data](../generate_inputs.md) being named `-Znll-facts`, and the files themselves `*.facts`).

In order to be used in various contexts (mainly: in-memory from `rustc`, and from on-disk test files in the Polonius repository) this structure is generic over the types of facts, only requiring them to be [`Atom`](https://github.com/rust-lang/polonius/blob/2cf8336f7ff9932270160a392ca5be3c804b7f41/polonius-engine/src/facts.rs#L108-L112)s. The goal is to use interned values, represented as numbers, in the polonius computations. 

These generic types of facts are the concepts Polonius manipulates: abstract `origins` containing `loans` at `points` in the CFG (in the liveness computation, and move/overwrite analysis, there are also: `variables` and `paths`), and the relations are their semantics (including the specific relationships between the different facts). More details about these atoms can be found in their [dedicated chapter](./atoms.md).

Let's start with the simplest relation: the representation of the Control Flow Graph, in the `cfg_edge` relation.

### 1. `cfg_edge`

`cfg_edge(point1, point2)`: as its name suggests, this relation stores that there's a CFG edge between the point `point1` and the point `point2`.

For each MIR statement location, 2 Polonius points are generated: the "Start" points and the "Mid" points (some of the other Polonius inputs will later be recorded at each of the points). These 2 points are linked by an edge recorded in this relation.

Then, another edge will be recorded, linking this MIR statement to its successor statement(s): from the mid point of the current location to the start point of the successor location. Even though it's encoded differently in MIR, this will similarly apply when the successor location is in another block, linking the mid point of the current location to the start point of the successor block's starting location.

For example, for this MIR (edited from the example for clarity, and to only show the parts related to the CFG):

```rust
bb0: {
  ...          // bb0[0]
  ...          // bb0[1]
  goto -> bb3; // bb0[2]
}

...

bb3: {
  ... // bb3[0]
}

```

we will record these input facts (as mentioned before, they'll be interned) in the `cfg_edge` relation, shown here as pseudo Rust:

```rust
cfg_edge = vec![
  // statement at location bb0[0]:
  (bb0-0-start, bb0-0-mid),
  (bb0-0-mid, bb0-1-start),
  
  // statement at location bb0[1]:
  (bb0-1-start, bb0-1-mid),
  (bb0-1-mid, bb0-2-start),
  
  // terminator at location bb0[2]:
  (bb0-2-start, bb0-2-mid),
  (bb0-2-mid, bb3-0-start),
];
```

### 2. `loan_issued_at`

`loan_issued_at(origin, loan, point)`: this relation stores that the loan `loan` was "issued" at the given point `point`, creating a reference with the origin `origin`. The origin `origin` may refer to data from loan `loan` from the point `point` and onwards (this is usually the point *after* a borrow rvalue). The origin in which the loan is issued is called the "issuing origin" (but has been called `borrow_region` historically, so you may still encounter this term in Polonius or rustc).

For every borrow expression, a loan will be created and there will be a fact stored in this relation to link this loan to the origin of the borrow expression.

For example, with:

```rust
let mut a = 0;
let r = &mut a; // this creates the loan L0
//      ^ let's call this 'a
```

there will be a `loan_issued_at` fact linking `L0` to `'a` at this point. This loan will flow along the CFG and the subset relationships between origins, and the computation will require that its terms are respected or it will generate an illegal access error.

### 3. `placeholder` (and `universal_region`)

`placeholder(origin, loan)`: stores that the `origin` is a placeholder origin, with its associated placeholder loan `loan` (`universal_region(origin)` currently still exists, describing the same thing about `origin`, without the loan, and is being phased out). These origins have been historically called different things, mostly in rustc, like "universal region" and "free region", but represent origins that are not defined in the MIR body we're checking. They are parts of the caller of this function: its loans are unknown to the current function and it cannot make assumptions about the origin (besides the relationships it may have with different placeholder origins, as we'll see below for the `known_placeholder_subset` relation). For computations where a loan from these placeholders can be useful (e.g. the illegal subset relationships errors), the associated placeholder loan can be used.

Those are the default placeholder origins (`'static`) and the ones defined on functions which are generic over a lifetime. For example, with

```rust
fn my_function<'a, 'b>(x: &'a u32, y: &'b u32) {
    ...
}
```

the `placeholder` relation will also contain facts for `'a`, and `'b`.

### 4. `loan_killed_at`

`loan_killed_at(loan, point)`: this relation stores that a prefix of the path borrowed in loan `loan` is assigned/overwritten at the point `point`. This indicates that the path borrowed by the `loan` has changed in some way that the loan no longer needs to be tracked. (In particular, mutations to the path that was borrowed no longer invalidate the loan)

For example, with:

```rust
let mut a = 1;
let mut b = 2;
let mut q = &mut a;
let r = &mut *q; // loan L0 of `*q`
// `q` can't be used here, one has to go through `r`
q = &mut b; // killed(L0)
// `q` and `r` can be used here
```

the loan `L0` will be "killed" by the assignment, and this fact stored in the `loan_killed_at` relation. When we compute which loans origins contain along the CFG, the `loan_killed_at` points will stop this loan's propagation to the next CFG point.

### 5. `subset_base`

`subset_base(origin1, origin2, point)`: this relation stores that the origin `origin1` outlives origin `origin2` at the point `point`.

This is the standard Rust syntax `'a: 'b` where the *lifetime* `'a` outlives the lifetime `'b`. From the point of view of origins as sets of loans, this is seen as a subset-relation: with all the loans in `'a` flowing into `'b`, `'a` contains a subset of the loans `'b` contains. 

The type system defines subtyping rules for references, which will create "outlives" facts to relate the reference type to the referent type as a `subset`.

(The `_base` suffix comes from the fact this relation is not transitive, and will be the base of the transitive closure computation)

For example:

```rust
let a: u32 = 1;
let b: &u32 = &a;
//            ^ let's call this 'a
//     ^ and let's call this 'b
```

To be valid, this last expression requires that the type `&'a u32` is a subtype of `&'b u32`. This requires `'a: 'b` and the `subset_base` relation will contain this basic fact that `'a` outlives / is a subset of / flows into `'b` at this point.

### 6. `origin_live_on_entry`

`origin_live_on_entry(origin, point)`: this relation stores that the origin `origin` appears in a live variable at the point `point`.

These facts are created by the liveness computation, and its facts and relations will be described later in a lot more detail. In the meantime, its implementation is in [liveness.rs here](https://github.com/rust-lang/polonius/blob/master/polonius-engine/src/output/liveness.rs).

### 7. `loan_invalidated_at`

`loan_invalidated_at(point, loan)`: this relation stores that a loan `loan` is invalidated by some action taking place at the point `point`.

Loans have terms which must be respected: ensuring shared loans are only used to read and not write or mutate, or that a mutable loan is the only way to access a referent. An illegal access of the path borrowed by the loan is said to *invalidate* the terms of the loan, and this fact will be recorded in the `loan_invalidated_at` relation. Any such action on a *live* loan will be an error.

Since the goal of the borrow checking analysis is to find these possible errors, this relation is important to the computation. Any loans it contains, and in turn, any origin containing those loans, are key facts the computation tracks.

### 8. `known_placeholder_subset`

`known_placeholder_subset(origin1, origin2)`: this relation store the relationship between two placeholder origins, that the `origin1` placeholder origin is a subset of the `origin2` placeholder origin. They can be declared by the user on function declarations, or inferred via implied bounds.

For example, the function:

```rust
fn foo<'a, 'b: 'a, 'c>(x: &'c &'a u32) {
    ...
}
```

would have two `known_placeholder_subset` entries:
- one for the user-supplied subset `'b: 'a`
- one for the `'a: 'c` implied bound from the `x` parameter

Note that the transitive subset `'b: 'c` resulting from these two entries is not necessarily included explicitly in this relation. Polonius will infer all the transitive subsets to do its illegal subset relationships errors analysis: if the function analysis finds that two placeholders are related, and this was not declared in the known subsets, that will be an error.
