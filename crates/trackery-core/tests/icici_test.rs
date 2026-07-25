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
    assert_eq!(txns.len(), 26);

    let first = &txns[0];
    assert_eq!(first.date.to_string(), "2026-01-01");
    assert_eq!(
        first.narration_raw,
        "UPI/500100000001/Payment from Ph/synth.grocer@paytm/YESB/"
    );
    assert_eq!(first.direction, Direction::Debit);
    assert_eq!(first.amount_paise, 25_000);
    assert_eq!(first.balance_paise, Some(4_975_000));
    assert_eq!(first.bank, Some(Bank::Icici));
    assert_eq!(first.origin, TransactionOrigin::StatementImport);

    let last = &txns[25];
    assert_eq!(last.date.to_string(), "2026-01-31");
    assert_eq!(
        last.narration_raw,
        "INT.PD:XXXX1234:01-10-2025 to 31-12-2025"
    );
    assert_eq!(last.direction, Direction::Credit);
    assert_eq!(last.amount_paise, 81_200);
    assert_eq!(last.balance_paise, Some(10_320_175));
    assert_eq!(last.bank, Some(Bank::Icici));
}

/// DD-Mon-YYYY normalizes to ISO, and `date` comes from the Transaction Date
/// column (salary row: value date 04-Jan-2026, transaction date 05-Jan-2026).
#[test]
fn normalizes_dd_mon_yyyy_using_transaction_date() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    assert_eq!(txns[3].date.to_string(), "2026-01-05");
}

#[test]
fn running_balance_is_consistent() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    for pair in txns.windows(2) {
        let prev = pair[0].balance_paise.expect("balance present");
        let signed = match pair[1].direction {
            Direction::Debit => -pair[1].amount_paise,
            Direction::Credit => pair[1].amount_paise,
        };
        assert_eq!(
            pair[1].balance_paise,
            Some(prev + signed),
            "balance mismatch after {:?}",
            pair[1].narration_raw
        );
    }
}

/// ICICI wraps long Transaction Remarks across lines mid-token; the parser
/// must join continuation lines onto the preceding row's narration.
#[test]
fn joins_wrapped_remarks() {
    let txns = parse_statement(&fixture_pages("icici")).expect("parse icici fixture");
    assert_eq!(
        txns[6].narration_raw,
        "UPI/500108000007/Payment from Ph/synth.electricity@billdesk/ICIC/"
    );
    assert_eq!(
        txns[18].narration_raw,
        "NEFT-HDFC0000002-SYNTH CONSULTING LLP-INVOICE 42"
    );
}

#[test]
fn detects_icici_but_not_unsupported() {
    let icici = fixture_pages("icici");
    assert!(IciciProfile::detect(&icici[0]));
    let unsupported = fixture_pages("unsupported");
    assert!(!IciciProfile::detect(&unsupported[0]));
}
