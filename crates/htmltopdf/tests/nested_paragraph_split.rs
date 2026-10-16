//! A paragraph taller than a page inside a block (`div`, `section`, `li`, `<td><p>`, a Tailwind `rounded-lg border p-4`
//! notes box) splits at line boxes like the same paragraph at body level (F30), instead of printing the lines that
//! fit its first page and drawing the rest below the page box (css-break-3 §4.1, CSS 2.1 §13.3.3).
//!
//! F30b: the same holds when the paragraph is itself a flex item, a grid item or a table cell (`flex-col > p`,
//! `grid > p`, `<td>` with direct text).
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
            && (word.starts_with("Line")
                || word.starts_with("BOX")
                || word.starts_with("BLine")
                || matches!(
                    word,
                    "1." | "right" | "AFTER" | "Item" | "NEXTROW" | "short"
                ))
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
    // The `ol`'s 16 px top margin collapses through <body> and shifts page 0 (F29): 49 + 50 + 50 + 50 + 1 items,
    // the pages Chrome prints. Before F29 the 50th item hung 8 px below the content box and the count was 4.
    assert_eq!(pages, 5, "16 px + 200 x 20 px in 1009 px pages");
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
fn the_body_margin_is_not_extra_room_on_the_first_page() {
    // The renderer shifts page 0 down by the body's own offset, so the first page holds less.
    for margin in ["8px", "40px", "80px"] {
        let pdf = render_with(
            &format!("body {{ margin: {margin} }}"),
            &format!("<div class=\"box\"><p>{}</p></div>", lines(1..=300)),
        );
        assert_paginated(&pdf, &expected(1..=300));
    }
}

#[test]
fn a_margin_collapsed_through_the_body_is_not_extra_room_either() {
    for margin in [40, 80, 120] {
        let pdf = render(&format!(
            "<div style=\"margin-top:{margin}px;height:10px\"></div><div class=\"box\"><p>{}</p></div>",
            lines(1..=300)
        ));
        assert_paginated(&pdf, &expected(1..=300));
        let pdf = render(&format!(
            "<div style=\"margin-top:{margin}px\"><p>{}</p></div>",
            lines(1..=300)
        ));
        assert_paginated(&pdf, &expected(1..=300));
    }
}

/// A page rasterised at 72 dpi (1 px = 1 pt) as `(width, height, RGB bytes)`.
fn page_rgb(pdf: &[u8], page: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let prefix = path.with_extension("");
    let status = std::process::Command::new("pdftoppm")
        .args([
            "-r",
            "72",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
        ])
        .arg(&path)
        .arg(&prefix)
        .status()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(status.success(), "pdftoppm failed");
    let ppm = prefix.with_extension("ppm");
    let bytes = std::fs::read(&ppm).expect("ppm");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&ppm);
    // "P6\n<w> <h>\n255\n" followed by the raw pixels.
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while !bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&bytes[start..at]).into_owned());
    }
    (
        fields[1].parse().unwrap(),
        fields[2].parse().unwrap(),
        bytes[at + 1..].to_vec(),
    )
}

/// Lowest pixel row on `page` holding the box's red border (#c00) or its fill (#ffe9a8).
fn lowest_box_row(pdf: &[u8], page: usize) -> usize {
    let (w, h, rgb) = page_rgb(pdf, page);
    let near = |p: &[u8], c: [u8; 3]| (0..3).all(|i| p[i].abs_diff(c[i]) < 12);
    (0..h)
        .rev()
        .find(|&y| {
            (0..w).any(|x| {
                let p = &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
                near(p, [0xcc, 0, 0]) || near(p, [0xff, 0xe9, 0xa8])
            })
        })
        .expect("no box on the page")
}

const FRAME_CSS: &str =
    ".box { border: 2px solid #c00; border-radius: 12px; padding: 16px; background: #ffe9a8 }";

#[test]
fn the_frame_of_a_box_around_wrappers_ends_at_the_content_bottom_on_every_page() {
    // `.box > div > p` and `.box > ul > li`: the paragraph is two levels below the box, whose frame on
    // the pages it leaves must end at the content bottom (799 pt), not run on to the page edge.
    let content_bottom = (PAGE_H_PT - MARGIN_PT).round() as usize;
    for body in [
        format!(
            "<div class=\"box\"><div><p>{}</p></div></div><p>After</p>",
            lines(1..=200)
        ),
        format!(
            "<div class=\"box\"><ul><li>{}</li></ul></div><p>After</p>",
            lines(1..=200)
        ),
        format!(
            "<div class=\"box\"><h3>Title</h3><div><p>{}</p></div></div>",
            lines(1..=200)
        ),
        format!(
            "<section><article><div class=\"box\"><div><p>{}</p></div></div></article></section>",
            lines(1..=200)
        ),
    ] {
        let pdf = render_with(FRAME_CSS, &body);
        let pages = assert_paginated(&pdf, &expected(1..=200));
        assert!(pages >= 4, "{pages}");
        for page in 1..pages {
            let bottom = lowest_box_row(&pdf, page);
            assert!(
                bottom <= content_bottom + 1 && bottom + 8 >= content_bottom,
                "page {page}: the frame ends at row {bottom}, the content bottom is {content_bottom}: {body}"
            );
        }
    }
}

#[test]
fn the_last_frame_stays_inside_the_content_box() {
    // However many lines the last page gets, the bottom padding and border of the box fit on it.
    let content_bottom = (PAGE_H_PT - MARGIN_PT).round() as usize;
    for n in 140..=160 {
        let pdf = render_with(
            FRAME_CSS,
            &format!("<div class=\"box\"><p>{}</p></div>", lines(1..=n)),
        );
        let pages = assert_paginated(&pdf, &expected(1..=n));
        let bottom = lowest_box_row(&pdf, pages);
        assert!(
            bottom <= content_bottom + 1,
            "{n} lines: the box ends at row {bottom} on page {pages}, the content bottom is {content_bottom}"
        );
    }
}

#[test]
fn a_paragraph_in_a_cell_repeats_the_table_header_on_every_page() {
    let rows = format!(
        "<table style=\"width:100%\"><thead><tr><th>HEADERA</th><th>HEADERB</th></tr></thead>\
         <tbody><tr><td>Left</td><td><p>{}</p></td></tr><tr><td>Row2</td><td>After</td></tr></tbody></table>",
        lines(1..=150)
    );
    let pdf = render(&rows);
    let pages = assert_paginated(&pdf, &expected(1..=150));
    // 150 lines of 20 px below a one-line header: 3 pages and a bit.
    assert_eq!(pages, 4, "the row starts on page 1 instead of moving whole");
    for page in 1..=pages {
        let path = temp(&pdf);
        let out = std::process::Command::new("pdftotext")
            .args(["-f", &page.to_string(), "-l", &page.to_string()])
            .arg(&path)
            .arg("-")
            .output()
            .expect("pdftotext");
        let _ = std::fs::remove_file(&path);
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(text.matches("HEADERA").count(), 1, "page {page}: {text:?}");
    }
    let found = words(&pdf);
    assert!(
        lines_on(&found, 1) >= 45,
        "page 1 is filled: {}",
        lines_on(&found, 1)
    );
}

fn word_at(found: &[(usize, f32, String)], text: &str) -> (usize, f32) {
    found
        .iter()
        .find(|(_, _, w)| w == text)
        .map(|(p, y, _)| (*p, *y))
        .unwrap_or_else(|| panic!("{text} not printed"))
}

#[test]
fn cells_after_a_split_cell_sit_at_the_top_of_the_row() {
    // The cell to the right of a `<td><p>` that continues on later pages stays beside the start of the row,
    // on its first page, not after the end of the split cell; what follows the table follows the split cell.
    for cols in [
        "<td><p>{L}</p></td><td>right</td>",
        "<td><p>{L}</p></td><td>x</td><td>right</td>",
        "<td>x</td><td><p>{L}</p></td><td>right</td>",
    ] {
        let row = cols.replace("{L}", &lines(1..=120));
        // `vertical-align: top`: a cell centred in a row taller than a page loses its text (a known gap).
        let pdf = render_with(
            "td, th { vertical-align: top }",
            &format!(
                "<table style=\"width:100%\"><thead><tr><th>HEAD</th><th>H2</th></tr></thead><tbody><tr>{row}</tr></tbody></table><p>AFTER</p>"
            ),
        );
        let pages = assert_paginated(&pdf, &expected(1..=120));
        let found = words(&pdf);
        let (right_page, right_y) = word_at(&found, "right");
        let (_, first_y) = word_at(&found, "Line0001");
        let (after_page, after_y) = word_at(&found, "AFTER");
        let (_, last_y) = word_at(&found, "Line0120");
        assert_eq!(right_page, 1, "{cols}");
        assert!(
            (right_y - first_y).abs() <= 1.0,
            "{cols}: right {right_y} vs first line {first_y}"
        );
        assert_eq!((after_page, pages), (3, 3), "{cols}");
        assert!(after_y > last_y, "{cols}: AFTER below the last line");
    }
}

#[test]
fn bottom_padding_taller_than_a_page_overflows_instead_of_adding_pages() {
    let content_bottom = (PAGE_H_PT - MARGIN_PT).round() as usize;
    for (pb, n) in [(1100, 60), (1500, 100), (600, 60)] {
        let pdf = render_with(
            FRAME_CSS,
            &format!(
                "<div class=\"box\" style=\"padding-bottom:{pb}px\"><div><p>{}</p></div></div><p>AFTER</p>",
                lines(1..=n)
            ),
        );
        let pages = assert_paginated(&pdf, &expected(1..=n));
        // The lines take ceil(n / 48) pages (the box's frame takes some of the first) and AFTER at most one more.
        assert!(pages <= n.div_ceil(48) + 1, "pb {pb}: {pages} pages");
        for page in 1..=pages {
            // The frame never runs on below the content box (a page that is only AFTER has none).
            let found = words(&pdf);
            if lines_on(&found, page) == 0 {
                continue;
            }
            let bottom = lowest_box_row(&pdf, page);
            assert!(
                bottom <= content_bottom + 1,
                "pb {pb}, page {page}: frame ends at row {bottom}"
            );
        }
    }
}

#[test]
fn direct_text_in_a_table_cell() {
    let pdf = render(&format!(
        "<table><tr><td>{}</td></tr></table>",
        lines(1..=400)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=400));
    assert!(pages >= 8, "{pages} pages");
}

/// `(page, y)` of the word `later` is below the word `earlier` in reading order.
fn assert_below(found: &[(usize, f32, String)], later: &str, earlier: &str) {
    let (lp, ly) = word_at(found, later);
    let (ep, ey) = word_at(found, earlier);
    assert!(
        lp > ep || (lp == ep && ly > ey),
        "{later} (page {lp}, y {ly}) is not below {earlier} (page {ep}, y {ey})"
    );
}

const FLEX_COL: &str = "display:flex;flex-direction:column";

#[test]
fn a_paragraph_that_is_a_flex_column_item_continues_on_the_next_pages() {
    let pdf = render(&format!(
        "<div style=\"{FLEX_COL}\"><p>{}</p><p>AFTER</p></div>",
        lines(1..=300)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=300));
    // Chrome (probe `flexcol_p`): 50 lines per page, 7 pages.
    assert_eq!(pages, 7);
    assert_below(&words(&pdf), "AFTER", "Line0300");
}

#[test]
fn direct_text_in_a_flex_column_item_continues_on_the_next_pages() {
    let pdf = render(&format!(
        "<div style=\"{FLEX_COL}\"><div class=\"box\">{}</div></div>",
        lines(1..=300)
    ));
    let pages = assert_paginated(&pdf, &expected(1..=300));
    let plain = render(&format!("<div class=\"box\">{}</div>", lines(1..=300)));
    assert_eq!(pages, assert_paginated(&plain, &expected(1..=300)));
}

#[test]
fn a_paragraph_that_is_a_grid_item_continues_on_the_next_pages() {
    let pdf = render(&format!(
        "<div style=\"display:grid;gap:16px\"><h3>T</h3><p>{}</p><p>AFTER</p></div>",
        lines(1..=300)
    ));
    assert_paginated(&pdf, &expected(1..=300));
    assert_below(&words(&pdf), "AFTER", "Line0300");
}

#[test]
fn flex_and_grid_items_split_like_the_same_paragraph_in_a_div() {
    let reference = render(&format!("<div><p>{}</p></div>", lines(1..=300)));
    let pages = assert_paginated(&reference, &expected(1..=300));
    let r = words(&reference);
    for wrapper in [FLEX_COL, "display:grid"] {
        let pdf = render(&format!(
            "<div style=\"{wrapper}\"><p>{}</p></div>",
            lines(1..=300)
        ));
        assert_eq!(
            assert_paginated(&pdf, &expected(1..=300)),
            pages,
            "{wrapper}"
        );
        let w = words(&pdf);
        for page in 1..=pages {
            assert_eq!(
                lines_on(&r, page),
                lines_on(&w, page),
                "{wrapper}, page {page}"
            );
        }
    }
}

#[test]
fn a_padded_flex_column_box_closes_below_its_split_item() {
    // The box is block flow for fragmentation (css-flexbox-1 §12): its frame ends at the content bottom on the
    // pages it leaves and the last frame, with its bottom padding and border, stays inside the content box.
    let content_bottom = (PAGE_H_PT - MARGIN_PT).round() as usize;
    for n in 140..=160 {
        let pdf = render_with(
            FRAME_CSS,
            &format!(
                "<div class=\"box\" style=\"{FLEX_COL};gap:16px\"><h3>T</h3><p>{}</p></div>",
                lines(1..=n)
            ),
        );
        let pages = assert_paginated(&pdf, &expected(1..=n));
        assert!(pages >= 3, "{n}: {pages} pages");
        for page in 1..pages {
            let bottom = lowest_box_row(&pdf, page);
            assert!(
                bottom <= content_bottom + 1 && bottom + 8 >= content_bottom,
                "{n} lines, page {page}: the frame ends at row {bottom}, the content bottom is {content_bottom}"
            );
        }
        let bottom = lowest_box_row(&pdf, pages);
        assert!(
            bottom <= content_bottom + 1,
            "{n} lines: the box ends at row {bottom} on page {pages}, the content bottom is {content_bottom}"
        );
    }
}

#[test]
fn a_paragraph_beside_a_square_in_a_flex_row_paginates_like_one_in_a_div() {
    let row = |second: String| {
        let pdf = render(&format!(
            "<div style=\"display:flex;align-items:flex-start;gap:16px\">\
             <div style=\"width:20px;height:20px;background:#ccc\"></div>{second}<p>AFTER</p></div>",
        ));
        assert_paginated(&pdf, &expected(1..=300))
    };
    let p = row(format!("<p>{}</p>", lines(1..=300)));
    let div = row(format!("<div><p>{}</p></div>", lines(1..=300)));
    assert_eq!(p, div);
}

#[test]
fn cells_with_direct_text_after_a_split_cell_sit_at_the_top_of_the_row() {
    for cols in [
        "<td>{L}</td><td>right</td>",
        "<td>{L}</td><td>x</td><td>right</td>",
        "<td>x</td><td>{L}</td><td>right</td>",
    ] {
        let row = cols.replace("{L}", &lines(1..=120));
        let pdf = render_with(
            "td, th { vertical-align: top }",
            &format!(
                "<table style=\"width:100%\"><thead><tr><th>HEAD</th><th>H2</th></tr></thead><tbody><tr>{row}</tr></tbody></table><p>AFTER</p>"
            ),
        );
        let pages = assert_paginated(&pdf, &expected(1..=120));
        let found = words(&pdf);
        let (right_page, right_y) = word_at(&found, "right");
        let (_, first_y) = word_at(&found, "Line0001");
        let (after_page, after_y) = word_at(&found, "AFTER");
        let (_, last_y) = word_at(&found, "Line0120");
        assert_eq!(right_page, 1, "{cols}");
        assert!(
            (right_y - first_y).abs() <= 1.0,
            "{cols}: right {right_y} vs first line {first_y}"
        );
        assert_eq!((after_page, pages), (3, 3), "{cols}");
        assert!(after_y > last_y, "{cols}: AFTER below the last line");
    }
}

#[test]
fn direct_text_in_a_cell_repeats_the_table_header_on_every_page() {
    let rows = format!(
        "<table style=\"width:100%\"><thead><tr><th>HEADERA</th><th>HEADERB</th></tr></thead>\
         <tbody><tr><td>Left</td><td>{}</td></tr><tr><td>Row2</td><td>After</td></tr></tbody></table>",
        lines(1..=150)
    );
    let pdf = render(&rows);
    let pages = assert_paginated(&pdf, &expected(1..=150));
    assert_eq!(pages, 4, "the row starts on page 1 instead of moving whole");
    for page in 1..=pages {
        let path = temp(&pdf);
        let out = std::process::Command::new("pdftotext")
            .args(["-f", &page.to_string(), "-l", &page.to_string()])
            .arg(&path)
            .arg("-")
            .output()
            .expect("pdftotext");
        let _ = std::fs::remove_file(&path);
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(text.matches("HEADERA").count(), 1, "page {page}: {text:?}");
    }
    let found = words(&pdf);
    assert!(
        lines_on(&found, 1) >= 45,
        "page 1 is filled: {}",
        lines_on(&found, 1)
    );
}

#[test]
fn a_row_background_covers_a_split_direct_text_cell_on_every_page() {
    // `<tr>` backgrounds are painted from the cell frames: on page 2 the fill starts at the content top, not
    // after the line span or at the padding edge of the first page.
    let pdf = render_with(
        "tr { background: #ffe9a8 } td { padding: 16px }",
        &format!(
            "<table style=\"width:100%\"><tr><td>{}</td></tr></table>",
            lines(1..=120)
        ),
    );
    let pages = assert_paginated(&pdf, &expected(1..=120));
    assert!(pages >= 3, "{pages}");
    let (w, h, rgb) = page_rgb(&pdf, 2);
    let first_fill = (0..h)
        .find(|&y| {
            (0..w).any(|x| {
                let p = &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
                [0xffu8, 0xe9, 0xa8]
                    .iter()
                    .zip(p)
                    .all(|(a, b)| a.abs_diff(*b) < 12)
            })
        })
        .expect("no fill on page 2");
    let top = MARGIN_PT.round() as usize;
    assert!(
        first_fill.abs_diff(top) <= 1,
        "the fill starts at row {first_fill}, the content top is {top}"
    );
}

#[test]
#[ignore = "F35: a cell centred in a row taller than a page is not placed beside the start of the row"]
fn a_short_cell_beside_a_split_cell_stays_on_the_first_page_when_centred() {
    let pdf = render(&format!(
        "<table style=\"width:100%\"><tr><td>Item</td><td>{}</td></tr></table>",
        lines(1..=120)
    ));
    assert_paginated(&pdf, &expected(1..=120));
    let found = words(&pdf);
    let (item_page, item_y) = word_at(&found, "Item");
    let (_, first_y) = word_at(&found, "Line0001");
    assert_eq!(item_page, 1);
    assert!(
        (item_y - first_y).abs() <= 1.0,
        "Item {item_y} vs Line0001 {first_y}"
    );
}

/// `BLine0001<br>…`: the second tall item of a row.
fn b_lines(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|i| format!("BLine{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

/// `BLine…` words of `found` as `(page, y_max, text)`.
fn b_words(found: &[(usize, f32, String)]) -> Vec<&(usize, f32, String)> {
    found
        .iter()
        .filter(|(_, _, w)| w.starts_with("BLine"))
        .collect()
}

/// Two (or three) tall siblings in one row, the first taller than the others: every line of both once and
/// in order, the row ends where its tallest item ends and what follows starts below it (Chrome: A on pages
/// 1-3, B on pages 1-2, AFTER on page 3).
fn assert_row_of_two_tall(pdf: &[u8], what: &str) {
    let pages = assert_paginated(pdf, &expected(1..=120));
    assert_eq!(pages, 3, "{what}");
    let found = words(pdf);
    let b: Vec<&str> = b_words(&found).iter().map(|(_, _, w)| w.as_str()).collect();
    let want: Vec<String> = (1..=80).map(|i| format!("BLine{i:04}")).collect();
    assert_eq!(
        b,
        want.iter().map(String::as_str).collect::<Vec<_>>(),
        "{what}: B lines"
    );
    let last_b = b_words(&found).last().map(|(p, _, _)| *p).unwrap();
    assert_eq!(last_b, 2, "{what}: B ends on page 2");
    assert_eq!(lines_on(&found, 1), 50, "{what}");
    assert_below(&found, "AFTER", "Line0120");
}

#[test]
fn a_row_of_two_tall_direct_text_cells_ends_with_the_taller_one() {
    for (what, cells) in [
        (
            "two cells",
            format!("<td>{}</td><td>{}</td>", lines(1..=120), b_lines(1..=80)),
        ),
        (
            "three cells",
            format!(
                "<td>{}</td><td>{}</td><td>short</td>",
                lines(1..=120),
                b_lines(1..=80)
            ),
        ),
        (
            "taller last",
            format!("<td>{}</td><td>{}</td>", b_lines(1..=80), lines(1..=120)),
        ),
    ] {
        let pdf = render_with(
            "td { vertical-align: top }",
            &format!("<table style=\"width:100%\"><tr>{cells}</tr></table><p>AFTER</p>"),
        );
        assert_row_of_two_tall(&pdf, what);
    }
}

#[test]
fn the_row_after_a_row_of_two_tall_direct_text_cells_starts_below_the_taller_one() {
    let pdf = render_with(
        "td { vertical-align: top }",
        &format!(
            "<table style=\"width:100%\"><tr><td>{}</td><td>{}</td></tr><tr><td>NEXTROW</td><td>x</td></tr></table>",
            lines(1..=120),
            b_lines(1..=80)
        ),
    );
    let pages = assert_paginated(&pdf, &expected(1..=120));
    assert_eq!(pages, 3);
    assert_below(&words(&pdf), "NEXTROW", "Line0120");
}

#[test]
fn a_flex_or_grid_row_of_two_tall_paragraphs_ends_with_the_taller_one() {
    // The first item is the taller one and both split: what follows the row starts below the taller item.
    for row in [
        "display:flex;align-items:flex-start;gap:16px",
        "display:grid;grid-template-columns:1fr 1fr;align-items:start;gap:16px",
    ] {
        let pdf = render(&format!(
            "<div style=\"{row}\"><p>{}</p><p>{}</p></div><p>AFTER</p>",
            lines(1..=120),
            b_lines(1..=80)
        ));
        assert_row_of_two_tall(&pdf, row);
        let pdf = render(&format!(
            "<div style=\"{row}\"><div><p>{}</p></div><div><p>{}</p></div></div><p>AFTER</p>",
            lines(1..=120),
            b_lines(1..=80)
        ));
        assert_row_of_two_tall(&pdf, row);
    }
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-nps-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
