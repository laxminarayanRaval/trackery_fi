//! Underscores deliberately mirror Indian digit grouping (rupees_paise).
#![allow(clippy::inconsistent_digit_grouping)]

use chrono::NaiveDate;
use trackery_core::banks::bob::BobProfile;
use trackery_core::banks::BankProfile;
use trackery_core::model::{Bank, Direction, TransactionOrigin};
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

#[test]
fn detects_bob_and_not_others() {
    let bob = fixture_pages("bob");
    assert!(BobProfile::detect(&bob[0]));
    let unsupported = fixture_pages("unsupported");
    assert!(!BobProfile::detect(&unsupported[0]));
}

#[test]
fn parses_full_statement() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    assert_eq!(txns.len(), 26);

    let first = &txns[0];
    assert_eq!(
        first.date,
        NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")
    );
    assert_eq!(
        first.narration_raw,
        "UPI/DR/615301111111/SYNTH GROCERY MART/YESB/synth"
    );
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 1_250_00);
    assert_eq!(first.balance_paise, Some(43_981_50));
    assert_eq!(first.bank, Some(Bank::Bob));
    assert_eq!(first.account_id, Uuid::nil());
    assert_eq!(first.origin, TransactionOrigin::StatementImport);
    assert_eq!(first.description, None);
    assert_eq!(first.counterparty, None);
    assert_eq!(first.category_id, None);
    assert!(!first.deleted);

    let last = &txns[25];
    assert_eq!(
        last.date,
        NaiveDate::from_ymd_opt(2026, 6, 30).expect("date")
    );
    assert_eq!(last.narration_raw, "Int.Coll:SB INT PAID UPTO 30/06/2026");
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 231_00);
    assert_eq!(last.balance_paise, Some(82_090_70));
    assert_eq!(last.bank, Some(Bank::Bob));
}

/// Every parsed date lies inside the statement period — proves DD/MM/YYYY
/// was normalized (a MM/DD mixup would throw dates outside June).
#[test]
fn dates_are_within_statement_period() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    let from = NaiveDate::from_ymd_opt(2026, 6, 1).expect("date");
    let to = NaiveDate::from_ymd_opt(2026, 6, 30).expect("date");
    for t in &txns {
        assert!(
            (from..=to).contains(&t.date),
            "date {} outside period",
            t.date
        );
    }
}

#[test]
fn running_balance_is_consistent() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    for pair in txns.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        let signed = match cur.direction {
            Direction::Credit => cur.amount_paise,
            Direction::Debit => -cur.amount_paise,
        };
        assert_eq!(
            cur.balance_paise,
            Some(prev.balance_paise.expect("balance") + signed),
            "balance chain broken at {} {}",
            cur.date,
            cur.narration_raw
        );
    }
}

/// BOB wraps long narrations onto indented continuation lines with no
/// date/amount; the parser must join them onto the preceding transaction.
#[test]
fn multi_line_narrations_are_joined() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    assert_eq!(
        txns[3].narration_raw,
        "NEFT/N152260123456789/SYNTH EMPLOYER PVT LTD/SALARY MAY 2026"
    );
    assert_eq!(
        txns[17].narration_raw,
        "NEFT/N152262456789012/SYNTH CLIENT SOLUTIONS LLP/INVOICE 2231"
    );
}
