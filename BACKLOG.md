# Backlog

Anything cut by YAGNI goes here, one line each.

- Account entity + multi-account support (parsers emit `account_id = Uuid::nil()`, app uses one implicit account)
- DB key in OS keystore (Android Keystore / DPAPI) — currently a file beside the DB
- Duplicate-import detection (same statement imported twice duplicates rows)
- Manual transaction entry UI (`TransactionOrigin::ManualEntry` exists in the model/db only)
- SMS ingestion (`TransactionOrigin::SmsImport` exists in the model/db only)
- Category management (`category_id` column exists, always NULL)
- Description editing (`description` column exists, importer leaves it NULL)
- Statement-page fixture loader assumes < 10 pages per fixture (lexical sort)
- Search, filters, reports, budgets, calendar, analytics, net worth, recurring/scheduled/split transactions
- CSV/email/bank-API import, backup & restore, device sync
- Release-keystore signing docs for maintainers (workflow supports it via secrets)
- Desktop app icons + bundling (`bundle.active` is false; Android gets icons via `tauri android init` in CI)
