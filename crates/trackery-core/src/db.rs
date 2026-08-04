//! SQLCipher-encrypted storage via rusqlite (`bundled-sqlcipher-vendored-openssl`).
//!
//! Single `transactions` table plus a `schema_migrations` table driven by a
//! minimal embedded runner — migrations are entries in [`MIGRATIONS`]; the
//! runner applies whatever is newer than the recorded max version.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, ErrorCode, Row};
use uuid::Uuid;

use crate::model::{Bank, Counterparty, Direction, Transaction, TxnMode};

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
        date          TEXT NOT NULL,
        narration_raw TEXT NOT NULL,
        direction     TEXT NOT NULL,
        amount_paise  INTEGER NOT NULL,
        balance_paise INTEGER,
        cp_name       TEXT,
        cp_vpa        TEXT,
        cp_reference  TEXT,
        cp_mode       TEXT,
        bank          TEXT NOT NULL,
        updated_at    TEXT NOT NULL,
        deleted       INTEGER NOT NULL DEFAULT 0
    );"];

pub struct Db {
    conn: Connection,
}

const INSERT_SQL: &str = "INSERT INTO transactions (id, date, narration_raw, direction, \
     amount_paise, balance_paise, cp_name, cp_vpa, cp_reference, cp_mode, bank, updated_at, deleted) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

const UPSERT_SUFFIX: &str = " ON CONFLICT(id) DO UPDATE SET \
     date = excluded.date, narration_raw = excluded.narration_raw, direction = excluded.direction, \
     amount_paise = excluded.amount_paise, balance_paise = excluded.balance_paise, \
     cp_name = excluded.cp_name, cp_vpa = excluded.cp_vpa, cp_reference = excluded.cp_reference, \
     cp_mode = excluded.cp_mode, bank = excluded.bank, updated_at = excluded.updated_at, \
     deleted = excluded.deleted";

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

    fn init(conn: Connection, key: &str) -> Result<Self, DbError> {
        conn.pragma_update(None, "key", key)?;
        // SQLCipher only fails on the first real read: a wrong key (or a
        // non-SQLCipher file) surfaces as NotADatabase on this probe.
        match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
                return Err(DbError::WrongKey);
            }
            Err(e) => return Err(e.into()),
        }
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn migrate(conn: &Connection) -> Result<(), DbError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;
        let current: i64 =
            conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| {
                r.get(0)
            })?;
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let version = (i + 1) as i64;
            if version <= current {
                continue;
            }
            conn.execute_batch(&format!(
                "BEGIN; {sql} \
                 INSERT INTO schema_migrations (version, applied_at) VALUES ({version}, datetime('now')); \
                 COMMIT;"
            ))?;
        }
        Ok(())
    }

    fn exec(&self, sql: &str, txn: &Transaction) -> Result<(), DbError> {
        let cp = txn.counterparty.as_ref();
        self.conn.execute(
            sql,
            params![
                txn.id.to_string(),
                txn.date.to_string(), // ISO %Y-%m-%d
                txn.narration_raw,
                txn.direction.as_str(),
                txn.amount_paise,
                txn.balance_paise,
                cp.and_then(|c| c.name.as_deref()),
                cp.and_then(|c| c.vpa.as_deref()),
                cp.and_then(|c| c.reference.as_deref()),
                cp.map(|c| c.mode.as_str()),
                txn.bank.as_str(),
                txn.updated_at.to_rfc3339(), // full nanosecond precision round-trip
                txn.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn insert(&self, txn: &Transaction) -> Result<(), DbError> {
        self.exec(INSERT_SQL, txn)
    }

    /// Insert or fully replace the row with the same id.
    pub fn upsert(&self, txn: &Transaction) -> Result<(), DbError> {
        self.exec(&format!("{INSERT_SQL}{UPSERT_SUFFIX}"), txn)
    }

    /// All non-deleted transactions, newest date first.
    pub fn list(&self) -> Result<Vec<Transaction>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, narration_raw, direction, amount_paise, balance_paise, \
             cp_name, cp_vpa, cp_reference, cp_mode, bank, updated_at, deleted \
             FROM transactions WHERE deleted = 0 ORDER BY date DESC, id",
        )?;
        let rows = stmt.query_map([], row_to_txn)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// Column value that failed to decode back into a domain type.
fn bad_column(idx: usize, err: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(err))
}

fn bad_enum(idx: usize, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        format!("unknown enum value: {value}").into(),
    )
}

fn row_to_txn(row: &Row) -> rusqlite::Result<Transaction> {
    let id: String = row.get(0)?;
    let date: String = row.get(1)?;
    let direction: String = row.get(3)?;
    let cp_mode: Option<String> = row.get(9)?;
    let bank: String = row.get(10)?;
    let updated_at: String = row.get(11)?;
    Ok(Transaction {
        id: Uuid::parse_str(&id).map_err(|e| bad_column(0, e))?,
        date: date.parse().map_err(|e| bad_column(1, e))?,
        narration_raw: row.get(2)?,
        direction: Direction::from_str_opt(&direction).ok_or_else(|| bad_enum(3, &direction))?,
        amount_paise: row.get(4)?,
        balance_paise: row.get(5)?,
        counterparty: match cp_mode {
            // cp_mode is the only non-optional Counterparty field, so it decides presence.
            Some(mode) => Some(Counterparty {
                name: row.get(6)?,
                vpa: row.get(7)?,
                reference: row.get(8)?,
                mode: TxnMode::from_str_opt(&mode).ok_or_else(|| bad_enum(9, &mode))?,
            }),
            None => None,
        },
        bank: Bank::from_str_opt(&bank).ok_or_else(|| bad_enum(10, &bank))?,
        updated_at: DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|e| bad_column(11, e))?
            .with_timezone(&Utc),
        deleted: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txn(date: &str, narration: &str, amount: i64) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            date: date.parse().expect("test date"),
            narration_raw: narration.to_string(),
            direction: if amount < 0 {
                Direction::Debit
            } else {
                Direction::Credit
            },
            amount_paise: amount.abs(),
            balance_paise: Some(1_000_00),
            counterparty: Some(Counterparty {
                name: Some("SYNTH MERCHANT".into()),
                vpa: Some("synth@ybl".into()),
                reference: Some("400000000001".into()),
                mode: TxnMode::Upi,
            }),
            bank: Bank::Hdfc,
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
        assert!(matches!(Db::open(&path, "wrong-key"), Err(DbError::WrongKey)));
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
