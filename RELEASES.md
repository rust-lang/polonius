# polonius

## v0.3.0

- adopt latest polonius-engine

## v0.2.0

- integrate the latest engine
- add graphviz output
- preliminary work towards a friendly front-end format

# polonius-engine

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

