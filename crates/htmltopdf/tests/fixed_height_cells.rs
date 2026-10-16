//! A table cell (or table) with a length `height` smaller than its content (F48). CSS 2.1 §17.5.3: a row is at
//! least as tall as each cell's computed `height` and at least as tall as the cells' content needs, and a table's
//! non-`auto` `height` is a minimum; so `td { height: 40px }` (Tailwind `h-12` under preflight) must neither clip
//! the content nor let it overlap the next row (§10.7: `max-height` on cells and tables is undefined, we ignore
//! it so content wins). Before F48 the row was only as tall as the `height`, the content overflowed it, and a
//! cell taller than a page was never entered by the fragmenter: its lines were lost.
//!
//! Asserted on word positions (`pdftotext -bbox`). Lines are forced with `<br>` (20 px = 15 pt each) so the numbers
//! do not depend on font metrics. "Paginates": every line once, in order, none below the content box, AFTER below
//! the last line, the page count of the same table without `height`.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin the renderer applies.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt.
const SLACK_PT: f32 = 5.0;
/// Tolerance for "same position", in pt.
const TOL: f32 = 1.0;

const CSS: &str = "body { margin: 0 } p { margin: 0; line-height: 20px } \
    td { padding: 4px; vertical-align: top; line-height: 20px } table { width: 100%; border-spacing: 0 }";

fn render(body: &str) -> Vec<u8> {
    render_with("", body)
}

/// `extra_css` is appended after the base CSS, so it wins.
fn render_with(extra_css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(format!("{CSS} {extra_css}"))
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// `Line0001<br>Line0002<br>…`: one line box per token.
fn lines(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|i| format!("Line{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn b_lines(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|i| format!("BLine{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn expected(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|i| format!("Line{i:04}")).collect()
}

/// Every `Line…`, `BLine…`, `NEXT` and `AFTER` word as `(page, y_max, text)`, in extraction order.
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
            && (word.starts_with("Line")
                || word.starts_with("BLine")
                || word.starts_with("NEXT")
                || word == "AFTER")
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

/// The words of `found` starting with `prefix`, with `Line` not matching `BLine`.
fn with_prefix<'a>(
    found: &'a [(usize, f32, String)],
    prefix: &str,
) -> Vec<&'a (usize, f32, String)> {
    found
        .iter()
        .filter(|(_, _, w)| w.starts_with(prefix))
        .collect()
}

/// `expected` `Line…` words, each exactly once, in this order, none below the content box. Returns the page count.
fn assert_paginated(pdf: &[u8], expected: &[String]) -> usize {
    let found = words(pdf);
    let lines = with_prefix(&found, "Line");
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
    for (page, y, word) in with_prefix(&found, "Line")
        .into_iter()
        .chain(with_prefix(&found, "BLine"))
    {
        assert!(
            *y <= limit,
            "{word} on page {page} ends at {y:.1}pt, below the content box ({limit:.1}pt)"
        );
    }
    let pages: Vec<usize> = lines.iter().map(|(p, _, _)| *p).collect();
    assert!(pages.windows(2).all(|w| w[0] <= w[1]), "pages out of order");
    page_count(pdf)
}

fn word_at(found: &[(usize, f32, String)], text: &str) -> (usize, f32) {
    found
        .iter()
        .find(|(_, _, w)| w == text)
        .map(|(p, y, _)| (*p, *y))
        .unwrap_or_else(|| panic!("{text} not printed"))
}

/// `later` is below `earlier` in reading order.
fn assert_below(found: &[(usize, f32, String)], later: &str, earlier: &str) {
    let (lp, ly) = word_at(found, later);
    let (ep, ey) = word_at(found, earlier);
    assert!(
        lp > ep || (lp == ep && ly > ey),
        "{later} (page {lp}, y {ly}) is not below {earlier} (page {ep}, y {ey})"
    );
}

/// `NEXT` sits `px` CSS px below `Line0001` on the same page.
fn assert_next_row_at(pdf: &[u8], px: f32) {
    let found = words(pdf);
    let (p1, y1) = word_at(&found, "Line0001");
    let (p2, y2) = word_at(&found, "NEXT");
    assert_eq!(p1, p2, "same page");
    let want = px * 0.75;
    assert!(
        (y2 - y1 - want).abs() <= TOL,
        "NEXT is {:.1}pt below Line0001, expected {want:.1}pt ({px} px)",
        y2 - y1
    );
}

/// A row of one cell with `cell_attrs`, holding `content`, then a row `NEXT`.
fn two_rows(cell_attrs: &str, content: &str) -> String {
    format!("<table><tr><td {cell_attrs}>{content}</td></tr><tr><td>NEXT</td></tr></table>")
}

// ---- a cell whose height is smaller than its content --------------------------------------------------------

#[test]
fn a_cell_height_smaller_than_the_content_is_only_a_minimum() {
    // Five 20 px lines + 2 x 4 px padding = a 108 px row, not the 48 px of `height: 40px` plus padding.
    let pdf = render(&two_rows("style=\"height:40px\"", &lines(1..=5)));
    assert_next_row_at(&pdf, 108.0);
}

#[test]
fn the_same_under_border_box() {
    // Tailwind preflight's `box-sizing: border-box` (`h-12`): the 40 px minimum includes the padding, the content
    // is still 100 px.
    let pdf = render_with(
        "td { box-sizing: border-box }",
        &two_rows("style=\"height:40px\"", &lines(1..=5)),
    );
    assert_next_row_at(&pdf, 108.0);
}

#[test]
fn a_cell_height_larger_than_the_content_still_sets_the_row_height() {
    // Guard (F13): `height` is a minimum. content-box: 60 + 2 x 4 = 68 px; border-box: 60 px.
    let pdf = render(&two_rows("style=\"height:60px\"", "Line0001"));
    assert_next_row_at(&pdf, 68.0);
    let pdf = render_with(
        "td { box-sizing: border-box }",
        &two_rows("style=\"height:60px\"", "Line0001"),
    );
    assert_next_row_at(&pdf, 60.0);
}

#[test]
fn a_cell_height_smaller_than_a_paragraph_taller_than_a_page_paginates() {
    // z_tdh_one: one cell, one 120-line `<p>`. Before F48: 53 lines on 1 page, AFTER drawn over them.
    let table = |attrs: &str| {
        format!(
            "<table><tr><td {attrs}><p>{}</p></td></tr></table><p>AFTER</p>",
            lines(1..=120)
        )
    };
    let pdf = render(&table("style=\"height:40px\""));
    let pages = assert_paginated(&pdf, &expected(1..=120));
    assert_below(&words(&pdf), "AFTER", "Line0120");
    let plain = render(&table(""));
    assert_eq!(pages, assert_paginated(&plain, &expected(1..=120)));
}

#[test]
fn two_cells_with_a_small_height_and_long_paragraphs_paginate() {
    // y_tdh_small: A x 120, B x 80, both `height: 40px`. Before F48: 94 words lost.
    let pdf = render(&format!(
        "<table><tr><td style=\"height:40px\"><p>{}</p></td><td style=\"height:40px\"><p>{}</p></td></tr></table>\
         <p>AFTER</p>",
        lines(1..=120),
        b_lines(1..=80)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=120));
    let found = words(&pdf);
    let b: Vec<&str> = with_prefix(&found, "BLine")
        .iter()
        .map(|(_, _, w)| w.as_str())
        .collect();
    let want: Vec<String> = (1..=80).map(|i| format!("BLine{i:04}")).collect();
    assert_eq!(
        b,
        want.iter().map(String::as_str).collect::<Vec<_>>(),
        "B lines"
    );
    assert_below(&found, "AFTER", "Line0120");
    assert_eq!(pages, 3);
}

#[test]
fn a_cell_max_height_does_not_clip_or_overlap() {
    // CSS 2.1 §10.7: undefined on cells; the content wins.
    let pdf = render(&two_rows("style=\"max-height:40px\"", &lines(1..=5)));
    assert_next_row_at(&pdf, 108.0);
    let pdf = render(&format!(
        "<table><tr><td style=\"max-height:40px\"><p>{}</p></td></tr></table><p>AFTER</p>",
        lines(1..=120)
    ));
    assert_paginated(&pdf, &expected(1..=120));
    assert_below(&words(&pdf), "AFTER", "Line0120");
}

#[test]
fn a_table_height_smaller_than_its_rows_is_only_a_minimum() {
    // CSS 2.1 §17.5.3: the table is at least as tall as its rows.
    let rows: String = (1..=5)
        .map(|i| format!("<tr><td>Line{i:04}</td></tr>"))
        .collect();
    let pdf = render(&format!(
        "<table style=\"height:40px\">{rows}</table><p>AFTER</p>"
    ));
    assert_below(&words(&pdf), "AFTER", "Line0005");
}

// ---- a table's own height (F48 follow-up: border and padding are inside it, or outside it, like in Chrome) ----

/// `AFTER` sits `px` CSS px below `Line0001` on the same page.
fn assert_after_below_first(pdf: &[u8], px: f32, what: &str) {
    let found = words(pdf);
    let (p1, y1) = word_at(&found, "Line0001");
    let (p2, y2) = word_at(&found, "AFTER");
    assert_eq!(p1, p2, "{what}: same page");
    let want = px * 0.75;
    assert!(
        (y2 - y1 - want).abs() <= TOL,
        "{what}: AFTER is {:.1}pt below Line0001, expected {want:.1}pt ({px} px)",
        y2 - y1
    );
}

/// A 500 px wide table with `style`, one cell of three 20 px lines (60 + 2 x 4 px padding), then `AFTER`.
fn sized_table(style: &str) -> Vec<u8> {
    render(&format!(
        "<table style=\"width:500px;{style}\"><tr><td>{}</td></tr></table><p>AFTER</p>",
        lines(1..=3)
    ))
}

const FRAME: &str = "border:6px solid #888;padding:10px";

#[test]
fn a_table_height_includes_its_border_and_padding_under_border_box() {
    // Chrome 141: the table is 100 px tall in total, so `Line0001` (6 + 10 + 4 px below the top) is 80 px above AFTER.
    for prop in ["height", "min-height"] {
        let pdf = sized_table(&format!("box-sizing:border-box;{prop}:100px;{FRAME}"));
        assert_after_below_first(&pdf, 80.0, prop);
    }
    // Tailwind's `h-32 border` shape, scaled down: only a border, only padding.
    let pdf = sized_table("box-sizing:border-box;height:100px;border:1px solid #888");
    assert_after_below_first(&pdf, 95.0, "border only");
    let pdf = sized_table("box-sizing:border-box;height:100px;padding:10px");
    assert_after_below_first(&pdf, 86.0, "padding only");
}

#[test]
fn a_table_height_is_the_content_box_under_content_box() {
    // 100 px of content box + 2 x (6 + 10) px = 132 px in total.
    for prop in ["height", "min-height"] {
        let pdf = sized_table(&format!("box-sizing:content-box;{prop}:100px;{FRAME}"));
        assert_after_below_first(&pdf, 112.0, prop);
    }
}

#[test]
fn a_collapsed_border_table_height_includes_the_border_under_border_box() {
    let pdf = sized_table(
        "box-sizing:border-box;height:100px;border:1px solid #888;border-collapse:collapse",
    );
    assert_after_below_first(&pdf, 95.0, "1px");
}

#[test]
fn a_table_height_smaller_than_its_rows_with_a_frame_changes_nothing() {
    // Guard: 68 px of row + 32 px of frame = 100 px, whatever the (smaller) height says.
    for bs in ["border-box", "content-box"] {
        let pdf = sized_table(&format!("box-sizing:{bs};height:30px;{FRAME}"));
        assert_after_below_first(&pdf, 80.0, bs);
    }
}

#[test]
fn a_display_table_div_height_includes_its_frame_under_border_box() {
    // Chrome 141: no cell padding on a `display: table-cell` div; 100 px in total, `Line0001` 16 px below the top.
    let div = |bs: &str| {
        render(&format!(
            "<div style=\"display:table;width:500px;height:100px;box-sizing:{bs};{FRAME}\"><div style=\"display:table-row\">\
             <div style=\"display:table-cell\"><p>{}</p></div></div></div><p>AFTER</p>",
            lines(1..=3)
        ))
    };
    assert_after_below_first(&div("border-box"), 84.0, "border-box");
    assert_after_below_first(&div("content-box"), 116.0, "content-box");
}

// ---- a table that is a flex or grid item ---------------------------------------------------------------------

/// Two tables of `height:30px` with five lines each, in `wrapper`, then AFTER.
fn two_tables_in(wrapper: &str, table_style: &str) -> Vec<u8> {
    let table = format!(
        "<table style=\"height:30px;{table_style}\"><tr><td>{}</td></tr></table>",
        lines(1..=5)
    );
    render(&format!(
        "<div style=\"{wrapper}\">{table}{table}</div><p>AFTER</p>"
    ))
}

#[test]
fn a_table_height_smaller_than_its_rows_is_a_minimum_for_a_flex_or_grid_item_too() {
    // 5 lines + 2 x 4 px padding = 108 px; `Line0001` is 4 px below the top. Chrome 141: 104 px (row, grid); in a
    // column the second table follows the first one: 108 + 104.
    for (what, wrapper, style, px) in [
        ("flex row", "display:flex", "width:50%", 104.0),
        (
            "flex column",
            "display:flex;flex-direction:column",
            "",
            212.0,
        ),
        (
            "grid",
            "display:grid;grid-template-columns:1fr 1fr",
            "",
            104.0,
        ),
    ] {
        assert_after_below_first(&two_tables_in(wrapper, style), px, what);
    }
}

#[test]
fn a_display_table_div_that_is_a_flex_item_is_not_clamped_to_its_height() {
    let cell = format!(
        "<div style=\"display:table-cell\"><p>{}</p></div>",
        lines(1..=5)
    );
    let one = format!(
        "<div style=\"display:table;height:30px;width:50%\"><div style=\"display:table-row\">{cell}</div></div>"
    );
    let pdf = render(&format!(
        "<div style=\"display:flex\">{one}{one}</div><p>AFTER</p>"
    ));
    // No cell padding: 100 px of lines, Chrome 141: AFTER 100 px below the first line.
    assert_after_below_first(&pdf, 100.0, "display:table div");
}

#[test]
fn a_taller_table_height_for_a_flex_item_keeps_working() {
    // Guard (green before and after): the item is 200 px (border-box) or 200 + 32 px (content-box) tall.
    let table = |bs: &str| {
        render(&format!(
            "<div style=\"display:flex\"><table style=\"height:200px;width:50%;box-sizing:{bs};{FRAME}\"><tr><td>{}</td></tr></table></div><p>AFTER</p>",
            lines(1..=5)
        ))
    };
    assert_after_below_first(&table("border-box"), 180.0, "border-box");
    assert_after_below_first(&table("content-box"), 212.0, "content-box");
}

// ---- guards: green before and after -------------------------------------------------------------------------

#[test]
fn a_cell_min_height_with_a_long_paragraph_paginates() {
    let pdf = render(&format!(
        "<table><tr><td style=\"min-height:40px\"><p>{}</p></td></tr></table><p>AFTER</p>",
        lines(1..=120)
    ));
    assert_paginated(&pdf, &expected(1..=120));
    assert_below(&words(&pdf), "AFTER", "Line0120");
}

#[test]
fn a_row_height_with_a_long_paragraph_paginates() {
    let pdf = render(&format!(
        "<table><tr style=\"height:40px\"><td><p>{}</p></td></tr></table><p>AFTER</p>",
        lines(1..=120)
    ));
    assert_paginated(&pdf, &expected(1..=120));
    assert_below(&words(&pdf), "AFTER", "Line0120");
}

#[test]
fn a_cell_height_taller_than_a_page_with_a_long_paragraph_paginates() {
    let pdf = render(&format!(
        "<table><tr><td style=\"height:1500px\"><p>{}</p></td></tr></table><p>AFTER</p>",
        lines(1..=120)
    ));
    assert_paginated(&pdf, &expected(1..=120));
    assert_below(&words(&pdf), "AFTER", "Line0120");
}

#[test]
fn a_middle_aligned_cell_with_a_small_height_lays_out_like_a_top_aligned_one() {
    let body = format!(
        "<table><tr><td style=\"height:40px\"><p>{}</p></td></tr></table><p>AFTER</p>",
        lines(1..=120)
    );
    let top = words(&render(&body));
    let middle = words(&render_with("td { vertical-align: middle }", &body));
    assert_eq!(top.len(), middle.len());
    for (t, m) in top.iter().zip(&middle) {
        assert_eq!((t.0, &t.2), (m.0, &m.2));
        assert!((t.1 - m.1).abs() <= TOL, "{}: {} vs {}", t.2, t.1, m.1);
    }
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-fhc-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
