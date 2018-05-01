This is generated from the clap-rs test in the [rustc-perf repo]. To
generate it yourself, do this:

```bash
> cd $CHECKOUT/rustc-perf/collector/benchmarks/clap-rs
> cargo +YOUR_RUSTC rustc -- -Znll-facts -Ztime-passes -Zborrowck=mir
> cp -r nll-facts/app-parser-{{impl}}-add_defaults/ ...
```

Make sure you are using a local build that supports `-Znll-facts`, of
course.  When you run that command, you will also be able to see how
long this takes using the "normal" NLL analysis by looking for a line
like this (naturally, your "time" and "rss" numbers will vary):

```
time: 14.114; rss: 556MB    solve_nll_region_constraints(DefId(0/0:197 ~ clap[d75c]::app[0]::parser[0]::{{impl}}[0]::add_defaults[0]))
```

[rustc-perf repo]: https://github.com/rust-lang-nursery/rustc-perf
