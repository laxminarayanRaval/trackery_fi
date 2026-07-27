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
    assert_eq!(txns.len(), 13);

    let first = &txns[0];
    assert_eq!(
        first.date,
        NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")
    );
    assert_eq!(
        first.narration_raw,
        "UPI/900000000001/10:15:22/UPI/synth.merchant1@ybl"
    );
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 250_00);
    assert_eq!(first.balance_paise, Some(4_750_00));
    assert_eq!(first.bank, Some(Bank::Bob));
    assert_eq!(first.account_id, Uuid::nil());
    assert_eq!(first.origin, TransactionOrigin::StatementImport);
    assert_eq!(first.description, None);
    assert_eq!(first.counterparty, None);
    assert_eq!(first.category_id, None);
    assert!(!first.deleted);

    let last = &txns[12];
    assert_eq!(
        last.date,
        NaiveDate::from_ymd_opt(2026, 1, 28).expect("date")
    );
    assert_eq!(
        last.narration_raw,
        "UPI/900000000012/14:44:44/UPI/synthgift14@okicici"
    );
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 5_00);
    assert_eq!(last.balance_paise, Some(9_755_00));
    assert_eq!(last.bank, Some(Bank::Bob));
}

/// Every parsed date lies inside the statement period — proves DD-MM-YYYY
/// was normalized (a MM/DD mixup would throw dates outside January).
#[test]
fn dates_are_within_statement_period() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    let from = NaiveDate::from_ymd_opt(2026, 1, 1).expect("date");
    let to = NaiveDate::from_ymd_opt(2026, 1, 31).expect("date");
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

/// BOB wraps long narrations onto undated continuation lines, sometimes
/// mid-token and sometimes as a lone stray character; the parser must join
/// them onto the preceding transaction with no separator.
#[test]
fn multi_line_narrations_are_joined() {
    let txns = BobProfile.parse(&fixture_pages("bob")).expect("parse bob");
    assert_eq!(
        txns[1].narration_raw,
        "UPI/900000000002/11:20:10/UPI/synth-merchant2-abc@axl"
    );
    assert_eq!(
        txns[2].narration_raw,
        "UPI/900000000003/12:05:44/UPI/synthqr3@ptys/"
    );
    assert_eq!(
        txns[8].narration_raw,
        "UPI/900000000008/10:00:00/UPI/synth-ref10-cd@ybl"
    );
}
