# trackery_fi — engineering doctrine

Privacy-first, local-only personal finance tracker for India. Tauri v2 mobile-first app
(Android primary, desktop secondary) with a pure-Rust core. Sprint 1 scope: password-protected
PDF parsing for BOB/HDFC/ICICI, narration decomposition, SQLCipher storage, plain transaction
table UI, tag-push APK releases.

## Engineering doctrine (non-negotiable)

- **YAGNI enforced.** No sync code, no cloud code, no LLM code, no dashboard charts, no settings screens, no i18n, no theming system. If a file or dependency is not required by the six items above, do not create it. When in doubt, leave it out and note it in `BACKLOG.md`.
- **Test-first on parsers.** Every parser profile is written against fixture files BEFORE the implementation. A parser task is not done until its tests pass against all its fixtures.
- **No real bank data in the repo. Ever.** Real PDFs live in `corpus/` which is git-ignored from commit #1. Committed fixtures are SYNTHETIC text files that mimic each bank's line format with fake names, fake amounts, fake account numbers (use `XXXX1234` style). If you ever find a real-looking PAN, account number, or phone number in a file about to be committed — stop and flag it.
- **Conventional commits, one commit per task.** Format: `feat(scope): description`, `test(scope): ...`, `ci: ...`, `chore: ...`. Each task below ends with exactly one commit (plus fixup commits only if CI fails). Write meaningful commit bodies: what + why, 2–4 lines.
- **Errors are values.** Rust core returns typed errors (`thiserror`): `WrongPassword`, `UnsupportedBank`, `MalformedStatement { line: usize }`, `EncryptedDbError`. The UI must be able to distinguish "wrong password" from "we don't support this bank yet". No `.unwrap()` outside tests.
- **Indian data reality.** Dates are DD/MM/YYYY or DD/MM/YY or DD-Mon-YYYY depending on bank. Amounts use Indian digit grouping (`1,23,456.78`) and Dr/Cr columns or signed columns depending on bank. Currency is ₹. Handle all of it in the core, normalized to ISO dates + integer paise internally (NEVER floats for money).
