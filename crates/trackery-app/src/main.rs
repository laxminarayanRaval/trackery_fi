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
use trackery_core::db::{Db, DbError, ImportRecord};
use trackery_core::model::Transaction;
use trackery_core::pdf::{self, PdfError};

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-device".into())
}

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
    total: usize,
    imported: usize,
    duplicates: usize,
}

#[derive(serde::Serialize)]
struct ImportDto {
    imported_at: String,
    device: String,
    bank: String,
    total_rows: i64,
    new_rows: i64,
    dup_rows: i64,
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
    let path = db_path(app)?;
    migrate_legacy_vault(&path, key)?;
    Db::open(&path, key).map_err(|e| match e {
        DbError::WrongKey => "wrong_db_key".to_string(),
        DbError::Sqlite(e) => format!("db: {e}"),
    })
}

/// One-time adoption of a v0 vault: the old app kept a random key in a
/// `db.key` file beside the database. On the first unlock after upgrade,
/// re-encrypt that vault under the user's passphrase and drop the key file.
fn migrate_legacy_vault(path: &std::path::Path, new_key: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("db path has no parent")?;
    let legacy_key_file = dir.join("db.key");
    let backup = dir.join("trackery.db.old-vault-20260805");
    if path.exists() || !legacy_key_file.exists() {
        return Ok(());
    }
    let source = if backup.exists() { backup } else { return Ok(()) };
    let legacy_key = std::fs::read_to_string(&legacy_key_file)
        .map_err(|e| format!("read legacy key: {e}"))?;
    std::fs::copy(&source, path).map_err(|e| format!("stage legacy vault: {e}"))?;
    if let Err(e) = Db::rekey(path, legacy_key.trim(), new_key) {
        let _ = std::fs::remove_file(path); // leave a clean slate for retry
        return Err(format!("legacy vault migration failed: {e}"));
    }
    let _ = std::fs::remove_file(&legacy_key_file); // plaintext key must not outlive migration
    Ok(())
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
    let mut imported = 0usize;
    for txn in &txns {
        if db.insert(txn).map_err(|e| format!("db: {e}"))? {
            imported += 1;
        }
    }
    let bank = txns.first().and_then(|t| t.bank);
    db.record_import(&ImportRecord {
        id: uuid::Uuid::new_v4(),
        imported_at: chrono::Utc::now(),
        device: device_name(),
        bank,
        total_rows: txns.len() as i64,
        new_rows: imported as i64,
        dup_rows: (txns.len() - imported) as i64,
    })
    .map_err(|e| format!("db: {e}"))?;
    Ok(ImportSummary {
        bank: bank.map(|b| b.as_str().to_string()).unwrap_or_default(),
        total: txns.len(),
        imported,
        duplicates: txns.len() - imported,
    })
}

#[tauri::command]
async fn list_imports(app: tauri::AppHandle, db_key: String) -> Result<Vec<ImportDto>, String> {
    let db = open_db(&app, &db_key)?;
    Ok(db
        .list_imports()
        .map_err(|e| format!("db: {e}"))?
        .into_iter()
        .map(|r| ImportDto {
            imported_at: r.imported_at.to_rfc3339(),
            device: r.device,
            bank: r.bank.map(|b| b.as_str().to_string()).unwrap_or_default(),
            total_rows: r.total_rows,
            new_rows: r.new_rows,
            dup_rows: r.dup_rows,
        })
        .collect())
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
        .invoke_handler(tauri::generate_handler![
            import_statement,
            list_transactions,
            list_imports
        ])
        .run(tauri::generate_context!())
        .expect("error while running trackery");
}
