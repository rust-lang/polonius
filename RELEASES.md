# polonius

## v0.6.0
- update to the latest `polonius-engine`
- update the test parser to add the new facts related to subset errors
- update, deduplicate, and remove some dependencies
- remove `--ignore-region-live-at`, as the `region_live_at` is now gone
  from the inputs and is always calculated!
- add a CLI option `--ignore-region-live-at` which ignores those facts and
  recompute them using Polonius even if provided.
- missing `region_live_at.facts` is no longer an error.
- consistently use the logging crate for error and warning logging.

## v0.5.0

Add a CLI option `--dump-liveness-graph` to dump a Graphviz file with a
(reduced) liveness-related graph for debugging.

## v0.4.0

- adopt latest polonius-engine
- extensions to the parser to incorporate syntax for the new facts

## v0.3.0

- adopt latest polonius-engine

## v0.2.0

- integrate the latest engine
- add graphviz output
- preliminary work towards a friendly front-end format

# polonius-engine

## v0.11.0

- adopt a new terminology for the Atoms, and begin documenting everything in a book
- use a new API to refer to the Atom types via associated types
- compute new errors: illegal subset relation errors, where for example
  a `fn foo<'a, 'b>` might require `'a: 'b` annotations to be valid.
- more work towards supporting initialization facts and errors
- more work towards defining different phases where each can have its
  own input facts or produce errors

## v0.10.0

- add the initialisation-tracking inputs `child`, `path_belongs_to_var`,
  `initialized_at`, `moved_out_at`, and `path_accessed_at`, as well as the new
  `Atom` `MovePath` to the type of `AllFacts` to capture move paths.
- remove the `var_maybe_initialized_on_exit` input, as it is now calculated by Polonius.
- remove the `region_live_at` input fact, as it is now calculated by Polonius.

## v0.9.0

- add the input `var_initialized_on_exit` which indicates if a variable may be
  initialized at a given point and is used to compute drop-liveness.

## v0.8.0

- Polonius now performs liveness analysis to calculate `region_live_at`, if it
  isn't present (#104)
- extend the type of `AllFacts` and `Output` with `Variable`
- new facts: `var_defined`, `var_used`, `var_drop_used`, `var_uses_region`, and
  `var_drops_region`
- `Output` now has a `var_live_at`, and a `var_drop_live_at` field

## v0.7.0

- add a naive hybrid algorithm that first executes the location-insensitive
  analysis and falls back to the full analysis as needed (#100)
- extend tests to cover the location-insensitive analysis
- invert loan and point arguments in loc insensitive check

## v0.6.2

- adopt the new datafrog 2.0 dependency (#95)
- some deduplicated dependencies and other improvements (#93, #91, #90)

## v0.6.1

- adopt the new datafrog 1.0 dependency and optimize with leapfrog joins (#88)

## v0.6.0

- bug: fixed bug in `DatafrogOpt` algorithm (#84)
- now builds on Rust 2018 beta (#84)
- optimization: remove symmetries in `subset` relation (#78)

## v0.5.0

- add a new algorithm that permits comparing naive and optimized

## v0.4.0

- avoid `Cow` for `errors_at`

## v0.3.0

- renamed field from `potential_errors` to `errors`

## v0.2.0

- added `Output` mode

## v0.1.1

- made default more lenient

## v0.1.0

