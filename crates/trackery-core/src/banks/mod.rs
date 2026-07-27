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

/// Longest first-page text we'll surface in a diagnostic dump. The full page
/// (real narrations, account numbers) never leaves the device — this exists
/// purely so a stuck import can be inspected on-screen or in an example run,
/// not logged or transmitted anywhere.
const PREVIEW_CHARS: usize = 1000;

/// Per-bank detection result, for surfacing *why* a statement wasn't matched.
#[derive(Debug, Clone)]
pub struct BankDetection {
    pub bank: &'static str,
    pub matched: bool,
}

/// Diagnostic snapshot of a statement that failed (or is about to be tried)
/// against the bank registry. Never persisted — for on-screen/console
/// inspection when `UnsupportedBank` or a parse failure needs explaining.
#[derive(Debug, Clone)]
pub struct StatementDiagnostics {
    pub page_count: usize,
    pub first_page_chars: usize,
    pub detections: Vec<BankDetection>,
    /// First [`PREVIEW_CHARS`] characters of the first page's extracted text.
    pub first_page_preview: String,
}

/// Sniff `pages` against every registered bank profile without parsing,
/// reporting which (if any) matched. Used by the diagnostics command/example
/// so a failed import can be explained instead of just reported as
/// "unsupported".
pub fn diagnose(pages: &[String]) -> StatementDiagnostics {
    let first = pages.first().map(String::as_str).unwrap_or("");
    StatementDiagnostics {
        page_count: pages.len(),
        first_page_chars: first.chars().count(),
        detections: vec![
            BankDetection {
                bank: "BOB",
                matched: bob::BobProfile::detect(first),
            },
            BankDetection {
                bank: "HDFC",
                matched: hdfc::HdfcProfile::detect(first),
            },
            BankDetection {
                bank: "ICICI",
                matched: icici::IciciProfile::detect(first),
            },
        ],
        first_page_preview: first.chars().take(PREVIEW_CHARS).collect(),
    }
}
