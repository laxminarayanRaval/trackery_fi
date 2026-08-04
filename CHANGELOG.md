# Changelog

## v0.1.0 — Sprint 1 (2026-07-27)

Branch: `worktree-sprint1-continue`. Release: Windows exe attached; APK build open (see NEXT_SESSION.md).

### Core (`crates/trackery-core`)
- Domain model per the sprint contract: `Transaction` (account_id, description, origin,
  category_id, created_at, optional bank, soft delete), `Counterparty` with merchant,
  `TransactionOrigin` (StatementImport / ManualEntry / SmsImport).
- SQLCipher-encrypted storage (rusqlite, vendored OpenSSL): embedded migration runner,
  wrong-key detection, insert/upsert, newest-first listing excluding soft-deleted.
- `BankProfile` trait + first-match detection registry (BOB → HDFC → ICICI, else
  `UnsupportedBank`); synthetic-fixture conventions + `just extract-fixture` recipe.
- Parsers, test-first against two-page synthetic fixtures: Bank of Baroda (26 txns),
  HDFC (27), ICICI (26) — UPI/IMPS/NEFT/ATM/card/interest/fee rows, multi-line
  narration joining, running-balance consistency, DD/MM/YYYY–DD/MM/YY–DD-Mon-YYYY
  normalization to ISO + integer paise.
- Narration decomposer: per-bank regex tables → `Counterparty`; `parse_statement`
  enriches parsed rows; unknown/truncated narrations stay `None`; fuzz totality test.
- 42 tests green (fmt + clippy -D warnings clean) in the CI-matching Linux container.

### App (`apps/mobile`)
- Tauri v2 + React + TS + Vite: pick PDF → password retry loop → parse → persist →
  virtualized 8-column table (Date, Description, Counterparty, Origin, Amount, Balance,
  Mode, Bank), Indian-grouped amounts, stable error kinds (wrong_password /
  unsupported_bank / corrupt_pdf / malformed_statement).
- SQLCipher db in app data dir; key beside db (OS keystore backlogged).
- Placeholder icon set (desktop + Android) — required by `tauri::generate_context!`.

### CI / release
- `ci.yml`: fmt, clippy, tests (with pdfium fetch), frontend build; cargo + npm caching.
- `release-apk.yml` on `v*` tags: Android APK (debug-signed, conditional release
  keystore via secrets) and Windows exe zipped with pdfium.dll, attached to the
  GitHub Release. Tauri invoked via `npx` (npm run mangles flag forwarding).

### Docs
- CLAUDE.md doctrine regenerated from the updated master prompt; README Android
  sideload guide; ACCEPTANCE.md phone-test script + parse-accuracy table; BACKLOG.md
  deferrals; NEXT_SESSION.md handoff for the open APK failure.
