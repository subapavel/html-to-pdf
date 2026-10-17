//! Multi-column text taller than the room under it flows over the pages, column after column (F30d).
//!
//! Blitz and Taffy lay a multicol container out as one very tall set of columns (each column is one slice many pages
//! tall); the paginator treated it as a box it could only cut into strips, recorded its text on the first page only,
//! and the renderer skipped every column slice that crossed a page break. A `column-count: 2` block of 4000 words printed
//! 15 pages and no word at all.
//!
//! Spec: css-break-3 §2 (columns and pages are both fragmentainers; a column box that does not fit continues in the
//! next page's columns), css-multicol-1 §8.2 (columns that do not fit the page move to the next page), §7.1
//! `column-fill: balance` (in a fragmented context only the last fragment is balanced; earlier ones are filled to the
//! page) and css-break-3 §4.1 (breaks inside a paragraph fall between line boxes).
//!
//! Asserted on each page's words with their boxes (`pdftotext -bbox`): every `wordNNNNN` exactly once and column-major
//! (page, then column, then top to bottom), inside the content box, the second column one `col_w + gap` to the right
//! of the first, every page but the last filled to the bottom in both columns, the last page balanced. The Chrome
//! numbers quoted in the comments come from `chrome-pdf.mjs` probes of the same HTML (A4, 15 mm, Inter,
//! `line-height: 20px`).

use htmltopdf::{RenderOptions, Renderer};

const PAGE_W_PT: f32 = 595.28;
const PAGE_H_PT: f32 = 841.89;
/// The default 15 mm page margin the renderer applies.
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// `line-height: 20px`, in pt.
const LINE_PT: f32 = 15.0;
/// `column-gap: normal` is 1em = 16 px = 12 pt.
const GAP_PT: f32 = 12.0;
/// Two columns of an A4 content box.
const COL_W_PT: f32 = (PAGE_W_PT - 2.0 * MARGIN_PT - GAP_PT) / 2.0;
/// A word whose left edge is further left than this is in the first column (the middle of the gutter).
const GUTTER_X_PT: f32 = MARGIN_PT + COL_W_PT + GAP_PT / 2.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_BELOW_PT: f32 = 5.0;
/// A word whose top is higher than this above the content top was drawn too high.
const SLACK_ABOVE_PT: f32 = 2.0;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// Words `word<from> … word<to-1>`, five digits each.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>\
                 body{{margin:0;font-family:Inter;font-size:16px;line-height:20px}}\
                 table{{width:100%}}td{{padding:0;height:20px}}\
                 </style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// One printed word: its 1-based page, its box in pt (from the page's top left) and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    text: String,
}

impl Word {
    fn column(&self) -> usize {
        usize::from(self.x_min >= GUTTER_X_PT)
    }
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
                x_min: attr(attrs, "xMin"),
                x_max: attr(attrs, "xMax"),
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
    let path = std::env::temp_dir().join(format!("htmltopdf-mc-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// The `wordNNNNN` words in reading order of a column layout: by page, then column, then top to bottom, left to right.
fn column_major(found: &[Word]) -> Vec<Word> {
    let mut v: Vec<Word> = found
        .iter()
        .filter(|w| w.text.starts_with("word"))
        .cloned()
        .collect();
    v.sort_by(|a, b| {
        (a.page, a.column(), (a.y_min * 10.0).round() as i64)
            .cmp(&(b.page, b.column(), (b.y_min * 10.0).round() as i64))
            .then(a.x_min.total_cmp(&b.x_min))
    });
    v
}

/// `words(first, last)` printed exactly once each, in column-major order, inside the content box.
fn assert_every_word_once(what: &str, found: &[Word], first: usize, last: usize) {
    let ordered = column_major(found);
    let expected: Vec<String> = (first..last).map(|i| format!("word{i:05}")).collect();
    let seen: Vec<&str> = ordered.iter().map(|w| w.text.as_str()).collect();
    let first_diff = seen
        .iter()
        .zip(&expected)
        .position(|(a, b)| a != b)
        .or((seen.len() != expected.len()).then_some(seen.len().min(expected.len())));
    assert!(
        first_diff.is_none(),
        "{what}: every word exactly once, column-major: printed {} of {}, first difference at index {first_diff:?} \
         (printed {:?}, expected {:?})",
        seen.len(),
        expected.len(),
        first_diff.and_then(|i| seen.get(i)),
        first_diff.and_then(|i| expected.get(i)),
    );
    let bottom = PAGE_H_PT - MARGIN_PT + SLACK_BELOW_PT;
    let top = MARGIN_PT - SLACK_ABOVE_PT;
    for w in found {
        assert!(
            w.y_max <= bottom && w.y_min >= top,
            "{what}: {} on page {} spans {:.1}..{:.1}pt, outside the content box ({top:.1}..{bottom:.1}pt)",
            w.text,
            w.page,
            w.y_min,
            w.y_max
        );
        assert!(
            w.x_min >= MARGIN_PT - 1.0 && w.x_max <= PAGE_W_PT - MARGIN_PT + 1.0,
            "{what}: {} on page {} spans x {:.1}..{:.1}pt, outside the content box",
            w.text,
            w.page,
            w.x_min,
            w.x_max
        );
    }
}

/// Where the text of column `col` (0 or 1) ends on `page` (largest `y_max` of its `wordNNNNN` words), if it has any.
fn column_end(found: &[Word], page: usize, col: usize) -> Option<f32> {
    found
        .iter()
        .filter(|w| w.text.starts_with("word") && w.page == page && w.column() == col)
        .map(|w| w.y_max)
        .reduce(f32::max)
}

/// Every page but the last is filled to the bottom in both columns, the last page's columns end within one line of
/// each other, and column 2 sits one `col_w + gap` to the right of column 1.
fn assert_filled_and_balanced(what: &str, found: &[Word], pages: usize) {
    let content_bottom = PAGE_H_PT - MARGIN_PT;
    for page in 1..pages {
        for col in 0..2 {
            let end = column_end(found, page, col)
                .unwrap_or_else(|| panic!("{what}: page {page} has no text in column {}", col + 1));
            assert!(
                end >= content_bottom - LINE_PT - SLACK_BELOW_PT,
                "{what}: column {} of page {page} ends at {end:.1}pt, more than a line above the content bottom ({content_bottom:.1}pt)",
                col + 1
            );
        }
    }
    let ends: Vec<f32> = (0..2)
        .map(|c| {
            column_end(found, pages, c)
                .unwrap_or_else(|| panic!("{what}: the last page has no text in column {}", c + 1))
        })
        .collect();
    assert!(
        (ends[0] - ends[1]).abs() <= LINE_PT + 0.5,
        "{what}: the last page is not balanced: the columns end at {:.1}pt and {:.1}pt",
        ends[0],
        ends[1]
    );
    for page in 1..=pages {
        let left = |col: usize| {
            found
                .iter()
                .filter(|w| w.text.starts_with("word") && w.page == page && w.column() == col)
                .map(|w| w.x_min)
                .reduce(f32::min)
        };
        if let (Some(a), Some(b)) = (left(0), left(1)) {
            assert!(
                ((b - a) - (COL_W_PT + GAP_PT)).abs() <= 1.0,
                "{what}: page {page}: column 2 starts {:.1}pt right of column 1, expected {:.1}pt",
                b - a,
                COL_W_PT + GAP_PT
            );
        }
    }
}

fn check_long_flow(what: &str, body: &str, n: usize) {
    let pdf = render(body);
    let found = printed_words(&pdf);
    assert_every_word_once(what, &found, 0, n);
    let pages = page_count(&pdf);
    // Chrome prints these 4000 words on 14 pages; a page holds 2 columns of 50 lines of 3 words.
    assert!(
        (13..=15).contains(&pages),
        "{what}: {pages} pages, Chrome prints 14"
    );
    assert_filled_and_balanced(what, &found, pages);
}

#[test]
fn a_paragraph_in_two_columns_flows_over_the_pages() {
    check_long_flow(
        "column-count:2 > p",
        &format!(
            "<div style=\"column-count:2\"><p style=\"margin:0\">{}</p></div>",
            words(0, 4000)
        ),
        4000,
    );
}

#[test]
fn bare_text_in_two_columns_flows_over_the_pages() {
    check_long_flow(
        "column-count:2 > text",
        &format!("<div style=\"column-count:2\">{}</div>", words(0, 4000)),
        4000,
    );
}

#[test]
fn a_column_width_container_flows_over_the_pages() {
    // `columns: 18em` is two 332 px columns in the 680 px content box.
    check_long_flow(
        "columns:18em > text",
        &format!("<div style=\"columns:18em\">{}</div>", words(0, 4000)),
        4000,
    );
}

#[test]
fn short_two_column_text_stays_where_it_was() {
    // Guard: text that fits its page goes through none of the new code. Chrome: the first line's left edge is at
    // 42.75 and 304.1 pt, its top at 39.8 pt. Not asserted: the split between the columns. A container that fits is
    // still balanced against Blitz's one-column height, so the columns hold 66 and 84 words where Chrome balances
    // 75 and 75 (F30d-c).
    let pdf = render(&format!(
        "<div style=\"column-count:2\"><p style=\"margin:0\">{}</p></div>",
        words(0, 150)
    ));
    let found = printed_words(&pdf);
    assert_every_word_once("short columns", &found, 0, 150);
    assert_eq!(page_count(&pdf), 1);
    let col = |c: usize| {
        found
            .iter()
            .filter(|w| w.column() == c)
            .cloned()
            .collect::<Vec<_>>()
    };
    let (c1, c2) = (col(0), col(1));
    assert!(!c1.is_empty() && !c2.is_empty(), "both columns have text");
    let min = |ws: &[Word], f: fn(&Word) -> f32| ws.iter().map(f).reduce(f32::min).unwrap();
    assert!((min(&c1, |w| w.x_min) - 42.75).abs() <= 1.0, "column 1 x");
    assert!((min(&c2, |w| w.x_min) - 304.1).abs() <= 1.0, "column 2 x");
    assert!(
        (min(&c1, |w| w.y_min) - 39.8).abs() <= LINE_PT,
        "first line y"
    );
    assert!(
        (min(&c2, |w| w.y_min) - 39.8).abs() <= LINE_PT,
        "first line y"
    );
}

fn items_table(rows: usize) -> String {
    let rows: String = (0..rows)
        .map(|i| format!("<tr><td>row{i:02}</td></tr>"))
        .collect();
    format!("<table>{rows}</table>")
}

#[test]
fn a_terms_block_under_a_table_starts_below_it_and_continues_on_the_next_page() {
    // 30 rows of 20 px take 450 pt of the 767 pt page; the 400-word block starts under them (Chrome: 489.8 pt,
    // 20 lines per column on page 1), page 2 holds the rest balanced (47 lines per column).
    let pdf = render(&format!(
        "{}<div style=\"column-count:2\"><p style=\"margin:0\">{}</p></div>",
        items_table(30),
        words(0, 400)
    ));
    let found = printed_words(&pdf);
    assert_every_word_once("terms under a table", &found, 0, 400);
    assert_eq!(page_count(&pdf), 2, "Chrome prints 2 pages");
    let last_row = found.iter().find(|w| w.text == "row29").expect("row29");
    assert_eq!(last_row.page, 1, "the table stays on page 1");
    let first = found.iter().find(|w| w.text == "word00000").unwrap();
    assert_eq!(first.page, 1, "the block starts on page 1, not moved whole");
    assert!(
        first.y_min >= last_row.y_max - 1.0 && first.y_min <= last_row.y_max + LINE_PT,
        "the first word of the block is at {:.1}pt, the table ends at {:.1}pt",
        first.y_min,
        last_row.y_max
    );
    assert_filled_and_balanced("terms under a table", &found, 2);
}

#[test]
fn a_long_terms_block_under_a_table_is_complete() {
    let pdf = render(&format!(
        "{}<div style=\"column-count:2\"><p style=\"margin:0\">{}</p></div>",
        items_table(30),
        words(0, 1500)
    ));
    let found = printed_words(&pdf);
    assert_every_word_once("1500 words under a table", &found, 0, 1500);
    // Chrome prints 6 pages.
    let pages = page_count(&pdf);
    assert!((5..=7).contains(&pages), "{pages} pages, Chrome prints 6");
    assert_filled_and_balanced("1500 words under a table", &found, pages);
}

#[test]
fn many_paragraphs_in_two_columns_flow_over_the_pages() {
    // 30 paragraphs of 40 words (13.3 lines each): block children of the column flow, cut between lines. Chrome: 5 pages.
    let paragraphs: String = (0..30)
        .map(|p| format!("<p style=\"margin:0\">{}</p>", words(p * 40, p * 40 + 40)))
        .collect();
    let pdf = render(&format!("<div style=\"column-count:2\">{paragraphs}</div>"));
    let found = printed_words(&pdf);
    assert_every_word_once("30 paragraphs", &found, 0, 1200);
    let pages = page_count(&pdf);
    assert!((4..=6).contains(&pages), "{pages} pages, Chrome prints 5");
    assert_filled_and_balanced("30 paragraphs", &found, pages);
}
