//! `white-space: nowrap` / `pre` / `pre-line` and the attributes and utilities that mean them (F66). Blitz laid
//! out every paragraph with soft wrapping on, so Tailwind's `whitespace-nowrap` IBAN cells broke at the spaces,
//! `<pre>` wrapped and `pre-line` kept the source indentation.
//!
//! Asserted on pixels (pdftoppm at 96 dpi, A4, 15 mm margins, Inter 14 px, `line-height: 20px`): "rows" are runs of
//! scanlines that contain dark (or red) pixels. The text sits in a 300 px box so that even the overflowing lines
//! end inside the page (Chrome's print shrink-to-fit would otherwise rescale the reference, F32a). The numbers
//! marked "Chrome" were measured with Chrome 141 on the same HTML (`out/scratch/f66/p*.html`, plan §7). Declarations
//! are Tailwind v4's own, written inline, because the committed `tailwind.css` contains none of these utilities.

use htmltopdf::{RenderOptions, Renderer};

/// About 418 px wide in Inter 14 px (Chrome: 57 to 475), well past the 300 px box.
const LONG: &str = "The quick brown fox jumps over the lazy dog and runs far away";
const SENTENCE: &str =
    "Dodavatel zboží a služeb poskytuje plnění podle objednávky a smlouvy uzavřené mezi stranami";
const IBAN: &str = "CZ65 0800 0000 1920 0014 5399";

/// Left edge of the content box: 15 mm at 96 dpi, rounded.
const LEFT: usize = 57;
const BOX: usize = 300;

// -- tests ----------------------------------------------------------------------------------------

#[test]
fn nowrap_paragraph_overflows_on_one_line() {
    // `whitespace-nowrap`.
    let r = rows(&render(&format!(
        r#"<div style="width:300px;white-space:nowrap">{LONG}</div>"#
    )));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].right > LEFT + BOX + 100,
        "the line ends at {} px, it should overflow the box",
        r[0].right
    );
    // Chrome: 475. The line is start-aligned and not clipped.
    assert!(
        r[0].left <= LEFT + 1 && r[0].right.abs_diff(475) <= 3,
        "{r:?}"
    );
}

#[test]
fn nowrap_cell_widens_its_column() {
    // Chrome: the IBAN cell is 222 px wide, the IBAN runs from 233 to 455, the table ends at 457.
    let pdf = render(&format!(
        r#"<table style="width:400px"><tr><td>{SENTENCE}</td>
           <td style="white-space:nowrap;color:rgb(200,0,0)">{IBAN}</td></tr></table>"#
    ));
    let iban = red_rows(&pdf);
    assert_eq!(iban.len(), 1, "the IBAN wrapped: {iban:?}");
    assert!(
        iban[0].left.abs_diff(233) <= 2 && iban[0].right.abs_diff(455) <= 2,
        "IBAN spans {}..{} px, Chrome has 233..455",
        iban[0].left,
        iban[0].right
    );
    assert!(iban[0].right <= LEFT + 400, "the IBAN leaves the table");
}

#[test]
fn pre_keeps_newlines_and_does_not_wrap() {
    // `whitespace-pre`, here on the element that usually carries it. Chrome: rows 57..475 and 58..90.
    let r = rows(&render(&format!(
        r#"<pre style="margin:0;font:14px/20px Inter;width:300px">{LONG}
short</pre>"#
    )));
    assert_eq!(r.len(), 2, "{r:?}");
    assert!(r[0].right > LEFT + BOX, "row 1 wrapped: {r:?}");
    assert!(r[1].right < LEFT + 60, "row 2 is not `short`: {r:?}");
}

#[test]
fn pre_wrap_wraps_and_keeps_newlines() {
    // Guard. `whitespace-pre-wrap`. Chrome: three rows, the first ends at 352.
    let r = rows(&render(&format!(
        r#"<pre style="margin:0;font:14px/20px Inter;width:300px;white-space:pre-wrap">{LONG}
short</pre>"#
    )));
    assert_eq!(r.len(), 3, "{r:?}");
    for row in &r {
        assert!(row.right <= LEFT + BOX, "{r:?}");
    }
}

#[test]
fn pre_line_collapses_indentation_and_keeps_newlines() {
    // `whitespace-pre-line`. Chrome: the first newline is an empty line; both rows start at 58 (the bearing of
    // `F` and `U`), where `pre` would keep the four spaces and start at 73.
    let r = rows(&render(
        "<div style=\"width:300px;white-space:pre-line\">\n    Firma s.r.o.\n    Ulice 1\n</div>",
    ));
    assert_eq!(r.len(), 2, "{r:?}");
    for row in &r {
        assert!(row.left.abs_diff(58) <= 1, "indentation kept: {r:?}");
    }
    assert!(r[1].top > r[0].top + 15, "the lines overlap: {r:?}");
}

#[test]
fn truncate_clips_at_the_box() {
    // Tailwind's `truncate`. The ellipsis is not drawn (F66c); Chrome stops at 347 with one.
    let r = rows(&render(&format!(
        r#"<div style="width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{LONG}</div>"#
    )));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].right <= LEFT + BOX,
        "text is painted past the box: {r:?}"
    );
    assert!(
        r[0].right > LEFT + BOX - 30,
        "the line was cut short: {r:?}"
    );
}

#[test]
fn td_nowrap_attribute_keeps_the_cell_on_one_line() {
    // HTML §15.3.8: `td[nowrap] { white-space: nowrap }`. Same numbers as the CSS version.
    let pdf = render(&format!(
        r#"<table style="width:400px"><tr><td>{SENTENCE}</td>
           <td nowrap style="color:rgb(200,0,0)">{IBAN}</td></tr></table>"#
    ));
    let iban = red_rows(&pdf);
    assert_eq!(iban.len(), 1, "the IBAN wrapped: {iban:?}");
    assert!(
        iban[0].left.abs_diff(233) <= 2 && iban[0].right.abs_diff(455) <= 2,
        "IBAN spans {}..{} px, Chrome has 233..455",
        iban[0].left,
        iban[0].right
    );
}

#[test]
fn nowrap_still_breaks_at_br_and_preserved_newlines() {
    // Guard: only soft wrap opportunities go away.
    let br = rows(&render(
        r#"<div style="width:300px;white-space:nowrap">a<br>b</div>"#,
    ));
    assert_eq!(br.len(), 2, "{br:?}");
    let pre = rows(&render(
        "<div style=\"width:300px;white-space:pre\">a\nb</div>",
    ));
    assert_eq!(pre.len(), 2, "{pre:?}");
}

#[test]
fn nowrap_long_lines_on_both_sides_of_a_br_stay_two_rows() {
    let both = rows(&render(&format!(
        r#"<div style="width:300px;white-space:nowrap">{LONG}<br>{LONG}</div>"#
    )));
    assert_eq!(both.len(), 2, "{both:?}");
    assert!(both.iter().all(|r| r.right > LEFT + BOX + 100), "{both:?}");
}

// -- helpers --------------------------------------------------------------------------------------

#[derive(Debug)]
struct Row {
    top: usize,
    left: usize,
    right: usize,
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .css("body { margin: 0; font: 14px/20px Inter } table { border-spacing: 0 } td { padding: 0; vertical-align: top }")
        .font_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/fonts/Inter-Regular.ttf"),
        )
        .expect("Inter-Regular.ttf — run scripts/fetch-fonts.sh")
        .build()
        .render(
            &format!(
                "<!doctype html><html><body>{body}<div style=\"width:10px;height:10px;background:rgb(0,0,255)\"></div></body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Rows of dark text: every channel below 140 (the blue marker and red text are not dark).
fn rows(pdf: &[u8]) -> Vec<Row> {
    clusters(pdf, |r, g, b| r < 140 && g < 140 && b < 140)
}

/// Rows of the red text of `rgb(200 0 0)`.
fn red_rows(pdf: &[u8]) -> Vec<Row> {
    clusters(pdf, |r, g, b| {
        let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
        r > g + 80 && r > b + 80
    })
}

fn clusters(pdf: &[u8], hit: impl Fn(u8, u8, u8) -> bool) -> Vec<Row> {
    let (w, rgb) = rasterise(pdf, 96);
    let h = rgb.len() / 3 / w;
    let mut out: Vec<Row> = Vec::new();
    let mut open = false;
    for y in 0..h {
        let xs: Vec<usize> = (0..w)
            .filter(|&x| {
                let i = (y * w + x) * 3;
                hit(rgb[i], rgb[i + 1], rgb[i + 2])
            })
            .collect();
        match (xs.first(), xs.last(), open) {
            (Some(&l), Some(&r), true) => {
                let row = out.last_mut().unwrap();
                row.left = row.left.min(l);
                row.right = row.right.max(r);
            }
            (Some(&l), Some(&r), false) => {
                out.push(Row {
                    top: y,
                    left: l,
                    right: r,
                });
                open = true;
            }
            _ => open = false,
        }
    }
    out
}

/// First page as (width, raw RGB bytes) via poppler's `pdftoppm`.
fn rasterise(pdf: &[u8], dpi: u32) -> (usize, Vec<u8>) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("htmltopdf-ws-{}-{n}", std::process::id()));
    let pdf_path = base.with_extension("pdf");
    std::fs::write(&pdf_path, pdf).unwrap();
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-f", "1", "-l", "1", "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(out.status.success(), "pdftoppm failed");
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
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
    (fields[0], ppm[i + 1..].to_vec())
}
