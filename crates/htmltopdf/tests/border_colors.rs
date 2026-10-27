//! Every border side is drawn in its own colour (F186).
//!
//! css-backgrounds-3 section 4.1 (`border-top-color` ... `border-left-color`: each side has its own colour, initial
//! `currentColor`), section 4.4 (the `border-color` shorthand, 1-4 values in top / right / bottom / left order) and
//! section 4.5 (`border-<side>` shorthands). Where two sides of different colour meet, the colour boundary runs from
//! the outer to the inner corner of the border (a mitre; css-backgrounds-3 section 5.2 "Corner shaping" / Chrome's
//! diagonal). Before the fix fulgur read only `border-top-color` and stroked all four sides with it: a lone
//! `border-bottom: 4px solid #c00` was black (the top colour is `currentColor`) and `border-color: a b c d` was all `a`.
//!
//! Every case renders A4 with a zero margin, rasterises with `pdftoppm` at 96 dpi (1 device px = 1 CSS px) and reads
//! pixels. The numbers are Chrome's (plan docs/plans/2026-10-27-f186-border-side-colours.md section 7, measured the
//! same way). The `g*` guards are uniform-colour borders: they pass before and after the change, and the rendered
//! bytes are additionally compared against the previous binary by hand (plan section 5, step 3).

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const PAGE_A4: &str = "@page { size: A4; margin: 0 }";

/// `border-color: top right bottom left` used by the mixed cases.
const MIXED: &str = "border-color: #c00 #0a0 #00c #fa0";
const TOP: [u8; 3] = [0xcc, 0x00, 0x00];
const RIGHT: [u8; 3] = [0x00, 0xaa, 0x00];
const BOTTOM: [u8; 3] = [0x00, 0x00, 0xcc];
const LEFT: [u8; 3] = [0xff, 0xaa, 0x00];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A document of `<html>` / `<body>` (margin 0) with `style` in a `<style>` element in front of `body`.
fn render(style: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(PAGE_A4)
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .build()
        .render(
            &format!(
                "<!doctype html><html><body style=\"margin:0\"><style>{style}</style>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-bcol-{}-{n}.pdf", std::process::id()));
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

/// Within `tol` per channel of `want`.
fn is_colour(p: [u8; 3], want: [u8; 3], tol: u8) -> bool {
    p.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= tol)
}

fn is_white(p: [u8; 3]) -> bool {
    is_colour(p, [255, 255, 255], 3)
}

/// Not (nearly) white.
fn is_ink(p: [u8; 3]) -> bool {
    !p.iter().all(|&c| c > 235)
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

/// The ink runs down column `x` / along row `y`.
fn col_ink(page: &Page, x: usize) -> Vec<(usize, usize)> {
    runs((0..page.h).map(|y| is_ink(page.at(x, y))))
}
fn row_ink(page: &Page, y: usize) -> Vec<(usize, usize)> {
    runs((0..page.w).map(|x| is_ink(page.at(x, y))))
}

/// Within 1 px of `[a, b)`.
fn near(run: (usize, usize), a: usize, b: usize) -> bool {
    run.0.abs_diff(a) <= 1 && run.1.abs_diff(b) <= 1
}

fn expect_colour(page: &Page, (x, y): (usize, usize), want: [u8; 3], what: &str) {
    let got = page.at(x, y);
    assert!(
        is_colour(got, want, 6),
        "{what}: ({x}, {y}) is {got:?}, want {want:?}"
    );
}

// --- B1. one side ------------------------------------------------------------------------------------------

#[test]
fn b1_a_lone_bottom_border_is_drawn_in_its_own_colour() {
    let pdf = render(
        "div { margin: 40px 40px 0; height: 100px; border-bottom: 4px solid #c00 }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: rows 140..143 are (204, 0, 0) at every x, rows 139 and 144 are white.
    for x in [100, 400, 752] {
        for y in 140..144 {
            expect_colour(&page, (x, y), TOP, "bottom band");
        }
        assert!(is_white(page.at(x, 139)) && is_white(page.at(x, 144)));
    }
    let down = col_ink(&page, 400);
    assert!(
        down.len() == 1 && near(down[0], 140, 144),
        "ink runs down x=400: {down:?} (Chrome: 140..144 only)"
    );
}

// --- B2. four colours --------------------------------------------------------------------------------------

#[test]
fn b2_each_side_has_its_own_colour() {
    let pdf = render(
        &format!("div {{ margin: 40px; width: 200px; height: 100px; border: 8px solid; {MIXED} }}"),
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: the box is x 40..256, y 40..156.
    expect_colour(&page, (148, 44), TOP, "top");
    expect_colour(&page, (252, 98), RIGHT, "right");
    expect_colour(&page, (148, 152), BOTTOM, "bottom");
    expect_colour(&page, (44, 98), LEFT, "left");
    assert!(is_white(page.at(148, 98)), "the inside is not painted");
}

// --- B3. corners: the colour boundary is the diagonal from the outer to the inner corner -----------------------

/// Checks the corner square of a border whose two sides are `horizontal` (the top or bottom side) and `vertical`
/// (the left or right side). `(cx, cy)` is the outer corner pixel, `(dx, dy)` the direction into the box (+1 / -1),
/// `lw` the width of the vertical side, `th` the height of the horizontal side. Pixels more than 1.5 px away from the
/// boundary line `u = (v + 0.5) * lw / th` must have the colour of their side (Chrome: exactly one anti-aliased pixel
/// lies on the line).
#[allow(clippy::too_many_arguments)]
fn check_corner(
    page: &Page,
    (cx, cy): (usize, usize),
    (dx, dy): (isize, isize),
    lw: usize,
    th: usize,
    horizontal: [u8; 3],
    vertical: [u8; 3],
    what: &str,
) {
    for v in 0..th {
        for u in 0..lw {
            let boundary = (v as f32 + 0.5) * lw as f32 / th as f32;
            let off = u as f32 + 0.5 - boundary;
            if off.abs() < 1.5 {
                continue;
            }
            // Past the boundary (larger u) is the horizontal side's triangle, short of it the vertical side's.
            let want = if off > 0.0 { horizontal } else { vertical };
            let p = (
                cx.checked_add_signed(dx * u as isize).unwrap(),
                cy.checked_add_signed(dy * v as isize).unwrap(),
            );
            let got = page.at(p.0, p.1);
            assert!(
                is_colour(got, want, 6),
                "{what}: corner pixel ({}, {}) (u={u}, v={v}) is {got:?}, want {want:?}",
                p.0,
                p.1
            );
        }
    }
}

#[test]
fn b3_the_corners_split_along_the_diagonal() {
    let pdf = render(
        &format!("div {{ margin: 40px; width: 88px; height: 48px; border: 16px solid; {MIXED} }}"),
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: box x 40..160, y 40..120; in each 16 x 16 corner square the pixels with v < u are the horizontal side's,
    // v > u the vertical side's, and the pixels with v == u are anti-aliased.
    check_corner(&page, (40, 40), (1, 1), 16, 16, TOP, LEFT, "top-left");
    check_corner(&page, (159, 40), (-1, 1), 16, 16, TOP, RIGHT, "top-right");
    check_corner(
        &page,
        (40, 119),
        (1, -1),
        16,
        16,
        BOTTOM,
        LEFT,
        "bottom-left",
    );
    check_corner(
        &page,
        (159, 119),
        (-1, -1),
        16,
        16,
        BOTTOM,
        RIGHT,
        "bottom-right",
    );
}

#[test]
fn b3b_unequal_widths_split_along_the_outer_to_inner_corner_line() {
    let pdf = render(
        &format!(
            "div {{ margin: 40px; width: 88px; height: 48px; border-style: solid; \
             border-width: 10px 4px 16px 8px; {MIXED} }}"
        ),
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: box x 40..140, y 40..114. Top-left: the boundary runs from (40, 40) to (48, 50), e.g. row 45 is
    // `LLLL.TTT`; bottom-left: from (40, 114) to (48, 98).
    check_corner(&page, (40, 40), (1, 1), 8, 10, TOP, LEFT, "top-left");
    check_corner(&page, (139, 40), (-1, 1), 4, 10, TOP, RIGHT, "top-right");
    check_corner(
        &page,
        (40, 113),
        (1, -1),
        8,
        16,
        BOTTOM,
        LEFT,
        "bottom-left",
    );
    check_corner(
        &page,
        (139, 113),
        (-1, -1),
        4,
        16,
        BOTTOM,
        RIGHT,
        "bottom-right",
    );
}

#[test]
fn b3c_mixed_colours_with_a_radius_keep_each_side_in_its_colour() {
    let pdf = render(
        &format!(
            "div {{ margin: 40px; width: 200px; height: 100px; border-style: solid; \
             border-width: 10px 4px 16px 8px; {MIXED}; border-radius: 12px }}"
        ),
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: box x 40..252, y 40..166; the middle of every side is its own colour (the corners follow the curve).
    expect_colour(&page, (146, 44), TOP, "top");
    expect_colour(&page, (250, 100), RIGHT, "right");
    expect_colour(&page, (146, 160), BOTTOM, "bottom");
    expect_colour(&page, (43, 100), LEFT, "left");
    assert!(is_white(page.at(146, 100)), "the inside is not painted");
}

// --- B4. table rows ----------------------------------------------------------------------------------------

#[test]
fn b3d_a_translucent_corner_is_painted_once() {
    // F186 vrt-validator: two sides of the same translucent colour next to a side of another colour overlapped in the
    // corner square and darkened it. Box at (0, 0), 20 px borders: the top-left corner square is (0..20, 0..20).
    for radius in ["", "border-radius: 20px;"] {
        let pdf = render(
            &format!(
                "div {{ width: 100px; height: 60px; border: 20px solid rgba(255, 0, 0, .5); \
                 border-bottom-color: rgba(0, 0, 255, .5); {radius} }}"
            ),
            "<div></div>",
        );
        let page = raster(&pdf);
        let side = page.at(70, 10);
        // Inside the corner square, on the top side's half of the diagonal (and inside the ring with a radius).
        let corner = page.at(14, 8);
        assert!(
            is_colour(corner, side, 6),
            "{radius:?}: the corner is {corner:?}, the top side {side:?} (a darker corner is painted twice)"
        );
    }
}

fn table_separators(border_collapse: &str) {
    let pdf = render(
        &format!(
            "table {{ margin: 40px; width: 300px; border-collapse: {border_collapse}; border-spacing: 0 }} \
             td {{ height: 29px; padding: 0; border-bottom: 1px solid #e5e7eb }}"
        ),
        "<table><tr><td></td></tr><tr><td></td></tr><tr><td></td></tr></table>",
    );
    let page = raster(&pdf);
    // Chrome (separate and collapse alike): one row of (229, 231, 235) at y 69, 99 and 129, nothing else down x=200.
    let down = col_ink(&page, 200);
    assert!(
        down.len() == 3
            && near(down[0], 69, 70)
            && near(down[1], 99, 100)
            && near(down[2], 129, 130),
        "{border_collapse}: ink runs down x=200: {down:?} (Chrome: 69..70, 99..100, 129..130)"
    );
    for (a, b) in down {
        for y in a..b {
            let p = page.at(200, y);
            assert!(
                p.iter().all(|&c| c >= 200),
                "{border_collapse}: separator pixel (200, {y}) is {p:?}, want light grey (229, 231, 235)"
            );
        }
    }
    expect_colour(&page, (200, 69), [229, 231, 235], border_collapse);
}

#[test]
fn b4_row_separators_are_light_grey_in_a_separate_table() {
    table_separators("separate");
}

#[test]
fn b4b_row_separators_are_light_grey_in_a_collapsed_table() {
    table_separators("collapse");
}

// --- B5. an accent on one side, with and without a radius ---------------------------------------------------

#[test]
fn b5_a_left_accent_with_a_radius_draws_only_the_left_band() {
    for (name, radius) in [("radius 8px", "; border-radius: 8px"), ("no radius", "")] {
        let pdf = render(
            &format!(
                "div {{ margin: 40px; width: 200px; height: 100px; border-left: 4px solid #2563eb{radius} }}"
            ),
            "<div></div>",
        );
        let page = raster(&pdf);
        // Chrome: (42, 90) is (37, 99, 235); the top, right and bottom sides and the inside stay white.
        expect_colour(&page, (42, 90), [0x25, 0x63, 0xeb], name);
        assert!(is_white(page.at(39, 90)), "{name}: outside the band");
        for (x, y) in [(140, 41), (237, 90), (140, 138), (100, 90)] {
            assert!(
                is_white(page.at(x, y)),
                "{name}: ({x}, {y}) is {:?}, want white",
                page.at(x, y)
            );
        }
    }
}

// --- G. guards: one colour on all sides keeps its look ---------------------------------------------------------

#[test]
fn g1_a_uniform_solid_border() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 6px solid #c00 }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: red x 40..46 and 246..252 along y=98, y 40..46 and 146..152 down x=140.
    let across = row_ink(&page, 98);
    assert!(
        across.len() == 2 && near(across[0], 40, 46) && near(across[1], 246, 252),
        "{across:?}"
    );
    let down = col_ink(&page, 140);
    assert!(
        down.len() == 2 && near(down[0], 40, 46) && near(down[1], 146, 152),
        "{down:?}"
    );
    expect_colour(&page, (41, 41), TOP, "corner");
}

#[test]
fn g2_a_uniform_solid_border_with_a_radius() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 6px solid #c00; border-radius: 12px }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: the corner pixel (40, 40) is outside the curve (white); the sides are red.
    assert!(is_white(page.at(40, 40)) && is_white(page.at(41, 41)));
    expect_colour(&page, (140, 43), TOP, "top");
    expect_colour(&page, (43, 98), TOP, "left");
    expect_colour(&page, (140, 148), TOP, "bottom");
    let across = row_ink(&page, 98);
    assert!(
        across.len() == 2 && near(across[0], 40, 46) && near(across[1], 246, 252),
        "{across:?}"
    );
}

#[test]
fn g3_a_dashed_border_has_dashes() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 4px dashed #00c }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome draws 18 dashes of 9 px with 3 px gaps along the top; fulgur's dash length differs (9 dashes of 12 px,
    // unchanged by F186). The guard is that the border is still dashed.
    let dashes = row_ink(&page, 42);
    assert!(
        (6..=21).contains(&dashes.len()),
        "dashes along y=42: {dashes:?} (fulgur: 9, Chrome: 18)"
    );
    expect_colour(&page, (45, 42), BOTTOM, "dash");
    assert!(
        is_white(page.at(140, 148)),
        "no band below the box's inside"
    );
}

#[test]
fn g4_a_dotted_border_has_dots() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 4px dotted #00c }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: 27 dots along the top.
    let dots = row_ink(&page, 42);
    assert!(
        (20..=34).contains(&dots.len()),
        "dots along y=42: {dots:?} (Chrome: 27)"
    );
}

#[test]
fn g5_a_double_border_is_two_lines() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 9px double #c00 }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: along y=98 red x 40..43 and 46..49 (left), 249..252 and 255..258 (right).
    let across = row_ink(&page, 98);
    assert!(
        across.len() == 4
            && near(across[0], 40, 43)
            && near(across[1], 46, 49)
            && near(across[2], 249, 252)
            && near(across[3], 255, 258),
        "{across:?}"
    );
}

#[test]
fn g6_a_uniform_colour_with_unequal_widths() {
    let pdf = render(
        "div { margin: 40px; width: 200px; height: 100px; border: 2px solid #c00; border-bottom-width: 8px }",
        "<div></div>",
    );
    let page = raster(&pdf);
    // Chrome: down x=140 red y 40..42 and 141..150 (the 8 px bottom band).
    let down = col_ink(&page, 140);
    assert!(
        down.len() == 2 && near(down[0], 40, 42) && near(down[1], 142, 150),
        "{down:?} (Chrome: 40..42, 142..150)"
    );
}
