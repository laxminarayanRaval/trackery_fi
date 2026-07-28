//! Underscores deliberately mirror Indian digit grouping (rupees_paise).
#![allow(clippy::inconsistent_digit_grouping)]

use trackery_core::banks::icici::IciciProfile;
use trackery_core::banks::{parse_statement, BankProfile};
use trackery_core::model::{Bank, Direction, TransactionOrigin};

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

#[test]
fn parses_full_icici_statement() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    assert_eq!(txns.len(), 7);

    // Real ICICI net-banking exports print no separate withdrawal/deposit
    // columns — a row ends in one amount + balance, so direction comes from
    // the balance delta, seeded by the table's own B/F row (10,000.00) and
    // not the `Opening`/first-row's own amount.
    let first = &txns[0];
    assert_eq!(first.date.to_string(), "2026-01-01");
    assert_eq!(
        first.narration_raw,
        "UPI/SYNTH MERCHANT/synthmerchant/UPI/StateBank/900000000001/SYNfake111122223333cd6e32222f93c/SYNTH MERCHANT PVT LTD"
    );
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 300_00);
    assert_eq!(first.balance_paise, Some(9_700_00));
    assert_eq!(first.bank, Some(Bank::Icici));
    assert_eq!(first.origin, TransactionOrigin::StatementImport);

    let last = &txns[6];
    assert_eq!(last.date.to_string(), "2026-01-20");
    assert_eq!(
        last.narration_raw,
        "INT.PD:XXXX1234:01-10-2025 to 31-12-2025"
    );
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 81_20);
    assert_eq!(last.balance_paise, Some(24_004_20));
    assert_eq!(last.bank, Some(Bank::Icici));
}

/// DD-MM-YYYY (numeric, not DD-Mon-YYYY) normalizes to ISO.
#[test]
fn normalizes_dd_mon_yyyy_using_transaction_date() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    assert_eq!(txns[2].date.to_string(), "2026-01-05");
}

#[test]
fn running_balance_is_consistent() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    let mut prev_balance = 10_000_00; // the fixture's B/F row
    for t in &txns {
        let signed = match t.direction {
            Direction::Debit => -t.amount_paise,
            Direction::Credit => t.amount_paise,
        };
        assert_eq!(
            t.balance_paise,
            Some(prev_balance + signed),
            "balance mismatch after {:?}",
            t.narration_raw
        );
        prev_balance = t.balance_paise.expect("balance present");
    }
}

/// ICICI wraps long Transaction Remarks across lines mid-token, sometimes
/// across a page boundary; the parser must join continuation lines onto the
/// preceding row's narration with no separator.
///
/// Row 3's narration deliberately ends in a digit ("...LTD9") right before
/// the amount/balance line — a real bug found against a live statement:
/// joining `...LTD9` and `20,000.00` with no separator reads back as a
/// single `920,000.00` token. Amount must still come out as exactly
/// 20,000.00, not corrupted by the fused digit.
#[test]
fn joins_wrapped_remarks() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    assert_eq!(
        txns[2].narration_raw,
        "UPI/SYNTH SALARY/synthsalary/Salary Credit/YESBANK/900000000003/SYNfakeaaaa2222bbbb3333cccc4444/SYNTH EMPLOYER PVT LTD9"
    );
    assert_eq!(txns[2].amount_paise, 20_000_00);
    assert_eq!(txns[2].balance_paise, Some(29_673_00));
    // Row 4 (page 2, index 3) wraps across three continuation lines.
    assert_eq!(
        txns[3].narration_raw,
        "UPI/SYNTH FRIEND/synthfriend/UPI/Bank OfBaroda/900000000004/SYNfakedddd5555eeee6666ffff7777/SYNTH FRIEND NAME"
    );
}

#[test]
fn detects_icici_but_not_unsupported() {
    let icici = fixture_pages("icici");
    assert!(IciciProfile::detect(&icici[0]));
    let unsupported = fixture_pages("unsupported");
    assert!(!IciciProfile::detect(&unsupported[0]));
}
