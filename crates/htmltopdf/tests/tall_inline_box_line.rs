//! A line that holds an `inline-block` (or another atomic inline) taller than the room left on the page moves to the
//! next page whole; its words are not lost (F213).
//!
//! css-break-3 §4.1: there is no break point inside a line box, and a UA may treat an inline-level box that
//! establishes a formatting context (`inline-block`) as monolithic. A line whose tallest box does not fit the room
//! below the cursor therefore goes to the next page with that box, through the Class A break before its paragraph;
//! CSS 2.1 §13.3.3 says the same (breaks between block boxes and between line boxes, never inside a line box).
//! Before the fix the fragmenter entered the box (its first row, a block child, fits the room), left the line on
//! page 1 and the line's baseline, at the bottom of the box, was painted below the page edge: `START` and `TAIL` were
//! printed on no page and the renderer warned "content extends N pt past the bottom page edge". Plan:
//! docs/plans/2026-11-03-f213-text-beside-tall-inline-block.md.
//!
//! The page is A4 with 15 mm margins (content box 1009.13 px), `body { font: 14px/20px Inter }`. `S` is a block of
//! 809 px that leaves 200 px below it; `B(h)` is a 150 px wide `inline-block` of height `h` with a one-line block
//! child `BOXTOP`. Every page number below is Chrome's (version of the committed references, measured with
//! `scripts/reference/render.mjs`'s settings and `pdftotext -bbox`, plan section 7). The y positions are not compared
//! with Chrome's except where the box is empty or replaced: Chrome puts a box with in-flow content on the baseline of
//! its last line box, we put every inline box bottom-on-baseline (F183c), so `START` sits at the box's bottom here and at
//! its top in Chrome.
//!
//! The `p1*` / `p5*` tests are red before the fix; the `g*` tests are guards that hold before and after.

use htmltopdf::{RenderOptions, Renderer};

const RED60: &[u8] = include_bytes!("data/red60.png");

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";
/// Bottom of the content box in pt: 841.89 - 15 mm.
const CONTENT_BOTTOM_PT: f32 = 841.89 - 15.0 / 25.4 * 72.0;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .image("red60.png", RED60.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// The room-making block: 809 px of 1009.13, 200 px left.
fn s() -> &'static str {
    "<div style=\"height:809px\"></div>"
}

/// A 150 px wide `inline-block` of height `h` px with a block child `BOXTOP`; `extra` is more style.
fn b(h: u32, extra: &str) -> String {
    format!(
        "<span style=\"display:inline-block;width:150px;height:{h}px;border:1px solid #000;{extra}\">\
         <span style=\"display:block\">BOXTOP</span></span>"
    )
}

/// An empty `inline-block` of height `h` px.
fn b_empty(h: u32) -> String {
    format!(
        "<span style=\"display:inline-block;width:150px;height:{h}px;border:1px solid #000\"></span>"
    )
}

/// An `inline-block` of auto height: a label, a block `<svg>` of height `h` px and an amount (the shape of the QR box
/// of fixture 43).
fn b_stack(h: u32) -> String {
    format!(
        "<span style=\"display:inline-block;width:150px;border:1px solid #000\">\
         <span style=\"display:block\">BOXTOP</span>\
         <svg style=\"display:block\" width=\"100\" height=\"{h}\"><rect width=\"100\" height=\"{h}\" fill=\"#444\"/></svg>\
         <span style=\"display:block\">AMOUNT</span></span>"
    )
}

/// One printed word: its 1-based page, the top and bottom of its box in pt from the page top, and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let text = String::from_utf8_lossy(&out.stdout);
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(name))
            .and_then(|v| v.trim_start_matches('=').trim_matches('"').parse().ok())
            .unwrap_or_else(|| panic!("word without {name}"))
    };
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push(Word {
                page,
                y_min: attr(attrs, "yMin"),
                y_max: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf);
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

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tib-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// The single printed `text`: panics when it is not printed (a lost word) or printed twice.
fn find(words: &[Word], text: &str) -> Word {
    let hits: Vec<&Word> = words.iter().filter(|w| w.text == text).collect();
    assert_eq!(
        hits.len(),
        1,
        "`{text}` must be printed exactly once, found {} (all words: {:?})",
        hits.len(),
        words.iter().map(|w| (w.page, &w.text)).collect::<Vec<_>>()
    );
    hits[0].clone()
}

/// Every word of `texts` is printed once, on page `page`, inside the content box.
fn assert_on_page(words: &[Word], page: usize, texts: &[&str]) {
    for t in texts {
        let w = find(words, t);
        assert_eq!(
            w.page, page,
            "`{t}` is on page {}, Chrome has it on page {page}",
            w.page
        );
        assert!(
            w.y_max <= CONTENT_BOTTOM_PT + 5.0,
            "`{t}` ends at {:.1} pt, below the content box ({CONTENT_BOTTOM_PT:.1} pt)",
            w.y_max
        );
    }
}

// --- P1: the finding ---------------------------------------------------------------------------------------------

#[test]
fn p1_a_line_with_an_inline_block_taller_than_the_room_moves_to_the_next_page() {
    // `S<p>START B(300) TAIL</p><p>AFTER</p>`. Chrome: 2 pages, page 1 empty; START, BOXTOP, TAIL at 41.7 pt and AFTER at
    // 267.4 pt on page 2. (Ours puts START at the bottom of the box, F183c, so y is not compared.)
    let pdf = render(&format!(
        "{}<p>START {} TAIL</p><p>AFTER</p>",
        s(),
        b(300, "")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "BOXTOP", "TAIL", "AFTER"]);
    assert!(
        find(&words, "AFTER").y_min > find(&words, "START").y_min,
        "AFTER follows the line that holds the box"
    );
    assert_eq!(page_count(&pdf), 2, "two pages like Chrome");
}

#[test]
fn p1_a_box_of_several_blocks_moves_with_its_line() {
    // The shape of fixture 43: an auto-height box of label, a 300 px block `<svg>` and an amount. Entering it, the
    // fragmenter kept the label on page 1, cut the box before the `<svg>` and left the line on page 1, below the edge.
    // Chrome: START, BOXTOP, AMOUNT, TAIL, AFTER on page 2 (START at 281.7 pt, level with AMOUNT).
    let pdf = render(&format!(
        "{}<p>START {} TAIL</p><p>AFTER</p>",
        s(),
        b_stack(300)
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "BOXTOP", "AMOUNT", "TAIL", "AFTER"]);
    assert_eq!(page_count(&pdf), 2, "two pages like Chrome");
}

#[test]
fn p1t_the_same_with_vertical_align_top() {
    // `vertical-align: top` loses the words as well: Parley 0.6 seats every inline box bottom-on-baseline (F183c).
    // Chrome: page 2 for all four words (START 40.9 pt, BOXTOP 41.7 pt). y is not compared (F183c).
    let pdf = render(&format!(
        "{}<p>START {} TAIL</p><p>AFTER</p>",
        s(),
        b(300, "vertical-align:top")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "BOXTOP", "TAIL", "AFTER"]);
    assert_eq!(page_count(&pdf), 2);
}

#[test]
fn p5_the_same_nested_in_a_section() {
    // P5: `<section>` around the paragraph. Chrome: all four words on page 2.
    let pdf = render(&format!(
        "{}<section><p>START {} TAIL</p></section><p>AFTER</p>",
        s(),
        b(300, "")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "BOXTOP", "TAIL", "AFTER"]);
    assert_eq!(page_count(&pdf), 2);
}

#[test]
fn p5b_the_same_after_a_paragraph_in_a_section() {
    // P5b: `<section><p>x</p><p>START B TAIL</p></section>`. Chrome: `x` stays on page 1 (647.7 pt), the line goes to
    // page 2.
    let pdf = render(&format!(
        "{}<section><p>x</p><p>START {} TAIL</p></section><p>AFTER</p>",
        s(),
        b(300, "")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 1, &["x"]);
    assert_on_page(&words, 2, &["START", "BOXTOP", "TAIL", "AFTER"]);
    assert_eq!(page_count(&pdf), 2);
}

// --- guards: green before and after ------------------------------------------------------------------------------

#[test]
fn g1_an_empty_box_moves_with_its_line() {
    // P1e: the leaf case. With no child the first row is the box itself, which does not fit, so the carrier moves.
    // Chrome: page 2, START and TAIL at 256.2 pt, AFTER at 271.2 pt.
    let pdf = render(&format!(
        "{}<p>START {} TAIL</p><p>AFTER</p>",
        s(),
        b_empty(300)
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "TAIL", "AFTER"]);
    let y = find(&words, "START").y_min;
    assert!(
        (y - 256.2).abs() <= 3.0,
        "START at {y:.1} pt, Chrome puts it at 256.2 pt"
    );
}

#[test]
fn g2_a_tall_image_moves_with_its_line() {
    // P3: `<img>` of 100 x 300 px (the 60 px red PNG, scaled). Chrome: page 2, START and TAIL at 254.7 pt.
    let pdf = render(&format!(
        "{}<p>START <img src=\"red60.png\" style=\"width:100px;height:300px\"> TAIL</p><p>AFTER</p>",
        s()
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "TAIL", "AFTER"]);
    let y = find(&words, "START").y_min;
    assert!(
        (y - 254.7).abs() <= 4.0,
        "START at {y:.1} pt, Chrome puts it at 254.7 pt"
    );
}

#[test]
fn g3_a_tall_svg_moves_with_its_line() {
    // P3s: inline `<svg width=100 height=300>`. Chrome: page 2, START and TAIL at 254.7 pt.
    let pdf = render(&format!(
        "{}<p>START <svg width=\"100\" height=\"300\" viewBox=\"0 0 10 30\"><rect width=\"10\" height=\"30\" fill=\"#444\"/></svg> TAIL</p><p>AFTER</p>",
        s()
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["START", "TAIL", "AFTER"]);
    let y = find(&words, "START").y_min;
    assert!(
        (y - 254.7).abs() <= 4.0,
        "START at {y:.1} pt, Chrome puts it at 254.7 pt"
    );
}

#[test]
fn g4_a_tall_box_on_the_third_line_splits_before_its_line() {
    // P4: `L1<br>L2<br>START B TAIL<br>L4<br>L5`. Two lines stay on page 1 (orphans 2), the line with the box and the
    // lines after it go to page 2. Chrome: L1, L2 on page 1; START, BOXTOP, TAIL, L4, L5, AFTER on page 2. (Ours
    // paints the box itself in the wrong place, F183c; the pages are right.)
    let pdf = render(&format!(
        "{}<p>L1<br>L2<br>START {} TAIL<br>L4<br>L5</p><p>AFTER</p>",
        s(),
        b(300, "")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 1, &["L1", "L2"]);
    assert_on_page(&words, 2, &["START", "BOXTOP", "TAIL", "L4", "L5", "AFTER"]);
}

#[test]
fn g5_a_tall_box_on_the_last_of_three_lines_moves_the_whole_paragraph() {
    // P4b: `L1<br>L2<br>START B TAIL`. Chrome splits it 2 / 1 and breaks orphans 2 + widows 2, which cannot both hold
    // in three lines. ADR-0010: when the relaxation is needed, we move the paragraph whole, so L1, L2 and the line go
    // to page 2 together. This is a deliberate difference from Chrome, not a bug to fix here.
    let pdf = render(&format!(
        "{}<p>L1<br>L2<br>START {} TAIL</p><p>AFTER</p>",
        s(),
        b(300, "")
    ));
    let words = printed_words(&pdf);
    assert_on_page(&words, 2, &["L1", "L2", "START", "BOXTOP", "TAIL", "AFTER"]);
}
