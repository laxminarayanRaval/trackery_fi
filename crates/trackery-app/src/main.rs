//! Trackery desktop app: thin Tauri shell over trackery-core.
//!
//! Commands are stateless — the UI holds the db passphrase in memory after
//! unlock and passes it per call. Errors cross IPC as stable string codes
//! (`wrong_pdf_password`, `wrong_db_key`, `unsupported_bank`, ...) so the UI
//! can branch without parsing prose.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use tauri::Manager;
use trackery_core::banks::{self, ParseError};
use trackery_core::db::{Db, DbError};
use trackery_core::model::Transaction;
use trackery_core::pdf::{self, PdfError};

#[derive(serde::Serialize)]
struct TxnDto {
    id: String,
    date: String,
    narration: String,
    direction: String,
    amount_paise: i64,
    balance_paise: Option<i64>,
    cp_name: Option<String>,
    cp_vpa: Option<String>,
    cp_reference: Option<String>,
    mode: Option<String>,
    bank: String,
}

impl From<&Transaction> for TxnDto {
    fn from(t: &Transaction) -> Self {
        let cp = t.counterparty.as_ref();
        TxnDto {
            id: t.id.to_string(),
            date: t.date.to_string(),
            narration: t.narration_raw.clone(),
            direction: t.direction.as_str().to_string(),
            amount_paise: t.amount_paise,
            balance_paise: t.balance_paise,
            cp_name: cp.and_then(|c| c.name.clone()),
            cp_vpa: cp.and_then(|c| c.vpa.clone()),
            cp_reference: cp.and_then(|c| c.reference.clone()),
            mode: cp.map(|c| c.mode.as_str().to_string()),
            bank: t.bank.map(|b| b.as_str().to_string()).unwrap_or_default(),
        }
    }
}

#[derive(serde::Serialize)]
struct ImportSummary {
    bank: String,
    imported: usize,
}

fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create app dir: {e}"))?;
    Ok(dir.join("trackery.db"))
}

fn open_db(app: &tauri::AppHandle, key: &str) -> Result<Db, String> {
    Db::open(&db_path(app)?, key).map_err(|e| match e {
        DbError::WrongKey => "wrong_db_key".to_string(),
        DbError::Sqlite(e) => format!("db: {e}"),
    })
}

#[tauri::command]
async fn import_statement(
    app: tauri::AppHandle,
    bytes: Vec<u8>,
    pdf_password: Option<String>,
    db_key: String,
) -> Result<ImportSummary, String> {
    let pages = pdf::open_statement(&bytes, pdf_password.as_deref()).map_err(|e| match e {
        PdfError::WrongPassword => "wrong_pdf_password".to_string(),
        PdfError::CorruptPdf => "corrupt_pdf".to_string(),
        PdfError::PdfiumUnavailable(e) => format!("pdfium_unavailable: {e}"),
    })?;
    let txns = banks::parse_statement(&pages).map_err(|e| match e {
        ParseError::UnsupportedBank => "unsupported_bank".to_string(),
        ParseError::MalformedStatement { line } => format!("malformed_statement:{line}"),
    })?;
    let db = open_db(&app, &db_key)?;
    for txn in &txns {
        db.insert(txn).map_err(|e| format!("db: {e}"))?;
    }
    Ok(ImportSummary {
        bank: txns
            .first()
            .and_then(|t| t.bank)
            .map(|b| b.as_str().to_string())
            .unwrap_or_default(),
        imported: txns.len(),
    })
}

#[tauri::command]
async fn list_transactions(
    app: tauri::AppHandle,
    db_key: String,
) -> Result<Vec<TxnDto>, String> {
    let db = open_db(&app, &db_key)?;
    let rows = db.list().map_err(|e| format!("db: {e}"))?;
    Ok(rows.iter().map(TxnDto::from).collect())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![import_statement, list_transactions])
        .run(tauri::generate_context!())
        .expect("error while running trackery");
}
