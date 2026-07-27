use trackery_core::model::{Bank, TxnMode};
use trackery_core::narration::decompose;

/// Assert a narration decomposes to the expected fields. Empty expected
/// strings mean `None` for that field.
fn check(bank: Bank, narration: &str, mode: TxnMode, name: &str, vpa: &str, reference: &str) {
    let cp = decompose(bank, narration)
        .unwrap_or_else(|| panic!("{bank:?} should decompose: {narration:?}"));
    assert_eq!(cp.mode, mode, "mode for {narration:?}");
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    assert_eq!(cp.name, opt(name), "name for {narration:?}");
    assert_eq!(cp.vpa, opt(vpa), "vpa for {narration:?}");
    assert_eq!(cp.reference, opt(reference), "reference for {narration:?}");
}

#[test]
fn bob_upi() {
    for (n, name, vpa, r) in [
        (
            "UPI/DR/509912345678/RAVI SYNTH/YESB/q509912345@ybl/PAYMENT",
            "RAVI SYNTH",
            "q509912345@ybl",
            "509912345678",
        ),
        (
            "UPI/CR/509912345679/SYNTH STORES/ICIC/synth.stores@icici/COLLECT",
            "SYNTH STORES",
            "synth.stores@icici",
            "509912345679",
        ),
        (
            "UPI/DR/509912345680/ANITA SYNTH/SBIN/anita.synth@oksbi/RENT JUL",
            "ANITA SYNTH",
            "anita.synth@oksbi",
            "509912345680",
        ),
        (
            "UPI/CR/509912345681/MOHAN SYNTH/HDFC/9999999001@ybl/SPLIT",
            "MOHAN SYNTH",
            "9999999001@ybl",
            "509912345681",
        ),
        (
            "UPI/DR/509912345682/SYNTH GROCERY/YESB/synthgrocery@ybl/",
            "SYNTH GROCERY",
            "synthgrocery@ybl",
            "509912345682",
        ),
        (
            "UPI/DR/509912345683/KAVITA SYNTH/PYTM/kavita-1@paytm/UPI",
            "KAVITA SYNTH",
            "kavita-1@paytm",
            "509912345683",
        ),
        (
            "UPI/CR/509912345684/SYNTH EMPLOYER/UTIB/reimb@okaxis/REIMB",
            "SYNTH EMPLOYER",
            "reimb@okaxis",
            "509912345684",
        ),
        (
            "UPI/DR/509912345685/SYNTH FUEL/BARB/synthfuel@barodampay/FUEL",
            "SYNTH FUEL",
            "synthfuel@barodampay",
            "509912345685",
        ),
        (
            "UPI/DR/509912345686/DINESH SYNTH/KKBK/dinesh.s@okicici/LUNCH",
            "DINESH SYNTH",
            "dinesh.s@okicici",
            "509912345686",
        ),
        (
            "UPI/CR/509912345687/PRIYA SYNTH/YESB/priya99@ybl/GIFT",
            "PRIYA SYNTH",
            "priya99@ybl",
            "509912345687",
        ),
    ] {
        check(Bank::Bob, n, TxnMode::Upi, name, vpa, r);
    }
}

/// bob World app export narrations carry no counterparty name field at all —
/// only a reference and the counterparty VPA are recoverable.
#[test]
fn bob_upi_bob_world_app_has_no_name() {
    for (n, vpa, r) in [
        (
            "UPI/900000000090/14:22:10/UPI/synth.merchant@ybl",
            "synth.merchant@ybl",
            "900000000090",
        ),
        (
            "UPI/900000000091/09:05:47/UPI/synthsender@i",
            "synthsender@i",
            "900000000091",
        ),
    ] {
        check(Bank::Bob, n, TxnMode::Upi, "", vpa, r);
    }
}

#[test]
fn bob_imps() {
    for (n, name, r) in [
        (
            "IMPS/P2A/509912345688/RAVI SYNTH/HDFC/RENT",
            "RAVI SYNTH",
            "509912345688",
        ),
        (
            "IMPS/P2P/509912345689/ANITA SYNTH/ICIC/LOAN RET",
            "ANITA SYNTH",
            "509912345689",
        ),
        (
            "IMPS/P2A/509912345690/SYNTH TRADERS/SBIN/INVOICE 12",
            "SYNTH TRADERS",
            "509912345690",
        ),
        (
            "IMPS/P2A/509912345691/MOHAN SYNTH/UTIB/",
            "MOHAN SYNTH",
            "509912345691",
        ),
        (
            "IMPS/P2P/509912345692/KAVITA SYNTH/YESB/FAMILY",
            "KAVITA SYNTH",
            "509912345692",
        ),
        (
            "IMPS/P2A/509912345693/DINESH SYNTH/KKBK/EMI",
            "DINESH SYNTH",
            "509912345693",
        ),
        (
            "IMPS/P2A/509912345694/PRIYA SYNTH/BARB/SHARED",
            "PRIYA SYNTH",
            "509912345694",
        ),
        (
            "IMPS/P2P/509912345695/SYNTH SERVICES/HDFC/AMC",
            "SYNTH SERVICES",
            "509912345695",
        ),
        (
            "IMPS/P2A/509912345696/RAHUL SYNTH/ICIC/TRIP",
            "RAHUL SYNTH",
            "509912345696",
        ),
        (
            "IMPS/P2A/509912345697/SYNTH RENTALS/SBIN/DEPOSIT",
            "SYNTH RENTALS",
            "509912345697",
        ),
    ] {
        check(Bank::Bob, n, TxnMode::Imps, name, "", r);
    }
}

#[test]
fn bob_neft_atm_pos() {
    for (n, name, r) in [
        (
            "NEFT/N182261234567890/SYNTH EMPLOYER PVT LTD",
            "SYNTH EMPLOYER PVT LTD",
            "N182261234567890",
        ),
        (
            "NEFT/N182261234567891/SYNTH INSURANCE CO",
            "SYNTH INSURANCE CO",
            "N182261234567891",
        ),
        (
            "NEFT/CMS1234567892/SYNTH MUTUAL FUND/REDEMPTION",
            "SYNTH MUTUAL FUND",
            "CMS1234567892",
        ),
        (
            "NEFT/N182261234567893/RAVI SYNTH/GIFT",
            "RAVI SYNTH",
            "N182261234567893",
        ),
        (
            "NEFT/N182261234567894/SYNTH BROKING LTD",
            "SYNTH BROKING LTD",
            "N182261234567894",
        ),
    ] {
        check(Bank::Bob, n, TxnMode::Neft, name, "", r);
    }
    for n in [
        "ATM-CASH/SELF/XXXX1234",
        "ATM-CASH/SELF/XXXX1234/SYNTH NAGAR",
        "ATM/CASH WDL/XXXX1234",
    ] {
        check(Bank::Bob, n, TxnMode::Atm, "", "", "");
    }
    for (n, _) in [
        ("POS/412345XXXXXX7890/SYNTH STORES", "SYNTH STORES"),
        (
            "POS/412345XXXXXX7890/SYNTH SUPERMARKET",
            "SYNTH SUPERMARKET",
        ),
    ] {
        let cp = decompose(Bank::Bob, n).expect("pos should decompose");
        assert_eq!(cp.mode, TxnMode::Card);
        assert!(cp.merchant.is_some(), "merchant for {n:?}");
    }
}

#[test]
fn hdfc_upi() {
    for (n, name, vpa, r) in [
        (
            "UPI-RAVI SYNTH-q509912345@ybl-YESB0000001-509912345678-PAYMENT",
            "RAVI SYNTH",
            "q509912345@ybl",
            "509912345678",
        ),
        (
            "UPI-SYNTH STORES-synth.stores@icici-ICIC0000001-509912345679-COLLECT",
            "SYNTH STORES",
            "synth.stores@icici",
            "509912345679",
        ),
        (
            "UPI-ANITA SYNTH-anita.synth@oksbi-SBIN0000001-509912345680-RENT JUL",
            "ANITA SYNTH",
            "anita.synth@oksbi",
            "509912345680",
        ),
        (
            "UPI-MOHAN SYNTH-9999999001@ybl-HDFC0000001-509912345681-SPLIT",
            "MOHAN SYNTH",
            "9999999001@ybl",
            "509912345681",
        ),
        (
            "UPI-SYNTH GROCERY-synthgrocery@ybl-YESB0000001-509912345682-GROCERY",
            "SYNTH GROCERY",
            "synthgrocery@ybl",
            "509912345682",
        ),
        (
            "UPI-KAVITA SYNTH-kavita-1@paytm-PYTM0123456-509912345683-UPI",
            "KAVITA SYNTH",
            "kavita-1@paytm",
            "509912345683",
        ),
        (
            "UPI-SYNTH EMPLOYER-reimb@okaxis-UTIB0000001-509912345684-REIMB",
            "SYNTH EMPLOYER",
            "reimb@okaxis",
            "509912345684",
        ),
        (
            "UPI-SYNTH FUEL-synthfuel@barodampay-BARB0000001-509912345685-FUEL",
            "SYNTH FUEL",
            "synthfuel@barodampay",
            "509912345685",
        ),
        (
            "UPI-DINESH SYNTH-dinesh.s@okicici-KKBK0000001-509912345686-LUNCH",
            "DINESH SYNTH",
            "dinesh.s@okicici",
            "509912345686",
        ),
        (
            "UPI-PRIYA SYNTH-priya99@ybl-YESB0000001-509912345687-GIFT",
            "PRIYA SYNTH",
            "priya99@ybl",
            "509912345687",
        ),
    ] {
        check(Bank::Hdfc, n, TxnMode::Upi, name, vpa, r);
    }
}

#[test]
fn hdfc_imps_neft_atm_pos() {
    for (n, name, r) in [
        (
            "IMPS-509912345688-RAVI SYNTH-HDFC-XXXX1234-RENT",
            "RAVI SYNTH",
            "509912345688",
        ),
        (
            "IMPS-509912345689-ANITA SYNTH-ICIC-XXXX1234-LOAN RET",
            "ANITA SYNTH",
            "509912345689",
        ),
        (
            "IMPS-509912345690-SYNTH TRADERS-SBIN-XXXX1234-INVOICE",
            "SYNTH TRADERS",
            "509912345690",
        ),
        (
            "IMPS-509912345691-MOHAN SYNTH-UTIB-XXXX1234-",
            "MOHAN SYNTH",
            "509912345691",
        ),
        (
            "IMPS-509912345692-KAVITA SYNTH-YESB-XXXX1234-FAMILY",
            "KAVITA SYNTH",
            "509912345692",
        ),
        (
            "IMPS-509912345693-DINESH SYNTH-KKBK-XXXX1234-EMI",
            "DINESH SYNTH",
            "509912345693",
        ),
        (
            "IMPS-509912345694-PRIYA SYNTH-BARB-XXXX1234-SHARED",
            "PRIYA SYNTH",
            "509912345694",
        ),
        (
            "IMPS-509912345695-SYNTH SERVICES-HDFC-XXXX1234-AMC",
            "SYNTH SERVICES",
            "509912345695",
        ),
        (
            "IMPS-509912345696-RAHUL SYNTH-ICIC-XXXX1234-TRIP",
            "RAHUL SYNTH",
            "509912345696",
        ),
        (
            "IMPS-509912345697-SYNTH RENTALS-SBIN-XXXX1234-DEPOSIT",
            "SYNTH RENTALS",
            "509912345697",
        ),
    ] {
        check(Bank::Hdfc, n, TxnMode::Imps, name, "", r);
    }
    for (n, name) in [
        (
            "NEFT CR-SBIN0000001-SYNTH EMPLOYER PVT LTD-SALARY JUN-XXXX1234",
            "SYNTH EMPLOYER PVT LTD",
        ),
        (
            "NEFT DR-HDFC0000001-SYNTH INSURANCE CO-PREMIUM-XXXX1234",
            "SYNTH INSURANCE CO",
        ),
        (
            "NEFT CR-ICIC0000001-SYNTH MUTUAL FUND-REDEMPTION-XXXX1234",
            "SYNTH MUTUAL FUND",
        ),
        ("NEFT DR-UTIB0000001-RAVI SYNTH-GIFT-XXXX1234", "RAVI SYNTH"),
        (
            "NEFT CR-KKBK0000001-SYNTH BROKING LTD-PAYOUT-XXXX1234",
            "SYNTH BROKING LTD",
        ),
    ] {
        check(Bank::Hdfc, n, TxnMode::Neft, name, "", "");
    }
    for n in [
        "ATW-412345XXXXXX7890-S1ACWD01-MUMBAI",
        "EAW-412345XXXXXX7890-S1ANWD14-SYNTH NAGAR",
        "NWD-412345XXXXXX7890-SYNTHATM01-DELHI",
    ] {
        check(Bank::Hdfc, n, TxnMode::Atm, "", "", "");
    }
    for n in [
        "POS 412345XXXXXX7890 SYNTH STORES PVT L",
        "POS 412345XXXXXX7890 SYNTH SUPERMARKET",
    ] {
        let cp = decompose(Bank::Hdfc, n).expect("pos should decompose");
        assert_eq!(cp.mode, TxnMode::Card);
        assert!(cp.merchant.is_some(), "merchant for {n:?}");
    }
}

#[test]
fn icici_upi() {
    for (n, vpa, r) in [
        (
            "UPI/509912345678/Payment from Ph/q509912345@ybl/YESB/",
            "q509912345@ybl",
            "509912345678",
        ),
        (
            "UPI/509912345679/Collect request/synth.stores@icici/ICIC/",
            "synth.stores@icici",
            "509912345679",
        ),
        (
            "UPI/509912345680/Rent July/anita.synth@oksbi/SBIN/",
            "anita.synth@oksbi",
            "509912345680",
        ),
        (
            "UPI/509912345681/Split bill/9999999001@ybl/HDFC/",
            "9999999001@ybl",
            "509912345681",
        ),
        (
            "UPI/509912345682/Grocery/synthgrocery@ybl/YESB/",
            "synthgrocery@ybl",
            "509912345682",
        ),
        (
            "UPI/509912345683/UPI/kavita-1@paytm/PYTM/",
            "kavita-1@paytm",
            "509912345683",
        ),
        (
            "UPI/509912345684/Reimbursement/reimb@okaxis/UTIB/",
            "reimb@okaxis",
            "509912345684",
        ),
        (
            "UPI/509912345685/Fuel/synthfuel@barodampay/BARB/",
            "synthfuel@barodampay",
            "509912345685",
        ),
        (
            "UPI/509912345686/Lunch/dinesh.s@okicici/KKBK/",
            "dinesh.s@okicici",
            "509912345686",
        ),
        (
            "UPI/509912345687/Gift/priya99@ybl/YESB/",
            "priya99@ybl",
            "509912345687",
        ),
    ] {
        check(Bank::Icici, n, TxnMode::Upi, "", vpa, r);
    }
}

#[test]
fn icici_imps_neft_atm_pos() {
    for (n, name, r) in [
        (
            "MMT/IMPS/509912345688/RENT/RAVI SYNTH/HDFC",
            "RAVI SYNTH",
            "509912345688",
        ),
        (
            "MMT/IMPS/509912345689/LOAN RET/ANITA SYNTH/ICIC",
            "ANITA SYNTH",
            "509912345689",
        ),
        (
            "MMT/IMPS/509912345690/INVOICE/SYNTH TRADERS/SBIN",
            "SYNTH TRADERS",
            "509912345690",
        ),
        (
            "MMT/IMPS/509912345691//MOHAN SYNTH/UTIB",
            "MOHAN SYNTH",
            "509912345691",
        ),
        (
            "MMT/IMPS/509912345692/FAMILY/KAVITA SYNTH/YESB",
            "KAVITA SYNTH",
            "509912345692",
        ),
        (
            "MMT/IMPS/509912345693/EMI/DINESH SYNTH/KKBK",
            "DINESH SYNTH",
            "509912345693",
        ),
        (
            "MMT/IMPS/509912345694/SHARED/PRIYA SYNTH/BARB",
            "PRIYA SYNTH",
            "509912345694",
        ),
        (
            "MMT/IMPS/509912345695/AMC/SYNTH SERVICES/HDFC",
            "SYNTH SERVICES",
            "509912345695",
        ),
        (
            "MMT/IMPS/509912345696/TRIP/RAHUL SYNTH/ICIC",
            "RAHUL SYNTH",
            "509912345696",
        ),
        (
            "MMT/IMPS/509912345697/DEPOSIT/SYNTH RENTALS/SBIN",
            "SYNTH RENTALS",
            "509912345697",
        ),
    ] {
        check(Bank::Icici, n, TxnMode::Imps, name, "", r);
    }
    for (n, name, r) in [
        (
            "NEFT-SBIN0000001-SYNTH EMPLOYER PVT LTD-SALARY",
            "SYNTH EMPLOYER PVT LTD",
            "",
        ),
        (
            "NEFT-HDFC0000001-SYNTH INSURANCE CO-PREMIUM",
            "SYNTH INSURANCE CO",
            "",
        ),
        (
            "NEFT-ICIC0000001-SYNTH MUTUAL FUND-REDEMPTION",
            "SYNTH MUTUAL FUND",
            "",
        ),
        ("NEFT-UTIB0000001-RAVI SYNTH-GIFT", "RAVI SYNTH", ""),
        (
            "NEFT-KKBK0000001-SYNTH BROKING LTD-PAYOUT",
            "SYNTH BROKING LTD",
            "",
        ),
    ] {
        check(Bank::Icici, n, TxnMode::Neft, name, "", r);
    }
    for n in ["ATM/CASH WDL/XXXX1234/SYNTH BRANCH", "ATM/SELF/XXXX1234"] {
        check(Bank::Icici, n, TxnMode::Atm, "", "", "");
    }
    for n in [
        "VIN/SYNTH STORES/412345XXXXXX7890/",
        "VIN/SYNTH SUPERMARKT/412345XXXXXX7890/1234",
    ] {
        let cp = decompose(Bank::Icici, n).expect("vin should decompose");
        assert_eq!(cp.mode, TxnMode::Card);
        assert!(cp.merchant.is_some(), "merchant for {n:?}");
    }
}

#[test]
fn garbage_returns_none() {
    for bank in [Bank::Bob, Bank::Hdfc, Bank::Icici] {
        for n in [
            "",
            "   ",
            "OPENING BALANCE",
            "TOTAL",
            "random text with no structure",
            "CHQ DEP 000123 SYNTH PAYER",
            "INT.PD:XXXX1234:01-07-2026",
            "SMS CHARGES JUN 2026",
            "UPI",
            "NEFT",
            "…",
        ] {
            assert_eq!(
                decompose(bank, n),
                None,
                "{bank:?} should NOT decompose {n:?}"
            );
        }
    }
}

/// End-to-end: `parse_statement` enriches parsed fixture rows via decompose,
/// and leaves truncated/unknown narrations without a counterparty.
#[test]
fn parse_statement_enriches_fixture_counterparties() {
    let pages = |dir: &str| {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(dir);
        let mut paths: Vec<_> = std::fs::read_dir(&base)
            .expect("fixture dir")
            .map(|e| e.expect("dir entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "txt"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|p| std::fs::read_to_string(p).expect("read fixture"))
            .collect::<Vec<_>>()
    };
    let find = |txns: &[trackery_core::model::Transaction], needle: &str| {
        txns.iter()
            .find(|t| t.narration_raw.contains(needle))
            .unwrap_or_else(|| panic!("no fixture row contains {needle:?}"))
            .clone()
    };

    let bob = trackery_core::banks::parse_statement(&pages("bob")).expect("bob parses");
    let upi = find(&bob, "synth.merchant1@ybl");
    let cp = upi.counterparty.expect("bob upi row decomposes");
    assert_eq!(cp.mode, TxnMode::Upi);
    // bob World app narrations carry no counterparty name field at all.
    assert_eq!(cp.name, None);
    assert_eq!(cp.vpa.as_deref(), Some("synth.merchant1@ybl"));
    assert_eq!(cp.reference.as_deref(), Some("900000000001"));
    // Interest-credit narration matches no BOB rule and must stay None.
    assert_eq!(find(&bob, "Int.Pd").counterparty, None);

    let hdfc = trackery_core::banks::parse_statement(&pages("hdfc")).expect("hdfc parses");
    let upi = find(&hdfc, "synth.merchant@okhdfcbank");
    let cp = upi.counterparty.expect("hdfc upi row decomposes");
    assert_eq!(cp.mode, TxnMode::Upi);
    assert_eq!(cp.name.as_deref(), Some("SYNTH MERCHANT"));
    assert_eq!(cp.vpa.as_deref(), Some("synth.merchant@okhdfcbank"));
    assert_eq!(cp.reference.as_deref(), Some("509912345678"));

    let icici = trackery_core::banks::parse_statement(&pages("icici")).expect("icici parses");
    let upi = find(&icici, "synth.grocer@paytm");
    let cp = upi.counterparty.expect("icici upi row decomposes");
    assert_eq!(cp.mode, TxnMode::Upi);
    assert_eq!(cp.vpa.as_deref(), Some("synth.grocer@paytm"));
    assert_eq!(cp.reference.as_deref(), Some("500100000001"));
}

/// No panic on arbitrary bytes — decompose must be total.
/// ponytail: xorshift fuzz loop instead of a fuzzing dependency.
#[test]
fn no_panic_on_random_input() {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..5_000 {
        let mut bytes = Vec::with_capacity(24);
        for _ in 0..24 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            bytes.push((x & 0xFF) as u8);
        }
        let s = String::from_utf8_lossy(&bytes);
        for bank in [Bank::Bob, Bank::Hdfc, Bank::Icici] {
            let _ = decompose(bank, &s);
        }
    }
}
