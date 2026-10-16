//! Outside `disc` / `circle` / `square` markers are painted (F33). Blitz laid the marker out with the font stack
//! `Bullet, monospace, sans-serif`; with bundled fonts only (`system_fonts(false)`) none of the three exists, Parley
//! dropped the run and `<ul><li>` printed no bullet while `inside` markers and `<ol>` numbers worked. CSS Lists 3 §3.1:
//! `::marker` inherits the item's font, so the marker must be shaped with it. CSS 2.1 §12.5.1: an outside marker sits
//! left of the principal box, in the first line box. Asserted on pixels via poppler's `pdftoppm`.
//!
//! Geometry: `body { font: 32px/48px "JetBrains Mono" }` (the built-in face), every list is indented by 100 px and
//! the page is rasterised at 96 dpi, so one device pixel is one CSS pixel; the item text `EEE` starts at x = 100 and
//! its ink spans cap height above the baseline. Row `r` (from 0) is device y `48 * r ..= 48 * r + 47`.

use std::path::{Path, PathBuf};

use htmltopdf::{Page, RenderOptions, Renderer};

/// Where the item text starts (CSS px).
const TEXT_X: usize = 100;
const LINE_H: usize = 48;

/// Ink = any pixel darker than this on its brightest channel.
const INK: u8 = 128;

#[test]
fn disc_is_painted_by_default() {
    // No `list-style` at all: the UA stylesheet's `ul { list-style-type: disc }`, position outside.
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px }",
        "<ul><li>EEE</li><li>EEE</li></ul>",
    ));
    for row in 0..2 {
        let m = marker(&page, row, "disc");
        assert_disc(&m, &page, row);
    }
}

#[test]
fn disc_with_bundled_inter() {
    // The face the fixtures use: the marker takes the item's face, so a template font works as well.
    let page = first_page(&render_inter(
        "ul { margin: 0; padding: 0 0 0 100px; list-style: disc } body { font-family: Inter }",
        "<ul><li>EEE</li></ul>",
    ));
    let m = marker(&page, 0, "disc");
    assert_disc(&m, &page, 0);
}

#[test]
fn circle_is_a_ring() {
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style-type: circle }",
        "<ul><li>EEE</li></ul>",
    ));
    let m = marker(&page, 0, "circle");
    assert_on_text_line(&m, &page, 0, "circle");
    assert!(m.w >= 6 && m.h >= 6, "circle too small to be a ring: {m:?}");
    assert!(
        m.fill() < 0.6,
        "a circle is hollow, but {:.2} of its box is ink: {m:?}",
        m.fill()
    );
    let (cx, cy) = m.centre();
    assert!(
        !page.is_ink(cx, cy),
        "the centre of a circle marker at ({cx}, {cy}) is ink"
    );
}

#[test]
fn square_is_filled_and_square() {
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style-type: square }",
        "<ul><li>EEE</li></ul>",
    ));
    let m = marker(&page, 0, "square");
    assert_on_text_line(&m, &page, 0, "square");
    assert!(m.fill() > 0.85, "a square is solid: {m:?}");
    assert!(m.w.abs_diff(m.h) <= 2, "a square is as wide as tall: {m:?}");
}

#[test]
fn nested_lists_cycle_disc_circle_square() {
    // UA stylesheet: `ul ul { circle }`, `ul ul ul { square }`. Rows: outer, middle, inner, outer again.
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px } li { height: 48px }",
        "<ul><li>EEE<ul><li>EEE<ul><li>EEE</li></ul></li></ul></li></ul>",
    ));
    // Rows are 48 px; the nested lists add their own indentation (100 px each), so look at x < text start of
    // each row separately.
    let outer = bbox(&page, 0, 100, 0, LINE_H).expect("outer marker");
    assert!(outer.fill() > 0.6, "outer marker is a disc: {outer:?}");
    // Middle row: text at x = 200; its marker is in x 100..200.
    let mid = bbox(&page, 100, 200, LINE_H, 2 * LINE_H).expect("middle marker");
    assert!(mid.fill() < 0.6, "middle marker is a ring: {mid:?}");
    // Inner row: text at x = 300; marker in x 200..300.
    let inner = bbox(&page, 200, 300, 2 * LINE_H, 3 * LINE_H).expect("inner marker");
    assert!(
        inner.fill() > 0.85 && inner.w.abs_diff(inner.h) <= 2,
        "inner marker is a square: {inner:?}"
    );
}

#[test]
fn marker_is_aligned_with_the_first_line_only() {
    // A wrapped item has one marker, level with its first line; a paragraph inside the item likewise.
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style: disc; width: 400px } p { margin: 0 }",
        "<ul><li>EEE EEE EEE EEE EEE EEE EEE EEE</li><li><p>EEE</p></li></ul>",
    ));
    // Item 1 is 31 characters in a 300 px column of 19.2 px advances (15 per line): two lines, rows 0 and 1.
    // Item 2 (`li > p`) is row 2.
    assert!(
        bbox(&page, TEXT_X, 700, LINE_H, 2 * LINE_H).is_some(),
        "the first item was expected to wrap onto row 1"
    );
    let first = marker(&page, 0, "first item");
    assert_on_text_line(&first, &page, 0, "first item");
    assert!(
        bbox(&page, 0, TEXT_X, LINE_H, 2 * LINE_H).is_none(),
        "the wrapped second line must not carry a marker"
    );
    let second = marker(&page, 2, "paragraph item");
    assert_on_text_line(&second, &page, 2, "paragraph item");
}

#[test]
fn list_style_none_paints_nothing() {
    // Tailwind preflight: `ul { list-style: none }`.
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style: none }",
        "<ul><li>EEE</li></ul>",
    ));
    assert!(
        bbox(&page, 0, TEXT_X, 0, LINE_H).is_none(),
        "`list-style: none` must not paint a marker"
    );
}

#[test]
fn inside_marker_is_unchanged() {
    // `inside` markers are text of the item's own line: ink starts at the item's left edge, the text is
    // pushed right, and nothing is painted in the margin.
    let page = first_page(&render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style: disc inside }",
        "<ul><li>EEE</li></ul>",
    ));
    assert!(
        bbox(&page, 0, TEXT_X - 1, 0, LINE_H).is_none(),
        "an inside marker paints nothing left of the item"
    );
    let bullet = bbox(&page, TEXT_X, TEXT_X + 30, 0, LINE_H).expect("inside marker");
    assert!(bullet.fill() > 0.6, "inside disc is filled: {bullet:?}");
    let text = bbox(&page, bullet.x1 + 3, 600, 0, LINE_H).expect("item text");
    assert!(
        text.h > bullet.h,
        "the item text is taller than the bullet: {text:?}"
    );
}

#[test]
fn ordered_numbers_are_unchanged() {
    let pdf = render(
        "ol { margin: 0; padding: 0 0 0 100px; list-style: decimal }",
        "<ol><li>EEE</li><li>EEE</li></ol>",
    );
    let words = pdf_words(&pdf);
    assert!(
        words.iter().any(|w| w == "1.") && words.iter().any(|w| w == "2."),
        "numbers of an outside <ol> are in the text layer, got {words:?}"
    );
    let page = first_page(&pdf);
    for row in 0..2 {
        let m = bbox(&page, 0, TEXT_X, row * LINE_H, (row + 1) * LINE_H)
            .unwrap_or_else(|| panic!("row {row}: no number painted left of the text"));
        assert!(m.x1 < TEXT_X, "the number is left of the text: {m:?}");
    }
}

#[test]
fn item_taller_than_a_page_has_one_marker() {
    // A single `<li>` of 60 lines (2880 px) splits over three pages; the marker belongs to the first line only.
    let lines = vec!["EEE"; 60].join("<br>");
    let pdf = render(
        "ul { margin: 0; padding: 0 0 0 100px; list-style: disc }",
        &format!("<ul><li>{lines}</li></ul>"),
    );
    let pages = page_count(&pdf);
    assert!(pages >= 2, "expected a split item, got {pages} page(s)");
    let first = rasterise(&pdf, 1);
    assert!(
        bbox(&first, 0, TEXT_X, 0, LINE_H).is_some(),
        "no marker on the first line of page 1"
    );
    for n in 2..=pages {
        let page = rasterise(&pdf, n);
        let ink = bbox(&page, 0, TEXT_X, 0, page.height);
        assert!(
            ink.is_none(),
            "page {n} repeats the marker of the split item: {ink:?}"
        );
    }
}

// -- assertions -----------------------------------------------------------------------------------

fn marker(page: &Raster, row: usize, what: &str) -> Blob {
    bbox(page, 0, TEXT_X, row * LINE_H, (row + 1) * LINE_H)
        .unwrap_or_else(|| panic!("row {row}: no {what} marker painted left of the text"))
}

/// The marker is left of the text, close to it, and level with the cap-height band of the item's first line.
fn assert_on_text_line(m: &Blob, page: &Raster, row: usize, what: &str) {
    let (y0, y1) = (row * LINE_H, (row + 1) * LINE_H);
    let text = bbox(page, TEXT_X, 700, y0, y1)
        .unwrap_or_else(|| panic!("row {row}: item text `EEE` is missing"));
    assert!(
        m.x1 + 2 <= text.x0,
        "{what} marker {m:?} must be left of the text {text:?}"
    );
    assert!(
        text.x0 - m.x1 <= 40,
        "{what} marker {m:?} is too far from the text {text:?}"
    );
    let (_, cy) = m.centre();
    assert!(
        cy >= text.y0 && cy <= text.y1,
        "{what} marker {m:?} is not level with the text's cap-height band {text:?}"
    );
    assert!(
        m.h >= 4 && m.h <= text.h,
        "{what} marker {m:?} has an implausible size next to the text {text:?}"
    );
}

fn assert_disc(m: &Blob, page: &Raster, row: usize) {
    assert_on_text_line(m, page, row, "disc");
    assert!(m.fill() > 0.6, "a disc is solid: {m:?}");
    let (cx, cy) = m.centre();
    assert!(
        page.is_ink(cx, cy),
        "the centre of a disc at ({cx}, {cy}) is not ink"
    );
}

// -- helpers --------------------------------------------------------------------------------------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A margin-less A4 page with `css` and `body` inside; the default renderer (built-in JetBrains Mono).
fn render(css: &str, body: &str) -> Vec<u8> {
    render_with(Renderer::builder(), css, body)
}

/// The same with the Inter faces of `fixtures/fonts`.
fn render_inter(css: &str, body: &str) -> Vec<u8> {
    let mut b = Renderer::builder();
    let mut fonts = std::fs::read_dir(root().join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect::<Vec<_>>();
    fonts.sort();
    assert!(!fonts.is_empty(), "run scripts/fetch-fonts.sh");
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    render_with(b, css, body)
}

fn render_with(b: htmltopdf::RendererBuilder, css: &str, body: &str) -> Vec<u8> {
    b.css(format!(
        "body {{ margin: 0; font: 32px/48px \"JetBrains Mono\" }} {css}"
    ))
    .build()
    .render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions {
            page: Page {
                margin_mm: 0.0,
                ..Page::A4
            },
            ..Default::default()
        },
    )
    .expect("render")
}

/// Bounding box and ink count of a region.
#[derive(Debug, Clone, Copy)]
struct Blob {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    w: usize,
    h: usize,
    ink: usize,
}

impl Blob {
    /// Share of the bounding box that is ink.
    fn fill(&self) -> f32 {
        self.ink as f32 / (self.w * self.h) as f32
    }
    fn centre(&self) -> (usize, usize) {
        ((self.x0 + self.x1) / 2, (self.y0 + self.y1) / 2)
    }
}

struct Raster {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Raster {
    fn is_ink(&self, x: usize, y: usize) -> bool {
        let i = (y * self.width + x) * 3;
        self.rgb[i..i + 3].iter().any(|&c| c < INK)
    }
}

/// Bounding box of the ink in `x0..x1` × `y0..y1` (end exclusive), `None` when the region is blank.
fn bbox(page: &Raster, x0: usize, x1: usize, y0: usize, y1: usize) -> Option<Blob> {
    let (mut bx0, mut bx1, mut by0, mut by1, mut ink) = (usize::MAX, 0, usize::MAX, 0, 0);
    for y in y0..y1.min(page.height) {
        for x in x0..x1.min(page.width) {
            if page.is_ink(x, y) {
                bx0 = bx0.min(x);
                bx1 = bx1.max(x);
                by0 = by0.min(y);
                by1 = by1.max(y);
                ink += 1;
            }
        }
    }
    (ink > 0).then(|| Blob {
        x0: bx0,
        x1: bx1,
        y0: by0,
        y1: by1,
        w: bx1 - bx0 + 1,
        h: by1 - by0 + 1,
        ink,
    })
}

fn first_page(pdf: &[u8]) -> Raster {
    rasterise(pdf, 1)
}

/// Page `n` (from 1) at 96 dpi via poppler's `pdftoppm`. Fails loudly when it is not installed.
fn rasterise(pdf: &[u8], n: usize) -> Raster {
    let pdf_path = temp(pdf, "pdf");
    let base = pdf_path.with_extension("");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "96"])
        .args(["-f", &n.to_string(), "-l", &n.to_string(), "-singlefile"])
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
    Raster {
        width: fields[0],
        height: fields[1],
        rgb: ppm[i + 1..].to_vec(),
    }
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// Whitespace-separated words of the text layer (`pdftotext`).
fn pdf_words(pdf: &[u8]) -> Vec<String> {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-listmarkers-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}
