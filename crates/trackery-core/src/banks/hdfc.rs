//! HDFC Bank e-statement profile.
//!
//! The real export's header (`Date Narration Chq./Ref.No. Value Dt
//! Withdrawal Amt. Deposit Amt. Closing Balance`) is a single line, but the
//! transaction rows below it are NOT column-aligned to those header
//! offsets — each row prints only whichever of Withdrawal/Deposit actually
//! applies, so a row ends in exactly one amount plus the closing balance,
//! never two. Direction is therefore derived from the balance delta versus
//! the previous row (seeded from the statement's own "Opening Balance"
//! summary line, not assumed to be zero), not from column position.
//!
//! Rows start at a `DD/MM/YY`-led line; long narrations wrap onto
//! continuation lines mid-token (joined with no separator), and — unlike
//! BOB/ICICI — those continuation lines can carry on *after* the
//! amount/balance pair has already appeared, so the row only ends at the
//! next dated line or footer/summary furniture.

use std::sync::OnceLock;

use chrono::{NaiveDate, Utc};
use regex::Regex;
use uuid::Uuid;

use super::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

const WITHDRAWAL: &str = "Withdrawal Amt.";

pub struct HdfcProfile;

/// Does the line start with a DD/MM/YY shape (digits and slashes)?
fn date_led(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() >= 8
        && b[..8].iter().enumerate().all(|(i, c)| {
            if i == 2 || i == 5 {
                *c == b'/'
            } else {
                c.is_ascii_digit()
            }
        })
        && b.get(8).is_none_or(|c| c.is_ascii_whitespace())
}

/// Parse a leading DD/MM/YY date; two-digit years are always 20xx.
fn leading_date(line: &str) -> Option<NaiveDate> {
    let b = line.as_bytes();
    let num = |i: usize| ((b[i] - b'0') * 10 + (b[i + 1] - b'0')) as u32;
    NaiveDate::from_ymd_opt(2000 + num(6) as i32, num(3), num(0))
}

/// Header/footer/account-summary furniture — never a continuation line.
fn is_furniture(line: &str) -> bool {
    let t = line.trim();
    t.is_empty()
        || t.starts_with("Date Narration")
        || t.starts_with("STATEMENT SUMMARY")
        || t.starts_with("Opening Balance")
        || t.starts_with("Page No")
        || t.starts_with("JOINT HOLDERS")
        || t.starts_with("Nomination")
        || t.starts_with("HDFC BANK LIMITED")
        || t.starts_with("Contents of this statement")
        || t.starts_with("this statement.")
        || t.starts_with("Generated On:")
        || t.starts_with("This is a computer generated statement")
        || t.starts_with("not require signature.")
        || t.starts_with("RTGS/NEFT IFSC")
        || t.starts_with("Account Branch")
        || t.starts_with("Address :")
        || t.starts_with("City :")
        || t.starts_with("State :")
        || t.starts_with("Phone no.")
        || t.starts_with("OD Limit")
        || t.starts_with("Currency")
        || t.starts_with("Email :")
        || t.starts_with("Cust ID")
        || t.starts_with("Account No")
        || t.starts_with("A/C Open Date")
        || t.starts_with("Account Status")
        || t.starts_with("Account Type")
        || t.starts_with("Branch Code")
        || t.starts_with("*Closing balance")
        || t.starts_with("State account branch")
        || t.starts_with("Registered Office Address")
        || summary_data_re().is_match(t)
}

/// An Indian-grouped money amount, e.g. `1,23,456.78` or `21.00`.
fn money_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d[\d,]*\.\d{2}").expect("static regex"))
}

/// The statement-summary data row: `<opening> <dr count> <cr count>
/// <debits> <credits> <closing>` — used both to seed the running balance
/// and to recognize the line as furniture rather than a stray continuation.
fn summary_data_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?P<opening>[\d,]+\.\d{2})\s+\d+\s+\d+\s+[\d,]+\.\d{2}\s+[\d,]+\.\d{2}\s+[\d,]+\.\d{2}$")
            .expect("static regex")
    })
}

/// The statement's declared opening balance, so the first row's direction
/// isn't guessed at — real accounts are rarely opened with a zero balance.
fn opening_balance_paise(pages: &[String]) -> Option<i64> {
    pages
        .iter()
        .flat_map(|p| p.lines())
        .find_map(|line| summary_data_re().captures(line.trim()))
        .and_then(|caps| Money::parse(&caps["opening"]).ok())
        .map(|m| m.paise())
}

impl BankProfile for HdfcProfile {
    fn bank(&self) -> Bank {
        Bank::Hdfc
    }

    /// Bank name alone is not enough — other banks' statements can carry
    /// "HDFC" inside UPI narrations, so require HDFC's withdrawal column
    /// label too.
    fn detect(first_page_text: &str) -> bool {
        first_page_text.contains("HDFC BANK") && first_page_text.contains(WITHDRAWAL)
    }

    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
        let mut txns = Vec::new();
        let mut balance_before = opening_balance_paise(pages).unwrap_or(0);
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
                let amount_paise = (balance_paise - balance_before).abs();
                let direction = if balance_paise >= balance_before {
                    Direction::Credit
                } else {
                    Direction::Debit
                };
                balance_before = balance_paise;

                let narration_raw = money_re().replace_all(&block[8..], "").trim().to_string();
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
                    bank: Some(Bank::Hdfc),
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
