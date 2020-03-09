# Testing Polonius

## Rust UI Tests with Polonius Compare Mode

There is a mode of the Rust test suite that compares Polonius' output to the
current NLL one. You can invoke it by using `--compare-mode polonius`. For
example, the following will run the UI tests:

```
$ ./x.py test -i --stage 1 --compare-mode polonius src/test/ui
```

## Polonius' Own Unit Test

(Not yet written, but this section should describe how to use `polonius-parser`
to generate input for unit tests.)
