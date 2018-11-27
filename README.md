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

To generate a [graphviz](https://www.graphviz.org) file with the CFG enriched
with input and output tuples at each point, do:

```bash
cargo +nightly run --release -- --graphviz_file=graph.dot inputs/issue-47680/nll-facts/main
```

### Want to run the code?

One of the goals with this repo is to experiment and compare different
implementations of the same algorithm. You can run the analysis by using `cargo run`
and you can choose the analysis with `-a`. So for example to run against an example
extract from clap, you might do:

```bash
> cargo +nightly run --release -- -a DatafrogOpt inputs/clap-rs/app-parser-{{impl}}-add_defaults/
    Finished release [optimized] target(s) in 0.05 secs
     Running `target/release/borrow-check 'inputs/clap-rs/app-parser-{{impl}}-add_defaults/'`
--------------------------------------------------
Directory: inputs/clap-rs/app-parser-{{impl}}-add_defaults/
Time: 3.856s
```

You could also try `-a Naive` to get the naive rules (more readable, slower)
or `-a LocationInsensitive` to use a location insensitive analysis.

By default, `cargo run` just prints timing. If you also want to see
the results, try `--show-tuples` (which will show errors) and maybe
`-v` (to show more intermediate computations). You can supply `--help`
to get more docs.

### How to generate your own inputs

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
