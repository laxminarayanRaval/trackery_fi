//! Domain model — the exact Sprint 1 contract, nothing more.

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    Bob,
    Hdfc,
    Icici,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnMode {
    Upi,
    Imps,
    Neft,
    Rtgs,
    Atm,
    Card,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterparty {
    pub name: Option<String>,
    /// UPI virtual payment address, e.g. `someone@ybl`.
    pub vpa: Option<String>,
    /// RRN / UTR reference.
    pub reference: Option<String>,
    pub mode: TxnMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: Uuid, // sync-ready from day one
    pub date: NaiveDate,
    pub narration_raw: String,
    pub direction: Direction,
    pub amount_paise: i64,
    pub balance_paise: Option<i64>,
    pub counterparty: Option<Counterparty>,
    pub bank: Bank,
    pub updated_at: DateTime<Utc>, // sync-ready
    pub deleted: bool,             // soft delete, sync-ready
}

macro_rules! str_enum {
    ($ty:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        impl $ty {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }

            pub fn from_str_opt(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

str_enum!(Direction { Debit => "debit", Credit => "credit" });
str_enum!(Bank { Bob => "bob", Hdfc => "hdfc", Icici => "icici" });
str_enum!(TxnMode {
    Upi => "upi",
    Imps => "imps",
    Neft => "neft",
    Rtgs => "rtgs",
    Atm => "atm",
    Card => "card",
    Other => "other",
});
