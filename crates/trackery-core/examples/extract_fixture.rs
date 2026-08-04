//! Dev tool: extract per-page text from a real statement PDF in `corpus/`
//! so it can be hand-redacted into a committed synthetic fixture, and report
//! how the bank registry sees it — the same detection + parse path the app
//! runs, so a real "unsupported bank" or "malformed statement" failure can be
//! diagnosed here first instead of guessed at from the app's error message.
//! Usage: cargo run --example extract_fixture -- <pdf> [password]
//! Writes `<pdf-stem>_page<N>.txt` next to the PDF (corpus/ is git-ignored).

use std::path::PathBuf;

use trackery_core::banks::ParseError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let pdf_path = PathBuf::from(
        args.next()
            .ok_or("usage: extract_fixture <pdf> [password]")?,
    );
    let password = args.next();

    let bytes = std::fs::read(&pdf_path)?;
    let pages = trackery_core::pdf::open_statement(&bytes, password.as_deref())?;

    let stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("statement");
    for (i, text) in pages.iter().enumerate() {
        let out = pdf_path.with_file_name(format!("{stem}_page{}.txt", i + 1));
        std::fs::write(&out, text)?;
        eprintln!("wrote {}", out.display());
    }
    eprintln!(
        "{} page(s). Redact per tests/fixtures/README.md before committing.",
        pages.len()
    );

    let diag = trackery_core::banks::diagnose(&pages);
    eprintln!(
        "\n--- bank detection (first page, {} chars) ---",
        diag.first_page_chars
    );
    for d in &diag.detections {
        eprintln!(
            "  {:<6} {}",
            d.bank,
            if d.matched { "MATCH" } else { "no match" }
        );
    }
    if diag.detections.iter().all(|d| !d.matched) {
        eprintln!(
            "\nfirst-page text preview (first {} chars):",
            diag.first_page_preview.chars().count()
        );
        eprintln!("{}", diag.first_page_preview);
    }

    match trackery_core::banks::parse_statement(&pages) {
        Ok(txns) => eprintln!("\nparsed {} transaction(s) OK", txns.len()),
        Err(ParseError::UnsupportedBank) => {
            eprintln!("\nparse result: UnsupportedBank (see detection report above)")
        }
        Err(ParseError::MalformedStatement { line }) => {
            eprintln!("\nparse result: MalformedStatement at line {line}")
        }
    }
    Ok(())
}
