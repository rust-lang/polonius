# Loan analysis

Loan analysis is the heart of the borrow checker, and will compute:
- illegal access errors: an action on a loan, that is illegal to perform
- illegal subset relations errors: missing relationships between placeholder origins

This is done in multiple variants, whose goals are different: performance, readability, tests and validation.

Broadly speaking, the goals of the analysis are 1) to track loans:
- from the point and origin in which they are issued, to the points where they are invalidated
- flowing from origin to origin at a single point, via their `subset` relationships
- flowing from point to point in the CFG, according to the origins' liveness (stopping at points where a loan is killed)

And 2) to track undeclared relationships between placeholder origins.

Any live loan which is invalidated will be an illegal access error, any placeholder which flows into another placeholder unexpectedly will be an illegal subset relation error.

### Inputs

The input relations will be described below, but the [dedicated page](./relations.md) will have more information about them.

```prolog
// Indicates that the `loan` was "issued" at the given `point`, creating a
// reference with the `origin`. Effectively, `origin` may refer to data from
// `loan` starting at `point` (this is usually the point *after* a borrow rvalue).
.decl loan_issued_at(Origin:origin, Loan:loan, Point:point)
.input loan_issued_at

// When some prefix of the path borrowed at `loan` is assigned at `point`.
// Indicates that the path borrowed by the `loan` has changed in some way that the
// loan no longer needs to be tracked. (In particular, mutations to the path that
// was borrowed no longer invalidate the loan)
.decl loan_killed_at(Loan:loan, Point:point)
.input loan_killed_at

// Indicates that the `loan` is invalidated by some action
// taking place at `point`; if any origin that references this loan is live,
// this is an error.
.decl loan_invalidated_at(Loan:loan, Point:point)
.input loan_invalidated_at

// When we require `origin1@point: origin2@point`.
// Indicates that `origin1 <= origin2` -- i.e., the set of loans in `origin1`
// are a subset of those in `origin2`.
.decl subset_base(Origin1:origin, Origin2:origin, Point:point)
.input subset_base

// Describes a placeholder `origin`, with its associated placeholder `loan`.
.decl placeholder(Origin:origin, Loan:loan)
.input placeholder

// These reflect the `'a: 'b` relations that are either declared by the user on
// function declarations or which are inferred via implied bounds.
// For example: `fn foo<'a, 'b: 'a, 'c>(x: &'c &'a u32)` would have two entries:
// - one for the user-supplied subset `'b: 'a`
// - and one for the `'a: 'c` implied bound from the `x` parameter,
// (note that the transitive relation `'b: 'c` is not necessarily included
// explicitly, but rather inferred by polonius).
.decl known_placeholder_subset(Origin1:origin, Origin2:origin)
.input known_placeholder_subset
```

The datalog rules below are considered the "naive" implementation, as it computes the whole transitive closure of the subset relation, but are easy to describe and explain. They are implemented using the datafrog engine in the [Naive variant](https://github.com/rust-lang/polonius/blob/master/polonius-engine/src/output/naive.rs).

Some trivial differences exist with the implementation:
- the use of the `;` alternative operator in the rules
- some API limitations about joins in the implementation, sometimes requiring intermediate steps per join (and these can sometimes be shared between different rules)

### Subsets between origins

The rules below compute the complete graph of subsets between origins: starting from the non-transitive subsets, we close over this relation at a given point in the CFG (regardless of liveness). Liveness is then taken into account to propagate these transitive subsets along the CFG: if an origin flows into another at a given point, and they both are live at the successor points (reminder: placeholder origins are considered live at all points), the relationship is propagated to the successor points.

```prolog
.decl subset(Origin1:origin, Origin2:origin, Point:point)

// R1: the initial subsets are the non-transitive `subset_base` static input
subset(Origin1, Origin2, Point) :-
  subset_base(Origin1, Origin2, Point).

// R2: compute the subset transitive closure, at a given point
subset(Origin1, Origin3, Point) :-
  subset(Origin1, Origin2, Point),
  subset(Origin2, Origin3, Point).

// R3: propagate subsets along the CFG, according to liveness
subset(Origin1, Origin2, TargetPoint) :-
  subset(Origin1, Origin2, SourcePoint),
  cfg_edge(SourcePoint, TargetPoint),
  (origin_live_on_entry(Origin1, TargetPoint); placeholder(Origin1, _)),
  (origin_live_on_entry(Origin2, TargetPoint); placeholder(Origin2, _)).
```

### The origins contain loans

The rules below compute what loans are contained in which origins, at given points of the CFG: starting from the "issuing point and origin", a loan is propagated via the subsets computed above, at a given point in the CFG. Liveness is then taken into account to propagate these loans along the CFG: if a loan is contained in an origin at a given point, and that the origin is live at the successor points, the loan is propagated to the successor points. A subtlety here is that there are points in the CFG where a loan can be killed, and that will stop propagation. Rule 6 uses both liveness and kill points to decide whether the loan should be propagated further in the CFG.

```prolog
.decl origin_contains_loan_on_entry(Origin:origin, Loan:loan, Point:point)

// R4: the issuing origins are the ones initially containing loans
origin_contains_loan_on_entry(Origin, Loan, Point) :-
  loan_issued_at(Origin, Loan, Point).

// R5: propagate loans within origins, at a given point, according to subsets
origin_contains_loan_on_entry(Origin2, Loan, Point) :-
  origin_contains_loan_on_entry(Origin1, Loan, Point),
  subset(Origin1, Origin2, Point).

// R6: propagate loans along the CFG, according to liveness
origin_contains_loan_on_entry(Origin, Loan, TargetPoint) :-
  origin_contains_loan_on_entry(Origin, Loan, SourcePoint),
  !loan_killed_at(Loan, SourcePoint),
  cfg_edge(SourcePoint, TargetPoint),
  (origin_live_on_entry(Origin, TargetPoint); placeholder(Origin, _)).
```

### Loan liveness, and illegal access errors

With the information computed above, we can compute illegal accesses errors. It is an error to invalidate a loan that is live at a given point. A loan is live at a point if it is contained in an origin that is live at that point.

```prolog
.decl loan_live_at(Loan:loan, Point:point)

// R7: compute whether a loan is live at a given point, i.e. whether it is
// contained in a live origin at this point
loan_live_at(Loan, Point) :-
  origin_contains_loan_on_entry(Origin, Loan, Point),
  (origin_live_on_entry(Origin, Point); placeholder(Origin, _)).

.decl errors(Loan:loan, Point:point)

// R8: compute illegal access errors, i.e. an invalidation of a live loan
errors(Loan, Point) :-
  loan_invalidated_at(Loan, Point),
  loan_live_at(Loan, Point).
```

### Placeholder subsets, and illegal subset relations errors

These errors can be computed differently depending on the variant, but the goal is the same: if the analysis detects that a placeholder origin ultimately flows into another placeholder origin, that relationship needs to be declared or it is an error.

The `Naive` rules variant computes the complete subset transitive closure and can more easily detect whether one of these facts links two placeholder origins. The `LocationInsensitive` rules variant does not compute transitive subsets at all, and uses loan propagation to detect these errors (if a placeholder loan flows into a placeholder origin). The `Opt` optimized rules variant only computes the transitive closure of some origins according to their liveness and possible contribution to any error (mostly the ones dying along an edge, and the origins they can reach), and track the transitive subsets of placeholders explicitly.

```prolog
.decl subset_errors(Origin1:origin, Origin2:origin, Point:point)

// R9: compute illegal subset relations errors, i.e. the undeclared subsets
// between two placeholder origins.
subset_errors(Origin1, Origin2, Point) :-
  subset(Origin1, Origin2, Point),
  placeholder_origin(Origin1),
  placeholder_origin(Origin2),
  !known_placeholder_subset(Origin1, Origin2).
```

### Location Insensitive analysis

The rules above document the `Naive` variant of loan analysis, as it is conceptually simple and describes all the important parts computed by the Polonius model. This variant is "naive" in the sense that to stay clear and simple, the rules compute more things than strictly required. In particular, it computes the complete transitive subsets of all origins, as well as the loans contained by each origin at every point of the CFG.

In practice, different "grades" of borrow-checking can be useful: each with different levels of precision in what it accepts and with different computational complexity requirements. The lowest of such grades, the `LocationInsensitive` variant, trades off precision for speed by ignoring both the location where subsets happen, and the origins' contents at the CFG points. The idea is: if an analysis would find no error when ignoring path- and flow-sensitivity, then the full analysis would find no error either. If it does find potential errors, then the full analysis will find a subset of these location-insensitive errors.

This can be used as a quick pre-pass: if there a no errors, a full, expensive, analysis does not need to run, otherwise, only the loans where potential errors occur would need to be fully checked to remove false positives.

The inputs are the same as the `Naive` variant, but remove the CFG points from the `subset`s. Subsets are not tracked, and are used to approximate loan propagation inside origins (regardless of liveness and location-sensitivity) in `origin_contains_loan`:

```prolog
.decl subset(Origin1:origin, Origin2:origin)

// R1: the subsets are the non-transitive `subset_base` static input,
// with their location stripped.
subset(Origin1, Origin2) :-
  subset_base(Origin1, Origin2, _).

.decl origin_contains_loan(Origin:origin, Loan:loan)

// R2: the issuing origins are the ones initially containing loans.
origin_contains_loan(Origin, Loan) :-
  loan_issued_at(Origin, Loan, _).

// R3: the placeholder origins also contain their placeholder loan.
origin_contains_loan(Origin, Loan) :-
  placeholder_loan(Origin, Loan).

// R4: propagate the loans from the origins to their subsets.
origin_contains_loan(Origin2, Loan) :-
  origin_contains_loan(Origin1, Loan),
  subset(Origin1, Origin2).

.decl loan_live_at(Loan:loan, Point:point)

// R5a: Approximate loan liveness. If an origin is live at a given
// point, and it contains a loan *anywhere* in the CFG, that loan is
// considered live at that point.
loan_live_at(Loan, Point) :-
  origin_contains_loan(Origin, Loan),
  (origin_live_on_entry(Origin, Point); placeholder_origin(Origin)).

.decl potential_errors(Loan:loan, Point:point)

// R5b: Compute potential illegal access errors, i.e. invalidations
// of live loans.
potential_errors(Loan, Point) :-
  loan_invalidated_at(Loan, Point),
  loan_live_at(Loan, Point).
```

Note: rules "5a" and "5b" above are named to match [the implementation](https://github.com/rust-lang/polonius/blob/master/polonius-engine/src/output/location_insensitive.rs) which computes `potential_errors` as a single "rule 5" without materializing the `loan_live_at` intermediate relation of "rule 5a".

Illegal subset relation errors (which are by definition about "subsets") can still be computed by propagating the placeholder loans, and detecting when they unexpectedly flow into another placeholder origin: one where this specific relationship between the two placeholders was not declared.

```prolog
.decl potential_subset_errors(Origin1:origin, Origin2:origin)

// R6: compute potential illegal subset relations errors, i.e. the
// placeholder loans which ultimately flowed into another placeholder
// origin unexpectedly.
potential_subset_errors(Origin1, Origin2) :-
  placeholder(Origin1, Loan1),
  placeholder(Origin2, _),
  origin_contains_loan(Origin2, Loan1),
  !placeholder_known_to_contain(Origin2, Loan1).
```

This requires a simple input equivalent to the transitive closure of `known_placeholder_subset`, tracking the placeholder loans a given placeholder origin is known to contain instead, and is computed like so:

```prolog
.decl placeholder_known_to_contain(Origin:origin, Loan:loan)

placeholder_known_to_contain(Origin, Loan) :-
  placeholder(Origin, Loan).

placeholder_known_to_contain(Origin2, Loan1) :-
  placeholder_known_to_contain(Origin1, Loan1),
  known_placeholder_subset(Origin1, Origin2).
```

### To be continued

In the current implementation, this quick `LocationInsensitive` filter is used as a pre-pass to another optimized variant, as part of [the `Hybrid` algorithm](https://github.com/rust-lang/polonius/blob/2cf8336f7ff9932270160a392ca5be3c804b7f41/polonius-engine/src/output/mod.rs#L42).

A more detailed description of the rules in this `Opt` variant will be added later but it computes the same data as the `Naive` variant described above, more efficiently, by limiting where the subset transitive closure is computed: some origins are short-lived, or part of a subsection of the subset graph into which no loan ever flows, and therefore don't contribute to errors or loan propagation. There's no need to track these specific cases.

In the meantime, [the implementation](https://github.com/rust-lang/polonius/blob/master/polonius-engine/src/output/datafrog_opt.rs) documents the relations and rules it uses in its computation.
