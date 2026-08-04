//! SQLCipher-encrypted storage via rusqlite (`bundled-sqlcipher-vendored-openssl`).
//!
//! Single `transactions` table plus a `schema_migrations` table driven by a
//! minimal embedded runner — migrations are entries in [`MIGRATIONS`]; the
//! runner applies whatever is newer than the recorded max version.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, ErrorCode};
use uuid::Uuid;

use crate::model::{Bank, Counterparty, Direction, Transaction, TransactionOrigin, TxnMode};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The encryption key is wrong (or the file is not an SQLCipher db).
    #[error("wrong encryption key")]
    WrongKey,
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// One SQL batch per schema version, index 0 = version 1. Append-only.
const MIGRATIONS: &[&str] = &["CREATE TABLE transactions (
        id            TEXT PRIMARY KEY,
        account_id    TEXT NOT NULL,
        date          TEXT NOT NULL,
        narration_raw TEXT NOT NULL,
        description   TEXT,
        direction     TEXT NOT NULL,
        amount_paise  INTEGER NOT NULL,
        balance_paise INTEGER,
        origin        TEXT NOT NULL,
        cp_name       TEXT,
        cp_vpa        TEXT,
        cp_reference  TEXT,
        cp_merchant   TEXT,
        cp_mode       TEXT,
        bank          TEXT,
        category_id   TEXT,
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL,
        deleted       INTEGER NOT NULL DEFAULT 0
    );",
    // v2: statement dedup + import history. The natural-row index makes
    // re-importing an overlapping statement a no-op per row; pre-existing
    // duplicates (from v1 re-imports) are collapsed to their earliest copy
    // first so the index can build. NULL balances stay distinct — statement
    // imports always carry a balance.
    "DELETE FROM transactions WHERE rowid NOT IN (
        SELECT MIN(rowid) FROM transactions
        GROUP BY account_id, bank, date, direction, amount_paise, narration_raw, balance_paise
    );
    CREATE UNIQUE INDEX idx_txn_natural_row ON transactions
        (account_id, bank, date, direction, amount_paise, narration_raw, balance_paise);
    CREATE TABLE imports (
        id          TEXT PRIMARY KEY,
        imported_at TEXT NOT NULL,
        device      TEXT NOT NULL,
        bank        TEXT,
        total_rows  INTEGER NOT NULL,
        new_rows    INTEGER NOT NULL,
        dup_rows    INTEGER NOT NULL
    );"];

/// One statement-import event, for the audit trail shown in the import dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecord {
    pub id: Uuid,
    pub imported_at: DateTime<Utc>,
    pub device: String,
    pub bank: Option<Bank>,
    pub total_rows: i64,
    pub new_rows: i64,
    pub dup_rows: i64,
}

const COLUMNS: &str = "id, account_id, date, narration_raw, description, direction, \
     amount_paise, balance_paise, origin, cp_name, cp_vpa, cp_reference, cp_merchant, \
     cp_mode, bank, category_id, created_at, updated_at, deleted";

pub struct Db {
    conn: Connection,
}

/// Decode a stored enum string, surfacing corruption as a sqlite conversion error.
fn decode<T>(idx: usize, s: &str, f: impl Fn(&str) -> Option<T>) -> rusqlite::Result<T> {
    f(s).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            format!("invalid enum value {s:?}").into(),
        )
    })
}

fn decode_utc(idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, e.into())
        })
}

fn decode_uuid(idx: usize, s: &str) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, e.into())
    })
}

impl Db {
    /// Open (creating if needed) an encrypted db at `path` with `key`.
    /// A wrong key on an existing db yields [`DbError::WrongKey`].
    pub fn open(path: &Path, key: &str) -> Result<Self, DbError> {
        Self::init(Connection::open(path)?, key)
    }

    /// In-memory encrypted db, for tests and previews.
    pub fn open_in_memory(key: &str) -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?, key)
    }

    /// Re-encrypt the database in place under a new key. Used once when
    /// migrating a legacy vault (random key in a file beside the db) to a
    /// user-chosen passphrase.
    pub fn rekey(path: &Path, old_key: &str, new_key: &str) -> Result<(), DbError> {
        let db = Self::open(path, old_key)?;
        db.conn.pragma_update(None, "rekey", new_key)?;
        Ok(())
    }

    fn init(conn: Connection, key: &str) -> Result<Self, DbError> {
        conn.pragma_update(None, "key", key)?;
        // First read decrypts page 1; a wrong key surfaces as "not a database".
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == ErrorCode::NotADatabase => {
                    DbError::WrongKey
                }
                other => DbError::Sqlite(other),
            })?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;
        let current: i64 = self.conn.query_row(
            "SELECT IFNULL(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )?;
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let version = (i + 1) as i64;
            if version > current {
                self.conn.execute_batch(sql)?;
                self.conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, Utc::now().to_rfc3339()],
                )?;
            }
        }
        Ok(())
    }

    /// Insert a transaction. Returns `false` (and stores nothing) when an
    /// identical statement row already exists — the dedup that makes
    /// re-importing an overlapping statement safe.
    pub fn insert(&self, txn: &Transaction) -> Result<bool, DbError> {
        self.write(txn, "INSERT OR IGNORE")
    }

    /// Insert or fully replace the row with the same id.
    pub fn upsert(&self, txn: &Transaction) -> Result<(), DbError> {
        self.write(txn, "INSERT OR REPLACE").map(|_| ())
    }

    fn write(&self, txn: &Transaction, verb: &str) -> Result<bool, DbError> {
        let cp = txn.counterparty.as_ref();
        self.conn.execute(
            &format!(
                "{verb} INTO transactions ({COLUMNS}) VALUES \
                 (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"
            ),
            params![
                txn.id.to_string(),
                txn.account_id.to_string(),
                txn.date.to_string(),
                txn.narration_raw,
                txn.description,
                txn.direction.as_str(),
                txn.amount_paise,
                txn.balance_paise,
                txn.origin.as_str(),
                cp.and_then(|c| c.name.as_deref()),
                cp.and_then(|c| c.vpa.as_deref()),
                cp.and_then(|c| c.reference.as_deref()),
                cp.and_then(|c| c.merchant.as_deref()),
                cp.map(|c| c.mode.as_str()),
                txn.bank.map(Bank::as_str),
                txn.category_id.map(|u| u.to_string()),
                txn.created_at.to_rfc3339(),
                txn.updated_at.to_rfc3339(),
                txn.deleted,
            ],
        )
        .map(|changed| changed > 0)
        .map_err(DbError::from)
    }

    pub fn record_import(&self, rec: &ImportRecord) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO imports (id, imported_at, device, bank, total_rows, new_rows, dup_rows) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.id.to_string(),
                rec.imported_at.to_rfc3339(),
                rec.device,
                rec.bank.map(Bank::as_str),
                rec.total_rows,
                rec.new_rows,
                rec.dup_rows,
            ],
        )?;
        Ok(())
    }

    /// Import history, newest first.
    pub fn list_imports(&self) -> Result<Vec<ImportRecord>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, imported_at, device, bank, total_rows, new_rows, dup_rows \
             FROM imports ORDER BY imported_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let bank: Option<String> = row.get(3)?;
            Ok(ImportRecord {
                id: decode_uuid(0, &row.get::<_, String>(0)?)?,
                imported_at: decode_utc(1, &row.get::<_, String>(1)?)?,
                device: row.get(2)?,
                bank: bank.map(|b| decode(3, &b, Bank::from_str_opt)).transpose()?,
                total_rows: row.get(4)?,
                new_rows: row.get(5)?,
                dup_rows: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// All non-deleted transactions, newest date first.
    pub fn list(&self) -> Result<Vec<Transaction>, DbError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM transactions WHERE deleted = 0 \
             ORDER BY date DESC, created_at DESC"
        ))?;
        let rows = stmt.query_map([], |row| {
            let mode: Option<String> = row.get(13)?;
            let counterparty = match mode {
                Some(m) => Some(Counterparty {
                    name: row.get(9)?,
                    vpa: row.get(10)?,
                    reference: row.get(11)?,
                    merchant: row.get(12)?,
                    mode: decode(13, &m, TxnMode::from_str_opt)?,
                }),
                None => None,
            };
            let bank: Option<String> = row.get(14)?;
            let category_id: Option<String> = row.get(15)?;
            Ok(Transaction {
                id: decode_uuid(0, &row.get::<_, String>(0)?)?,
                account_id: decode_uuid(1, &row.get::<_, String>(1)?)?,
                date: decode(2, &row.get::<_, String>(2)?, |s| s.parse().ok())?,
                narration_raw: row.get(3)?,
                description: row.get(4)?,
                direction: decode(5, &row.get::<_, String>(5)?, Direction::from_str_opt)?,
                amount_paise: row.get(6)?,
                balance_paise: row.get(7)?,
                origin: decode(
                    8,
                    &row.get::<_, String>(8)?,
                    TransactionOrigin::from_str_opt,
                )?,
                counterparty,
                bank: bank
                    .map(|b| decode(14, &b, Bank::from_str_opt))
                    .transpose()?,
                category_id: category_id.map(|c| decode_uuid(15, &c)).transpose()?,
                created_at: decode_utc(16, &row.get::<_, String>(16)?)?,
                updated_at: decode_utc(17, &row.get::<_, String>(17)?)?,
                deleted: row.get(18)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
// Underscores deliberately mirror Indian digit grouping (rupees_paise).
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;

    fn txn(date: &str, narration: &str, amount: i64) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            date: date.parse().expect("test date"),
            narration_raw: narration.to_string(),
            description: Some("synthetic test row".into()),
            direction: if amount < 0 {
                Direction::Debit
            } else {
                Direction::Credit
            },
            amount_paise: amount.abs(),
            balance_paise: Some(1_000_00),
            origin: TransactionOrigin::StatementImport,
            counterparty: Some(Counterparty {
                name: Some("SYNTH MERCHANT".into()),
                vpa: Some("synth@ybl".into()),
                reference: Some("400000000001".into()),
                merchant: Some("SYNTH STORES PVT LTD".into()),
                mode: TxnMode::Upi,
            }),
            bank: Some(Bank::Hdfc),
            category_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted: false,
        }
    }

    #[test]
    fn crud_roundtrip_in_memory() {
        let db = Db::open_in_memory("hunter2").expect("open in-memory");
        let a = txn("2026-01-05", "UPI-SYNTH MERCHANT-synth@ybl", -25_000);
        let b = txn("2026-01-07", "NEFT-SALARY XXXX1234", 50_000_00);
        db.insert(&a).expect("insert a");
        db.insert(&b).expect("insert b");

        let rows = db.list().expect("list");
        assert_eq!(rows.len(), 2);
        // Newest date first, full field round-trip.
        assert_eq!(rows[0], b);
        assert_eq!(rows[1], a);
    }

    #[test]
    fn nullable_fields_roundtrip() {
        let db = Db::open_in_memory("hunter2").expect("open in-memory");
        let mut a = txn("2026-01-09", "MANUAL: CASH CHAI", -2_000);
        a.description = None;
        a.counterparty = None;
        a.bank = None;
        a.balance_paise = None;
        a.origin = TransactionOrigin::ManualEntry;
        db.insert(&a).expect("insert");
        assert_eq!(db.list().expect("list"), vec![a]);
    }

    #[test]
    fn upsert_replaces_by_id_and_soft_delete_hides() {
        let db = Db::open_in_memory("hunter2").expect("open in-memory");
        let mut a = txn("2026-02-01", "ATM WDL XXXX1234", -10_000_00);
        db.insert(&a).expect("insert");

        a.narration_raw = "ATM WDL CORRECTED".into();
        db.upsert(&a).expect("upsert existing");
        let rows = db.list().expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].narration_raw, "ATM WDL CORRECTED");

        a.deleted = true;
        db.upsert(&a).expect("upsert deleted");
        assert!(db.list().expect("list").is_empty());

        // Upsert of an unknown id inserts.
        let b = txn("2026-02-02", "IMPS-REF-SYNTH", 7_500);
        db.upsert(&b).expect("upsert new");
        assert_eq!(db.list().expect("list").len(), 1);
    }

    #[test]
    fn reimporting_identical_rows_is_ignored() {
        let db = Db::open_in_memory("k").expect("open");
        let a = txn("2026-05-01", "UPI/DUPCHECK/synth@ybl", -50_00);
        assert!(db.insert(&a).expect("first insert"));
        // Same statement row re-imported later: new uuid, same content.
        let mut b = a.clone();
        b.id = Uuid::new_v4();
        assert!(!db.insert(&b).expect("dup insert is a no-op"));
        assert_eq!(db.list().expect("list").len(), 1);
    }

    #[test]
    fn import_history_roundtrip() {
        let db = Db::open_in_memory("k").expect("open");
        let rec = ImportRecord {
            id: Uuid::new_v4(),
            imported_at: Utc::now(),
            device: "SYNTH-DEVICE".into(),
            bank: Some(Bank::Icici),
            total_rows: 45,
            new_rows: 40,
            dup_rows: 5,
        };
        db.record_import(&rec).expect("record");
        assert_eq!(db.list_imports().expect("list"), vec![rec]);
    }

    #[test]
    fn rekey_moves_vault_to_new_key() {
        let path = std::env::temp_dir().join(format!(
            "trackery_rekey_{}_{}.db",
            std::process::id(),
            Uuid::new_v4()
        ));
        {
            let db = Db::open(&path, "legacy-random-key").expect("create");
            db.insert(&txn("2026-04-01", "UPI-REKEY-CHECK", -1_00)).expect("insert");
        }
        Db::rekey(&path, "legacy-random-key", "user passphrase").expect("rekey");
        assert!(matches!(
            Db::open(&path, "legacy-random-key"),
            Err(DbError::WrongKey)
        ));
        let db = Db::open(&path, "user passphrase").expect("open with new key");
        assert_eq!(db.list().expect("list").len(), 1);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wrong_key_is_distinguished() {
        let path = std::env::temp_dir().join(format!(
            "trackery_test_{}_{}.db",
            std::process::id(),
            Uuid::new_v4()
        ));
        {
            let db = Db::open(&path, "correct-key").expect("create encrypted db");
            db.insert(&txn("2026-03-01", "CHG: SMS ALERT FEE", -25_00))
                .expect("insert");
        }
        assert!(matches!(
            Db::open(&path, "wrong-key"),
            Err(DbError::WrongKey)
        ));
        let db = Db::open(&path, "correct-key").expect("reopen with right key");
        assert_eq!(db.list().expect("list").len(), 1);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migrations_recorded_and_idempotent() {
        let db = Db::open_in_memory("k").expect("open");
        let version: i64 = db
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .expect("migration version");
        assert_eq!(version as usize, MIGRATIONS.len());
    }
}
