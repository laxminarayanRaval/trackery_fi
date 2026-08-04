# SQLCipher storage layer — design

Date: 2026-08-01. Scope: finish the WIP `crates/trackery-core/src/db.rs` so its four
RED-phase tests pass unmodified. No new features.

## Scope

Implement the four `todo!()` stubs — `Db::open`, `Db::open_in_memory`, `insert`,
`upsert`, `list` — against the existing schema and `model.rs`. Dedup on re-import of
overlapping statements is deferred to `BACKLOG.md`.

## Open/keying

`open(path, key)`: `Connection::open(path)`, then `conn.pragma_update(None, "key", key)`
(rusqlite parameterizes the value — no injection), then probe the key by running
`SELECT count(*) FROM sqlite_master`. A wrong key (or non-SQLCipher file) surfaces as
SQLite's `NotADatabase` error, which maps to `DbError::WrongKey`; any other error stays
`DbError::Sqlite`. Then run migrations. `open_in_memory(key)` is the same minus the file.
SQLCipher's built-in KDF derives the page key from the passphrase; we do not roll our own.

Rejected alternatives: `PRAGMA cipher_integrity_check` (O(file) on every open) and
reading `user_version` (wrong key can return garbage instead of an error on some builds).

## Migrations

`CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY,
applied_at TEXT NOT NULL)`. Read `MAX(version)`, apply each newer entry of `MIGRATIONS`
plus its version row inside one SQL transaction per entry. Idempotent by construction.

## CRUD

- `insert`: plain `INSERT` of all 13 columns. Enums via `as_str()`; `NaiveDate` and
  `DateTime<Utc>` stored as ISO-8601 TEXT (chrono's `ToSql`).
- `upsert`: `INSERT ... ON CONFLICT(id) DO UPDATE SET` every non-id column.
- `list`: `SELECT ... WHERE deleted = 0 ORDER BY date DESC, id`. Row mapping folds the
  `cp_*` columns into `Option<Counterparty>`: `Some` iff `cp_mode` is non-NULL (mode is
  the only non-optional `Counterparty` field).

## Errors

Exactly the existing variants: `WrongKey`, `Sqlite(rusqlite::Error)`. Parse-layer errors
(`MalformedStatement`, `UnsupportedBank`) do not belong here.

## Testing

The four existing tests in `db.rs` are the contract and must pass unmodified. If
`updated_at` sub-second precision fails the TEXT round-trip, fix the storage format
(RFC3339 with fixed precision), not the test. Verification runs in the Docker Linux
container (`rust:1`, cached cargo/target volumes) since host SQLCipher builds are
fragile on Windows.
