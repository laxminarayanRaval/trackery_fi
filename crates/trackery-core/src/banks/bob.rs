//! Bank of Baroda e-statement profile ("bob World" mobile app export).
//!
//! The real export's header ("Serial No / Transaction Date / Value Date /
//! Description Cheque Number / Debit Credit Balance") wraps across several
//! physical lines with no reliable column offsets, and the letterhead is a
//! logo image — "BANK OF BARODA" never appears as extractable text. Parsing
//! anchors on row *shape* instead of header offsets:
//!
//! - The opening-balance row (`<serial> <date> Opening Balance - - <bal>`)
//!   is a running-balance anchor, not a transaction; it's skipped.
//! - A transaction row is `<serial> <date> <value date> <balance>
//!   <narration> <debit> <credit>`. Narration commonly wraps onto an undated
//!   continuation line — sometimes more than one — split mid-token, so
//!   continuations join with no separator. Debit/credit are each an amount
//!   or `-`, and the pair sometimes sits hard against the narration with no
//!   separating space.

use std::sync::OnceLock;

use chrono::{NaiveDate, Utc};
use regex::Regex;
use uuid::Uuid;

use super::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

pub struct BobProfile;

/// `<date> <value date> <balance> <body>`; `body` still holds the trailing
/// debit/credit fields, peeled off by [`trailing_fields`].
fn row_head_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?s)^(?P<date>\d{2}-\d{2}-\d{4})\s+(?P<value_date>\d{2}-\d{2}-\d{4})\s+(?P<balance>[\d,]+\.\d{2})\s+(?P<body>.*)$",
        )
        .expect("static regex")
    })
}

/// Splits `body` into `(narration, debit, credit)`, each of the latter two
/// being `-` or an Indian-grouped amount. Anchored at both ends so an
/// embedded `-` inside the narration (VPA/reference text) can't be mistaken
/// for the debit field — only the trailing pair can satisfy the whole match.
fn trailing_fields_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?s)^(?P<narration>.*?)\s*(?P<debit>-|[\d,]+\.\d{2})\s*(?P<credit>-|[\d,]+\.\d{2})$",
        )
        .expect("static regex")
    })
}

/// Does `line` start a new row (`<serial> <DD-MM-YYYY> ...`)? Distinguishes
/// real rows from wrapped continuation lines and header/footer furniture.
fn row_start(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let (serial, rest) = line.split_once(char::is_whitespace)?;
    if serial.is_empty() || !serial.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let rest = rest.trim_start();
    let b = rest.as_bytes();
    let date_led = b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| {
            if i == 2 || i == 5 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && b.get(10).is_none_or(|c| c.is_ascii_whitespace());
    date_led.then_some(rest)
}

/// Header/footer text repeated on every page — never a continuation line.
fn is_furniture(line: &str) -> bool {
    let t = line.trim();
    t.is_empty()
        || matches!(
            t,
            "Serial"
                | "No"
                | "Transaction"
                | "Date"
                | "Value"
                | "Description Cheque"
                | "Number"
                | "Debit Credit Balance"
        )
        || t.starts_with("Account Statement from")
        || t.starts_with("This is a computer-generated statement")
        || t.starts_with("This is a computer generated statement")
        || t.starts_with("maintained in the bank")
        || t.starts_with("Page ")
        || t.starts_with("Note:")
        || t.starts_with("Closing Balance")
}

impl BankProfile for BobProfile {
    fn bank(&self) -> Bank {
        Bank::Bob
    }

    /// "BANK OF BARODA" covers statements where the bank name is real text;
    /// `BARB0` is the bank's universal IFSC prefix (every branch code starts
    /// with it) and is what actually survives extraction on bob World app
    /// statements, whose letterhead is a logo image with no extractable text.
    fn detect(first_page_text: &str) -> bool {
        first_page_text.contains("BANK OF BARODA") || first_page_text.contains("BARB0")
    }

    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
        let mut rows: Vec<(usize, String)> = Vec::new(); // (starting line_no, joined raw row)
        let mut line_no = 0usize; // 1-based across all pages

        for page in pages {
            let mut lines = page.lines().peekable();
            while let Some(line) = lines.next() {
                line_no += 1;
                let Some(rest) = row_start(line) else {
                    continue; // furniture, or a continuation already folded in
                };
                let start_line = line_no;
                let mut joined = rest.to_string();
                while let Some(&next) = lines.peek() {
                    if row_start(next).is_some() || is_furniture(next) {
                        break;
                    }
                    joined.push_str(lines.next().expect("peeked Some"));
                    line_no += 1;
                }
                rows.push((start_line, joined));
            }
        }

        let mut txns = Vec::with_capacity(rows.len());
        for (line_no, row) in rows {
            let malformed = || ParseError::MalformedStatement { line: line_no };
            if row.contains("Opening Balance") {
                continue; // running-balance anchor, not a transaction
            }
            let caps = row_head_re().captures(&row).ok_or_else(malformed)?;
            let date =
                NaiveDate::parse_from_str(&caps["date"], "%d-%m-%Y").map_err(|_| malformed())?;
            let balance_paise = Money::parse(&caps["balance"])
                .map_err(|_| malformed())?
                .paise();
            let fields = trailing_fields_re()
                .captures(caps["body"].trim())
                .ok_or_else(malformed)?;
            let narration_raw = fields["narration"].trim().to_string();
            let (debit, credit) = (&fields["debit"], &fields["credit"]);
            let (direction, amount) = match (debit, credit) {
                (d, "-") if d != "-" => (Direction::Debit, d),
                ("-", c) if c != "-" => (Direction::Credit, c),
                _ => return Err(malformed()),
            };
            let amount_paise = Money::parse(amount).map_err(|_| malformed())?.paise();
            let now = Utc::now();
            txns.push(Transaction {
                id: Uuid::new_v4(),
                account_id: Uuid::nil(), // app layer assigns the real account
                date,
                narration_raw,
                description: None,
                direction,
                amount_paise,
                balance_paise: Some(balance_paise),
                origin: TransactionOrigin::StatementImport,
                counterparty: None,
                bank: Some(Bank::Bob),
                category_id: None,
                created_at: now,
                updated_at: now,
                deleted: false,
            });
        }
        Ok(txns)
    }
}
