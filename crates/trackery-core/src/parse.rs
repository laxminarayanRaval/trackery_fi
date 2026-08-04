//! Bank statement parsing: extracted page text → typed transactions.
//!
//! Row amounts are validated against the running balance: each transaction's
//! direction comes from the balance delta, and a delta/amount mismatch is a
//! [`ParseError::MalformedStatement`] pointing at the offending line. This
//! sidesteps the classic ambiguity of collapsed empty Withdrawal/Deposit
//! columns in extracted PDF text.
//!
//! Sprint scope: ICICI only. BOB/HDFC profiles land the same way — fixture
//! first, then a `detect` marker and a row shape here.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::model::{Bank, Counterparty, Direction, Transaction, TxnMode};
use crate::money::Money;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The statement text does not match any supported bank profile.
    #[error("unsupported or unrecognized bank statement")]
    UnsupportedBank,
    /// Statement-shaped but a row failed to parse or reconcile.
    #[error("malformed statement at line {line}")]
    MalformedStatement { line: usize },
}

#[derive(Debug)]
pub struct ParsedStatement {
    pub bank: Bank,
    pub transactions: Vec<Transaction>,
}

/// Parse extracted statement pages (as returned by `pdf::open_statement`).
pub fn parse_statement(pages: &[String]) -> Result<ParsedStatement, ParseError> {
    let lines: Vec<&str> = pages.iter().flat_map(|p| p.lines()).collect();
    if !lines.iter().any(|l| l.contains("ICICI Bank")) {
        return Err(ParseError::UnsupportedBank);
    }
    let transactions = parse_rows(&lines)?;
    Ok(ParsedStatement {
        bank: Bank::Icici,
        transactions,
    })
}

/// One statement row accumulated across wrapped lines.
struct Row {
    line: usize, // 1-based, across concatenated pages
    date: NaiveDate,
    tokens: Vec<String>,
}

fn parse_rows(lines: &[&str]) -> Result<Vec<Transaction>, ParseError> {
    let mut rows: Vec<Row> = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let mut tokens = raw.split_whitespace().map(str::to_string).peekable();
        let Some(first) = tokens.peek() else { continue };
        if let Some(date) = parse_date(first) {
            rows.push(Row {
                line: idx + 1,
                date,
                tokens: tokens.skip(1).collect(),
            });
        } else if let Some(row) = rows.last_mut() {
            // Continuation of a wrapped narration — but only while the row's
            // amounts haven't arrived yet; once it ends in amount+balance,
            // trailing furniture (footers, next page headers) is ignored.
            if trailing_amounts(&row.tokens) < 2 {
                row.tokens.extend(tokens.map(|t| t.to_string()));
            }
        }
    }

    let mut prev_balance: Option<i64> = None;
    let mut out = Vec::new();
    for row in rows {
        let malformed = || ParseError::MalformedStatement { line: row.line };
        let tail = trailing_amounts(&row.tokens);
        let narration = row.tokens[..row.tokens.len() - tail].join(" ");

        if tail == 1 && is_opening_row(&narration) {
            prev_balance = Some(parse_amount(row.tokens.last().expect("tail == 1")).ok_or_else(malformed)?);
            continue;
        }
        if tail < 2 {
            return Err(malformed());
        }

        let amounts: Vec<i64> = row.tokens[row.tokens.len() - tail..]
            .iter()
            .map(|t| parse_amount(t).ok_or_else(malformed))
            .collect::<Result<_, _>>()?;
        let balance = *amounts.last().expect("tail >= 2");
        let prev = prev_balance.ok_or_else(malformed)?;
        let delta = balance - prev;
        // The row's real amount is whichever printed column matches the
        // balance movement (empty columns vanish in extracted text).
        if delta == 0 || !amounts[..amounts.len() - 1].contains(&delta.abs()) {
            return Err(malformed());
        }
        prev_balance = Some(balance);

        out.push(Transaction {
            id: Uuid::new_v4(),
            date: row.date,
            counterparty: decompose(&narration),
            narration_raw: narration,
            direction: if delta > 0 {
                Direction::Credit
            } else {
                Direction::Debit
            },
            amount_paise: delta.abs(),
            balance_paise: Some(balance),
            bank: Bank::Icici,
            updated_at: Utc::now(),
            deleted: false,
        });
    }
    Ok(out)
}

fn is_opening_row(narration: &str) -> bool {
    let upper = narration.to_ascii_uppercase();
    upper.contains("B/F") || upper.contains("BROUGHT FORWARD") || upper.contains("OPENING")
}

fn parse_date(token: &str) -> Option<NaiveDate> {
    ["%d-%m-%Y", "%d/%m/%Y"]
        .iter()
        .find_map(|fmt| NaiveDate::parse_from_str(token, fmt).ok())
}

/// Statement amounts always carry two decimals; anything else (refs, dates,
/// masked numbers) is narration.
fn is_amount(token: &str) -> bool {
    let t = token.strip_prefix('-').unwrap_or(token);
    let Some((int, frac)) = t.rsplit_once('.') else {
        return false;
    };
    frac.len() == 2
        && frac.bytes().all(|b| b.is_ascii_digit())
        && !int.is_empty()
        && int.bytes().next().is_some_and(|b| b.is_ascii_digit())
        && int.bytes().all(|b| b.is_ascii_digit() || b == b',')
}

fn trailing_amounts(tokens: &[String]) -> usize {
    tokens.iter().rev().take_while(|t| is_amount(t)).count()
}

fn parse_amount(token: &str) -> Option<i64> {
    is_amount(token)
        .then(|| Money::parse(token).ok())
        .flatten()
        .map(Money::paise)
}

/// Best-effort ICICI narration decomposition. Unknown shapes return `None`;
/// the raw narration is always preserved on the transaction.
fn decompose(narration: &str) -> Option<Counterparty> {
    let upper = narration.to_ascii_uppercase();
    let parts: Vec<&str> = narration.split('/').map(str::trim).collect();
    let digit_ref = || {
        parts
            .iter()
            .find(|p| p.len() >= 10 && p.bytes().all(|b| b.is_ascii_digit()))
            .map(|p| p.to_string())
    };

    let cp = if upper.starts_with("UPI/") {
        Counterparty {
            name: clean(parts.get(2)),
            vpa: parts.iter().find(|p| p.contains('@')).map(|p| p.to_string()),
            reference: digit_ref(),
            mode: TxnMode::Upi,
        }
    } else if upper.starts_with("NEFT") || upper.starts_with("RTGS") {
        // NEFT-<ref>-<name>-<note...> (slash-separated variants exist too)
        let sep = if narration.contains('-') { '-' } else { '/' };
        let fields: Vec<&str> = narration.splitn(4, sep).map(str::trim).collect();
        Counterparty {
            name: clean(fields.get(2)),
            vpa: None,
            reference: fields.get(1).map(|r| r.to_string()),
            mode: if upper.starts_with("NEFT") {
                TxnMode::Neft
            } else {
                TxnMode::Rtgs
            },
        }
    } else if upper.starts_with("MMT/IMPS") || upper.starts_with("IMPS") {
        Counterparty {
            name: clean(parts.last()),
            vpa: None,
            reference: digit_ref(),
            mode: TxnMode::Imps,
        }
    } else if upper.starts_with("ATM") {
        Counterparty {
            name: clean(parts.get(2)),
            vpa: None,
            reference: None,
            mode: TxnMode::Atm,
        }
    } else if upper.starts_with("POS") || upper.starts_with("VIN") {
        let name = narration.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
        Counterparty {
            name: (!name.is_empty()).then_some(name),
            vpa: None,
            reference: None,
            mode: TxnMode::Card,
        }
    } else if upper.starts_with("BIL/") {
        Counterparty {
            name: clean(parts.get(3)),
            vpa: None,
            reference: digit_ref(),
            mode: TxnMode::Other,
        }
    } else {
        return None;
    };
    Some(cp)
}

fn clean(part: Option<&&str>) -> Option<String> {
    part.map(|p| p.trim().to_string()).filter(|p| !p.is_empty())
}
