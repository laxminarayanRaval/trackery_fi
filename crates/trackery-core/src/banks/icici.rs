//! ICICI Bank e-statement profile.
//!
//! Real net-banking exports print a `DATE MODE** PARTICULARS DEPOSITS
//! WITHDRAWALS BALANCE` header, but rows below it aren't column-aligned to
//! it: each row's date (`DD-MM-YYYY`, numeric) often sits alone on its own
//! line, the narration ("Particulars") wraps across several undated
//! continuation lines mid-token, and the row ends in a single trailing
//! `<amount> <balance>` pair — never separate, always-populated deposit and
//! withdrawal columns. Direction is derived from the balance delta against
//! the previous row, seeded by the table's own `B/F` (brought forward)
//! opening-balance row rather than assumed.

use std::sync::OnceLock;

use chrono::{NaiveDate, Utc};
use regex::Regex;
use uuid::Uuid;

use crate::banks::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

pub struct IciciProfile;

/// Does `line` start a new row (`DD-MM-YYYY` alone, or leading a longer
/// line)?
fn date_led(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| {
            if i == 2 || i == 5 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && b.get(10).is_none_or(|c| c.is_ascii_whitespace())
}

fn leading_date(line: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&line[..10], "%d-%m-%Y").ok()
}

/// An Indian-grouped money amount, e.g. `1,23,456.78` or `27.00`.
fn money_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d[\d,]*\.\d{2}").expect("static regex"))
}

/// Header/footer furniture repeated on every page — never a continuation.
fn is_furniture(line: &str) -> bool {
    let t = line.trim();
    t.is_empty()
        || t.starts_with("DATE MODE")
        || t.starts_with("Page ")
        || t.starts_with("Statement of Transactions")
        || t.starts_with("Your Base Branch")
        || t.starts_with("Visit www.icicibank.com")
        || t.starts_with("Dial your Bank")
        || t.starts_with("TOTAL")
}

impl BankProfile for IciciProfile {
    fn bank(&self) -> Bank {
        Bank::Icici
    }

    /// Case varies by template ("ICICI Bank" on older layouts, "ICICI BANK
    /// LTD." on the current net-banking export) — the logo carries no
    /// extractable text either way, so this literal is all there is.
    fn detect(first_page_text: &str) -> bool {
        first_page_text.to_uppercase().contains("ICICI BANK")
    }

    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
        let mut txns = Vec::new();
        let mut balance_before: Option<i64> = None;
        let mut line_no = 0usize; // 1-based across all pages

        for page in pages {
            let mut lines = page.lines().peekable();
            while let Some(line) = lines.next() {
                line_no += 1;
                if !date_led(line) {
                    continue; // furniture, or a continuation already folded in
                }
                let start_line = line_no;
                let mut block = line.to_string();
                while let Some(&next) = lines.peek() {
                    if date_led(next) || is_furniture(next) {
                        break;
                    }
                    block.push_str(lines.next().expect("peeked Some"));
                    line_no += 1;
                }

                let malformed = || ParseError::MalformedStatement { line: start_line };
                let date = leading_date(&block).ok_or_else(malformed)?;
                let balance_str = money_re()
                    .find_iter(&block)
                    .last()
                    .ok_or_else(malformed)?
                    .as_str();
                let balance_paise = Money::parse(balance_str).map_err(|_| malformed())?.paise();

                // `B/F` (brought forward) is the running-balance anchor, not
                // a transaction — it seeds the delta baseline instead.
                if block.contains("B/F") {
                    balance_before = Some(balance_paise);
                    continue;
                }
                let prev = balance_before.ok_or_else(malformed)?;
                let amount_paise = (balance_paise - prev).abs();
                let direction = if balance_paise >= prev {
                    Direction::Credit
                } else {
                    Direction::Debit
                };
                balance_before = Some(balance_paise);

                let narration_raw = money_re().replace_all(&block[10..], "").trim().to_string();
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
                    bank: Some(Bank::Icici),
                    category_id: None,
                    created_at: now,
                    updated_at: now,
                    deleted: false,
                });
            }
        }
        Ok(txns)
    }
}
