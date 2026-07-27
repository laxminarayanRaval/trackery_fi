//! Underscores deliberately mirror Indian digit grouping (rupees_paise).
#![allow(clippy::inconsistent_digit_grouping)]

use chrono::NaiveDate;
use trackery_core::banks::hdfc::HdfcProfile;
use trackery_core::banks::{parse_statement, BankProfile};
use trackery_core::model::{Bank, Direction, Transaction, TransactionOrigin};
use uuid::Uuid;

fn fixture_pages(dir: &str) -> Vec<String> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut paths: Vec<_> = std::fs::read_dir(&base)
        .unwrap_or_else(|e| panic!("fixture dir {}: {e}", base.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort(); // page1, page2, ... — zero-padding not needed below 10 pages
    paths
        .iter()
        .map(|p| std::fs::read_to_string(p).expect("read fixture page"))
        .collect()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
}

#[test]
fn detects_hdfc_but_not_unsupported() {
    assert!(HdfcProfile::detect(&fixture_pages("hdfc")[0]));
    assert!(!HdfcProfile::detect(&fixture_pages("unsupported")[0]));
}

#[test]
fn parses_full_statement() {
    let txns = parse_statement(&fixture_pages("hdfc")).expect("hdfc statement should parse");
    assert_eq!(txns.len(), 7);

    // First row: single-line credit. Real HDFC exports print only whichever
    // of withdrawal/deposit applies — never both — so direction comes from
    // the balance delta against the statement's declared opening balance.
    let first = &txns[0];
    assert_eq!(first.date, date(2026, 1, 1));
    assert_eq!(
        first.narration_raw,
        "CHQ DEP - CTS CLG1 - FAKE ROAD-WBO: 0000000000000099 01/01/26"
    );
    assert_eq!(first.direction, Direction::Credit);
    assert_eq!(first.amount_paise, 5_000_00);
    assert_eq!(first.balance_paise, Some(15_000_00));
    common_fields(first);

    // A row whose continuation lines carry on *after* the amount/balance
    // pair has already appeared — real HDFC exports interleave them.
    let neft = &txns[4];
    assert_eq!(neft.date, date(2026, 1, 15));
    assert_eq!(
        neft.narration_raw,
        "NEFT DR-FAKE0001234-SYNTHNAMECONTINUED0000516600000001 15/01/26  UATION TEXT-NETBANK,MORECONT-FAKE0001234-LASTPART"
    );
    assert_eq!(neft.direction, Direction::Debit);
    assert_eq!(neft.amount_paise, 9_000_00);
    assert_eq!(neft.balance_paise, Some(24_525_00));
    common_fields(neft);

    // Last row.
    let last = &txns[6];
    assert_eq!(last.date, date(2026, 1, 31));
    assert_eq!(last.narration_raw, "SALARY 0000000000509999 31/01/26");
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 12_000_00);
    assert_eq!(last.balance_paise, Some(34_525_00));
    common_fields(last);

    // Running balance is consistent across every consecutive pair,
    // seeded from the statement's declared opening balance (10,000.00,
    // parsed from the summary row on page 2).
    let opening = 10_000_00;
    let mut prev_balance = opening;
    for t in &txns {
        let signed = match t.direction {
            Direction::Credit => t.amount_paise,
            Direction::Debit => -t.amount_paise,
        };
        assert_eq!(
            prev_balance + signed,
            t.balance_paise.expect("balance"),
            "balance mismatch at {:?}",
            t.narration_raw
        );
        prev_balance = t.balance_paise.expect("balance");
    }
}

fn common_fields(t: &Transaction) {
    assert_eq!(t.bank, Some(Bank::Hdfc));
    assert_eq!(t.origin, TransactionOrigin::StatementImport);
    assert_eq!(t.account_id, Uuid::nil());
    assert_ne!(t.id, Uuid::nil());
    assert_eq!(t.description, None);
    // counterparty is filled by parse_statement's narration enrichment;
    // its exact contents are asserted in narration_test.rs.
    assert_eq!(t.category_id, None);
    assert!(!t.deleted);
}
