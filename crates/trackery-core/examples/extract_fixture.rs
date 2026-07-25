//! Dev tool: extract per-page text from a real statement PDF in `corpus/`
//! so it can be hand-redacted into a committed synthetic fixture.
//! Usage: cargo run --example extract_fixture -- <pdf> [password]
//! Writes `<pdf-stem>_page<N>.txt` next to the PDF (corpus/ is git-ignored).

use std::path::PathBuf;

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
    Ok(())
}
