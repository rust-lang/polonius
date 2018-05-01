This is a core library that models the borrow check. It implements the analysis
[described in this blogpost][post].

[post]: http://smallcultfollowing.com/babysteps/blog/2018/04/27/an-alias-based-formulation-of-the-borrow-checker/

### How to use

First off, you must use the **nightly** channel. To build, do something like this:

```bash
cargo +nightly build --release
```

You can try it on one our input tests like so:

```bash
cargo +nightly run --release -- inputs/issue-47680/nll-facts/main
```

This will generate a bunch of output tuples:

```
# borrow_live_at

"Mid(bb3[2])"   "bw0"
"Mid(bb3[2])"   "bw2"
"Mid(bb10[2])"  "bw0"
...
```

### How to generate your own inputs

To run the borrow checker on an input, you first need to generate the
input facts.  For that, you will need to run rustc with the
`-Znll-facts` option (note: [PR #50370] adds the `-Znll-facts` option,
and it has not landed at the time of this writing, so you may need to
build rustc yourself):

```
> rustc -Znll-facts inputs/issue-47680/issue-47680.rs
```

[PR #50370]: https://github.com/rust-lang/rust/pull/50370

This will generate a `nll-facts` directory with one subdirectory per function:

```bash
> ls -F nll-facts
{{impl}}-maybe_next/  main/
```

You can then run on these directories.
