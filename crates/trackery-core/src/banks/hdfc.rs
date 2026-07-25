//! HDFC Bank e-statement profile.
//!
//! HDFC lays transactions out in fixed columns under a header line
//! (`Date  Narration  Chq./Ref.No.  Value Dt  Withdrawal Amt.  Deposit Amt.
//! Closing Balance`). Column spans are derived from that header's label
//! offsets on each page — nothing about page dimensions is hardcoded, so
//! header/footer furniture is skipped for free (only lines after the column
//! header that start with a DD/MM/YY date are transactions). Long narrations
//! wrap mid-token onto undated lines confined to the narration column; those
//! are appended to the previous row's narration with no separator.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use super::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

const NARRATION: &str = "Narration";
const REF_NO: &str = "Chq./Ref.No.";
const WITHDRAWAL: &str = "Withdrawal Amt.";
const DEPOSIT: &str = "Deposit Amt.";
const BALANCE: &str = "Closing Balance";

pub struct HdfcProfile;

/// Byte offsets of the column labels in a page's header line.
struct Columns {
    narration: usize,
    ref_no: usize,
    withdrawal: usize,
    deposit: usize,
    balance: usize,
}

impl Columns {
    fn from_header(header: &str) -> Option<Self> {
        Some(Columns {
            narration: header.find(NARRATION)?,
            ref_no: header.find(REF_NO)?,
            withdrawal: header.find(WITHDRAWAL)?,
            deposit: header.find(DEPOSIT)?,
            balance: header.find(BALANCE)?,
        })
    }
}

/// Trimmed slice of `line` between byte columns, clamped to the line and
/// nudged to char boundaries (extracted text is effectively ASCII, so the
/// nudge is a no-op in practice).
fn col(line: &str, from: usize, to: usize) -> &str {
    let clamp = |i: usize| {
        let mut i = i.min(line.len());
        while !line.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let (a, b) = (clamp(from), clamp(to));
    line[a..b.max(a)].trim()
}

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

fn parse_row(line: &str, line_no: usize, c: &Columns) -> Result<Transaction, ParseError> {
    let malformed = || ParseError::MalformedStatement { line: line_no };
    let date = leading_date(line).ok_or_else(malformed)?;
    let withdrawal = col(line, c.withdrawal, c.deposit);
    let deposit = col(line, c.deposit, c.balance);
    let (direction, amount) = match (withdrawal.is_empty(), deposit.is_empty()) {
        (false, true) => (Direction::Debit, withdrawal),
        (true, false) => (Direction::Credit, deposit),
        _ => return Err(malformed()),
    };
    let amount_paise = Money::parse(amount).map_err(|_| malformed())?.paise();
    let balance_paise = Money::parse(col(line, c.balance, line.len()))
        .map_err(|_| malformed())?
        .paise();
    let now = Utc::now();
    Ok(Transaction {
        id: Uuid::new_v4(),
        account_id: Uuid::nil(), // app layer assigns the real account
        date,
        narration_raw: col(line, c.narration, c.ref_no).to_string(),
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
    })
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
        let mut txns: Vec<Transaction> = Vec::new();
        let mut line_no = 0usize; // 1-based across all pages
        for page in pages {
            let mut cols: Option<Columns> = None;
            for line in page.lines() {
                line_no += 1;
                let Some(c) = &cols else {
                    // Everything above the column header is page furniture.
                    if line.contains(WITHDRAWAL) && line.contains(DEPOSIT) {
                        cols = Columns::from_header(line);
                    }
                    continue;
                };
                if date_led(line) {
                    txns.push(parse_row(line, line_no, c)?);
                } else if let Some(prev) = txns.last_mut() {
                    // Wrapped narration: undated line whose text sits wholly
                    // inside the narration column. HDFC wraps mid-token, so
                    // append with no separator. Footer furniture starts at
                    // column 0 and is ignored here.
                    let chunk = col(line, c.narration, c.ref_no);
                    if !chunk.is_empty()
                        && col(line, 0, c.narration).is_empty()
                        && col(line, c.ref_no, line.len()).is_empty()
                    {
                        prev.narration_raw.push_str(chunk);
                    }
                }
            }
        }
        Ok(txns)
    }
}
