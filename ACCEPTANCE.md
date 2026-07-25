# Sprint 1 acceptance — manual test script

Run on a real Android phone with the sideloaded APK (see README) and real statements
from `corpus/` (never committed). Desktop (tauri dev) works for a faster first pass.

## Test script (repeat per bank: BOB, HDFC, ICICI)

1. Launch the app in **airplane mode**. It must open with an empty (or previous) table.
2. Tap **Pick statement PDF**, choose the bank's password-protected statement.
3. Expect a **password prompt**. First enter a *wrong* password → the app must say the
   password is wrong and re-prompt (NOT crash, NOT show "unsupported bank").
4. Enter the correct password → transactions appear in the table **within 5 seconds**.
5. Check the table columns: Date (DD/MM/YYYY display), Description, Counterparty,
   Transaction Origin ("Statement Import"), Amount (Indian grouping, debits negative),
   Balance, Mode (UPI/IMPS/NEFT/ATM/CARD/…), Bank.
6. Spot-check 5 rows against the PDF: date, amount, balance, narration.
7. Scroll the full table — virtualized list must stay smooth for the whole statement.
8. Re-import the same PDF → no crash; row count behavior noted below in results.
9. Pick a non-bank PDF (any random PDF) → must show "this bank isn't supported yet".
10. Pick a corrupt/truncated file renamed to .pdf → must show a corrupt-file message.
11. Kill and relaunch the app → previously imported transactions still there
    (persisted, decrypted with the stored key).

## Known limitations (Sprint 1)

- Single implicit account; `account_id` is not yet user-visible or configurable.
- DB key is stored beside the DB file (OS keystore integration is backlogged).
- Narrations that match no known pattern show no counterparty (by design — never guess).
- Re-importing the same statement duplicates rows (dedup is backlogged).
- Debug-signed APK unless a release keystore secret is configured.

## Parse accuracy record

Fill one row per real statement tested.

| Bank | Statement period | Pages | Rows in PDF | Rows parsed | Narrations decomposed | Notes |
|------|-----------------|-------|-------------|-------------|----------------------|-------|
| BOB  |                 |       |             |             |                      |       |
| HDFC |                 |       |             |             |                      |       |
| ICICI|                 |       |             |             |                      |       |
