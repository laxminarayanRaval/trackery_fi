//! ICICI Bank e-statement profile.
//!
//! Row shape (extracted text): `<value date> <transaction date> <remarks...>
//! <withdrawal> <deposit> <balance>`, dates DD-Mon-YYYY, exactly one of
//! withdrawal/deposit populated. Long Transaction Remarks wrap onto undated
//! continuation lines mid-token, so continuations join with no separator.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::banks::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

pub struct IciciProfile;

impl BankProfile for IciciProfile {
    fn bank(&self) -> Bank {
        Bank::Icici
    }

    fn detect(first_page_text: &str) -> bool {
        first_page_text.contains("ICICI Bank")
    }

    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
        let mut txns: Vec<Transaction> = Vec::new();
        let mut line_no = 0usize; // 1-based across all pages
        for page in pages {
            // Each page re-prints the column header; only lines between it
            // and the next blank line are table content, the rest is
            // header/footer furniture regardless of page dimensions.
            let mut in_table = false;
            for line in page.lines() {
                line_no += 1;
                let trimmed = line.trim();
                if !in_table {
                    in_table = trimmed.contains("Transaction Remarks");
                } else if trimmed.is_empty() {
                    in_table = false;
                } else if leading_date(trimmed).is_some() {
                    txns.push(parse_row(trimmed, line_no)?);
                } else if let Some(last) = txns.last_mut() {
                    // ponytail: wraps are mid-token, so no joining space;
                    // revisit if a corpus statement wraps at a word boundary.
                    last.narration_raw.push_str(trimmed);
                }
            }
        }
        Ok(txns)
    }
}

fn leading_date(line: &str) -> Option<NaiveDate> {
    let token = line.split_whitespace().next()?;
    NaiveDate::parse_from_str(token, "%d-%b-%Y").ok()
}

fn parse_row(line: &str, line_no: usize) -> Result<Transaction, ParseError> {
    let malformed = || ParseError::MalformedStatement { line: line_no };
    let tokens: Vec<&str> = line.split_whitespace().collect();
    // value date, transaction date, remarks (>= 1 token), 3 amounts
    if tokens.len() < 6 {
        return Err(malformed());
    }
    // `date` is the Transaction Date column, not the value date.
    let date = NaiveDate::parse_from_str(tokens[1], "%d-%b-%Y").map_err(|_| malformed())?;
    let n = tokens.len();
    let withdrawal = Money::parse(tokens[n - 3]).map_err(|_| malformed())?;
    let deposit = Money::parse(tokens[n - 2]).map_err(|_| malformed())?;
    let balance = Money::parse(tokens[n - 1]).map_err(|_| malformed())?;
    let (direction, amount_paise) = match (withdrawal.paise(), deposit.paise()) {
        (w, 0) if w > 0 => (Direction::Debit, w),
        (0, d) if d > 0 => (Direction::Credit, d),
        _ => return Err(malformed()),
    };
    let now = Utc::now();
    Ok(Transaction {
        id: Uuid::new_v4(),
        account_id: Uuid::nil(), // app layer assigns the real account
        date,
        narration_raw: tokens[2..n - 3].join(" "),
        description: None,
        direction,
        amount_paise,
        balance_paise: Some(balance.paise()),
        origin: TransactionOrigin::StatementImport,
        counterparty: None,
        bank: Some(Bank::Icici),
        category_id: None,
        created_at: now,
        updated_at: now,
        deleted: false,
    })
}
