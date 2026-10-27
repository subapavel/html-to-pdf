//! An inline `<img>` / `<svg>` sits on the baseline of a line box that keeps its strut (F183).
//!
//! CSS 2.1 section 10.8.1: every line box starts with a strut, a zero-width inline box with the font and line height
//! of its block, and a replaced inline element with `vertical-align: baseline` puts its bottom margin edge on the
//! baseline. The strut's descent plus half-leading therefore stays below the image: a line holding only a 60 px image
//! is about 64 px tall (16 px Inter), not 60. Parley 0.6 counted an inline box only as ascent and gave a line the
//! height of its tallest box, so the next block (or the border of the cell, the rule under a header) sat 4 px too high,
//! and on a line with text the image started above the paragraph top. Plan: docs/plans/2026-10-27-f183-inline-img-strut.md.
//!
//! Every case renders an A4 page with `@page { margin: 0 }`, `body { margin: 0; font: 16px Inter }`, rasterises it with
//! `pdftoppm` at 96 dpi (1 device px = 1 CSS px) and measures the rows of the red image (`#f00`), the blue block
//! (`#00f`, 20 px) and black ink. The numbers are Chrome's (plan section 7.1), measured the same way. The `s*` cases
//! are red before the fix, the `g*` cases are guards that hold before and after.

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const RED60: &[u8] = include_bytes!("data/red60.png");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The style every case starts from (the Chrome probe uses the same sheet).
const BASE: &str = "@page { size: A4; margin: 0 } body { margin: 0; font: 16px Inter } .blk { height: 20px; background: #00f }";

fn renderer(tailwind: bool) -> Renderer {
    let mut b = Renderer::builder()
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("red60.png", RED60.to_vec())
        .strict_images(true);
    if tailwind {
        b = b
            .css_file(root().join("fixtures/tailwind/tailwind.css"))
            .unwrap();
    }
    b.build()
}

fn render_with(tailwind: bool, extra_css: &str, body: &str) -> Vec<u8> {
    renderer(tailwind)
        .render(
            &format!(
                "<!doctype html><html><head><style>{BASE} {extra_css}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

fn img(w: u32, h: u32, style: &str) -> String {
    let style = if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{style}\"")
    };
    format!("<img src=\"red60.png\" width=\"{w}\" height=\"{h}\"{style}>")
}

fn img60() -> String {
    img(60, 60, "")
}

const BLK: &str = "<div class=\"blk\"></div>";

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-strut-{}-{n}.pdf", std::process::id()));
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
    raster_page(pdf, 1)
}

/// Page `n` (1-based) at 96 dpi.
fn raster_page(pdf: &[u8], n: usize) -> Page {
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

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 80 && p[1] < 80
}
fn is_black(p: [u8; 3]) -> bool {
    p.iter().all(|&c| c < 60)
}

/// Rows `[y0, y1)` and columns `[x0, x1)` of the pixels `pred` accepts.
#[derive(Debug, PartialEq)]
struct Ink {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
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

/// The vertical extent of `ink` is within 1 px of `[y0, y1)` (anti-aliasing at the edges).
#[track_caller]
fn rows(ink: &Option<Ink>, y0: usize, y1: usize, what: &str) {
    let ink = ink
        .as_ref()
        .unwrap_or_else(|| panic!("{what}: nothing is drawn"));
    assert!(
        ink.y0.abs_diff(y0) <= 1 && ink.y1.abs_diff(y1) <= 1,
        "{what}: rows {}..{}, Chrome {y0}..{y1}",
        ink.y0,
        ink.y1
    );
}

struct Shot {
    page: Page,
}

impl Shot {
    fn red(&self) -> Option<Ink> {
        bbox(&self.page, is_red)
    }
    fn blue(&self) -> Option<Ink> {
        bbox(&self.page, is_blue)
    }
    fn black(&self) -> Option<Ink> {
        bbox(&self.page, is_black)
    }
}

fn shot(extra_css: &str, body: &str) -> Shot {
    Shot {
        page: raster(&render_with(false, extra_css, body)),
    }
}

// --- S. the strut stays below the image (red before the fix) ------------------------------------------------

#[test]
fn s1_image_only_line_keeps_the_strut_descent() {
    // Chrome: image 0..60, the line is 64 px tall (Inter 16 px: descent 3.86 rounds to 4), the block starts at 64.
    let s = shot("", &format!("{}{BLK}", img60()));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 64, 84, "block after the image line");
}

#[test]
fn s2_line_height_1_5_adds_half_leading_below() {
    // 24 px line height: leading 4 px, 2 px below -> descent 4 + 2 = 6 -> 66.
    let s = shot("body { line-height: 1.5 }", &format!("{}{BLK}", img60()));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 66, 86, "block");
}

#[test]
fn s3_font_size_14px() {
    // Ascent 13.56 -> 14, descent 3.38 -> 3 (rounded separately, like Chrome): 63, not 64.
    let s = shot("body { font-size: 14px }", &format!("{}{BLK}", img60()));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 63, 83, "block");
}

#[test]
fn s4_line_height_1_squeezes_the_strut() {
    // 16 px line height: leading -4 px, -2 px below -> 4 - 2 = 2 -> 62.
    let s = shot("body { line-height: 1 }", &format!("{}{BLK}", img60()));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 62, 82, "block");
}

#[test]
fn s5_small_image_sits_on_the_strut_baseline() {
    // A 10 px image is shorter than the strut's ascent (16): it starts at 16 - 10 = 6, the line is 20 tall.
    let s = shot("", &format!("{}{BLK}", img(10, 10, "")));
    rows(&s.red(), 6, 16, "image");
    rows(&s.blue(), 20, 40, "block");
}

#[test]
fn s6_image_and_text_start_at_the_paragraph_top() {
    // The image is taller than the text above the baseline: it starts at the line top (was -2 px: the line was 60 tall
    // and the strut's half-leading was added on top of it), the text shares the baseline at the image's bottom edge.
    let s = shot("", &format!("{}Dodavatel{BLK}", img60()));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 64, 84, "block");
    // Chrome: the glyphs of "Dodavatel" (no descender) end at row 59, on the baseline at 60.
    let black = s.black().expect("the text is drawn");
    assert!(
        black.y1.abs_diff(59) <= 1,
        "text bottom {} (Chrome 59: the baseline is the image's bottom edge)",
        black.y1
    );
}

#[test]
fn s7_two_wrapped_images_make_two_64px_lines() {
    let s = shot(
        "",
        &format!(
            "<div style=\"width:100px\">{}{}</div>{BLK}",
            img60(),
            img60()
        ),
    );
    let column = runs((0..s.page.h).map(|y| is_red(s.page.at(30, y))));
    assert_eq!(column.len(), 2, "two image rows, got {column:?}");
    rows(
        &Some(Ink {
            x0: 0,
            x1: 0,
            y0: column[0].0,
            y1: column[0].1,
        }),
        0,
        60,
        "first image",
    );
    rows(
        &Some(Ink {
            x0: 0,
            x1: 0,
            y0: column[1].0,
            y1: column[1].1,
        }),
        64,
        124,
        "second image",
    );
    rows(&s.blue(), 128, 148, "block after two lines");
}

#[test]
fn s8_image_in_a_flex_item() {
    let s = shot(
        "",
        &format!(
            "<div style=\"display:flex\"><div>{}</div></div>{BLK}",
            img60()
        ),
    );
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 64, 84, "block after the flex container");
}

#[test]
fn s9_header_logo_with_a_bottom_rule() {
    // 16 + 64 + 16 = 96: the rule is at 96..98 (92 before the fix).
    let s = shot(
        "",
        &format!(
            "<div style=\"padding:16px 0;border-bottom:2px solid #000\">{}</div>{BLK}",
            img60()
        ),
    );
    rows(&s.red(), 16, 76, "logo");
    rows(&s.black(), 96, 98, "rule");
    rows(&s.blue(), 98, 118, "block");
}

#[test]
fn s10_qr_in_a_table_cell() {
    // The cell: 8 + (100 + 4) + 8 padding + 1 border = 121 (117 before the fix).
    let s = shot(
        "table { border-collapse: separate; border-spacing: 0 } td { padding: 8px; border-bottom: 1px solid #000 }",
        &format!(
            "<table><tr><td>{}</td></tr></table>{BLK}",
            img(100, 100, "")
        ),
    );
    rows(&s.red(), 8, 108, "QR");
    rows(&s.black(), 120, 121, "cell bottom border");
    rows(&s.blue(), 121, 141, "block after the table");
}

#[test]
fn s11_inline_svg_is_replaced_like_an_img() {
    let s = shot(
        "",
        &format!(
            "<svg width=\"60\" height=\"60\"><rect width=\"60\" height=\"60\" fill=\"#f00\"/></svg>{BLK}"
        ),
    );
    rows(&s.red(), 0, 60, "svg");
    rows(&s.blue(), 64, 84, "block");
}

#[test]
fn s12_inline_block_image_is_the_same_box() {
    let s = shot("", &format!("{}{BLK}", img(60, 60, "display:inline-block")));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 64, 84, "block");
}

// --- G. guards: green before and after -----------------------------------------------------------------------

#[test]
fn g1_block_image_has_no_strut() {
    let s = shot("", &format!("{}{BLK}", img(60, 60, "display:block")));
    rows(&s.red(), 0, 60, "image");
    rows(&s.blue(), 60, 80, "block");
}

#[test]
fn g2_vertical_align_top_middle_bottom_keep_the_box_height() {
    // Not baseline-aligned: the line is as tall as the image (Chrome: 60 for all three; the strut fits inside).
    for va in ["top", "middle", "bottom"] {
        let s = shot(
            "",
            &format!("{}{BLK}", img(60, 60, &format!("vertical-align:{va}"))),
        );
        rows(
            &s.blue(),
            60,
            80,
            &format!("block after vertical-align:{va}"),
        );
    }
}

#[test]
fn g3_tailwind_preflight_images_are_block_and_inline_blocks_do_not_move() {
    // Preflight (`img { display: block }`): no strut, fixtures 01-26 depend on it.
    let pdf = render_with(true, "", &format!("{}{BLK}", img60()));
    let page = raster(&pdf);
    rows(
        &bbox(&page, is_blue),
        60,
        80,
        "block after a preflight image",
    );
    // An empty `inline-block` is not an <img>: unchanged by F183 (Chrome puts it on the baseline: 52, F183b).
    let pdf = render_with(
        true,
        "",
        &format!("<span class=\"inline-block h-12 w-12 bg-gray-900\"></span>{BLK}"),
    );
    let blue = bbox(&raster(&pdf), is_blue);
    rows(
        &blue,
        48,
        68,
        "block after an empty inline-block (F183b: Chrome 52)",
    );
}

#[test]
fn g4_absolutely_positioned_image_adds_no_line() {
    let s = shot(
        "",
        &format!(
            "{}{BLK}",
            img(60, 60, "position:absolute;left:200px;top:100px")
        ),
    );
    rows(&s.blue(), 0, 20, "block (the abspos image is out of flow)");
    rows(&s.red(), 100, 160, "abspos image");
}

#[test]
fn g5_inline_block_badge_on_a_text_line() {
    let s = shot(
        "",
        &format!(
            "Total <span style=\"display:inline-block;padding:2px 8px;background:#f00;color:#fff\">Paid</span>{BLK}"
        ),
    );
    rows(&s.blue(), 24, 44, "block after a line with a badge");
}

// --- V. vrt-validator findings on the first version (plan section 7.3) ---------------------------------------------

#[test]
fn v1_short_line_height_does_not_pull_the_next_block_up() {
    // CSS 2.1 section 10.8: the line box ends at the lowest box bottom. The image's bottom is the baseline, so the
    // part below it is max(strut descent + half-leading, 0), never negative. Chrome: block at 60 for all of these
    // (the first version gave 54 for `line-height: 0`).
    for lh in ["0", ".25", ".5", "10px", "calc(1px + .5em)"] {
        let s = shot(
            &format!("div.l {{ line-height: {lh} }}"),
            &format!("<div class=\"l\">{}</div>{BLK}", img60()),
        );
        rows(&s.red(), 0, 60, &format!("image, line-height {lh}"));
        rows(&s.blue(), 60, 80, &format!("block, line-height {lh}"));
    }
}

#[test]
fn v1_short_line_height_with_text_and_a_30px_image() {
    // Chrome: image 0..30, block 30..50 for `line-height: 0` and `.5` (the first version gave 24 and 28).
    for lh in ["0", ".5"] {
        let s = shot(
            &format!("div.l {{ line-height: {lh} }}"),
            &format!("<div class=\"l\">x {} y</div>{BLK}", img(30, 30, "")),
        );
        rows(&s.red(), 0, 30, &format!("image, line-height {lh}"));
        rows(&s.blue(), 30, 50, &format!("block, line-height {lh}"));
    }
}

#[test]
fn v2_image_inside_an_inline_block_stays_where_it_was_until_f183b() {
    // The inner line is 64 tall (right), but the outer line puts the bottom of an inline-block on its baseline
    // instead of its last line's baseline (F183b; Chrome puts the block at 64), so a strut inside drew the image 4 px
    // low. Until F183b an image inside an atomic inline gets no strut: the positions of the version before F183.
    let ib = ".ib { display: inline-block; background: #0f0 }";
    let cases = [
        (
            "inline-block wrapper",
            ib,
            format!("<div><span class=\"ib\">{}</span></div>{BLK}", img60()),
            (0, 60),
            60,
        ),
        (
            "inline-block between text",
            ib,
            format!("<div>a <span class=\"ib\">{}</span> b</div>{BLK}", img60()),
            (0, 60),
            60,
        ),
        (
            "block inside an inline-block",
            "",
            format!(
                "<div style=\"display:inline-block\"><div>{}</div></div>{BLK}",
                img60()
            ),
            (0, 60),
            60,
        ),
        (
            "image in a button",
            "",
            format!(
                "<div><button style=\"font:16px Inter;padding:0;border:0\">{}</button></div>{BLK}",
                img(30, 30, "")
            ),
            (0, 30),
            30,
        ),
    ];
    for (what, css, body, (y0, y1), block) in cases {
        let s = shot(css, &body);
        rows(&s.red(), y0, y1, &format!("image, {what}"));
        rows(&s.blue(), block, block + 20, &format!("block, {what}"));
    }
}

#[test]
fn v3_unflagged_box_taller_than_a_baseline_image_keeps_the_old_line() {
    // `vertical-align: top` images are not baseline-aligned. Parley draws every box bottom-on-baseline, so the
    // baseline cannot be above the tallest one: the line is 80 / 50 tall as before F183, not 84 / 54 (Chrome: 80 / 50).
    for (top, blue) in [(80, 80), (50, 50)] {
        let s = shot(
            "",
            &format!(
                "<div>{}{}</div>{BLK}",
                img(40, 40, ""),
                img(top, top, "vertical-align:top")
            ),
        );
        rows(
            &s.red(),
            0,
            top as usize,
            &format!("images, top-aligned {top}"),
        );
        rows(
            &s.blue(),
            blue,
            blue + 20,
            &format!("block, top-aligned {top}"),
        );
    }
}

#[test]
fn v3_inline_block_taller_than_a_baseline_image_keeps_the_old_line() {
    // A 100 px inline-block (an unflagged box) and a 20 px baseline image: the old line, 100 (Chrome 104: F183b).
    let s = shot(
        ".ib { display: inline-block; width: 100px; height: 100px; background: #0f0 }",
        &format!(
            "<div><span class=\"ib\"></span>{}</div>{BLK}",
            img(20, 20, "")
        ),
    );
    rows(&s.red(), 80, 100, "image");
    rows(&s.blue(), 100, 120, "block (F183b: Chrome 104)");
}

// --- W. a row that does not fit moves whole (plan section 7.4) -------------------------------------------------

/// The first page of `pdf` has no ink (grey filler excluded) and page 2 holds the whole bordered cell.
fn row_on_page_2(extra_css: &str, cell: &str, filler: usize) -> (Page, Page) {
    let pdf = render_with(
        false,
        &format!("td {{ padding: 8px; border: 1px solid #000 }} {extra_css}"),
        &format!(
            "<div style=\"height: {filler}px; background: #ccc\"></div>\
             <table style=\"border-collapse: collapse\"><tr><td>{cell}</td></tr></table>"
        ),
    );
    (raster_page(&pdf, 1), raster_page(&pdf, 2))
}

#[test]
fn w1_collapsed_cell_with_an_image_moves_to_page_2_whole() {
    // A 1 px collapsed border puts the first line 1 px below `border + padding` of the cell (half borders). The
    // fragmenter read that as a margin gap in front of the first child (a class C break point), entered the cell at the
    // bottom of page 1 and left a 10 px stub there; the image went to page 2 as a box of its own, 60 px tall, in a cell
    // fragment of 60 px, while its line is 64 px tall (strut). The paragraph painter drops a line that is taller than
    // its fragment, so the image, and the text beside it, were gone. Chrome: nothing on page 1; page 2 has the
    // image at rows 8..69 and the cell border at rows 0..82.
    let (p1, p2) = row_on_page_2("", &img60(), 1080);
    rows(&bbox(&p2, is_red), 8, 69, "image on page 2");
    rows(&bbox(&p2, is_black), 0, 82, "cell on page 2");
    let ink1 = bbox(&p1, is_black);
    assert!(ink1.is_none(), "page 1 keeps a stub of the cell: {ink1:?}");
    assert!(bbox(&p1, is_red).is_none(), "page 1 has the image");
}

#[test]
fn w2_collapsed_cell_with_text_beside_the_image_keeps_both() {
    let (p1, p2) = row_on_page_2("", &format!("Řádek {} text", img60()), 1080);
    rows(&bbox(&p2, is_red), 8, 69, "image on page 2");
    rows(&bbox(&p2, is_black), 0, 82, "cell on page 2");
    assert!(bbox(&p1, is_black).is_none(), "page 1 keeps a stub");
}

#[test]
fn w3_100px_image_in_a_collapsed_cell() {
    let (p1, p2) = row_on_page_2("", &img(100, 100, ""), 1080);
    rows(&bbox(&p2, is_red), 8, 109, "image on page 2");
    rows(&bbox(&p2, is_black), 0, 122, "cell on page 2");
    assert!(bbox(&p1, is_black).is_none(), "page 1 keeps a stub");
}

// --- Fixture 27: a hand-written invoice with inline images --------------------------------------------------

#[test]
fn fixture_27_header_rule_and_payment_cell_match_chrome() {
    // Plain CSS, no preflight: the logo (with the company name beside it) and the QR code in a table cell are inline
    // images. The strut under each moves the header rule and the bottom of the payment cell down by 4 px (Chrome's
    // PNG of the reference, 96 dpi: rule rows 121..123, QR 454..566, cell bottom border row 583).
    let root = root();
    let dir = root.join("fixtures/invoices/27-inline-logo");
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = Renderer::builder()
        .strict_fonts(true)
        .strict_images(true)
        .font_file(root.join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .font_file(root.join("fixtures/fonts/Inter-SemiBold.ttf"))
        .unwrap()
        .font_file(root.join("fixtures/fonts/Inter-Bold.ttf"))
        .unwrap()
        .image("logo.png", std::fs::read(dir.join("logo.png")).unwrap())
        .image("qr.png", std::fs::read(dir.join("qr.png")).unwrap())
        .build()
        .render(&html, &RenderOptions::default())
        .expect("fixture 27 renders");
    let page = raster(&pdf);
    // The header rule: the first row below the logo that is dark across the content width.
    let dark_row = |y: usize| (57..737).filter(|&x| is_black(page.at(x, y))).count() > 600;
    let rule: Vec<usize> = (0..300).filter(|&y| dark_row(y)).collect();
    assert_eq!(
        (rule.first().copied(), rule.last().copied()),
        (Some(rule[0]), Some(rule[0] + 1)),
        "the rule is two rows: {rule:?}"
    );
    assert!(
        rule[0].abs_diff(121) <= 1,
        "header rule at row {} (Chrome 121)",
        rule[0]
    );
    // The QR code (dark modules) in the payment cell.
    let qr_rows: Vec<usize> = (430..620)
        .filter(|&y| (60..200).any(|x| is_black(page.at(x, y))))
        .collect();
    let (first, last) = (qr_rows[0], *qr_rows.last().unwrap() + 1);
    // The items table above has its own 2 px of row-height drift against Chrome (text metrics), so the absolute rows
    // get 3 px and the distance to the cell's border, which is the strut's, 1 px.
    assert!(
        first.abs_diff(454) <= 3 && last.abs_diff(566) <= 3,
        "QR rows {first}..{last} (Chrome 454..566)"
    );

    // The cell's bottom border (grey, 1 px) is the last grey row across the cell.
    let grey_row = |y: usize| {
        (57..737)
            .filter(|&x| {
                let p = page.at(x, y);
                p[0].abs_diff(209) < 8 && p[1].abs_diff(213) < 8 && p[2].abs_diff(219) < 8
            })
            .count()
            > 600
    };
    let bottom = (440..700)
        .rev()
        .find(|&y| grey_row(y))
        .expect("cell border");
    assert!(
        (bottom - last).abs_diff(583 - 566) <= 1,
        "the cell's bottom border is {} rows below the QR code (Chrome 17: quiet zone, padding and the strut)",
        bottom - last
    );
    assert!(
        bottom.abs_diff(583) <= 3,
        "payment cell bottom border at row {bottom} (Chrome 583)"
    );
}
