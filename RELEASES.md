# polonius

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

