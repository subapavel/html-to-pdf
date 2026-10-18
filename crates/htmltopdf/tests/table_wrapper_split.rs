//! Text in a `display: table` box, an orphan table cell and an `inline-table` alone on its line keep every line (F30e).
//!
//! `<div style="display:table"><p>…</p></div>` already prints every line since F50 (Blitz wraps the `<p>` in an
//! anonymous row and cell instead of skipping it). What was still lost went through the fragmenter's `TablePart`
//! barrier, which put whatever it did not know as a cell of a table box in one piece: text or `<span>`s directly in a
//! `display: table` box or in a `display:table-row` (the anonymous cell Blitz makes is the paragraph), a `table-cell`
//! box outside a table, and an `inline-table` alone on its line (an atomic inline the F30f barrier did not look
//! through). Their lines below the first page were drawn below it and lost.
//!
//! Spec: CSS 2.1 §17.2.1 (anonymous table objects), §17.4 (a cell is a block container), css-break-3 §4.1 (breaks
//! between line boxes; an `inline-table` may be treated as monolithic) and §4.4 (orphans and widows 2). Every
//! expectation was measured with headless Chromium (m141) first, the numbers are in the F30e plan §7. Chrome fragments
//! table cells between lines; it **slices** an `inline-table` like an `inline-block` (the line that straddles the page
//! edge is printed on both pages) and moves one that is not first on its page to the next, so for an `inline-table` we
//! follow ADR-0004: every line once, between line boxes.
//!
//! The page is A4 with 15 mm margins (content box 1009 px); lines are forced with `<br>` and 20 px high, so 50 fit a
//! page. Words are found with `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the 15 mm page margin.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_PT: f32 = 6.0;

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}\
    p{margin:0;line-height:20px}table{border-collapse:collapse}td{padding:0;line-height:20px}";

/// `display: table`, as the finding writes it.
const TABLE: &str = "display:table";
/// An `inline-table` that is alone on its line.
const INLINE_TABLE: &str = "display:inline-table;width:100%";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    let tailwind = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tailwind/tailwind.css");
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .css_file(tailwind)
        .unwrap()
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// `Line0001<br>Line0002<br>…`: one 20 px line box per token.
fn lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("Line{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

/// One printed word: its 1-based page, the bottom of its box in pt from the page top, and its text.
#[derive(Clone)]
struct Word {
    page: usize,
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
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            let y_max = attrs
                .split_whitespace()
                .find_map(|a| a.strip_prefix("yMax=\""))
                .and_then(|v| v.trim_end_matches('"').parse::<f32>().ok())
                .expect("word without yMax");
            found.push(Word {
                page,
                y_max,
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

/// How many times `text` is printed, and on which pages.
fn pages_of(pdf: &[u8], text: &str) -> Vec<usize> {
    printed_words(pdf)
        .into_iter()
        .filter(|w| w.text == text)
        .map(|w| w.page)
        .collect()
}

/// `Line0001 … Line{n:04}`, each exactly once, in order, none below the content box. Returns the page count.
fn assert_every_line_once(what: &str, pdf: &[u8], n: usize) -> usize {
    let found: Vec<Word> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("Line"))
        .collect();
    let first_diff = found
        .iter()
        .enumerate()
        .position(|(i, w)| w.text != format!("Line{:04}", i + 1));
    assert!(
        found.len() == n && first_diff.is_none(),
        "{what}: every line exactly once, in order: printed {} of {n}, first difference at index {first_diff:?}",
        found.len()
    );
    let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    for w in &found {
        assert!(
            w.y_max <= limit,
            "{what}: {} on page {} ends at {:.1}pt, below the content box ({limit:.1}pt)",
            w.text,
            w.page,
            w.y_max
        );
    }
    page_count(pdf)
}

/// Lines printed on each page, from page 1.
fn lines_per_page(pdf: &[u8]) -> Vec<usize> {
    let mut per = vec![0; page_count(pdf)];
    for w in printed_words(pdf) {
        if w.text.starts_with("Line") {
            per[w.page - 1] += 1;
        }
    }
    per
}

// B1 -------------------------------------------------------------------------------------------------------------

#[test]
fn a_paragraph_in_a_display_table_box_keeps_every_line() {
    // The finding's markup. Fixed by F50; a guard. Chrome: 3 pages, 50 / 50 / 20.
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><p>{}</p></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > p", &pdf, 120), 3);
    assert_eq!(lines_per_page(&pdf), [50, 50, 20]);
}

// B2 -------------------------------------------------------------------------------------------------------------

#[test]
fn direct_text_in_a_display_table_box_keeps_every_line() {
    // The text is wrapped in an anonymous row and cell (CSS 2.1 §17.2.1), a block container. Chrome: 3 pages.
    let pdf = render(&format!("<div style=\"{TABLE}\">{}</div>", lines(120)));
    assert_eq!(assert_every_line_once("table text", &pdf, 120), 3);
    assert_eq!(lines_per_page(&pdf), [50, 50, 20]);
}

#[test]
fn spans_in_a_display_table_box_keep_every_line() {
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><span>{}</span></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > span", &pdf, 120), 3);
}

#[test]
fn a_bordered_display_table_box_reserves_its_bottom_inset() {
    // Chrome slices the frame (50 / 50 / 20); we close it on every page (F96), so a page holds fewer lines. No line
    // may sit below the content box.
    let pdf = render(&format!(
        "<div style=\"{TABLE};border:2px solid #000;padding:8px\">{}</div>",
        lines(120)
    ));
    let pages = assert_every_line_once("bordered table text", &pdf, 120);
    assert_eq!(pages, 3);
}

#[test]
fn a_display_table_box_that_is_not_first_on_its_page_starts_where_it_is() {
    // Chrome (P1m): 5 lines under the 900 px spacer on page 1, then 50 / 50 / 15 on three more pages. A row whose
    // first cell is a paragraph starts in place, as a `<td>` with direct text does since F30b.
    let pdf = render(&format!(
        "<div style=\"height:900px\">spacer</div><div style=\"{TABLE}\">{}</div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("spacer + table text", &pdf, 120), 4);
    assert_eq!(pages_of(&pdf, "spacer"), [1]);
    let per = lines_per_page(&pdf);
    assert!(per[0] >= 2, "the table starts on page 1: {per:?}");
}

// B3 -------------------------------------------------------------------------------------------------------------

#[test]
fn a_tailwind_table_box_keeps_every_line() {
    // `.table` from the compiled Tailwind sheet. With a `<p>` it is a guard.
    let pdf = render(&format!(
        "<div class=\"table w-full\"><p>{}</p></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("tailwind table > p", &pdf, 120), 3);
    let pdf = render(&format!("<div class=\"table w-full\">{}</div>", lines(120)));
    assert_eq!(assert_every_line_once("tailwind table text", &pdf, 120), 3);
}

// B4 -------------------------------------------------------------------------------------------------------------

#[test]
fn cells_of_a_display_table_box_keep_every_line() {
    // Cells and rows of a `display: table` box are split since F30b: guards. Chrome: 3 pages each.
    for (what, markup) in [
        (
            "cell > p",
            format!(
                "<div style=\"{TABLE}\"><div style=\"display:table-cell\"><p>{}</p></div></div>",
                lines(120)
            ),
        ),
        (
            "cell text",
            format!(
                "<div style=\"{TABLE}\"><div style=\"display:table-cell\">{}</div></div>",
                lines(120)
            ),
        ),
        (
            "row > cell text",
            format!(
                "<div style=\"{TABLE}\"><div style=\"display:table-row\">\
                 <div style=\"display:table-cell\">{}</div></div></div>",
                lines(120)
            ),
        ),
    ] {
        let pdf = render(&markup);
        assert_eq!(assert_every_line_once(what, &pdf, 120), 3, "{what}");
    }
}

// B5 -------------------------------------------------------------------------------------------------------------

#[test]
fn direct_text_in_a_display_table_row_keeps_every_line() {
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><div style=\"display:table-row\">{}</div></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > row text", &pdf, 120), 3);
}

// B6 -------------------------------------------------------------------------------------------------------------

#[test]
fn a_table_cell_outside_a_table_keeps_every_line() {
    // Blitz lays an orphan cell out as a block container; Chrome wraps it in an anonymous table (F50b does that).
    for (what, markup) in [
        (
            "orphan cell",
            format!(
                "<div><div style=\"display:table-cell\">{}</div></div>",
                lines(120)
            ),
        ),
        (
            "orphan row > cell",
            format!(
                "<div style=\"display:table-row\"><div style=\"display:table-cell\">{}</div></div>",
                lines(120)
            ),
        ),
    ] {
        let pdf = render(&markup);
        assert_eq!(assert_every_line_once(what, &pdf, 120), 3, "{what}");
    }
}

#[test]
fn a_paragraph_in_an_orphan_table_row_keeps_every_line() {
    // Guard. Blitz prints html5ever noise on stdout for an orphan row (F50b); not asserted.
    let pdf = render(&format!(
        "<div style=\"display:table-row\"><p>{}</p></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("orphan row > p", &pdf, 120), 3);
}

// B7 -------------------------------------------------------------------------------------------------------------

#[test]
fn an_inline_table_alone_on_its_line_keeps_every_line() {
    // ADR-0004: split between line boxes, each line once. Chrome slices (3 pages, one line on both pages at each edge).
    for (what, markup) in [
        (
            "inline-table > p",
            format!("<div style=\"{INLINE_TABLE}\"><p>{}</p></div>", lines(120)),
        ),
        (
            "inline-table text",
            format!("<div style=\"{INLINE_TABLE}\">{}</div>", lines(120)),
        ),
    ] {
        let pdf = render(&markup);
        assert_eq!(assert_every_line_once(what, &pdf, 120), 3, "{what}");
    }
}

#[test]
fn an_inline_table_in_a_bordered_frame_keeps_every_line() {
    // Chrome moves the table to page 2 (4 pages, an empty frame on page 1); we start it where it is, like a
    // paragraph (ADR-0004), and every line is on a page.
    for (what, inner) in [
        ("frame > it > p", format!("<p>{}</p>", lines(120))),
        ("frame > it text", lines(120)),
    ] {
        let pdf = render(&format!(
            "<div style=\"border:2px solid #000;padding:8px\"><div style=\"{INLINE_TABLE}\">{inner}</div></div>"
        ));
        let pages = assert_every_line_once(what, &pdf, 120);
        assert!((3..=4).contains(&pages), "{what}: {pages} pages");
    }
}

#[test]
fn an_inline_table_below_a_heading_keeps_every_line() {
    let pdf = render(&format!(
        "<h1 style=\"font-size:24px;margin:0\">Podmínky</h1><div style=\"{INLINE_TABLE}\"><p>{}</p></div>",
        lines(120)
    ));
    let pages = assert_every_line_once("h1 + it > p", &pdf, 120);
    assert!((3..=4).contains(&pages), "{pages} pages");
    assert_eq!(pages_of(&pdf, "Podmínky"), [1]);
}

// B8 -------------------------------------------------------------------------------------------------------------

#[test]
fn an_inline_table_that_shares_its_line_is_unchanged() {
    // Not split (F30f-b): the lines below the page are lost, as on main, and the warning says why
    // (`unsplit_paragraph_warning.rs`). Chrome slices it.
    let pdf = render(&format!(
        "<p>Úvod <span style=\"display:inline-table;width:60%\">{}</span> konec</p>",
        lines(120)
    ));
    assert_eq!(page_count(&pdf), 1);
    let found: Vec<String> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("Line"))
        .map(|w| w.text)
        .collect();
    assert!(found.len() < 120, "{} lines", found.len());
    let want: Vec<String> = (1..=found.len()).map(|i| format!("Line{i:04}")).collect();
    assert_eq!(found, want, "a prefix of the lines, each once");
}

// B9 -------------------------------------------------------------------------------------------------------------

#[test]
fn a_display_table_box_with_a_caption_div_keeps_every_line() {
    // `display: table-caption` on a div: the lines are a guard (F30e-b is the caption itself, which Blitz drops).
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><div style=\"display:table-caption\">Obchodní podmínky</div><p>{}</p></div>",
        lines(120)
    ));
    assert_eq!(
        assert_every_line_once("table > caption div + p", &pdf, 120),
        3
    );
}

#[test]
#[ignore = "F30e-b: Blitz drops a display:table-caption box that is not a <caption> in a <table>"]
fn a_table_caption_div_is_printed_above_the_table() {
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><div style=\"display:table-caption\">Obchodní podmínky</div><p>{}</p></div>",
        lines(120)
    ));
    assert_eq!(pages_of(&pdf, "Obchodní"), [1]);
    let found = printed_words(&pdf);
    let caption = found.iter().find(|w| w.text == "Obchodní").unwrap();
    let first = found.iter().find(|w| w.text == "Line0001").unwrap();
    assert!(caption.y_max < first.y_max, "the caption is above");
}

#[test]
fn a_table_caption_element_is_printed_once_above_the_rows() {
    let pdf = render(&format!(
        "<table class=\"w-full\"><caption>Obchodní podmínky</caption><tr><td><p>{}</p></td></tr></table>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > caption", &pdf, 120), 3);
    assert_eq!(pages_of(&pdf, "Obchodní"), [1]);
    assert_eq!(pages_of(&pdf, "podmínky"), [1]);
}

// B10 ------------------------------------------------------------------------------------------------------------

#[test]
fn nested_display_table_boxes_keep_every_line() {
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><div style=\"{TABLE}\"><p>{}</p></div></div>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > table > p", &pdf, 120), 3);
}

#[test]
fn a_display_table_box_around_a_table_keeps_every_row() {
    // 60 rows of 20 px: Chrome 2 pages (50 + 10).
    let rows: String = (1..=60)
        .map(|i| format!("<tr><td>Radek{i:02}</td><td>B</td></tr>"))
        .collect();
    let pdf = render(&format!(
        "<div style=\"{TABLE}\"><table style=\"width:100%\">{rows}</table></div>"
    ));
    assert_eq!(page_count(&pdf), 2);
    // `pdftotext` joins a cell's word with the next cell's when nothing separates them (`Radek02B`).
    let found = printed_words(&pdf);
    for i in 1..=60 {
        let token = format!("Radek{i:02}");
        let n = found.iter().filter(|w| w.text.starts_with(&token)).count();
        assert_eq!(n, 1, "{token}");
    }
}

// B11 ------------------------------------------------------------------------------------------------------------

#[test]
fn a_split_cell_in_a_clipped_table_is_painted_on_every_page() {
    // The `draw_under_clip_table` path: a `<table>` that clips, with a cell that is a block (F30f planner's
    // question). Guard.
    let pdf = render(&format!(
        "<table style=\"width:100%;overflow:hidden\"><tr><td style=\"display:block\">{}</td></tr></table>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("clipped table", &pdf, 120), 3);
}

#[test]
fn a_table_with_a_paragraph_cell_is_unchanged() {
    let pdf = render(&format!(
        "<table style=\"width:100%\"><tr><td><p>{}</p></td></tr></table>",
        lines(120)
    ));
    assert_eq!(assert_every_line_once("table > tr > td > p", &pdf, 120), 3);
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tw-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
