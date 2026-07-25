//! Tauri commands for Sprint 1: import a statement PDF, list transactions.

use std::fs;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_fs::{FilePath, FsExt};
use trackery_core::banks::ParseError;
use trackery_core::db::{Db, DbError};
use trackery_core::model::{Direction, Transaction, TransactionOrigin};
use trackery_core::money::Money;
use trackery_core::pdf::PdfError;
use uuid::Uuid;

/// Command error with a stable `kind` the frontend matches on:
/// "wrong_password" | "unsupported_bank" | "corrupt_pdf" | "malformed_statement" | "other".
#[derive(Debug, Serialize)]
pub struct ImportError {
    kind: &'static str,
    message: String,
}

impl ImportError {
    fn other(message: impl ToString) -> Self {
        Self {
            kind: "other",
            message: message.to_string(),
        }
    }
}

impl From<PdfError> for ImportError {
    fn from(e: PdfError) -> Self {
        let kind = match e {
            PdfError::WrongPassword => "wrong_password",
            PdfError::CorruptPdf => "corrupt_pdf",
            PdfError::PdfiumUnavailable(_) => "other",
        };
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<ParseError> for ImportError {
    fn from(e: ParseError) -> Self {
        let kind = match e {
            ParseError::UnsupportedBank => "unsupported_bank",
            ParseError::MalformedStatement { .. } => "malformed_statement",
        };
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<DbError> for ImportError {
    fn from(e: DbError) -> Self {
        Self::other(e)
    }
}

/// Lazily-opened encrypted db, shared across commands.
struct AppDb(Mutex<Option<Db>>);

fn open_db(app: &AppHandle) -> Result<Db, ImportError> {
    let dir = app.path().app_data_dir().map_err(ImportError::other)?;
    fs::create_dir_all(&dir).map_err(ImportError::other)?;

    // ponytail: key stored beside db — replace with OS keystore when key management lands
    let key_path = dir.join("db.key");
    let key = if key_path.exists() {
        fs::read_to_string(&key_path)
            .map_err(ImportError::other)?
            .trim()
            .to_string()
    } else {
        // Random 32-byte hex key; two v4 UUIDs supply the randomness
        // (uuid is already a dependency, no extra rand crate needed).
        let key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        fs::write(&key_path, &key).map_err(ImportError::other)?;
        key
    };

    Db::open(&dir.join("trackery.db"), &key).map_err(Into::into)
}

/// Run `f` against the app db, opening it on first use.
fn with_db<T>(
    app: &AppHandle,
    f: impl FnOnce(&Db) -> Result<T, ImportError>,
) -> Result<T, ImportError> {
    let state: State<AppDb> = app.state();
    let mut guard = state
        .0
        .lock()
        .map_err(|_| ImportError::other("database lock poisoned"))?;
    if guard.is_none() {
        *guard = Some(open_db(app)?);
    }
    let db = guard
        .as_ref()
        .ok_or_else(|| ImportError::other("database unavailable"))?;
    f(db)
}

/// Flat row DTO for the transaction table. All display formatting happens
/// here so the frontend just renders strings.
#[derive(Serialize)]
struct TxnRow {
    id: String,
    date: String,
    description: String,
    counterparty: String,
    origin: String,
    amount: String,
    balance: String,
    mode: String,
    bank: String,
}

impl TxnRow {
    fn from_txn(t: &Transaction) -> Self {
        let cp = t.counterparty.as_ref();
        // Dr/Cr sign convention: debits shown negative.
        let signed_paise = match t.direction {
            Direction::Debit => -t.amount_paise,
            Direction::Credit => t.amount_paise,
        };
        Self {
            id: t.id.to_string(),
            date: t.date.to_string(),
            description: t
                .description
                .clone()
                .unwrap_or_else(|| t.narration_raw.clone()),
            counterparty: cp
                .and_then(|c| {
                    c.name
                        .clone()
                        .or_else(|| c.vpa.clone())
                        .or_else(|| c.merchant.clone())
                })
                .unwrap_or_default(),
            origin: match t.origin {
                TransactionOrigin::StatementImport => "Statement Import",
                TransactionOrigin::ManualEntry => "Manual Entry",
                TransactionOrigin::SmsImport => "SMS Import",
            }
            .to_string(),
            amount: Money(signed_paise).to_string(),
            balance: t
                .balance_paise
                .map(|b| Money(b).to_string())
                .unwrap_or_default(),
            mode: cp.map(|c| c.mode.as_str().to_string()).unwrap_or_default(),
            bank: t
                .bank
                .map(|b| b.as_str().to_uppercase())
                .unwrap_or_default(),
        }
    }
}

#[tauri::command]
fn import_statement(
    app: AppHandle,
    path: String,
    password: Option<String>,
) -> Result<usize, ImportError> {
    // The fs plugin resolves Android content:// URIs as well as plain paths.
    let file_path: FilePath = path
        .parse()
        .map_err(|e: std::convert::Infallible| ImportError::other(e))?;
    let bytes = app.fs().read(file_path).map_err(ImportError::other)?;

    let pages = trackery_core::pdf::open_statement(&bytes, password.as_deref())?;
    let txns = trackery_core::banks::parse_statement(&pages)?;
    // Parsers emit account_id = Uuid::nil(): the single implicit Sprint 1
    // account. Kept as-is.

    with_db(&app, |db| {
        for t in &txns {
            db.insert(t)?;
        }
        Ok(txns.len())
    })
}

#[tauri::command]
fn list_transactions(app: AppHandle) -> Result<Vec<TxnRow>, ImportError> {
    with_db(&app, |db| {
        Ok(db.list()?.iter().map(TxnRow::from_txn).collect())
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppDb(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![import_statement, list_transactions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
