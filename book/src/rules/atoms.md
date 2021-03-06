# Atoms

Polonius defines the following **atoms**. To Polonius, these are
opaque identifiers that identify particular things within the input
program (literally they are newtype'd integers). Their meaning and
relationships come entirely from the input relations.

## Example

We'll use this snippet of Rust code to illustrate the various kinds of
atoms that can exist.

```rust
let x = (vec![22], vec![44]);
let y = &x.1;
let z = x.0;
drop(y);
```

## Variables

A **variable** represents a user variable defined by the Rust source
code. In our snippet, `x`, `y`, and `z` are variables. Other kinds of
variables include parameters.

## Path

A **path** indicates a path through memory to a memory location --
these roughly correspond to **places** in MIR, although we only
support a subset of the full places (that is, every MIR place maps to
a Path, but sometimes a single Path maps back to multiple MIR places).

Each path begins with a variable (e.g., `x`) but can be extended with
fields (e.g., `x.1`), with an "index" (e.g., `x[]`) or with a deref `*x`.
Note that the index paths (`x[]`) don't track the actual index that was
accessed, since the borrow check treats all indices as equivalent.

The grammar for paths would thus look something like this:

```
Path = Variable
     | Path "." Field // field access
     | Path "[" "]"   // index
     | "*" Path
```

Each path has a distinct atom associated with it. So there would be an
atom P1 for the path `x` and another atom P2 for the path `x.0`.
These atoms are related to one another through the `path_parent`
relation.

## Node

Nodes are, well, *nodes* in the control-flow graph. They are related
to one another by the `cfg_edge` relation.

For each statement (resp. terminator) S in the MIR, there are actually
two associated nodes. One represents the "start" of S -- before S has
begun executing -- the other is called the "mid node" -- which
represents the point where S "takes effect". Each start node has
exactly one successor, the mid node.

## Loans

A **loan** represents some borrow that occurs in the source.  Each
loan has an associated path that was borrowed along with a mutability.
So, in our example, there would be a single loan, for the `&x.1`
expression.

## Origins

An **origin** is what it typically called in Rust a **lifetime**. In
Polonius, an **origin** refers to the set of loans from which a
reference may have been created.


