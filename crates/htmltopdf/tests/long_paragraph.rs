//! A paragraph of more than 64 KiB is one run in the shaper, whose cluster offsets are `u16` (Parley
//! `ClusterData::text_offset`) and wrap. The glyphs were drawn right, but the text layer repeated the head for the
//! tail, and the dropped-text check took the tail for text without a font: a warning, and an error under
//! `--strict-fonts` (F70 review).

use htmltopdf::{RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// `n` words `w00000 w00001 …`, seven bytes each.
fn words(n: usize) -> String {
    (0..n).map(|i| format!("w{i:05} ")).collect()
}

#[test]
fn a_paragraph_over_64_kib_renders_strict_with_every_word_once_in_order() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    // 70 000 and 140 000 bytes: one and two wraps of the offsets, on A4 with the default margins. An integer
    // line-height keeps the line boxes tiling, so this is a guard for the text layer; the page-break line loss of
    // F78 is covered by `long_paragraph_pages.rs`.
    for n in [10_000usize, 20_000] {
        let html = format!(
            "<!doctype html><html><head><style>p{{margin:0;font-size:7px;line-height:8px}}</style></head>\
             <body style=\"font-family:Inter\"><p>{}</p></body></html>",
            words(n)
        );
        let pdf = r
            .render(&html, &RenderOptions::default())
            .unwrap_or_else(|e| panic!("{n} words: {e}"));
        let text = pdftotext(&pdf);
        let found: Vec<&str> = text
            .split_whitespace()
            .filter(|w| w.len() == 6 && w.starts_with('w'))
            .collect();
        assert_eq!(found.len(), n, "{n} words: {} found", found.len());
        for (i, w) in found.iter().enumerate() {
            assert_eq!(*w, format!("w{i:05}"), "{n} words: word {i}");
        }
    }
}

/// The PDF's text via poppler's `pdftotext`. Fails loudly when it is not installed.
fn pdftotext(pdf: &[u8]) -> String {
    let path = std::env::temp_dir().join(format!("htmltopdf-long-{}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}
