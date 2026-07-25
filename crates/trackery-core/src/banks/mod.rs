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

/// Detect the bank from the first page and parse the whole statement.
pub fn parse_statement(pages: &[String]) -> Result<Vec<Transaction>, ParseError> {
    // Detection order: BOB → HDFC → ICICI, first match wins. Each bank parser
    // task registers itself here with
    // `if <Profile>::detect(first) { return <Profile>.parse(pages); }`.
    let first = pages.first().map(String::as_str).unwrap_or("");
    let _ = first;
    Err(ParseError::UnsupportedBank)
}
