//! Media queries are evaluated for the `print` medium (F7): the engine evaluates them as `screen`, so
//! Tailwind's `print:` variants were ignored and `@media screen` rules leaked into the PDF. Each box
//! must come out red (the rule for print applied) or black (the rule for screen not applied); blue means
//! the wrong medium won.

use htmltopdf::{RenderOptions, Renderer};

const RULES: &str = r#"
    div { width: 200px; height: 100px; margin-bottom: 20px; background: rgb(0 0 0); }
    @media print { .print { background: rgb(255 0 0) } }
    @media screen { .screen { background: rgb(0 0 255) } }
    @media not print { .not-print { background: rgb(0 0 255) } }
    @media not screen { .not-screen { background: rgb(255 0 0) } }
    @media only print and (min-width: 1px) { .only-print { background: rgb(255 0 0) } }
    @media screen, PRINT { .list { background: rgb(255 0 0) } }
    @media (min-width: 1px) and (max-width: 100000px) { .feature { background: rgb(255 0 0) } }
"#;

/// (class, expected colour) in the order the boxes are laid out.
const BOXES: &[(&str, [u8; 3])] = &[
    ("print", RED),
    ("screen", BLACK),
    ("not-print", BLACK),
    ("not-screen", RED),
    ("only-print", RED),
    ("list", RED),
    ("feature", RED),
];
const RED: [u8; 3] = [255, 0, 0];
const BLACK: [u8; 3] = [0, 0, 0];

fn body() -> String {
    BOXES
        .iter()
        .map(|(class, _)| format!(r#"<div class="{class}"></div>"#))
        .collect()
}

#[test]
fn bundled_css_is_evaluated_for_print() {
    let pdf = render(Renderer::builder().css(RULES), &body());
    assert_boxes(&pdf);
}

#[test]
fn document_style_element_is_evaluated_for_print() {
    let pdf = render(
        Renderer::builder(),
        &format!("<style>{RULES}</style>{}", body()),
    );
    assert_boxes(&pdf);
}

fn render(builder: htmltopdf::RendererBuilder, body: &str) -> Vec<u8> {
    builder
        .css("@page { margin: 0 } body { margin: 0 }")
        .build()
        .render(
            &format!("<!doctype html><html><head></head><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn assert_boxes(pdf: &[u8]) {
    let got = column(pdf);
    let want: Vec<[u8; 3]> = BOXES.iter().map(|(_, c)| *c).collect();
    assert_eq!(
        got,
        want,
        "box colours top to bottom, for classes {:?}",
        BOXES.iter().map(|(c, _)| *c).collect::<Vec<_>>()
    );
}

/// Colours of the boxes down a column through them, read from the rasterised first page: each run of
/// at least 10 non-white rows is one box. The column is inside the default page margin plus the box
/// width, whatever the margin.
fn column(pdf: &[u8]) -> Vec<[u8; 3]> {
    let (w, h, rgb) = rasterise(pdf);
    let x = 120;
    let mut runs: Vec<([u8; 3], usize)> = Vec::new();
    for y in 0..h {
        let i = (y * w + x) * 3;
        let px = [rgb[i], rgb[i + 1], rgb[i + 2]];
        match runs.last_mut() {
            Some((c, n)) if *c == px => *n += 1,
            _ => runs.push((px, 1)),
        }
    }
    runs.into_iter()
        .filter(|(c, n)| *n >= 10 && *c != [255, 255, 255])
        .map(|(c, _)| c)
        .collect()
}

/// First page as (width, height, raw RGB bytes) via poppler's `pdftoppm`. Fails loudly when it is not
/// installed: a silent skip would hide wrong media.
fn rasterise(pdf: &[u8]) -> (usize, usize, Vec<u8>) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("htmltopdf-media-{}-{n}", std::process::id()));
    let pdf_path = base.with_extension("pdf");
    std::fs::write(&pdf_path, pdf).unwrap();
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "48", "-f", "1", "-l", "1", "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(
        out.status.success(),
        "pdftoppm failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
    // Header: "P6\n<w> <h>\n<max>\n".
    assert!(ppm.starts_with(b"P6"), "not a binary PPM");
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(
            std::str::from_utf8(&ppm[start..i])
                .unwrap()
                .parse::<usize>()
                .unwrap(),
        );
    }
    (fields[0], fields[1], ppm[i + 1..].to_vec())
}
