use trackery_core::banks::{parse_statement, ParseError};

fn fixture_pages(dir: &str) -> Vec<String> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut paths: Vec<_> = std::fs::read_dir(&base)
        .unwrap_or_else(|e| panic!("fixture dir {}: {e}", base.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort(); // page1, page2, ... — zero-padding not needed below 10 pages
    paths
        .iter()
        .map(|p| std::fs::read_to_string(p).expect("read fixture page"))
        .collect()
}

#[test]
fn unknown_bank_statement_is_unsupported() {
    let pages = fixture_pages("unsupported");
    assert!(!pages.is_empty(), "unsupported fixture must have pages");
    assert!(matches!(
        parse_statement(&pages),
        Err(ParseError::UnsupportedBank)
    ));
}

#[test]
fn empty_statement_is_unsupported() {
    assert!(matches!(
        parse_statement(&[]),
        Err(ParseError::UnsupportedBank)
    ));
}
