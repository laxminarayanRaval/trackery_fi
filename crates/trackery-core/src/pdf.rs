//! PDF text extraction via pdfium, with password support.
//!
//! # Pdfium provisioning
//!
//! `pdfium-render` binds at runtime to a prebuilt pdfium dynamic library from
//! <https://github.com/bblanchon/pdfium-binaries>. The library is searched in
//! this order (see [`bind`]):
//!
//! 1. The directory named by the `PDFIUM_LIB_DIR` environment variable.
//! 2. The directory containing the current executable (desktop bundles ship
//!    `pdfium.dll` / `libpdfium.dylib` / `libpdfium.so` next to the binary).
//! 3. Debug builds only: `<workspace>/target/pdfium/` — for dev and tests,
//!    download e.g. `pdfium-win-x64.tgz` from pdfium-binaries and place the
//!    library file there (git-ignored via `target/`).
//! 4. The system library search path. On **Android** this is the one that
//!    fires: bundle `libpdfium.so` per ABI under the Tauri app's
//!    `gen/android/app/src/main/jniLibs/<abi>/` and the loader resolves it
//!    from the APK's native library directory.

use std::path::PathBuf;
use std::sync::OnceLock;

use pdfium_render::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// The PDF is password-protected and the password was missing or wrong.
    #[error("wrong or missing password")]
    WrongPassword,
    /// Not a readable PDF (truncated, garbage, or unsupported format).
    #[error("corrupt or unreadable PDF")]
    CorruptPdf,
    /// The pdfium dynamic library could not be located/loaded.
    #[error("pdfium library unavailable: {0}")]
    PdfiumUnavailable(String),
}

/// Open a bank statement PDF and extract per-page text.
///
/// Returns one `String` per page. `password` is required for protected
/// statements; a missing or wrong password yields [`PdfError::WrongPassword`]
/// so the UI can re-prompt, while unreadable files yield
/// [`PdfError::CorruptPdf`].
pub fn open_statement(bytes: &[u8], password: Option<&str>) -> Result<Vec<String>, PdfError> {
    let pdfium = pdfium()?;
    let doc = pdfium
        .load_pdf_from_byte_slice(bytes, password)
        .map_err(|e| match e {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                PdfError::WrongPassword
            }
            _ => PdfError::CorruptPdf,
        })?;
    doc.pages()
        .iter()
        .map(|page| {
            page.text()
                .map(|t| t.all())
                .map_err(|_| PdfError::CorruptPdf)
        })
        .collect()
}

/// Process-wide pdfium instance. The `thread_safe` crate feature serializes
/// all pdfium calls behind an internal mutex, so sharing one instance is safe.
fn pdfium() -> Result<&'static Pdfium, PdfError> {
    static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    PDFIUM
        .get_or_init(|| bind().map(Pdfium::new).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| PdfError::PdfiumUnavailable(e.clone()))
}

/// Locate the pdfium dynamic library per the search order in the module docs.
fn bind() -> Result<Box<dyn PdfiumLibraryBindings>, PdfiumError> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("PDFIUM_LIB_DIR") {
        candidates.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        candidates.push(exe_dir);
    }
    #[cfg(debug_assertions)]
    candidates.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/pdfium"
    )));

    let mut last_err = None;
    for dir in candidates {
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir)) {
            Ok(bindings) => return Ok(bindings),
            Err(e) => last_err = Some(e),
        }
    }
    // Android resolves the APK-bundled libpdfium.so through this path.
    Pdfium::bind_to_system_library().map_err(|e| last_err.unwrap_or(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use md5::{Digest, Md5};

    const PASSWORD: &str = "test123";
    const PAGE_TEXT: &str = "TRACKERY SYNTHETIC STATEMENT XXXX1234";

    /// PDF standard security handler padding string (ISO 32000-1, 7.6.3.3).
    const PAD: [u8; 32] = [
        0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
        0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
        0x69, 0x7A,
    ];

    /// RC4 stream cipher — fixture-generation crypto only, not a security boundary.
    fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }
        let (mut i, mut j) = (0u8, 0u8);
        data.iter()
            .map(|&b| {
                i = i.wrapping_add(1);
                j = j.wrapping_add(s[i as usize]);
                s.swap(i as usize, j as usize);
                b ^ s[s[i as usize].wrapping_add(s[j as usize]) as usize]
            })
            .collect()
    }

    fn pad_password(pw: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = pw.as_bytes();
        let n = bytes.len().min(32);
        out[..n].copy_from_slice(&bytes[..n]);
        out[n..].copy_from_slice(&PAD[..32 - n]);
        out
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }

    /// Build a tiny synthetic one-page PDF, optionally encrypted with the
    /// 40-bit RC4 standard security handler (revision 2) — the oldest scheme,
    /// still accepted by pdfium and simple enough to implement by hand.
    fn build_pdf(password: Option<&str>) -> Vec<u8> {
        let content = format!("BT /F1 12 Tf 72 720 Td ({PAGE_TEXT}) Tj ET");
        let file_id: [u8; 16] = *b"trackery-fixture";

        let (content_bytes, encrypt_obj) = match password {
            None => (content.into_bytes(), None),
            Some(pw) => {
                let padded = pad_password(pw);
                // Rev 2, owner password == user password.
                let o = rc4(&Md5::digest(padded)[..5], &padded);
                let p: i32 = -1; // all permissions
                let mut h = Md5::new();
                h.update(padded);
                h.update(&o);
                h.update(p.to_le_bytes());
                h.update(file_id);
                let key = h.finalize()[..5].to_vec();
                let u = rc4(&key, &PAD);
                // Per-object key for content stream object 4, generation 0.
                let objkey_src: Vec<u8> = key.iter().copied().chain([4u8, 0, 0, 0, 0]).collect();
                let objkey = Md5::digest(&objkey_src)[..10].to_vec();
                let dict = format!(
                    "<</Filter/Standard/V 1/R 2/O <{}>/U <{}>/P -1>>",
                    hex(&o),
                    hex(&u)
                );
                (rc4(&objkey, content.as_bytes()), Some(dict))
            }
        };

        let mut objects: Vec<Vec<u8>> = vec![
            b"<</Type/Catalog/Pages 2 0 R>>".to_vec(),
            b"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_vec(),
            b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents 4 0 R\
              /Resources<</Font<</F1 5 0 R>>>>>>"
                .to_vec(),
            {
                let mut o = format!("<</Length {}>>\nstream\n", content_bytes.len()).into_bytes();
                o.extend_from_slice(&content_bytes);
                o.extend_from_slice(b"\nendstream");
                o
            },
            b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_vec(),
        ];
        if let Some(dict) = &encrypt_obj {
            objects.push(dict.clone().into_bytes());
        }

        let mut buf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            buf.extend_from_slice(body);
            buf.extend_from_slice(b"\nendobj\n");
        }

        let xref_at = buf.len();
        buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        buf.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            buf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        let encrypt_ref = if encrypt_obj.is_some() {
            format!("/Encrypt {} 0 R", objects.len())
        } else {
            String::new()
        };
        let id_hex = hex(&file_id);
        buf.extend_from_slice(
            format!(
                "trailer\n<</Size {}/Root 1 0 R/ID[<{id_hex}><{id_hex}>]{encrypt_ref}>>\n\
                 startxref\n{xref_at}\n%%EOF",
                objects.len() + 1
            )
            .as_bytes(),
        );
        buf
    }

    #[test]
    fn extracts_text_with_correct_password() {
        let pdf = build_pdf(Some(PASSWORD));
        let pages = open_statement(&pdf, Some(PASSWORD)).expect("should open with password");
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("XXXX1234"), "page text: {:?}", pages[0]);
    }

    #[test]
    fn extracts_text_from_unprotected_pdf() {
        let pdf = build_pdf(None);
        let pages = open_statement(&pdf, None).expect("should open without password");
        assert!(
            pages[0].contains("TRACKERY SYNTHETIC"),
            "page text: {:?}",
            pages[0]
        );
    }

    #[test]
    fn wrong_password_is_distinguished() {
        let pdf = build_pdf(Some(PASSWORD));
        assert!(matches!(
            open_statement(&pdf, Some("wrong")),
            Err(PdfError::WrongPassword)
        ));
        assert!(matches!(
            open_statement(&pdf, None),
            Err(PdfError::WrongPassword)
        ));
    }

    #[test]
    fn corrupt_pdf_is_distinguished() {
        assert!(matches!(
            open_statement(b"definitely not a pdf", None),
            Err(PdfError::CorruptPdf)
        ));
        assert!(matches!(
            open_statement(&[], None),
            Err(PdfError::CorruptPdf)
        ));
    }
}
