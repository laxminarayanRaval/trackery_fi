//! Bank of Baroda e-statement parser.
//!
//! BOB extracted text is layout-preserving: a column header line
//! (`DATE NARRATION CHQ.NO. WITHDRAWAL(DR) DEPOSIT(CR) BALANCE(INR)`)
//! precedes the rows on every page. Transaction rows lead with a
//! DD/MM/YYYY date and end with `<amount> <balance>`; whether the amount
//! is a withdrawal or a deposit is positional, so we anchor on the header
//! labels instead of hardcoding page dimensions. Long narrations wrap onto
//! continuation lines indented to the narration column, with no date or
//! amounts — those are joined onto the preceding transaction.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use super::{BankProfile, ParseError};
use crate::model::{Bank, Direction, Transaction, TransactionOrigin};
use crate::money::Money;

pub struct BobProfile;

/// Column anchors read off a page's header line.
struct Cols {
    /// Byte offset of `NARRATION` — continuation lines indent to here.
    narration: usize,
    /// Byte offset of `DEPOSIT` — an amount whose midpoint lies at or past
    /// this is a credit, otherwise a debit (amounts are right-aligned).
    deposit: usize,
}

impl BankProfile for BobProfile {
    fn bank(&self) -> Bank {
        Bank::Bob
    }

    fn detect(first_page_text: &str) -> bool {
        first_page_text.contains("BANK OF BARODA")
    }

    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
        let mut txns: Vec<Transaction> = Vec::new();
        // Carried across pages in case a page omits the repeated header.
        let mut cols: Option<Cols> = None;
        let mut line_no = 0usize; // 1-based, counted across all pages

        for page in pages {
            for line in page.lines() {
                line_no += 1;

                if let (Some(narration), Some(deposit)) =
                    (line.find("NARRATION"), line.find("DEPOSIT"))
                {
                    cols = Some(Cols { narration, deposit });
                    continue;
                }

                let date = match leading_date(line) {
                    Some(d) => d,
                    None => {
                        // Continuation lines indent to the narration column;
                        // furniture (headers, footers, page numbers) starts at
                        // the left margin or precedes the first header line.
                        let indent = line.len() - line.trim_start().len();
                        if let (Some(cols), Some(last), false) =
                            (cols.as_ref(), txns.last_mut(), line.trim().is_empty())
                        {
                            if indent >= cols.narration {
                                last.narration_raw.push(' ');
                                last.narration_raw.push_str(line.trim());
                            }
                        }
                        continue;
                    }
                };
                let date = date.ok_or(ParseError::MalformedStatement { line: line_no })?;
                let cols = match cols.as_ref() {
                    Some(c) => c,
                    // Date-led line before any column header: furniture
                    // (e.g. a period line) — a real row can't precede its header.
                    None => continue,
                };

                // Row shape: DD/MM/YYYY <narration...> <amount> <balance>
                let malformed = || ParseError::MalformedStatement { line: line_no };
                let toks = tokens_with_offsets(line);
                if toks.len() < 3 {
                    return Err(malformed());
                }
                let (_, balance_tok) = toks[toks.len() - 1];
                let (amount_off, amount_tok) = toks[toks.len() - 2];
                let balance = Money::parse(balance_tok).map_err(|_| malformed())?;
                let amount = Money::parse(amount_tok).map_err(|_| malformed())?;
                let narration = line[10..amount_off].trim();
                if narration.is_empty() {
                    return Err(malformed());
                }

                let amount_mid = amount_off + amount_tok.len() / 2;
                let direction = if amount_mid >= cols.deposit {
                    Direction::Credit
                } else {
                    Direction::Debit
                };

                let now = Utc::now();
                txns.push(Transaction {
                    id: Uuid::new_v4(),
                    account_id: Uuid::nil(), // app layer assigns the account
                    date,
                    narration_raw: narration.to_string(),
                    description: None,
                    direction,
                    // Magnitude only; column position carries the sign.
                    amount_paise: amount.paise().unsigned_abs() as i64,
                    balance_paise: Some(balance.paise()),
                    origin: TransactionOrigin::StatementImport,
                    counterparty: None, // decomposer is a later task
                    bank: Some(Bank::Bob),
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

/// `None` if the line doesn't lead with a DD/MM/YYYY shape;
/// `Some(None)` if it does but isn't a valid calendar date.
fn leading_date(line: &str) -> Option<Option<NaiveDate>> {
    let head = line.get(..10)?;
    let shaped = head.bytes().enumerate().all(|(i, b)| match i {
        2 | 5 => b == b'/',
        _ => b.is_ascii_digit(),
    });
    let followed_by_space = line.as_bytes().get(10).is_none_or(|b| *b == b' ');
    if !shaped || !followed_by_space {
        return None;
    }
    Some(NaiveDate::parse_from_str(head, "%d/%m/%Y").ok())
}

/// Whitespace-split tokens with their byte offsets in the line.
fn tokens_with_offsets(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push((s, &line[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &line[s..]));
    }
    out
}
