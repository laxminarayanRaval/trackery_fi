# trackery_fi — engineering doctrine

Privacy-first, local-only personal finance tracker for India. Tauri v2 mobile-first app
(Android primary, desktop secondary) with a pure-Rust core. Sprint 1 scope: password-protected
PDF parsing for BOB/HDFC/ICICI, narration decomposition, SQLCipher storage, plain transaction
table UI, tag-push APK releases. The master prompt is `trackery_fi_sprint1_prompt.md` — when
it conflicts with anything else, it wins.

## Product vision

trackery_fi is a privacy-first, offline-first personal finance operating system built
specifically for India. Sprint 1 intentionally delivers only the smallest vertical slice,
but every architectural decision should assume the application will eventually become a
complete finance manager capable of replacing commercial apps like Money Manager (Realbyte),
Wallet, and Bluecoins. Build today's features without limiting tomorrow's possibilities.

Prioritize: clean domain boundaries, extensible architecture, predictable APIs, strong
typing, high performance, privacy by default, offline-first, testability.

## Engineering doctrine (non-negotiable)

- **YAGNI with architectural foresight.** Do not implement future features. However, design interfaces, database schemas, traits, and domain models so future features can be added without breaking existing code. When a reusable abstraction naturally exists, prefer the abstraction over feature-specific implementations. Anything intentionally deferred belongs in BACKLOG.md.
- **Test-first on parsers.** Every parser profile is written against fixture files BEFORE the implementation. A parser task is not done until its tests pass against all its fixtures.
- **No real bank data in the repo. Ever.** Real PDFs live in `corpus/` which is git-ignored from commit #1. Committed fixtures are SYNTHETIC text files that mimic each bank's line format with fake names, fake amounts, fake account numbers (use `XXXX1234` style). If you ever find a real-looking PAN, account number, or phone number in a file about to be committed — stop and flag it.
- **Conventional commits, one commit per task.** Format: `feat(scope): description`, `test(scope): ...`, `ci: ...`, `chore: ...`. Each task below ends with exactly one commit (plus fixup commits only if CI fails). Write meaningful commit bodies: what + why, 2–4 lines.
- **Errors are values.** Rust core returns typed errors (`thiserror`): `WrongPassword`, `UnsupportedBank`, `MalformedStatement { line: usize }`, `EncryptedDbError`. The UI must be able to distinguish "wrong password" from "we don't support this bank yet". No `.unwrap()` outside tests.
- **Indian data reality.** Dates are DD/MM/YYYY or DD/MM/YY or DD-Mon-YYYY depending on bank. Amounts use Indian digit grouping (`1,23,456.78`) and Dr/Cr columns or signed columns depending on bank. Currency is ₹. Handle all of it in the core, normalized to ISO dates + integer paise internally (NEVER floats for money).

## Future architecture constraints

Sprint 1 implements only the required functionality, but the architecture must naturally
support future modules: manual transaction entry, SMS parsing, email parsing, CSV
import/export, bank API imports, multiple accounts, cash wallets, credit cards, loans,
investments, budgets, categories, tags, notes, attachments, recurring transactions,
scheduled transactions, split transactions, search, analytics, net worth, backup & restore,
device synchronization. Do NOT implement these features — make today's architecture
compatible with tomorrow's requirements.

## Data entry origins

The domain model and persistence layer MUST support transactions from multiple origins:
`StatementImport`, `ManualEntry`, `SmsImport`. Only `StatementImport` has a working
implementation in Sprint 1; the others exist as domain concepts, enums, and database
support so implementing them later requires no breaking changes.
