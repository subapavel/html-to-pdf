//! `<body>`'s own box (its background, border, radius and image) is painted as the box it is: as tall as its content,
//! cut at the page breaks like any fragmented box, not as tall as a page on every page (F37).
//!
//! css-break-3 section 5.4 (`box-decoration-break: slice`): a box that breaks is rendered as if it were not broken and
//! then cut, so borders and `border-radius` appear only at the real start (page 1) and the real end (last page), and a
//! background is one continuous image over the stitched box. css-break-3 section 5.2 / box model of a breaking box:
//! the box fills its fragmentainer up to the page-area bottom on every page but the last. CSS 2.1 section 10.6.3: an
//! auto-height body ends at its last in-flow child plus padding and border. css-backgrounds-3 section 2.11.2: when
//! `html` has a background of its own, body's is not propagated to the canvas and is painted on body's box (the
//! grey-page / white-card invoice pattern). Plan: docs/plans/2026-10-27-f37-body-own-background.md.
//!
//! Every case renders A4, rasterises with `pdftoppm` at 96 dpi (1 device px = 1 CSS px) and reads pixels. The numbers
//! are Chrome's (plan section 7, measured the same way); Chrome's A4 is 793.7 x 1122.56 px, rasterised to 794 x 1123,
//! and its page area at 15 mm is x 57..737, y 57..1066.

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const QUAD_SVG: &[u8] = include_bytes!("data/quad80x40.svg");

const PAGE_A4: &str = "@page { size: A4; margin: 0 }";
const PAGE_A4_15MM: &str = "@page { size: A4; margin: 15mm }";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A document of `<html>` / `<body>` with `style` in a `<style>` element (so the rules reach `html`); `page_rule` is
/// the author sheet.
fn render(page_rule: &str, style: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(page_rule)
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("quad80x40.svg", QUAD_SVG.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!("<!doctype html><html><body><style>{style}</style>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-bodybox-{}-{n}.pdf", std::process::id()));
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

fn pdfinfo(pdf: &[u8]) -> String {
    let path = temp(pdf);
    let out = Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn page_count(pdf: &[u8]) -> usize {
    pdfinfo(pdf)
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
/// Within `tol` per channel of `want`.
fn is_colour(p: [u8; 3], want: [u8; 3], tol: u8) -> bool {
    p.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= tol)
}
const WHITE: [u8; 3] = [255, 255, 255];
const GREY: [u8; 3] = [238, 238, 238];
fn is_white(p: [u8; 3]) -> bool {
    is_colour(p, WHITE, 3)
}
fn is_grey(p: [u8; 3]) -> bool {
    is_colour(p, GREY, 3)
}
/// A saturated pixel: the quarters of `quad80x40.svg`, not white, grey or text.
fn is_chroma(p: [u8; 3]) -> bool {
    p.iter().max().unwrap() - p.iter().min().unwrap() > 60
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

/// The runs of `pred` down column `x`.
fn col(page: &Page, x: usize, pred: fn([u8; 3]) -> bool) -> Vec<(usize, usize)> {
    runs((0..page.h).map(|y| pred(page.at(x, y))))
}

/// The runs of `pred` along row `y`.
fn row(page: &Page, y: usize, pred: fn([u8; 3]) -> bool) -> Vec<(usize, usize)> {
    runs((0..page.w).map(|x| pred(page.at(x, y))))
}

/// Within 1 px of `[a, b)`.
fn near(run: (usize, usize), a: usize, b: usize) -> bool {
    run.0.abs_diff(a) <= 1 && run.1.abs_diff(b) <= 1
}

/// `n` blocks, 20 px high each: a line of text with a 20 px line box, so that Chrome moves a block whole to the next
/// page like fulgur does (an empty block of fixed height is cut mid-way by Chrome).
fn blocks(n: usize) -> String {
    "<div style=\"height:20px\">x</div>".repeat(n)
}

// --- K1. a card: body ends where its content ends ----------------------------------------------------------

const CARD: &str =
    "html { background: #eee } body { background: #fff; margin: 40px; padding: 20px }";

#[test]
fn k1_a_card_body_is_as_tall_as_its_content() {
    let pdf = render(PAGE_A4, CARD, "<div style=\"height:300px\"></div>");
    assert_eq!(page_count(&pdf), 1);
    let page = raster(&pdf, 1);
    // Chrome: white x 40..754, y 40..380 (300 px content + 2 x 20 padding); html's grey around and below.
    let down = col(&page, 400, is_white);
    assert!(
        down.len() == 1 && near(down[0], 40, 380),
        "white runs down x=400: {down:?} (Chrome: 40..380)"
    );
    let across = row(&page, 200, is_white);
    assert!(
        across.len() == 1 && near(across[0], 40, 754),
        "white runs along y=200: {across:?} (Chrome: 40..754)"
    );
    for (x, y) in [(400, 390), (400, 600), (400, 1100), (20, 600), (785, 600)] {
        assert!(
            is_grey(page.at(x, y)),
            "({x}, {y}) is {:?}, want html's grey",
            page.at(x, y)
        );
    }
}

#[test]
fn k1b_a_card_body_in_the_page_area_of_15_mm() {
    let pdf = render(PAGE_A4_15MM, CARD, "<div style=\"height:300px\"></div>");
    let page = raster(&pdf, 1);
    // Chrome: the card is x 97..697 (698), y 97..437; the grey is the page area (57..737 x 57..1066) only.
    let down = col(&page, 400, is_white);
    assert!(
        down.len() == 3 && near(down[1], 97, 437),
        "white runs down x=400: {down:?} (Chrome: 0..57 page margin, 97..437 card, 1066.. page margin)"
    );
    let across = row(&page, 200, is_white);
    assert!(
        across.len() == 3 && near(across[1], 97, 698),
        "white runs along y=200: {across:?} (Chrome: 97..698 card)"
    );
    assert!(is_grey(page.at(400, 450)) && is_grey(page.at(400, 1000)));
    assert_eq!(page.at(400, 1100), WHITE, "the page margin stays white");
}

// --- K2. three pages: the frame is open at the page breaks -------------------------------------------------

#[test]
fn k2_a_bordered_body_is_open_at_the_page_breaks() {
    let pdf = render(
        PAGE_A4,
        "html { background: #eee } body { margin: 40px; border: 4px solid #c00; border-radius: 16px; \
         background: #fff; font: 16px/20px Inter }",
        &blocks(150),
    );
    assert_eq!(page_count(&pdf), 3);
    let (p1, p2, p3) = (raster(&pdf, 1), raster(&pdf, 2), raster(&pdf, 3));
    // Chrome page 1: top border y 40..44; side borders (x 40..44, 750..754) run to the page bottom; no bottom border.
    let top = col(&p1, 400, is_red);
    assert!(
        top.len() == 1 && near(top[0], 40, 44),
        "page 1: red runs down x=400: {top:?} (Chrome: the top border 40..44 only)"
    );
    for y in [100, 600, 1100, 1118] {
        assert!(
            is_red(p1.at(42, y)) && is_red(p1.at(752, y)),
            "page 1: the sides are open down to the page bottom, y={y}"
        );
    }
    assert!(
        is_white(p1.at(400, 1118)),
        "page 1: no bottom border at the page bottom"
    );
    // Chrome page 2: sides only, over the whole page; white inside.
    for y in [0, 2, 600, 1120] {
        assert!(
            is_red(p2.at(42, y)) && is_red(p2.at(752, y)),
            "page 2: the sides, y={y}"
        );
        assert!(is_white(p2.at(400, y)), "page 2: white inside, y={y}");
    }
    // Chrome page 3: sides from the top; bottom border y 820..824 (3 x ... content: 41 blocks of 20 px = 820), grey below;
    // the bottom corners are rounded (the outer corner pixel is html's grey), the top ones are not there.
    let bottom = col(&p3, 400, is_red);
    assert!(
        bottom.len() == 1 && near(bottom[0], 820, 824),
        "page 3: red runs down x=400: {bottom:?} (Chrome: the bottom border 820..824 only)"
    );
    assert!(is_white(p3.at(400, 2)), "page 3: no top border");
    assert!(is_red(p3.at(42, 2)) && is_red(p3.at(752, 400)));
    assert!(is_grey(p3.at(400, 830)) && is_grey(p3.at(42, 900)));
    assert!(
        is_grey(p3.at(41, 822)) && is_grey(p3.at(752, 823)),
        "page 3: the bottom corners are rounded: ({:?}, {:?})",
        p3.at(41, 822),
        p3.at(752, 823)
    );
    // page 1's top corners are rounded too, and its bottom is not (the box goes on).
    assert!(is_grey(p1.at(41, 41)));
    assert!(is_red(p1.at(41, 1110)), "page 1: no rounding at the break");
    assert!(is_red(p2.at(41, 3)), "page 2: no rounding at the break");
}

// --- K3. page 1 does not overshoot the page area -----------------------------------------------------------

#[test]
fn k3_a_body_frame_does_not_overshoot_the_page_area_at_15_mm() {
    let pdf = render(
        PAGE_A4_15MM,
        "body { margin: 40px; border: 3px solid #00f; font: 16px/20px Inter }",
        &blocks(130),
    );
    assert_eq!(page_count(&pdf), 3);
    let p1 = raster(&pdf, 1);
    // Chrome: the side borders (x 97..100, 694..697) run to the page-area bottom (row 1066) and not further.
    assert!(is_blue(p1.at(98, 1060)) && is_blue(p1.at(695, 1060)));
    let past: Vec<usize> = (1068..p1.h)
        .filter(|&y| (0..p1.w).any(|x| is_blue(p1.at(x, y))))
        .collect();
    assert!(
        past.is_empty(),
        "page 1: blue in rows {:?}..{:?} below the page area (bottom 1066), the frame is closed under it",
        past.first(),
        past.last()
    );
    // Chrome page 3: the frame closes at the content end, y 697..700.
    let p3 = raster(&pdf, 3);
    let bottom = col(&p3, 400, is_blue);
    assert!(
        bottom.len() == 1 && near(bottom[0], 697, 700),
        "page 3: blue runs down x=400: {bottom:?} (Chrome: the bottom border 697..700 only)"
    );
}

// --- K4. an image is positioned against body's whole box ---------------------------------------------------

#[test]
fn k4_a_bottom_right_image_sits_at_the_bottom_right_of_body() {
    let pdf = render(
        PAGE_A4,
        "html { background: #eee } body { margin: 40px; padding: 20px; \
         background: #fff url(quad80x40.svg) no-repeat 100% 100% }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster(&pdf, 1);
    // Chrome: ink x 674..754, y 340..380 (the padding box's bottom right corner).
    let x = runs((0..page.w).map(|x| (0..page.h).any(|y| is_chroma(page.at(x, y)))));
    let y = runs((0..page.h).map(|y| (0..page.w).any(|x| is_chroma(page.at(x, y)))));
    assert!(
        x.len() == 1 && near(x[0], 674, 754) && y.len() == 1 && near(y[0], 340, 380),
        "image ink x {x:?}, y {y:?} (Chrome: x 674..754, y 340..380)"
    );
}

// --- K5. a repeating image is one grid over the pages -----------------------------------------------------

#[test]
fn k5_a_repeating_body_image_continues_over_the_page_break() {
    let pdf = render(
        PAGE_A4,
        "html { background: #eee } body { margin: 0 40px; background: url(quad80x40.svg) repeat }",
        "<div style=\"height:3000px\"></div>",
    );
    assert_eq!(page_count(&pdf), 3);
    let pages: Vec<Page> = (1..=3).map(|n| raster(&pdf, n)).collect();
    // The grid starts at body's x (40), not at the page's.
    let across = row(&pages[1], 10, is_red);
    assert!(
        across.first().is_some_and(|r| r.0.abs_diff(40) <= 1),
        "page 2: the first red tile starts at x {across:?} (Chrome: 40)"
    );
    // Page k shows the box from k * 1122.56 px (body starts at the page top): the red/green seam of the first tile row
    // moves up by that much modulo the 40 px pitch. Chrome (300 dpi): 19.52 / 16.96 / 12.8 px, so 2.56 and 6.72 (nominal
    // 5.12; Chrome advances the canvas by whole pixels, F184). Today the grid restarts on every page (shift 0).
    let first_red = |p: &Page| *col(p, 50, is_red).first().expect("a red run in the column");
    let ends: Vec<usize> = pages.iter().map(|p| first_red(p).1).collect();
    for (i, p) in pages.iter().enumerate() {
        assert!(
            first_red(p).0 <= 1,
            "page {}: the red band starts at the top",
            i + 1
        );
    }
    let shift = |k: usize| ends[0] as f32 - ends[k] as f32;
    assert!(
        (shift(1) - 2.56).abs() <= 1.5,
        "page 2 phase: the seam moved up by {} px, want 2.56",
        shift(1)
    );
    assert!(
        (shift(2) - 5.12).abs() <= 2.5,
        "page 3 phase: the seam moved up by {} px, want 5.12 (Chrome 6.7)",
        shift(2)
    );
}

// --- K6. body is the canvas source and has a border --------------------------------------------------------

#[test]
fn k6_a_bordered_canvas_source_body_closes_its_border_at_the_content_end() {
    let pdf = render(
        PAGE_A4,
        "body { margin: 40px; border: 3px solid #00f; background: #ffd }",
        "<div style=\"height:300px\"></div>",
    );
    let page = raster(&pdf, 1);
    let tint = [255, 255, 221];
    // Chrome: the tint fills the page; the border is top 40..43, bottom 343..346.
    assert!(is_colour(page.at(10, 10), tint, 2) && is_colour(page.at(400, 1000), tint, 2));
    let down = col(&page, 400, is_blue);
    assert!(
        down.len() == 2 && near(down[0], 40, 43) && near(down[1], 343, 346),
        "blue runs down x=400: {down:?} (Chrome: 40..43 and 343..346)"
    );
}

// --- K7. body is the inline root ---------------------------------------------------------------------------

#[test]
fn k7_an_inline_root_body_ends_at_its_last_line() {
    // Lines are separated by `<br>`, none follows the last: fulgur lays out a trailing `<br>` as one more, empty line box
    // (Blitz) and Chrome does not, which would end body 20 px below its last line here (F37d).
    let lines = (0..200)
        .map(|i| format!("Line{i:03}"))
        .collect::<Vec<_>>()
        .join("<br>");
    let pdf = render(
        PAGE_A4,
        "html { background: #eee } body { margin: 40px; background: #fff; font: 16px/20px Inter }",
        &lines,
    );
    assert_eq!(page_count(&pdf), 4);
    // Chrome: page 1 white from 40, pages 2 and 3 white all down, page 4 white to 680 (34 lines), grey below.
    let (p1, p2, p3, p4) = (
        raster(&pdf, 1),
        raster(&pdf, 2),
        raster(&pdf, 3),
        raster(&pdf, 4),
    );
    let down = col(&p1, 400, is_white);
    assert!(
        down.len() == 1 && near(down[0], 40, 1123),
        "page 1: white runs down x=400: {down:?} (Chrome: 40..1123)"
    );
    for (n, p) in [(2, &p2), (3, &p3)] {
        let down = col(p, 400, is_white);
        assert!(
            down.len() == 1 && near(down[0], 0, 1123),
            "page {n}: white runs down x=400: {down:?} (Chrome: 0..1123)"
        );
    }
    let down = col(&p4, 400, is_white);
    assert!(
        down.len() == 1 && near(down[0], 0, 680),
        "page 4: white runs down x=400: {down:?} (Chrome: 0..680, the last line's bottom)"
    );
    assert!(is_grey(p4.at(400, 700)) && is_grey(p4.at(400, 1100)));
    // every line is printed
    let path = temp(&pdf);
    let out = Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    for i in 0..200 {
        assert_eq!(
            text.matches(&format!("Line{i:03}")).count(),
            1,
            "Line{i:03} is printed once"
        );
    }
}
