//! Bank statement profiles: detection + parsing, one module per bank.
//!
//! [`parse_statement`] is the single entry point: it sniffs the first page
//! against each registered profile in order (BOB → HDFC → ICICI, first match
//! wins) and hands all pages to the matching parser. Parsers receive extracted
//! page text (see `tests/fixtures/README.md`), never PDF bytes.
//!
//! Parsers do not know about accounts — they emit `account_id = Uuid::nil()`
//! and the app layer assigns the real account (single implicit account in
//! Sprint 1, see BACKLOG.md).

pub mod bob;
pub mod hdfc;
pub mod icici;

use crate::model::{Bank, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// No registered bank profile recognized the statement.
    #[error("this bank isn't supported yet")]
    UnsupportedBank,
    /// The statement matched a bank but a line could not be parsed.
    #[error("malformed statement at line {line}")]
    MalformedStatement { line: usize },
}

pub trait BankProfile {
    fn bank(&self) -> Bank;
    /// Cheap sniff on extracted first-page text. No false positives allowed.
    fn detect(first_page_text: &str) -> bool
    where
        Self: Sized;
    fn parse(&self, pages: &[String]) -> Result<Vec<Transaction>, ParseError>;
}

/// Detect the bank from the first page, parse the whole statement, and
/// decompose each narration into counterparty fields where a pattern matches.
pub fn parse_statement(pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
    let mut txns = detect_and_parse(pages)?;
    for txn in &mut txns {
        if let (None, Some(bank)) = (&txn.counterparty, txn.bank) {
            txn.counterparty = crate::narration::decompose(bank, &txn.narration_raw);
        }
    }
    Ok(txns)
}

fn detect_and_parse(pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
    // Detection order: BOB → HDFC → ICICI, first match wins. Each bank parser
    // task registers itself here with
    // `if <Profile>::detect(first) { return <Profile>.parse(pages); }`.
    let first = pages.first().map(String::as_str).unwrap_or("");
    if bob::BobProfile::detect(first) {
        return bob::BobProfile.parse(pages);
    }
    if hdfc::HdfcProfile::detect(first) {
        return hdfc::HdfcProfile.parse(pages);
    }
    if icici::IciciProfile::detect(first) {
        return icici::IciciProfile.parse(pages);
    }
    Err(ParseError::UnsupportedBank)
}
