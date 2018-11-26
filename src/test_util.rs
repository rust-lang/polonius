#![cfg(test)]

use std::fmt::Debug;

/// Test that two values are equal, with a better error than `assert_eq`
pub fn assert_equal<A>(expected_value: &A, actual_value: &A)
where
    A: ?Sized + Debug + Eq,
{
    // First check that they have the same debug text. This produces a better error.
    let expected_text = format!("{:#?}", expected_value);
    assert_expected_debug(&expected_text, actual_value);

    // Then check that they are `eq` too, for good measure.
    assert_eq!(expected_value, actual_value);
}

/// Test that the debug output of `actual_value` is as expected. Gives
/// a nice diff if things fail.
pub fn assert_expected_debug<A>(expected_text: &str, actual_value: &A)
where
    A: ?Sized + Debug,
{
    let actual_text = format!("{:#?}", actual_value);

    if expected_text == actual_text {
        return;
    }

    println!("# expected_text");
    println!("{}", expected_text);

    println!("# actual_text");
    println!("{}", actual_text);

    println!("# diff");
    for diff in diff::lines(&expected_text, &actual_text) {
        match diff {
            diff::Result::Left(l) => println!("-{}", l),
            diff::Result::Both(l, _) => println!(" {}", l),
            diff::Result::Right(r) => println!("+{}", r),
        }
    }

    panic!("debug comparison failed");
}
