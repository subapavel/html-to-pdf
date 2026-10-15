//! `text-transform` changes the text that is laid out and extracted (F4). Blitz pushed text nodes into the
//! line builder as written, so Tailwind's `uppercase` headings printed in source case. Asserted on the PDF's
//! text (`pdftotext`), which is what a reader copies and what the glyphs show.

use htmltopdf::{RenderOptions, Renderer};

fn text(css: &str, body: &str) -> String {
    let pdf = Renderer::builder()
        .css(css)
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    pdftotext(&pdf)
}

#[test]
fn uppercase_lowercase_and_inheritance() {
    let t = text(
        ".u { text-transform: uppercase } .l { text-transform: lowercase } .n { text-transform: none }",
        r#"<p class="u">čeština straße <span>vnořeno</span> <span class="n">beze změny</span></p>
           <p class="l">MALÁ PÍSMENA</p>"#,
    );
    assert!(t.contains("ČEŠTINA STRASSE VNOŘENO beze změny"), "{t:?}");
    assert!(t.contains("malá písmena"), "{t:?}");
}

#[test]
fn capitalize_words_across_inline_elements() {
    let t = text(
        ".c { text-transform: capitalize }",
        r#"<p class="c">dobrý den, e-mail don't 1st wo<b>rd</b> 'citát'</p>"#,
    );
    assert!(
        t.contains("Dobrý Den, E-Mail Don't 1st Word 'Citát'"),
        "{t:?}"
    );
}

#[test]
fn capitalize_restarts_after_br_and_ignores_marks_inside_words() {
    let t = text(
        ".c { text-transform: capitalize }",
        "<p class=\"c\">hello<br>world<br>third line</p><p class=\"c\">re\u{0301}zat pre\u{00AD}fix</p>",
    );
    assert!(t.contains("Hello World Third Line"), "{t:?}");
    // A decomposed accent and a soft hyphen stay inside the word: nothing after them is capitalized.
    // (pdftotext may drop the mark and split at the soft hyphen, so only the case is asserted.)
    assert!(t.contains("zat") && !t.contains("Zat"), "{t:?}");
    assert!(t.contains("fix") && !t.contains("Fix"), "{t:?}");
}

#[test]
fn tailwind_uppercase_class_from_bundled_css() {
    let t = text(
        ".uppercase { text-transform: uppercase; }",
        r#"<table><tr><th class="uppercase">Položka</th></tr></table>"#,
    );
    assert!(t.contains("POLOŽKA"), "{t:?}");
}

/// The PDF's text via poppler's `pdftotext`, whitespace collapsed. Fails loudly when it is not installed.
fn pdftotext(pdf: &[u8]) -> String {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tt-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
