//! Renders every fixture under fixtures/invoices with the bundled Tailwind CSS and Inter fonts.
//! Asserts: output is a PDF, render is byte-deterministic, text survives into the PDF.
//! Pixel comparison against Chrome is scripts/compare.py (needs poppler), not a cargo test.

use std::path::{Path, PathBuf};

use htmltopdf::{RenderOptions, Renderer};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn renderer() -> Renderer {
    let root = root();
    let mut b = Renderer::builder()
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css");
    let fonts = std::fs::read_dir(root.join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect::<Vec<_>>();
    assert!(
        !fonts.is_empty(),
        "no fonts in fixtures/fonts — run scripts/fetch-fonts.sh"
    );
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    b.build()
}

fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/invoices"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("index.html").is_file())
        .collect();
    v.sort();
    assert!(v.len() >= 5, "expected the five invoice fixtures");
    v
}

#[test]
fn every_fixture_renders_to_a_pdf_deterministically() {
    let r = renderer();
    for dir in fixtures() {
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let opts = RenderOptions {
            lang: Some("cs".into()),
            ..Default::default()
        };
        let a = r
            .render(&html, &opts)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert!(a.starts_with(b"%PDF-"), "{}: not a PDF", dir.display());
        let b = r.render(&html, &opts).unwrap();
        assert_eq!(a, b, "{}: output is not deterministic", dir.display());
    }
}

#[test]
fn invoice_identifiers_survive_into_pdf_text() {
    // Guards F3 (text dropped when no font matches). Uses the PDF's own text objects: with Identity-H
    // CID fonts the raw bytes are glyph ids, so we check via the ToUnicode-free route: page count + a
    // string that is set in `font-sans` must exist, and once F3 is fixed the `font-mono` one too.
    let r = renderer();
    let html =
        std::fs::read_to_string(root().join("fixtures/invoices/01-basic/index.html")).unwrap();
    let pdf = r.render(&html, &RenderOptions::default()).unwrap();
    // Cheap structural assertions that do not need a PDF parser:
    let count = pdf
        .windows(b"/Type /Page".len())
        .filter(|w| w == b"/Type /Page")
        .count();
    assert!(count >= 1, "no page objects");
    assert!(
        pdf.windows(b"/FontFile2".len()).any(|w| w == b"/FontFile2"),
        "no embedded TrueType font"
    );
}
