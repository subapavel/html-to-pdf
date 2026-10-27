//! The background of `html` (or, when `html` has none, of `body`) is the background of the canvas (F184).
//!
//! css-backgrounds-3 section 2.11.1 / 2.11.2: the root element's background (or the first `body` child's, when the
//! root has no image and a transparent colour) is painted over the whole canvas, while images are sized and positioned
//! against the root element as if painted for it alone. In paged media the canvas of a page is its page area (the
//! page box's margin area stays white), and the root is one fragmented box, so a repeating image is continuous across
//! pages. fulgur painted `html` and `body` as ordinary boxes on every page: an image sat at the body's box, a tile grid
//! restarted at every page top, and a colour left strips. Plan: docs/plans/2026-10-27-f184-canvas-background.md.
//!
//! Every case renders an A4 page, rasterises it with `pdftoppm` (96 dpi unless a case says otherwise: 1 device px =
//! 1 CSS px) and measures ink. The numbers are Chrome's (plan section 7), measured the same way; Chrome's A4 is
//! 793.7 x 1122.56 px, rasterised to 794 x 1123.

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const QUAD_SVG: &[u8] = include_bytes!("data/quad80x40.svg");
const QUAD_PNG: &[u8] = include_bytes!("data/quad40x20.png");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// `css` is the author sheet (the renderer's), `body` the content of `<body>`.
fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(css)
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("quad80x40.svg", QUAD_SVG.to_vec())
        .image("quad40x20.png", QUAD_PNG.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

/// A document of `<html>` / `<body>` with `style` in a `<style>` element (so the rules reach `html`).
fn render_page(page_rule: &str, style: &str, body: &str) -> Vec<u8> {
    render(page_rule, &format!("<style>{style}</style>{body}"))
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-canvas-{}-{n}.pdf", std::process::id()));
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

/// Page `n` (1-based) at `dpi`.
fn raster_at(pdf: &[u8], n: usize, dpi: usize) -> Page {
    let path = temp(pdf);
    let out = Command::new("pdftoppm")
        .args([
            "-r",
            &dpi.to_string(),
            "-f",
            &n.to_string(),
            "-l",
            &n.to_string(),
        ])
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

fn raster(pdf: &[u8], n: usize) -> Page {
    raster_at(pdf, n, 96)
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

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 80 && p[1] < 80
}
fn is_ink(p: [u8; 3]) -> bool {
    p.iter().any(|&c| c < 250)
}
/// Within `tol` per channel of `want`.
fn is_colour(p: [u8; 3], want: [u8; 3], tol: u8) -> bool {
    p.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= tol)
}

/// The bounding box `[x0, x1) x [y0, y1)` of the pixels `pred` accepts.
#[derive(Debug, PartialEq)]
struct Ink {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

impl Ink {
    /// Within 1 px of `[x0, x1) x [y0, y1)`.
    fn near(&self, x0: usize, x1: usize, y0: usize, y1: usize) -> bool {
        let close = |a: usize, b: usize| a.abs_diff(b) <= 1;
        close(self.x0, x0) && close(self.x1, x1) && close(self.y0, y0) && close(self.y1, y1)
    }
}

fn bbox(page: &Page, pred: fn([u8; 3]) -> bool) -> Option<Ink> {
    let mut b: Option<Ink> = None;
    for y in 0..page.h {
        for x in 0..page.w {
            if pred(page.at(x, y)) {
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

/// Runs `[start, end)` of consecutive `true` in `line`.
fn runs(line: impl Iterator<Item = bool>) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    let mut n = 0;
    for (i, v) in line.enumerate() {
        n = i + 1;
        match (v, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, n));
    }
    out
}

const PAGE_A4: &str = "@page { size: A4; margin: 0 }";
const PAGE_A4_15MM: &str = "@page { size: A4; margin: 15mm }";

/// The page area of A4 with 15 mm margins, in CSS px (Chrome: x 57..737, y 57..1066).
const AREA: (usize, usize, usize, usize) = (57, 737, 57, 1066);

// --- A. `07-body`: the image of body's background sits at the root origin ------------------------------

#[test]
fn a_body_image_is_positioned_against_the_root_not_the_body() {
    let pdf = render_page(
        PAGE_A4,
        "body { margin: 20px; background: url(quad80x40.svg) no-repeat 0 0 }",
        "<div style=\"height:300px;margin:30px\"></div>",
    );
    let page = raster(&pdf, 1);
    let ink = bbox(&page, is_ink).expect("the image is drawn");
    assert!(
        ink.near(0, 80, 0, 40),
        "the image is at the root origin (0, 0), got {ink:?} (Chrome: x 0..80, y 0..40)"
    );
    assert!(is_red(page.at(10, 10)) && is_blue(page.at(50, 10)));
}

// --- B. `07b-html`: html tiles cover the whole page ----------------------------------------------------

#[test]
fn b_html_tiles_cover_the_whole_page() {
    let pdf = render_page(
        PAGE_A4,
        "html { background: url(quad80x40.svg) repeat 0 0 / auto } \
         body { margin: 20px; background: url(quad40x20.png) no-repeat 100% 100% }",
        "<div style=\"height:300px;margin:30px\"></div>",
    );
    assert_eq!(page_count(&pdf), 1);
    let page = raster(&pdf, 1);
    // Every pixel of rows 0..1122 is a tile pixel (red, blue, green or yellow; none is white). The html box is only
    // 360 px tall: the tiles stopped there.
    let mut bare = 0;
    for y in 0..1122 {
        for x in 0..page.w {
            if !is_ink(page.at(x, y)) {
                bare += 1;
            }
        }
    }
    assert!(
        bare < 794 * 1122 / 500,
        "{bare} of {} px are not covered by a tile",
        794 * 1122
    );
    let first_bare_row = (0..1122).find(|&y| (0..page.w).all(|x| !is_ink(page.at(x, y))));
    assert_eq!(first_bare_row, None, "a page row without any tile");
}

// --- C. `07c`: a repeating body image is continuous across pages -------------------------------------

/// `[start, end)` of the first red run down column `x` (inside the red 40 x 20 quarter of a tile).
fn first_red_run(page: &Page, x: usize) -> (usize, usize) {
    *runs((0..page.h).map(|y| is_red(page.at(x, y))))
        .first()
        .expect("a red run in the column")
}

#[test]
fn c_repeating_image_continues_over_the_page_break() {
    let pdf = render_page(
        PAGE_A4,
        "body { background: url(quad80x40.svg) repeat 0 0; margin: 20px }",
        "<div style=\"height:2500px;margin:30px\"></div>",
    );
    assert_eq!(page_count(&pdf), 3);
    let pages: Vec<Page> = (1..=3).map(|n| raster(&pdf, n)).collect();
    // The grid starts at x 0 on every page (not at the body's x 20).
    for (i, p) in pages.iter().enumerate() {
        let row = runs((0..p.w).map(|x| is_red(p.at(x, 10))));
        let first = row
            .first()
            .unwrap_or_else(|| panic!("page {}: no red in row 10", i + 1));
        assert!(
            first.0 <= 1,
            "page {}: tiles start at x {} (want 0)",
            i + 1,
            first.0
        );
    }
    // Page k shows the root from k * 1122.56 px (the page height): the red/green seam of the first tile row moves up
    // by that much modulo the 40 px pitch. Chrome: -2.6 px on page 2, -6.7 on page 3 (nominal -5.1; Chrome's pages
    // jitter by up to 1.6 px), measured on the 300 dpi raster.
    let ends: Vec<usize> = pages.iter().map(|p| first_red_run(p, 10).1).collect();
    for (i, p) in pages.iter().enumerate() {
        assert!(
            first_red_run(p, 10).0 <= 1,
            "page {}: the red band starts at the top",
            i + 1
        );
    }
    let shift = |k: usize| ends[0] as f32 - ends[k] as f32;
    assert!(
        (shift(1) - 2.56).abs() <= 1.0,
        "page 2 phase: the seam moved up by {} px, want 2.56",
        shift(1)
    );
    assert!(
        (shift(2) - 5.12).abs() <= 2.0,
        "page 3 phase: the seam moved up by {} px, want 5.12 (Chrome 6.7)",
        shift(2)
    );
}

// --- D. `07d`: a no-repeat 50% 50% image is drawn once, centred on the whole root -------------------

#[test]
fn d_centred_image_is_drawn_once_on_the_page_that_holds_the_root_centre() {
    let pdf = render_page(
        PAGE_A4,
        "body { background: url(quad80x40.svg) no-repeat 50% 50% / auto; margin: 20px }",
        "<div style=\"height:2500px;margin:30px\"></div>",
    );
    assert_eq!(page_count(&pdf), 3);
    // The root is 2560 px tall: the image is at y 1260, i.e. 137 px down page 2.
    assert!(
        bbox(&raster(&pdf, 1), is_ink).is_none(),
        "page 1 has no ink"
    );
    let ink = bbox(&raster(&pdf, 2), is_ink).expect("the image is on page 2");
    assert!(
        ink.near(357, 437, 137, 177),
        "got {ink:?} (Chrome: x 357..437, y 137..177)"
    );
    assert!(
        bbox(&raster(&pdf, 3), is_ink).is_none(),
        "page 3 has no ink"
    );
}

// --- E. a body colour fills the page area, not the page margin --------------------------------------

#[test]
fn e_body_colour_fills_the_page_area() {
    let pdf = render_page(
        PAGE_A4_15MM,
        "body { margin: 40px; background: #ffd }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster(&pdf, 1);
    let tint = [255, 255, 221];
    let (x0, x1, y0, y1) = AREA;
    // outside the body's box (x 97..697, y 97..397), inside the page area
    for (x, y) in [
        (x0 + 3, y0 + 3),
        (x1 - 4, y0 + 3),
        (x0 + 3, y1 - 4),
        (x1 - 4, y1 - 4),
        (400, 800),
    ] {
        assert!(
            is_colour(page.at(x, y), tint, 2),
            "({x}, {y}) is {:?}, want the body colour",
            page.at(x, y)
        );
    }
    // the page margin stays white
    for (x, y) in [
        (20, 20),
        (770, 20),
        (400, 1100),
        (20, 600),
        (770, 600),
        (400, 30),
    ] {
        assert_eq!(
            page.at(x, y),
            [255, 255, 255],
            "({x}, {y}) is in the page margin"
        );
    }
    let ink = bbox(&page, is_ink).unwrap();
    assert!(
        ink.near(x0, x1, y0, y1),
        "tinted area {ink:?}, Chrome {AREA:?}"
    );
}

// --- F. no unpainted strip at the right / bottom edge -----------------------------------------------

#[test]
fn f_html_colour_reaches_the_page_edge() {
    let pdf = render_page(
        PAGE_A4,
        "html { background: #00f } body { margin: 0 }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster_at(&pdf, 1, 300);
    // The last column is the 0.3 px sliver poppler adds to round the 595.28 pt page up to 2481 px: not asserted.
    for x in [0, 1, page.w - 3, page.w - 2] {
        for y in [0, 700, page.h - 1] {
            assert!(
                is_blue(page.at(x, y)),
                "({x}, {y}) of {}x{} is {:?}",
                page.w,
                page.h,
                page.at(x, y)
            );
        }
    }
    for y in [page.h - 2, page.h - 1] {
        assert!(
            is_blue(page.at(900, y)),
            "row {y} of {} is {:?}",
            page.h,
            page.at(900, y)
        );
    }
}

// --- F2. the root's opacity does not apply to the canvas ------------------------------------------

#[test]
fn f2_root_opacity_does_not_fade_the_canvas() {
    // Chrome paints the canvas at full colour under `html { opacity: .5 }` (F184 vrt-validator): the canvas is not
    // part of the root's stacking context. Pixel (400, 600) is (0, 102, 204) in Chrome.
    for style in [
        "html { opacity: .5; background: #06c } body { margin: 0 }",
        "html { opacity: .5 } body { margin: 0; background: #06c }",
    ] {
        let pdf = render_page(PAGE_A4, style, "<div style=\"height:300px\"></div>");
        let page = raster(&pdf, 1);
        assert!(
            is_colour(page.at(400, 600), [0, 102, 204], 3),
            "{style}: (400, 600) is {:?}",
            page.at(400, 600)
        );
    }
}

// --- G. a gradient is sized to the root box and repeats down the canvas -------------------------

#[test]
fn g_gradient_repeats_every_root_height() {
    let pdf = render_page(
        PAGE_A4,
        "body { margin: 0; background: linear-gradient(#f00, #00f) }",
        "<div style=\"height:200px\"></div>",
    );
    let page = raster(&pdf, 1);
    // The gradient runs red -> blue over 200 px; it restarts (red jumps up) at 200, 400, ... 1000.
    let x = 400;
    let restarts: Vec<usize> = (1..page.h)
        .filter(|&y| page.at(x, y)[0] > page.at(x, y - 1)[0].saturating_add(100))
        .collect();
    let want = [200, 400, 600, 800, 1000];
    assert!(
        want.iter()
            .all(|&w| restarts.iter().any(|&r| r.abs_diff(w) <= 1)),
        "restarts at {restarts:?}, want {want:?} (Chrome: 199/200, 399, 599, 800, 1000)"
    );
    // at the end of the page the last period is cut mid-way, not painted blue/white
    assert!(
        page.at(x, 1100)[0] > 20 && page.at(x, 1100)[2] > 20,
        "{:?}",
        page.at(x, 1100)
    );
}

// --- H. guards: green before and after ----------------------------------------------------------------

#[test]
fn h_html_and_body_colours_keep_their_own_areas() {
    let pdf = render_page(
        PAGE_A4,
        "html { background: #fdd } body { margin: 40px; background: #dfd }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster(&pdf, 1);
    let html = [255, 221, 221];
    let body = [221, 255, 221];
    // html's colour around the body box, body's colour inside it only (x 40..754, y 40..340)
    assert!(is_colour(page.at(10, 10), html, 2));
    assert!(
        is_colour(page.at(20, 200), html, 2),
        "the ring left of body is html's"
    );
    assert!(
        is_colour(page.at(400, 200), body, 2),
        "body's own box is body's"
    );
    assert!(is_colour(page.at(400, 30), html, 2), "above body is html's");
}

#[test]
fn h2_html_colour_fills_the_page_beside_and_below_the_body_box() {
    // Red before F184: the html box ended at the end of its content. Body's own box below its content is not
    // asserted: its height is the content area's (F37).
    let pdf = render_page(
        PAGE_A4,
        "html { background: #fdd } body { margin: 40px; background: #dfd }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster(&pdf, 1);
    let html = [255, 221, 221];
    for (x, y) in [(10, 600), (10, 1100), (785, 1100), (785, 600)] {
        assert!(
            is_colour(page.at(x, y), html, 2),
            "({x}, {y}) is {:?}, want html's colour",
            page.at(x, y)
        );
    }
    // body's colour stays on its own box
    assert!(is_colour(page.at(400, 200), [221, 255, 221], 2));
}

#[test]
fn h3_running_footer_stays_visible_over_a_body_colour() {
    let pdf = render(
        "@page { size: A4; margin: 15mm; @bottom-center { content: element(foot) } } \
         .foot { position: running(foot); font: 16px Inter; color: #000 } \
         body { background: #ffd; font: 16px Inter }",
        "<div class=\"foot\">FOOTERTXT</div><p>Dobrý den</p>",
    );
    let path = temp(&pdf);
    let out = Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(
        text.matches("FOOTERTXT").count(),
        1,
        "the footer is printed once: {text:?}"
    );
    // and the glyphs are dark ink in the bottom margin band, below the tinted page area
    let page = raster(&pdf, 1);
    let dark = (1066..page.h)
        .flat_map(|y| (0..page.w).map(move |x| (x, y)))
        .filter(|&(x, y)| page.at(x, y).iter().all(|&c| c < 100))
        .count();
    assert!(dark > 20, "{dark} dark px in the bottom margin band");
}
