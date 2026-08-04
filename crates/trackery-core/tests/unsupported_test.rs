use trackery_core::parse::{parse_statement, ParseError};

#[test]
fn unknown_bank_is_unsupported() {
    let pages = vec![include_str!("fixtures/unsupported/unsupported_page1.txt").to_string()];
    assert!(matches!(
        parse_statement(&pages),
        Err(ParseError::UnsupportedBank)
    ));
}
