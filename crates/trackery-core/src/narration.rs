//! UPI/IMPS/NEFT narration decomposer: per-bank ordered regex tables.
//!
//! Each bank's e-statement encodes counterparty data differently inside the
//! narration string (BOB slash-delimited, HDFC dash-delimited, ICICI mixed).
//! [`decompose`] tries that bank's rules in order and builds a
//! [`Counterparty`] from the named capture groups `name`, `vpa`, `ref`,
//! `merchant`. Unknown narrations return `None` — never guess.

use std::sync::OnceLock;

use regex::Regex;

use crate::model::{Bank, Counterparty, TxnMode};

struct Rule {
    re: Regex,
    mode: TxnMode,
}

/// (pattern, mode) tables, tried in order. First match wins.
const BOB_RULES: &[(&str, TxnMode)] = &[
    // `\s*` before the VPA: multi-line narrations are re-joined with a space,
    // which can land right before the continuation-line VPA.
    (
        r"^UPI/(?:DR|CR)/(?P<ref>\d+)/(?P<name>[^/]+)/[^/]*/\s*(?P<vpa>[^/@\s]+@[A-Za-z][\w.]*)(?:/.*)?$",
        TxnMode::Upi,
    ),
    (
        r"^IMPS/P2[AP]/(?P<ref>\d+)/(?P<name>[^/]+)/[^/]*(?:/.*)?$",
        TxnMode::Imps,
    ),
    (
        r"^NEFT/(?P<ref>[A-Z0-9]+)/(?P<name>[^/]+)(?:/.*)?$",
        TxnMode::Neft,
    ),
    (r"^ATM[-/].*$", TxnMode::Atm),
    (r"^POS/[0-9X]+/(?P<merchant>.+)$", TxnMode::Card),
];

const HDFC_RULES: &[(&str, TxnMode)] = &[
    // VPA local parts may themselves contain dashes (kavita-1@paytm), so the
    // vpa group allows internal dashes while the delimiters stay single dashes.
    (
        r"^UPI-(?P<name>[^-]+)-(?P<vpa>[^\s@-]+(?:-[^\s@-]+)*@[A-Za-z][\w.]*)-[^-]*-(?P<ref>\d+)(?:-.*)?$",
        TxnMode::Upi,
    ),
    (r"^IMPS-(?P<ref>\d+)-(?P<name>[^-]+)-.*$", TxnMode::Imps),
    (
        r"^NEFT (?:CR|DR)-[A-Z0-9]+-(?P<name>[^-]+)-.*$",
        TxnMode::Neft,
    ),
    (r"^(?:ATW|EAW|NWD)-[0-9X]+-.*$", TxnMode::Atm),
    (r"^POS [0-9X]+ (?P<merchant>.+)$", TxnMode::Card),
];

const ICICI_RULES: &[(&str, TxnMode)] = &[
    (
        r"^UPI/(?P<ref>\d+)/[^/]*/(?P<vpa>[^/@\s]+@[A-Za-z][\w.]*)(?:/.*)?$",
        TxnMode::Upi,
    ),
    (
        r"^MMT/IMPS/(?P<ref>\d+)/[^/]*/(?P<name>[^/]+)(?:/.*)?$",
        TxnMode::Imps,
    ),
    (r"^NEFT-[A-Z0-9]+-(?P<name>[^-]+)(?:-.*)?$", TxnMode::Neft),
    (r"^ATM/.*$", TxnMode::Atm),
    (r"^VIN/(?P<merchant>[^/]+)(?:/.*)?$", TxnMode::Card),
];

fn rules(bank: Bank) -> &'static [Rule] {
    static BOB: OnceLock<Vec<Rule>> = OnceLock::new();
    static HDFC: OnceLock<Vec<Rule>> = OnceLock::new();
    static ICICI: OnceLock<Vec<Rule>> = OnceLock::new();
    let build = |defs: &'static [(&str, TxnMode)]| {
        defs.iter()
            .map(|(pattern, mode)| Rule {
                // Static patterns; every rule is exercised by narration tests,
                // so a bad pattern cannot reach a release build.
                re: Regex::new(pattern).expect("static narration regex"),
                mode: *mode,
            })
            .collect()
    };
    match bank {
        Bank::Bob => BOB.get_or_init(|| build(BOB_RULES)),
        Bank::Hdfc => HDFC.get_or_init(|| build(HDFC_RULES)),
        Bank::Icici => ICICI.get_or_init(|| build(ICICI_RULES)),
    }
}

/// Decompose a raw narration into structured counterparty fields.
/// Returns `None` when no rule for that bank matches.
pub fn decompose(bank: Bank, narration_raw: &str) -> Option<Counterparty> {
    let narration = narration_raw.trim();
    for rule in rules(bank) {
        if let Some(caps) = rule.re.captures(narration) {
            let get = |group: &str| {
                caps.name(group)
                    .map(|m| m.as_str().trim())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            return Some(Counterparty {
                name: get("name"),
                vpa: get("vpa"),
                reference: get("ref"),
                merchant: get("merchant"),
                mode: rule.mode,
            });
        }
    }
    None
}
