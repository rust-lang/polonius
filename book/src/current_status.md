# Current status

Polonius has been provisionally integrated into rustc. You can run a nightly
rustc with the `-Zpolonius` flag to try it out. However, it is not really
ready for widespread use.

Our current roadmap is as follows:

- **Complete the analysis:** Polonius represents only a portion of the full
  borrow checker analysis. We would like to move as much as possible from the
  handwritten Rust code in rustc into the datalog-based approach of polonius.
- **Optimize:** Naively implementing the polonius rules can be quite
  slow, so it's important that we optimize the rules and produce a
  more optimized version.  This will also likely require some special
  case optimizations to help with specific cases, such as large static
  constants.

After those two steps are done, and presuming all goes well, we expect
to replace the existing rustc borrow checker with the polonius
crate. The crate will continue to exist.

## Want to help?

Check out the [polonius working group][wg] of the compiler team.

[wg]: https://rust-lang.github.io/compiler-team/working-groups/polonius/
