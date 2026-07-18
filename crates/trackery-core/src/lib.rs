//! trackery-core: pure-Rust core for trackery_fi.
//!
//! PDF text extraction, bank statement parsing (BOB/HDFC/ICICI),
//! narration decomposition, and encrypted storage. No I/O with the
//! outside world — everything runs on-device.

pub mod money;
pub mod pdf;
