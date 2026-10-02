# Want to run the code?

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

You could also try `-a Naive` to get the naive rules (more readable,
slower) -- these are the exact rules described in [the
blogpost][post]. You can also use `-a LocationInsensitive` to use a
location insensitive analysis (faster, but may yield spurious errors).

By default, `cargo run` will print any errors found, but otherwise
just prints timing. If you also want to see successful results, try
`--show-tuples` and maybe `-v` (to show more intermediate computations).
You can supply `--help` to get more docs.

[post]: http://smallcultfollowing.com/babysteps/blog/2018/04/27/an-alias-based-formulation-of-the-borrow-checker/
