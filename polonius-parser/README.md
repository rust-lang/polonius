# Polonius Parser

The polonius parser is a small, hand-written parser used to parse polonius test cases that are _not_ obtained from `rustc`:

```
// program description
placeholders { 'a, 'b, 'c }

// block description
block B0 {
    // 0:
    loan_invalidated_at(L0);

    // 1:
    loan_killed_at(L2);

    loan_invalidated_at(L1) / use('a, 'b);

    // CFG
    goto B1, B2;
}

block B1 {
    use('a), outlives('a: 'b), loan_issued_at('b, L1);
}
```

## Usage
The `polonius_parser` crate provides a single function `parse_input`, which takes a program description as its input string.
Input will either be successfully parsed into an `ir::Input`, or a `ParseError` will be returned.
The `ir` structs are a small, public data model.

## Architecture
`polonius_parser` is implemented as a `Lexer`, which is an `Iterator` that extracts `Token`s from the source text, and a `Parser`, which operates on these `Tokens`.
The parser is a simple recursive descent parser with 1 token of look-ahead (using `Peekable`).
We use the `T!` macro across the implementation to quickly and legibly reference token kinds.

## Adding Facts
To extend the parser with a new fact `loan_bazzles_var_at`, perform the following changes:

- Add a new variant `KwLoanBazzlesVarAt` to `TokenKind` for the `loan_bazzles_var_at` keyword (`token.rs`).
- Add a shorthand for the keyword to the `T!` macro and add the new keyword kind to the `Display` impl for `TokenKind`.
- Repeat the above for any additional tokens introduced as new syntax, e.g., if the relation is represented as `L1 $ V1`, add `$`.
- In `Lexer::valid_token` (`lexer.rs`), add a case
    ```rs
    kw if kw.starts_with("loan_bazzles_var_at".as_bytes()) => {
        ("loan_bazzles_var_at".len() as u32, T![loan_bazzles_var_at])
    }
    ```
- For additional token kinds, add cases as appropriate. In our example, add
    ```rs
    [b'$', ..] => (1, T![$]),
    ```
- Add a variant to the `Fact` enum in the `ir` datamodel to represent the new fact. For us, that's
    ```rs
    LoanBazzlesVarAt { loan: String, variable: String },
    ```
- Add a case for the new fact to `Parser::parse_fact` (`parser.rs`)
    ```rs
    T![loan_bazzles_var_at] => { /* New parsing logic here */ }
    ```
  which returns `Ok(Fact::LoanBazzlesVarAt { .. })` if successful.
  For our example:
  ```rs
    T![loan_bazzles_var_at] => { 
        self.consume(T![loan_bazzles_var_at])?;
        self.consume(T!['('])?;
        let loan = self.parse_parameter(T![loan])?;
        self.consume(T![$])?;
        let variable = self.parse_parameter(T![variable])?;
        self.consume(T![')'])?;
        Ok(Fact::LoanBazzlesVarAt { loan, variable })
     }
  ```

### Custom Relations
Of course, it's also possible to add custom relations by following the same steps to add tokens to the lexer, writing their own parsing method, and add them to `parse_input`.
A correponding `ir` representation should be added to the data model in this case.
