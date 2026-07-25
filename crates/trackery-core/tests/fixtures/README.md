# Fixture conventions

Committed fixtures are **synthetic text**, never real bank data.

## Rules

1. One `.txt` file per page of extracted statement text, named
   `<bank>/<bank>_page<N>.txt` (e.g. `bob/bob_page1.txt`). The file content
   must be exactly what `pdf::open_statement` would return for that page —
   parsers are tested against extracted text, not PDFs.
2. All names, amounts, dates, and narrations are invented. Account numbers,
   card numbers, and customer IDs use `XXXX1234`-style masking. Phone numbers
   use the `9999999xxx` reserved-looking range, VPAs use obviously fake
   handles (`synth.merchant@ybl`). No real PAN, no real account or phone
   number, ever — if something looks real, stop and flag it.
3. Fixtures mimic each bank's real line format as closely as possible:
   header/footer furniture, column layout, date format, Dr/Cr convention,
   multi-line narration wrapping. Update fidelity by re-extracting from
   `corpus/` (git-ignored) with `just extract-fixture <pdf> <password>` and
   hand-redacting the output following rule 2.
4. Tests assert exact transaction counts and exact first/last rows, so any
   fixture edit must update the expected values in the matching
   `<bank>_test.rs` — test logic itself should never need to change.
5. `unsupported/` holds a statement-shaped page from a bank we deliberately
   do not support; it must keep failing detection with `UnsupportedBank`.

Real PDFs live only in `corpus/`, which is git-ignored. Do not commit binary
PDFs at all.
