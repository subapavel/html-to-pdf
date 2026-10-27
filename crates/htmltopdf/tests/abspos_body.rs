//! A box positioned against the initial containing block is placed in the page area, whatever body's own box is (F187,
//! also F92 and, for `position: fixed`, F40).
//!
//! CSS 2.1 section 10.1 (4): the containing block of `position: absolute` is the padding box of the nearest ancestor whose
//! `position` is not `static`, else the initial containing block, which in paged media is the page area (section 10.1 (1),
//! css-page-3). With a static body its margin, border and padding do not move the box. `left` / `top` / `right` / `bottom`
//! are offsets from the containing block's padding edges (section 10.3.7 / 10.6.4); with both insets `auto` on an axis the
//! box keeps its static position (which includes body's offset). css-break-3: the box is placed as if the page were not
//! fragmented and then sliced, so `top: 2000px` lands on the page holding y 2000 of the stitched containing block.
//! Section 9.6.1: a `position: fixed` box repeats on every page, relative to the page area.
//!
//! fulgur stored the geometry of such a box in a frame that is short by body's x offset on every page and by body's y offset
//! on every page after the first. Plan: docs/plans/2026-10-27-f187-abspos-body-margin.md.
//!
//! Every case renders an A4 page, rasterises it with `pdftoppm` at 96 dpi (1 device px = 1 CSS px) and measures the ink box
//! of a 100 x 100 `#0a0` box. The numbers are Chrome's (plan section 7), measured the same way; Chrome's A4 is 793.7 x
//! 1122.56 px, rasterised to 794 x 1123. Page area at 15 mm margins: x 57..737, y 57..1066.

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(css)
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

/// `style` goes in a `<style>` element at the start of `<body>`.
fn render_page(page_rule: &str, style: &str, body: &str) -> Vec<u8> {
    render(page_rule, &format!("<style>{style}</style>{body}"))
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-abspos-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

struct Page {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Page {
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.w + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

/// Page `n` (1-based) at 96 dpi.
fn raster(pdf: &[u8], n: usize) -> Page {
    let path = temp(pdf);
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", &n.to_string(), "-l", &n.to_string()])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while data[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while !data[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..at]).into_owned());
    }
    at += 1;
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    Page {
        w,
        h,
        rgb: data[at..at + w * h * 3].to_vec(),
    }
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|v| v.trim().parse().ok())
        .expect("pdfinfo prints the page count")
}

fn is_green(p: [u8; 3]) -> bool {
    p[1] > 150 && p[0] < 60 && p[2] < 60
}

/// The bounding box `[x0, x1) x [y0, y1)` of the green pixels of a page.
#[derive(Debug, PartialEq, Clone, Copy)]
struct Ink {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

impl Ink {
    /// Within 1 px of `[x0, x1) x [y0, y1)`.
    fn near(&self, want: (usize, usize, usize, usize)) -> bool {
        let close = |a: usize, b: usize| a.abs_diff(b) <= 1;
        close(self.x0, want.0)
            && close(self.x1, want.1)
            && close(self.y0, want.2)
            && close(self.y1, want.3)
    }
}

fn green(page: &Page) -> Option<Ink> {
    let mut b: Option<Ink> = None;
    for y in 0..page.h {
        for x in 0..page.w {
            if is_green(page.at(x, y)) {
                let r = b.get_or_insert(Ink {
                    x0: x,
                    x1: x + 1,
                    y0: y,
                    y1: y + 1,
                });
                r.x0 = r.x0.min(x);
                r.x1 = r.x1.max(x + 1);
                r.y1 = y + 1;
            }
        }
    }
    b
}

/// The green box of every page of `pdf` as `(page number, ink)`.
fn boxes(pdf: &[u8]) -> Vec<(usize, Ink)> {
    (1..=page_count(pdf))
        .filter_map(|n| green(&raster(pdf, n)).map(|ink| (n, ink)))
        .collect()
}

/// The pdf has exactly one green box, on page `n`, within 1 px of `want`.
#[track_caller]
fn assert_box(pdf: &[u8], n: usize, want: (usize, usize, usize, usize), what: &str) {
    let found = boxes(pdf);
    assert!(
        found.len() == 1 && found[0].0 == n && found[0].1.near(want),
        "{what}: expected the box on page {n} at x {}..{}, y {}..{} (Chrome); got {found:?}",
        want.0,
        want.1,
        want.2,
        want.3
    );
}

const M0: &str = "@page { size: A4; margin: 0 }";
const M15: &str = "@page { size: A4; margin: 15mm }";
/// A margined, bordered body: the offset of its content box from the page area is (34, 34).
const BODY: &str = "body { margin: 30px; border: 4px solid #c00 }";
const BOX: &str = "position:absolute;width:100px;height:100px;background:#0a0";

fn abs(inset: &str) -> String {
    format!("<div style=\"{BOX};{inset}\"></div>")
}

/// `n` one-line blocks of 20 px.
fn rows(n: usize) -> String {
    "<div style=\"height:20px;line-height:20px\">Řádek</div>".repeat(n)
}

// --- A1: top / left ---------------------------------------------------------------------------------------

#[test]
fn a1_top_left_m0() {
    let pdf = render_page(M0, BODY, &abs("top:50px;left:50px"));
    assert_box(&pdf, 1, (50, 150, 50, 150), "A1 m0");
}

#[test]
fn a1_top_left_m15() {
    let pdf = render_page(M15, BODY, &abs("top:50px;left:50px"));
    assert_box(&pdf, 1, (107, 207, 107, 207), "A1 m15");
}

// --- A2: right / bottom -----------------------------------------------------------------------------------

#[test]
fn a2_right_bottom_m0() {
    let pdf = render_page(M0, BODY, &abs("right:50px;bottom:50px"));
    assert_box(&pdf, 1, (643, 743, 972, 1072), "A2 m0");
}

#[test]
fn a2_right_bottom_m15() {
    let pdf = render_page(M15, BODY, &abs("right:50px;bottom:50px"));
    assert_box(&pdf, 1, (587, 687, 916, 1016), "A2 m15");
}

// --- A3: percentages of the initial containing block ---------------------------------------------------

#[test]
fn a3_percent_insets_m0() {
    let pdf = render_page(M0, BODY, &abs("left:10%;top:10%"));
    assert_box(&pdf, 1, (79, 179, 112, 212), "A3 m0");
}

#[test]
fn a3_percent_insets_m15() {
    let pdf = render_page(M15, BODY, &abs("left:10%;top:10%"));
    assert_box(&pdf, 1, (125, 225, 158, 258), "A3 m15");
}

// --- A4: an `auto` axis is the static position, which includes body's offset -------------------------------

#[test]
fn a4_static_position_m0() {
    let body = format!("<div style=\"height:100px\"></div>{}", abs("left:50px"));
    let pdf = render_page(M0, BODY, &body);
    assert_box(&pdf, 1, (50, 150, 134, 234), "A4 m0");
}

#[test]
fn a4_static_position_m15() {
    let body = format!("<div style=\"height:100px\"></div>{}", abs("left:50px"));
    let pdf = render_page(M15, BODY, &body);
    assert_box(&pdf, 1, (107, 207, 191, 291), "A4 m15");
}

// --- A5: a positioned body is the containing block ---------------------------------------------------------

const REL_BODY: &str = "body { margin: 30px; border: 4px solid #c00; position: relative }";

#[test]
fn a5_relative_body_top_left_m0() {
    let body = format!(
        "<div style=\"height:300px\"></div>{}",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M0, REL_BODY, &body);
    assert_box(&pdf, 1, (84, 184, 84, 184), "A5a m0");
}

#[test]
fn a5_relative_body_top_left_m15() {
    let body = format!(
        "<div style=\"height:300px\"></div>{}",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M15, REL_BODY, &body);
    assert_box(&pdf, 1, (141, 241, 141, 241), "A5a m15");
}

#[test]
fn a5_relative_body_right_bottom_m0() {
    let body = format!(
        "<div style=\"height:300px\"></div>{}",
        abs("right:50px;bottom:50px")
    );
    let pdf = render_page(M0, REL_BODY, &body);
    assert_box(&pdf, 1, (610, 710, 184, 284), "A5b m0");
}

#[test]
fn a5_relative_body_right_bottom_m15() {
    let body = format!(
        "<div style=\"height:300px\"></div>{}",
        abs("right:50px;bottom:50px")
    );
    let pdf = render_page(M15, REL_BODY, &body);
    assert_box(&pdf, 1, (554, 654, 241, 341), "A5b m15");
}

// --- A6, A7: guards (F30g places these already) --------------------------------------------------------------

#[test]
fn a6_nested_positioned_ancestor_m0() {
    let body = format!(
        "<div style=\"position:relative;margin:40px 0 0 20px;height:200px\">{}</div>",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M0, BODY, &body);
    assert_box(&pdf, 1, (104, 204, 124, 224), "A6 m0");
}

#[test]
fn a6_nested_positioned_ancestor_m15() {
    let body = format!(
        "<div style=\"position:relative;margin:40px 0 0 20px;height:200px\">{}</div>",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M15, BODY, &body);
    assert_box(&pdf, 1, (161, 261, 181, 281), "A6 m15");
}

#[test]
fn a7_static_wrapper_with_margin_m0() {
    let body = format!(
        "<div style=\"margin-left:20px\">{}</div>",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M0, BODY, &body);
    assert_box(&pdf, 1, (50, 150, 50, 150), "A7 m0");
}

#[test]
fn a7_static_wrapper_with_margin_m15() {
    let body = format!(
        "<div style=\"margin-left:20px\">{}</div>",
        abs("top:50px;left:50px")
    );
    let pdf = render_page(M15, BODY, &body);
    assert_box(&pdf, 1, (107, 207, 107, 207), "A7 m15");
}

// --- A8, A9, A10: a box on a later page --------------------------------------------------------------------

#[test]
fn a8_box_on_page_two_m0() {
    let body = format!("{}{}", rows(150), abs("top:2000px;left:50px"));
    let pdf = render_page(M0, BODY, &body);
    assert_box(&pdf, 2, (50, 150, 877, 977), "A8 m0");
}

#[test]
fn a9_box_on_page_two_m15() {
    let body = format!("{}{}", rows(150), abs("top:1800px;left:50px"));
    let pdf = render_page(M15, BODY, &body);
    // Chrome 847..947 (1800 - 1009.17 + 57 = 847.8).
    assert_box(&pdf, 2, (107, 207, 847, 947), "A9 m15");
}

#[test]
fn a10_only_a_y_offset_m0() {
    // `margin-top` of the first child collapses through body (margin 0): body's content starts at y 40 and there is no x offset.
    let body = format!(
        "<div style=\"margin-top:40px\">{}</div>{}",
        rows(150),
        abs("top:2000px;left:50px")
    );
    let pdf = render_page(M0, "body { margin: 0 }", &body);
    assert_box(&pdf, 2, (50, 150, 877, 977), "A10 m0");
}

#[test]
fn a10_only_a_y_offset_m15() {
    let body = format!(
        "<div style=\"margin-top:40px\">{}</div>{}",
        rows(150),
        abs("top:1800px;left:50px")
    );
    let pdf = render_page(M15, "body { margin: 0 }", &body);
    assert_box(&pdf, 2, (107, 207, 847, 947), "A10 m15");
}

// --- A11: position: fixed repeats on every page (F40) --------------------------------------------------------

#[test]
fn a11_fixed_box_on_every_page_m0() {
    let fixed = format!(
        "<div style=\"{};top:50px;left:50px\"></div>",
        BOX.replace("absolute", "fixed")
    );
    let pdf = render_page(M0, BODY, &format!("{}{fixed}", rows(150)));
    let found = boxes(&pdf);
    assert_eq!(page_count(&pdf), 3);
    assert!(
        found.len() == 3
            && found
                .iter()
                .enumerate()
                .all(|(i, (n, ink))| *n == i + 1 && ink.near((50, 150, 50, 150))),
        "A11 m0: expected the box at x 50..150, y 50..150 on pages 1-3 (Chrome); got {found:?}"
    );
}

#[test]
fn a11_fixed_box_on_every_page_m15() {
    let fixed = format!(
        "<div style=\"{};top:50px;left:50px\"></div>",
        BOX.replace("absolute", "fixed")
    );
    let pdf = render_page(M15, BODY, &format!("{}{fixed}", rows(150)));
    let found = boxes(&pdf);
    assert_eq!(page_count(&pdf), 4);
    assert!(
        found.len() == 4
            && found
                .iter()
                .enumerate()
                .all(|(i, (n, ink))| *n == i + 1 && ink.near((107, 207, 107, 207))),
        "A11 m15: expected the box at x 107..207, y 107..207 on pages 1-4 (Chrome); got {found:?}"
    );
}

// --- A12: guards with a zero body margin (the frames coincide: green before and after) ----------------------

#[test]
fn a12_zero_body_margin_top_left() {
    let pdf = render_page(M0, "body { margin: 0 }", &abs("top:50px;left:50px"));
    assert_box(&pdf, 1, (50, 150, 50, 150), "G1 m0");
    let pdf = render_page(M15, "body { margin: 0 }", &abs("top:50px;left:50px"));
    assert_box(&pdf, 1, (107, 207, 107, 207), "G1 m15");
}

#[test]
fn a12_zero_body_margin_box_on_page_two() {
    let body = format!("{}{}", rows(150), abs("top:2000px;left:50px"));
    let pdf = render_page(M0, "body { margin: 0 }", &body);
    assert_box(&pdf, 2, (50, 150, 877, 977), "G8 m0");
}
