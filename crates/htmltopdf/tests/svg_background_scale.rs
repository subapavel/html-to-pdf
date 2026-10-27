//! A `background-image` is drawn at its natural size, not at 4/3 of it (F180, F23).
//!
//! The natural size of an SVG (usvg user units = CSS px, css-images-3 §5.1) and of a raster (one image pixel = one CSS
//! px at 1 dppx, css-images-4 §6.1) was stored as a bare number and handed to the PDF side, which reads pt: a 300 x 60
//! SVG came out 400 x 80 px and a 40 px PNG tiled at a 53.3 px pitch. `<img>` (sized by layout) and
//! `list-style-image` were right. Plan: docs/plans/2026-10-27-f180-svg-background-scale.md.
//!
//! Every case renders one block at (30, 20) px with the body margin 0, rasterises page 1 with `pdftoppm` at 96 dpi
//! (1 device px = 1 CSS px) and measures the red and blue ink inside the block, relative to the block's corner. The
//! numbers asserted are Chrome's (plan section 7), measured the same way: `shape.svg` is 300 x 60 with a red 40 px
//! square at x 10 and a blue one at x 200, both at y 10.

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const SHAPE: &[u8] = include_bytes!("data/shape.svg");
const MARKER: &[u8] = include_bytes!("data/marker.svg");
const RED_PNG: &[u8] = include_bytes!("data/red16.png");

/// Where the block under test starts on the page, in CSS px.
const OX: usize = 30;
const OY: usize = 20;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .css("@page { size: A4; margin: 0 } body { margin: 0; font: 16px/1.2 Inter }")
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("shape.svg", SHAPE.to_vec())
        .image("marker.svg", MARKER.to_vec())
        .image("red16.png", RED_PNG.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

/// The block under test: `w` x `h` at (OX, OY) with `background` given.
fn block(w: usize, h: usize, background: &str) -> String {
    format!(
        "<div style=\"margin:{OY}px 0 0 {OX}px;width:{w}px;height:{h}px;background:{background}\"></div>"
    )
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-bgscale-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

struct Page {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

/// Page 1 at 96 dpi.
fn raster(pdf: &[u8]) -> Page {
    let path = temp(pdf);
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", "1", "-l", "1"])
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

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 80 && p[1] < 80
}

/// A rectangle of ink, `[x0, x1) x [y0, y1)`, relative to the block's corner.
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

/// The ink of `pred` inside the `w` x `h` block at `(ox, oy)`, split into groups of adjacent columns (tiles of one colour that
/// touch merge into one group).
fn ink(
    page: &Page,
    (ox, oy): (usize, usize),
    w: usize,
    h: usize,
    pred: fn([u8; 3]) -> bool,
) -> Vec<Ink> {
    let mut groups: Vec<Ink> = Vec::new();
    let mut last_col: Option<usize> = None;
    for x in 0..w {
        let (mut y0, mut y1) = (usize::MAX, 0);
        for y in 0..h {
            let (px, py) = (ox + x, oy + y);
            if px >= page.w || py >= page.h {
                continue;
            }
            let i = (py * page.w + px) * 3;
            if pred([page.rgb[i], page.rgb[i + 1], page.rgb[i + 2]]) {
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
        if y1 == 0 {
            last_col = None;
            continue;
        }
        match (last_col, groups.last_mut()) {
            (Some(c), Some(g)) if c + 1 == x => {
                g.x1 = x + 1;
                g.y0 = g.y0.min(y0);
                g.y1 = g.y1.max(y1);
            }
            _ => groups.push(Ink {
                x0: x,
                x1: x + 1,
                y0,
                y1,
            }),
        }
        last_col = Some(x);
    }
    groups
}

fn red(page: &Page, w: usize, h: usize) -> Vec<Ink> {
    ink(page, (OX, OY), w, h, is_red)
}
fn blue(page: &Page, w: usize, h: usize) -> Vec<Ink> {
    ink(page, (OX, OY), w, h, is_blue)
}

/// Asserts the groups are exactly `want` (each `x0, x1, y0, y1`), every edge within 1 px.
#[track_caller]
fn assert_ink(what: &str, got: &[Ink], want: &[(usize, usize, usize, usize)]) {
    let ok = got.len() == want.len()
        && got
            .iter()
            .zip(want)
            .all(|(g, &(x0, x1, y0, y1))| g.near(x0, x1, y0, y1));
    assert!(ok, "{what}: got {got:?}, want {want:?} (x0, x1, y0, y1)");
}

// --- 1. auto, no-repeat ---------------------------------------------------------------------------------

#[test]
fn auto_size_is_the_natural_size() {
    let page = raster(&render(&block(300, 60, "url(shape.svg) no-repeat")));
    assert_ink("red", &red(&page, 300, 60), &[(10, 50, 10, 50)]);
    assert_ink("blue", &blue(&page, 300, 60), &[(200, 240, 10, 50)]);
}

#[test]
fn auto_size_equals_the_img_element() {
    let bg = raster(&render(&block(300, 60, "url(shape.svg) no-repeat")));
    let img = raster(&render(&format!(
        "<img src=\"shape.svg\" style=\"display:block;margin:{OY}px 0 0 {OX}px\">"
    )));
    for (what, a, b) in [
        ("red", red(&bg, 300, 60), red(&img, 300, 60)),
        ("blue", blue(&bg, 300, 60), blue(&img, 300, 60)),
    ] {
        assert_eq!(a.len(), 1, "{what}: background {a:?}");
        assert_eq!(b.len(), 1, "{what}: img {b:?}");
        assert!(
            a[0].near(b[0].x0, b[0].x1, b[0].y0, b[0].y1),
            "{what}: background {a:?} differs from <img> {b:?}"
        );
    }
}

// --- 2. repeat ------------------------------------------------------------------------------------------

#[test]
fn tiles_repeat_at_the_natural_size() {
    let page = raster(&render(&block(700, 60, "url(shape.svg)")));
    assert_ink(
        "red",
        &red(&page, 700, 60),
        &[(10, 50, 10, 50), (310, 350, 10, 50), (610, 650, 10, 50)],
    );
    assert_ink(
        "blue",
        &blue(&page, 700, 60),
        &[(200, 240, 10, 50), (500, 540, 10, 50)],
    );
}

// --- 3. position ----------------------------------------------------------------------------------------

#[test]
fn percentage_position_aligns_the_natural_size() {
    // (400 - 300) * 100% = 100 px
    let page = raster(&render(&block(400, 60, "url(shape.svg) no-repeat 100% 0")));
    assert_ink("red", &red(&page, 400, 60), &[(110, 150, 10, 50)]);
    assert_ink("blue", &blue(&page, 400, 60), &[(300, 340, 10, 50)]);
}

#[test]
fn length_position_offsets_the_natural_size() {
    let page = raster(&render(&block(
        400,
        60,
        "url(shape.svg) no-repeat 20px 5px",
    )));
    assert_ink("red", &red(&page, 400, 60), &[(30, 70, 15, 55)]);
    assert_ink("blue", &blue(&page, 400, 60), &[(220, 260, 15, 55)]);
}

// --- 4. explicit sizes (guard: right before the fix) ----------------------------------------------------

#[test]
fn explicit_sizes_scale_the_image() {
    for size in ["150px 30px", "150px auto"] {
        let page = raster(&render(&block(
            300,
            60,
            &format!("url(shape.svg) no-repeat 0 0 / {size}"),
        )));
        assert_ink(size, &red(&page, 300, 60), &[(5, 25, 5, 25)]);
        assert_ink(size, &blue(&page, 300, 60), &[(100, 120, 5, 25)]);
    }
}

// --- 5. contain / cover (guard) -------------------------------------------------------------------------

#[test]
fn contain_and_cover() {
    let page = raster(&render(&block(
        600,
        60,
        "url(shape.svg) no-repeat 0 0 / contain",
    )));
    assert_ink("contain red", &red(&page, 600, 60), &[(10, 50, 10, 50)]);
    // cover: scale 2 -> 600 x 120, clipped by the box at y 60
    let page = raster(&render(&block(
        600,
        60,
        "url(shape.svg) no-repeat 0 0 / cover",
    )));
    assert_ink("cover red", &red(&page, 600, 60), &[(20, 100, 20, 60)]);
    assert_ink("cover blue", &blue(&page, 600, 60), &[(400, 480, 20, 60)]);
}

// --- 6. list-style-image (guard) ------------------------------------------------------------------------

#[test]
fn list_marker_image_keeps_its_natural_size() {
    let page = raster(&render(
        "<ul style=\"margin:0;padding:0 0 0 60px;list-style-image:url(marker.svg)\"><li>Item</li></ul>",
    ));
    // the marker is the only red on the page; measured over the whole page, from its corner
    let marks = ink(&page, (0, 0), page.w, page.h, is_red);
    assert_eq!(marks.len(), 1, "one marker: {marks:?}");
    let m = &marks[0];
    assert!(
        (m.x1 - m.x0).abs_diff(12) <= 1 && (m.y1 - m.y0).abs_diff(12) <= 1,
        "marker ink is 12 x 12 px, got {m:?}"
    );
    assert!(m.x1 <= 60, "the marker is left of the item text: {m:?}");
}

// --- 7. raster (F23) ------------------------------------------------------------------------------------

#[test]
fn raster_background_keeps_its_natural_size() {
    // red16.png is 16 x 16 pixels = 16 x 16 CSS px
    let page = raster(&render(&block(100, 40, "url(red16.png) no-repeat")));
    assert_ink("top left", &red(&page, 100, 40), &[(0, 16, 0, 16)]);
    let page = raster(&render(&block(100, 40, "url(red16.png) no-repeat 100% 0")));
    assert_ink("top right", &red(&page, 100, 40), &[(84, 100, 0, 16)]);
}
