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
    assert_eq!(txns.len(), 27);

    // First row: wrapped UPI debit — narration reassembled across 3 lines.
    let first = &txns[0];
    assert_eq!(first.date, date(2026, 6, 1));
    assert_eq!(
        first.narration_raw,
        "UPI-SYNTH MERCHANT-synth.merchant@okhdfcbank-YESB0000001-509912345678-PAYMENT FOR ORDER"
    );
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 45_000);
    assert_eq!(first.balance_paise, Some(51_890_19));
    common_fields(first);

    // Last row: interest credit on the last page.
    let last = &txns[26];
    assert_eq!(last.date, date(2026, 6, 30));
    assert_eq!(last.narration_raw, "CREDIT INTEREST CAPITALISED");
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 31_200);
    assert_eq!(last.balance_paise, Some(97_937_69));
    common_fields(last);

    // DD/MM/YY normalizes to 20xx ISO dates.
    assert_eq!(txns[3].date, date(2026, 6, 3)); // 03/06/26 ATM withdrawal
    assert_eq!(txns[1].date, date(2026, 6, 1)); // 01/06/26 NEFT CR salary

    // Running balance is consistent across every consecutive pair,
    // including the page 1 → page 2 boundary.
    for pair in txns.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        let signed = match next.direction {
            Direction::Credit => next.amount_paise,
            Direction::Debit => -next.amount_paise,
        };
        assert_eq!(
            prev.balance_paise.expect("balance") + signed,
            next.balance_paise.expect("balance"),
            "balance mismatch after {:?}",
            next.narration_raw
        );
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
