# How to generate your own inputs

To run the borrow checker on an input, you first need to generate the
input facts.  For that, you will need to run rustc with the
`-Znll-facts` option:

```
> rustc -Znll-facts inputs/issue-47680/issue-47680.rs
```

Or, for generating the input facts of a crate using the `#![feature(nll)]` flag:

```
> cargo rustc -- -Znll-facts
```

This will generate a `nll-facts` directory with one subdirectory per function:

```bash
> ls -F nll-facts
{{impl}}-maybe_next/  main/
```

You can then run on these directories.
