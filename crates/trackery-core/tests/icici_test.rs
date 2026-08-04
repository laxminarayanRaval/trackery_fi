//! ICICI parser fixture tests. Per fixtures/README.md: exact counts,
//! exact first/last rows; fixture edits update expectations here.

use trackery_core::model::{Bank, Direction, TxnMode};
use trackery_core::parse::{parse_statement, ParseError};

fn pages() -> Vec<String> {
    vec![include_str!("fixtures/icici/icici_page1.txt").to_string()]
}

#[test]
fn parses_icici_fixture() {
    let stmt = parse_statement(&pages()).expect("fixture should parse");
    assert_eq!(stmt.bank, Bank::Icici);
    assert_eq!(stmt.transactions.len(), 10);

    let first = &stmt.transactions[0];
    assert_eq!(first.date.to_string(), "2026-03-02");
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 85_000);
    assert_eq!(first.balance_paise, Some(4_415_000));
    assert_eq!(first.bank, Bank::Icici);
    let cp = first.counterparty.as_ref().expect("upi counterparty");
    assert_eq!(cp.mode, TxnMode::Upi);
    assert_eq!(cp.vpa.as_deref(), Some("synth.mart@okicici"));
    assert_eq!(cp.reference.as_deref(), Some("504412345678"));
    assert_eq!(cp.name.as_deref(), Some("Groceries"));

    let last = &stmt.transactions[9];
    assert_eq!(last.date.to_string(), "2026-03-31");
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 2_000_000);
    assert_eq!(last.balance_paise, Some(8_096_070));
    let cp = last.counterparty.as_ref().expect("neft counterparty");
    assert_eq!(cp.mode, TxnMode::Neft);
    assert_eq!(cp.name.as_deref(), Some("SYNTH CLIENT"));
    assert_eq!(cp.reference.as_deref(), Some("N090261234567891"));
}

#[test]
fn decomposes_modes_across_fixture() {
    let stmt = parse_statement(&pages()).expect("fixture should parse");
    let modes: Vec<Option<TxnMode>> = stmt
        .transactions
        .iter()
        .map(|t| t.counterparty.as_ref().map(|c| c.mode))
        .collect();
    assert_eq!(
        modes,
        vec![
            Some(TxnMode::Upi),
            Some(TxnMode::Neft),
            Some(TxnMode::Imps),
            Some(TxnMode::Upi),
            Some(TxnMode::Atm),
            Some(TxnMode::Card),
            Some(TxnMode::Upi),
            Some(TxnMode::Other), // BIL/BPAY
            None,                 // INT.PD — no decomposable counterparty
            Some(TxnMode::Neft),
        ]
    );
    // Wrapped narration (BIL/BPAY row) is joined across lines.
    assert!(stmt.transactions[7]
        .narration_raw
        .contains("SYNTH POWER CO/MAR BILL"));
}

#[test]
fn running_balance_mismatch_is_malformed_with_line() {
    let mut page = include_str!("fixtures/icici/icici_page1.txt").to_string();
    // Corrupt one withdrawal so it no longer matches the balance delta.
    page = page.replace("850.00", "851.00");
    match parse_statement(&[page]) {
        Err(ParseError::MalformedStatement { line }) => assert_eq!(line, 9),
        other => panic!("expected MalformedStatement, got {other:?}"),
    }
}
