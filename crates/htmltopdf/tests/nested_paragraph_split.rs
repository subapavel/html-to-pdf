//! A paragraph taller than a page inside a block (`div`, `section`, `li`, `<td><p>`, a Tailwind `rounded-lg border p-4`
//! notes box) splits at line boxes like the same paragraph at body level (F30), instead of printing the lines that
//! fit its first page and drawing the rest below the page box (css-break-3 §4.1, CSS 2.1 §13.3.3).
//!
//! Every `LineNNNN` must appear exactly once, in page order, and none may sit below the content box. Asserted on
//! each page's words with their positions (`pdftotext -bbox`). Lines are forced with `<br>` (20 px each) so the
//! number of lines per page does not depend on font metrics.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin the renderer applies.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt.
const SLACK_PT: f32 = 5.0;
/// Content height of one A4 page in CSS px (see `consecutive_tables.rs`).
const CONTENT_PX: usize = 1009;

const CSS: &str = "body { margin: 0 } p { margin: 0; line-height: 20px } li { line-height: 20px } \
    pre { margin: 0; line-height: 20px; font-family: inherit } table { border-collapse: collapse } td { padding: 0; line-height: 20px } \
    .box { border: 1px solid #333; border-radius: 8px; padding: 16px } ul { margin: 0 }";

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(CSS)
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// `Line0001<br>Line0002<br>…Line0400`: one line box per token.
fn lines(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|i| format!("Line{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn expected(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|i| format!("Line{i:04}")).collect()
}

/// Every `LineNNNN` (and the first item's `1.` marker) as `(page, y_max_pt, text)`, in extraction order.
fn words(pdf: &[u8]) -> Vec<(usize, f32, String)> {
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
            && (word.starts_with("Line") || word.starts_with("BOX") || word == "1.")
        {
            let y_max = attrs
                .split_whitespace()
                .find_map(|a| a.strip_prefix("yMax=\""))
                .and_then(|v| v.trim_end_matches('"').parse::<f32>().ok())
                .expect("word without yMax");
            found.push((page, y_max, word.to_string()));
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

/// The `Line…` words of `found`, as `(page, y_max, text)`.
fn line_words(found: &[(usize, f32, String)]) -> Vec<&(usize, f32, String)> {
    found
        .iter()
        .filter(|(_, _, w)| w.starts_with("Line"))
        .collect()
}

/// `expected` lines, each exactly once, in this order, none below the content box. Returns the page count.
fn assert_paginated(pdf: &[u8], expected: &[String]) -> usize {
    let found = words(pdf);
    let lines = line_words(&found);
    let seen: Vec<&str> = lines.iter().map(|(_, _, w)| w.as_str()).collect();
    let want: Vec<&str> = expected.iter().map(String::as_str).collect();
    let first_diff = seen.iter().zip(&want).position(|(a, b)| a != b);
    assert!(
        seen == want,
        "every line exactly once, in order (extraction is page by page, top to bottom): printed {} of {} lines, \
         first difference at index {first_diff:?}, last printed {:?}",
        seen.len(),
        want.len(),
        seen.last()
    );
    let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    for (page, y, word) in &lines {
        assert!(
            *y <= limit,
            "{word} on page {page} ends at {y:.1}pt, below the content box ({limit:.1}pt)"
        );
    }
    let pages: Vec<usize> = lines.iter().map(|(p, _, _)| *p).collect();
    assert!(pages.windows(2).all(|w| w[0] <= w[1]), "pages out of order");
    page_count(pdf)
}

/// Number of lines printed on `page` (1-based).
fn lines_on(found: &[(usize, f32, String)], page: usize) -> usize {
    line_words(found)
        .iter()
        .filter(|(p, _, _)| *p == page)
        .count()
}

#[test]
fn a_notes_box_with_a_long_paragraph_continues_on_the_next_pages() {
    // The Tailwind shape from invoices: `rounded-lg border p-4` around a `<p>`.
    let pdf = render(&format!(
        "<div class=\"box\"><p>{}</p></div>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages for 400 lines of 20 px");
}

#[test]
fn direct_text_in_a_bordered_div_continues_on_the_next_pages() {
    let pdf = render(&format!("<div class=\"box\">{}</div>", lines(1..=400)));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

#[test]
fn direct_text_in_a_nested_bordered_div_continues_on_the_next_pages() {
    // Not a body child: the box is its own paragraph, so its border and padding are reserved on every page.
    let pdf = render(&format!(
        "<section><div class=\"box\">{}</div></section>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

#[test]
fn a_long_list_item_keeps_one_marker() {
    // `<ol>`: the marker is text (`1.`), which `pdftotext` can see; a disc is a vector shape.
    let pdf = render(&format!("<ol><li>{}</li></ol>", lines(1..=300)));
    let pages = assert_paginated(&pdf, &expected(1..=300));
    assert!(pages >= 6, "{pages} pages");
    let found = words(&pdf);
    let markers: Vec<_> = found.iter().filter(|(_, _, w)| w == "1.").collect();
    assert_eq!(markers.len(), 1, "one marker per item: {markers:?}");
    assert_eq!(markers[0].0, 1, "the marker belongs to the first line");
}

#[test]
fn nested_sections_with_a_long_paragraph() {
    let pdf = render(&format!(
        "<section><article><p>{}</p></article></section>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

#[test]
fn a_paragraph_in_a_table_cell() {
    let pdf = render(&format!(
        "<table><tr><td><p>{}</p></td></tr></table>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

#[test]
fn preformatted_text_in_a_div() {
    let text = (1..=400)
        .map(|i| format!("Line{i:04}"))
        .collect::<Vec<_>>()
        .join("\n");
    let pdf = render(&format!("<div class=\"box\"><pre>{text}</pre></div>"));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
    let pdf = render(&format!("<div style=\"white-space:pre-wrap\">{text}</div>"));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

#[test]
fn many_short_list_items_are_untouched() {
    // Guard: items shorter than a page keep moving whole; nothing is split or lost.
    let items: String = (1..=200).map(|i| format!("<li>Line{i:04}</li>")).collect();
    let pdf = render(&format!("<ol>{items}</ol>"));
    let found = words(&pdf);
    // Each item's marker is its own number, so only the first reads "1.".
    assert_eq!(found.iter().filter(|(_, _, w)| w == "1.").count(), 1);
    let pages = assert_paginated(&pdf, &expected(1..=200));
    assert_eq!(pages, 4, "200 x 20 px in 1009 px pages");
}

#[test]
fn a_div_around_a_paragraph_paginates_like_the_bare_paragraph() {
    let bare = render(&format!("<p>{}</p>", lines(1..=400)));
    let wrapped = render(&format!("<div><p>{}</p></div>", lines(1..=400)));
    let bare_pages = assert_paginated(&bare, &expected(1..=400));
    let wrapped_pages = assert_paginated(&wrapped, &expected(1..=400));
    assert_eq!(bare_pages, wrapped_pages);
    let (b, w) = (words(&bare), words(&wrapped));
    for page in 1..=bare_pages {
        assert_eq!(lines_on(&b, page), lines_on(&w, page), "page {page}");
    }
}

#[test]
fn the_first_page_is_filled_to_the_bottom() {
    // 100 px of other content, then a paragraph taller than the page: it starts on page 1 and takes every line
    // that fits (CSS 2.1 §13.3.3), the rest continues on page 2.
    let pdf = render(&format!(
        "<div><div style=\"height:100px\"></div><p>{}</p></div>",
        lines(1..=80)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=80));
    assert_eq!(pages, 2);
    let found = words(&pdf);
    // (1009 - 100) / 20 = 45 lines fit under the spacer.
    assert_eq!(lines_on(&found, 1), 45, "{found:?}");
    assert_eq!(lines_on(&found, 2), 35);
}

#[test]
fn a_single_line_that_fits_is_not_left_alone_at_the_bottom() {
    // Orphans (2): room for one line only below the spacer, so the paragraph starts on the next page.
    let spacer = CONTENT_PX - 30;
    let pdf = render(&format!(
        "<div><div style=\"height:{spacer}px\"></div><p>{}</p></div>",
        lines(1..=100)
    ));
    assert_paginated(&pdf, &expected(1..=100));
    let found = words(&pdf);
    assert_eq!(lines_on(&found, 1), 0, "{found:?}");
}

#[test]
fn a_single_line_is_not_left_alone_on_the_last_page() {
    // Widows (2): the greedy break would leave one line for the last page; one more moves over with it.
    // A page holds 50 lines (1009 / 20); 51 would leave 1.
    let pdf = render(&format!("<div><p>{}</p></div>", lines(1..=151)));
    let pages = assert_paginated(&pdf, &expected(1..=151));
    assert_eq!(pages, 4);
    let found = words(&pdf);
    assert!(lines_on(&found, 4) >= 2, "{found:?}");
}

#[test]
fn inline_blocks_in_a_split_paragraph_follow_their_line() {
    // Atomic inlines belong to the page that holds their line: each printed once, on that page.
    let text = (1..=130)
        .map(|i| match i {
            5 | 70 | 130 => format!(
                "Line{i:04} <span style=\"display:inline-block;border:1px solid red\">BOX{i}</span>"
            ),
            _ => format!("Line{i:04}"),
        })
        .collect::<Vec<_>>()
        .join("<br>");
    let pdf = render(&format!("<div class=\"box\"><p>{text}</p></div>"));
    assert_paginated(&pdf, &expected(1..=130));
    let found = words(&pdf);
    let boxes: Vec<(usize, &str)> = found
        .iter()
        .filter(|(_, _, w)| w.starts_with("BOX"))
        .map(|(p, _, w)| (*p, w.as_str()))
        .collect();
    assert_eq!(boxes, vec![(1, "BOX5"), (2, "BOX70"), (3, "BOX130")]);
}

#[test]
#[ignore = "F30b: a table cell with direct text is not split"]
fn direct_text_in_a_table_cell() {
    let pdf = render(&format!(
        "<table><tr><td>{}</td></tr></table>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-nps-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
