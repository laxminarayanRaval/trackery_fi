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

use crate::model::{AccountType, Bank, Transaction};

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

/// Account facts sniffed from a statement's header. All best-effort: a bank
/// that prints nothing usable yields `None`s, never an error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatementMeta {
    pub holder_name: Option<String>,
    pub number_last4: Option<String>,
    pub account_type: Option<AccountType>,
}

#[derive(Debug)]
pub struct ParsedStatement {
    pub meta: StatementMeta,
    pub transactions: Vec<Transaction>,
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

/// [`parse_statement`] plus header metadata (holder, masked number, type) —
/// what an importing app should call so statements land in the right account.
pub fn parse_statement_full(pages: &[String]) -> Result<ParsedStatement, ParseError> {
    let transactions = parse_statement(pages)?;
    let first = pages.first().map(String::as_str).unwrap_or("");
    let meta = match transactions.first().and_then(|t| t.bank) {
        Some(Bank::Icici) => icici_meta(first),
        Some(Bank::Bob) => bob_meta(first),
        Some(Bank::Hdfc) => hdfc_meta(first),
        _ => StatementMeta::default(),
    };
    Ok(ParsedStatement { meta, transactions })
}

/// Last 4 digits of a masked account token like `XXXX1234` / `00000XXXX1299`.
fn last4(token: &str) -> Option<String> {
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    (digits.len() >= 4).then(|| digits[digits.len() - 4..].to_string())
}

fn type_in(text: &str) -> Option<AccountType> {
    let upper = text.to_uppercase();
    if upper.contains("SAVING") || upper.contains("SBA") {
        Some(AccountType::Savings)
    } else if upper.contains("CURRENT") {
        Some(AccountType::Current)
    } else {
        None
    }
}

/// A header line that names the holder: `MR SYNTH ACCOUNT HOLDER` etc.
fn titled_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = ["MR. ", "MR ", "MRS. ", "MRS ", "MS. ", "MS ", "DR. ", "DR "]
        .iter()
        .find_map(|t| trimmed.strip_prefix(t))?;
    (!rest.is_empty() && rest.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '.'))
        .then(|| rest.trim().to_string())
}

fn icici_meta(first_page: &str) -> StatementMeta {
    let mut meta = StatementMeta::default();
    for line in first_page.lines() {
        if let Some(idx) = line.find("Account Number:") {
            meta.account_type = type_in(line);
            meta.number_last4 = line[idx + "Account Number:".len()..]
                .split_whitespace()
                .next()
                .and_then(last4);
        } else if meta.holder_name.is_none() {
            meta.holder_name = titled_name(line);
        }
    }
    meta
}

fn bob_meta(first_page: &str) -> StatementMeta {
    // BOB prints a label block (Account Name / Number / Type / ...) and then
    // the values as their own block; the holder is the first value line.
    let lines: Vec<&str> = first_page.lines().collect();
    let mut meta = StatementMeta::default();
    let after_labels = lines
        .iter()
        .position(|l| l.trim() == "Branch Address")
        .map(|i| i + 1)
        .unwrap_or(0);
    for line in &lines[after_labels..] {
        let t = line.trim();
        if t.is_empty() || t == "Account Details" {
            continue;
        }
        if meta.holder_name.is_none()
            && t.len() > 3
            && t.chars().all(|c| c.is_ascii_uppercase() || c == ' ' || c == '.')
        {
            meta.holder_name = Some(t.to_string());
            continue;
        }
        if meta.number_last4.is_none()
            && t.chars().all(|c| c == 'X' || c.is_ascii_digit())
            && t.contains('X')
        {
            meta.number_last4 = last4(t);
            continue;
        }
        if meta.account_type.is_none() {
            // Type prints as a product code line, e.g. `SBA XXXXXXXXX`.
            meta.account_type = t.split_whitespace().next().and_then(type_in);
        }
    }
    meta
}

fn hdfc_meta(first_page: &str) -> StatementMeta {
    let mut meta = StatementMeta::default();
    for line in first_page.lines() {
        if let Some(idx) = line.find("Account No") {
            let tail = line[idx..].split(':').nth(1).unwrap_or("");
            meta.number_last4 = tail.split_whitespace().next().and_then(last4);
            meta.account_type = type_in(tail);
        } else if meta.holder_name.is_none() {
            meta.holder_name = titled_name(line);
        }
    }
    meta
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
