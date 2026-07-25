# Extract per-page text from a real statement PDF (in git-ignored corpus/)
# for hand-redaction into synthetic fixtures. See tests/fixtures/README.md.
extract-fixture pdf password="":
    cargo run -p trackery-core --example extract_fixture -- "{{pdf}}" "{{password}}"
