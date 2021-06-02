# Rules

These chapters document and explain the Polonius rules, primarily in
datalog form.

First, we'll describe the [atoms](./rules/atoms.md), and the [relations](./rules/relations.md) they are stored in. Then, we'll look at the polonius computation in more detail. It's a pipeline consisting of multiple steps and analyses:

- [Initialization analysis](./rules/initialization.md) will compute move and initialization errors, as well as the initialization and uninitialization data used by the next step.
- [Liveness analysis](./rules/liveness.md) will compute which origins are live at which points in the control flow graph, used by the next step.
- [Loan analysis](./rules/loans.md) (the core of "borrow checking") will compute illegal access errors, and illegal subset relationships errors. This is currently done with different variants (with different datalog rules) which will be described in that section.
