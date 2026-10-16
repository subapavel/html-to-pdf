//! Page 0 is paginated against the room it really has (F29). The renderer paints page 0 lower than the fragmenter
//! counts by the body's own offset (`body` margin, a first child's `margin-top` collapsed through `<body>`, `html`
//! padding), so content that "fits" by that many px used to hang below the content box, or below the page box and
//! get lost (CSS 2.1 §8.3.1, §13.3.3; css-break-3 §4.1).
//!
//! Every token must appear exactly once, in page order, none below the content box, and the page count must be the
//! one Chrome 141 prints for the same HTML (`@page { size: A4; margin: 15mm }`, the renderer's default page).
//! Asserted on each page's words with their positions (`pdftotext -bbox`). Lines are forced with `<br>` (20 px
//! each) or are one-line paragraphs, so nothing depends on font metrics.
//!
//! `F29_DUMP_DIR=<dir> cargo test --test body_offset` also writes every case's HTML to `<dir>/<name>.html`, which
//! is how the Chrome page counts below were measured.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin the renderer applies.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt.
const SLACK_PT: f32 = 5.0;

const CSS: &str = "body { margin: 0 } p { margin: 0; line-height: 20px } \
    .box { border: 2px solid #c00; border-radius: 12px; padding: 16px; background: #ffe9a8 } \
    .spacer { background: #eee } table { border-collapse: collapse } \
    td, th { padding: 0; line-height: 20px; vertical-align: top } .t td { padding: 4px } .blk { background: #def }";

/// A word printed in the PDF: 1-based page and the bottom of its box in pt from the top of the page.
#[derive(Debug)]
struct Word {
    page: usize,
    y_max: f32,
    text: String,
}

/// The HTML of one case: the shared CSS, `extra_css` after it (so it wins), and the body.
fn html(extra_css: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{CSS} {extra_css}</style></head>\
         <body>{body}</body></html>"
    )
}

/// Renders a case, and writes its HTML when `F29_DUMP_DIR` is set.
fn render(name: &str, extra_css: &str, body: &str) -> Vec<u8> {
    let html = html(extra_css, body);
    if let Ok(dir) = std::env::var("F29_DUMP_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.html")),
            &html,
        )
        .expect("write dump");
    }
    Renderer::builder()
        .build()
        .render(&html, &RenderOptions::default())
        .expect("render")
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-bodyoffset-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// Every word that starts with one of the token prefixes, in extraction order (page by page, top to bottom).
fn words(pdf: &[u8]) -> Vec<Word> {
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
            && ["Line", "Radek", "Row", "AFTER", "THEAD"]
                .iter()
                .any(|p| word.starts_with(p))
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

/// `Line0001<br>Line0002<br>…`: one line box per token.
fn lines(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|i| format!("Line{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn names(prefix: &str, width: usize, range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|i| format!("{prefix}{i:0width$}")).collect()
}

/// The words that start with `prefix` and are numbered (not `AFTER`, `THEAD`).
fn tokens<'a>(found: &'a [Word], prefix: &str) -> Vec<&'a Word> {
    found
        .iter()
        .filter(|w| w.text.starts_with(prefix))
        .collect()
}

/// `expected`, each exactly once, in this order, on pages that never go back, none below the content box.
fn assert_paginated(found: &[Word], prefix: &str, expected: &[String]) {
    let seen: Vec<&str> = tokens(found, prefix)
        .iter()
        .map(|w| w.text.as_str())
        .collect();
    let want: Vec<&str> = expected.iter().map(String::as_str).collect();
    let first_diff = seen.iter().zip(&want).position(|(a, b)| a != b);
    assert!(
        seen == want,
        "every token exactly once, in order: printed {} of {}, first difference at index {first_diff:?}, last \
         printed {:?}",
        seen.len(),
        want.len(),
        seen.last()
    );
    assert_inside(found);
    let pages: Vec<usize> = tokens(found, prefix).iter().map(|w| w.page).collect();
    assert!(pages.windows(2).all(|w| w[0] <= w[1]), "pages out of order");
}

/// No word sits below the content box.
fn assert_inside(found: &[Word]) {
    let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    for w in found {
        assert!(
            w.y_max <= limit,
            "{} on page {} ends at {:.1}pt, below the content box ({limit:.1}pt)",
            w.text,
            w.page,
            w.y_max
        );
    }
}

/// Tokens (with `prefix`) on each page, 1-based, as counts.
fn per_page(found: &[Word], prefix: &str, pages: usize) -> Vec<usize> {
    (1..=pages)
        .map(|p| tokens(found, prefix).iter().filter(|w| w.page == p).count())
        .collect()
}

fn page_of(found: &[Word], text: &str) -> usize {
    found
        .iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("{text} is not printed"))
        .page
}

/// A: three paragraphs, each with a 500 px top margin. The first margin collapses through `<body>` and shifts page
/// 0; the second paragraph then starts 520 px into the content, 1020 px into the page: it moves to page 2.
#[test]
fn a_500px_margin_collapsed_through_body_moves_the_second_paragraph_on() {
    let body: String = (1..=3)
        .map(|i| format!("<p style=\"margin:500px 0 0\">Line{i:04}</p>"))
        .collect();
    let pdf = render("a-three-500px-margins", "", &body);
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=3));
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
    assert_eq!(page_of(&found, "Line0001"), 1);
    assert_eq!(page_of(&found, "Line0002"), 2);
}

/// B and C: sixty one-row tables with `margin-top: 80px`, bare and in a `<div>` each. Chrome prints 7 pages.
fn one_row_tables(wrapped: bool) -> String {
    (1..=60)
        .map(|i| {
            let table = format!("<table class=\"t\"><tr><td>Radek{i:03}</td></tr></table>");
            if wrapped {
                format!("<div style=\"margin-top:80px\">{table}</div>")
            } else {
                table.replace("class=\"t\"", "class=\"t\" style=\"margin-top:80px\"")
            }
        })
        .collect()
}

#[test]
fn tables_with_a_top_margin_keep_to_the_content_box() {
    let pdf = render("b-tables-margin-top", "", &one_row_tables(false));
    let found = words(&pdf);
    assert_paginated(&found, "Radek", &names("Radek", 3, 1..=60));
    assert_eq!(page_count(&pdf), 7, "Chrome: 7 pages");
}

#[test]
fn divs_around_tables_with_a_top_margin_keep_to_the_content_box() {
    let pdf = render("c-divs-tables-margin-top", "", &one_row_tables(true));
    let found = words(&pdf);
    assert_paginated(&found, "Radek", &names("Radek", 3, 1..=60));
    assert_eq!(page_count(&pdf), 7, "Chrome: 7 pages");
}

/// D: `<h1 class="mt-8">` as the first child, then 200 one-line paragraphs. The heading's margin shifts page 0.
#[test]
fn a_heading_with_a_top_margin_then_paragraphs() {
    let mut body =
        String::from("<h1 style=\"margin:32px 0 0;font-size:24px;line-height:40px\">Nadpis</h1>");
    for i in 1..=200 {
        body.push_str(&format!("<p>Line{i:04}</p>"));
    }
    let pdf = render("d-heading-margin", "", &body);
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=200));
    assert_eq!(page_count(&pdf), 5, "Chrome: 5 pages");
}

/// E: `body { margin: 40px }` and a body-level paragraph of 120 lines: its first page holds 48 lines (969 px).
#[test]
fn a_body_level_paragraph_under_a_body_margin_splits_inside_the_content_box() {
    let pdf = render(
        "e-body-margin-paragraph",
        "body { margin: 40px }",
        &format!("<p>{}</p>", lines(1..=120)),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=120));
    let pages = page_count(&pdf);
    assert_eq!(pages, 3, "Chrome: 3 pages");
    assert_eq!(per_page(&found, "Line", pages)[0], 48);
}

/// J1: a 940 px spacer, then a 120-line paragraph and one more: the paragraph does not start on page 1.
#[test]
fn a_body_level_paragraph_after_a_spacer_under_a_body_margin() {
    let pdf = render(
        "j1-body-paragraph-spacer",
        "body { margin: 40px }",
        &format!(
            "<div class=\"spacer\" style=\"height:940px\">SPACER</div><p>{}</p><p>AFTER</p>",
            lines(1..=120)
        ),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=120));
    assert!(found.iter().any(|w| w.text == "AFTER"));
    assert_eq!(page_count(&pdf), 4, "Chrome: 4 pages");
    // 29 px are left on page 1, ORPHANS lines need 40: the paragraph starts page 2, as in Chrome.
    assert_eq!(page_of(&found, "Line0001"), 2);
}

/// F: a bordered box around a long paragraph, after a spacer, under a body margin.
fn box_after_spacer(spacer: usize, wrap: bool) -> String {
    let b = format!(
        "<div class=\"spacer\" style=\"height:{spacer}px\">SPACER</div>\
         <div class=\"box\"><div><p>{}</p></div></div>",
        lines(1..=120)
    );
    if wrap {
        format!("<main>{b}</main><p>AFTER</p>")
    } else {
        format!("{b}<p>AFTER</p>")
    }
}

#[test]
fn k3_a_box_after_a_spacer_under_a_body_margin_of_100px() {
    let pdf = render(
        "k3-box-spacer950-bm100",
        "body { margin: 100px 40px }",
        &box_after_spacer(950, false),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=120));
    assert!(found.iter().any(|w| w.text == "AFTER"));
    // The box does not fit the rest of page 1 (its border, padding and two lines do not): it starts page 2, as in Chrome.
    assert_eq!(page_of(&found, "Line0001"), 2);
    assert_eq!(page_count(&pdf), 4, "Chrome: 4 pages");
}

#[test]
fn h2b_a_box_after_a_spacer_under_a_body_margin_of_40px() {
    let pdf = render(
        "h2b-box-spacer940-bm40",
        "body { margin: 40px }",
        &box_after_spacer(940, false),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=120));
    assert!(found.iter().any(|w| w.text == "AFTER"));
    // The box does not fit the rest of page 1 (its border, padding and two lines do not): it starts page 2, as in Chrome.
    assert_eq!(page_of(&found, "Line0001"), 2);
    assert_eq!(page_count(&pdf), 4, "Chrome: 4 pages");
}

#[test]
fn k4_a_wrapped_box_after_a_spacer_under_a_body_margin_of_100px() {
    let pdf = render(
        "k4-wrapped-box-spacer930-bm100",
        "body { margin: 100px 40px }",
        &box_after_spacer(930, true),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=120));
    assert!(found.iter().any(|w| w.text == "AFTER"));
    // The box does not fit the rest of page 1 (its border, padding and two lines do not): it starts page 2, as in Chrome.
    assert_eq!(page_of(&found, "Line0001"), 2);
    assert_eq!(page_count(&pdf), 4, "Chrome: 4 pages");
}

/// G: a long table under a body margin: every row once, the header on every page.
#[test]
fn g_a_long_table_under_a_body_margin_repeats_its_header_on_every_page() {
    let rows: String = (1..=110)
        .map(|i| format!("<tr><td>Row{i:03}</td><td>x</td></tr>"))
        .collect();
    let pdf = render(
        "g-long-table-bm40",
        "body { margin: 40px }",
        &format!(
            "<table style=\"width:100%\"><thead><tr><th>THEAD</th><th>H2</th></tr></thead><tbody>{rows}</tbody>\
             </table><p>AFTER</p>"
        ),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Row", &names("Row", 3, 1..=110));
    let pages = page_count(&pdf);
    assert_eq!(pages, 3, "Chrome: 3 pages");
    for p in 1..=pages {
        let heads = found
            .iter()
            .filter(|w| w.page == p && w.text == "THEAD")
            .count();
        assert_eq!(heads, 1, "THEAD on page {p}");
    }
    assert!(found.iter().any(|w| w.text == "AFTER"));
}

/// H: `body { margin: 40px }` and two 500 px blocks: 40 + 1000 px is more than one page holds.
#[test]
fn two_500px_blocks_under_a_body_margin_take_two_pages() {
    let body = "<div class=\"blk\" style=\"height:500px\">Line0001</div>\
                <div class=\"blk\" style=\"height:500px\">Line0002</div>";
    let pdf = render("h-two-500px-blocks", "body { margin: 40px }", body);
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=2));
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
    assert_eq!(page_of(&found, "Line0002"), 2);
}

/// I: `html { padding-top: 60px }` shifts the body (and page 0) like a body margin does: 949 px hold 47 lines.
#[test]
fn html_padding_shifts_page_zero_like_a_body_margin() {
    let body: String = (1..=60).map(|i| format!("<p>Line{i:04}</p>")).collect();
    let pdf = render("i-html-padding", "html { padding-top: 60px }", &body);
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=60));
    let pages = page_count(&pdf);
    assert_eq!(pages, 2, "Chrome: 2 pages");
    assert_eq!(per_page(&found, "Line", pages), vec![47, 13]);
}

/// J (guard): body padding and border are inside the body, so they do not shift page 0 and are not repeated.
#[test]
fn body_padding_and_border_are_not_an_offset() {
    let body: String = (1..=100).map(|i| format!("<p>Line{i:04}</p>")).collect();
    let pdf = render(
        "j-body-padding-border",
        "body { padding: 30px; border: 5px solid #333 }",
        &body,
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=100));
    assert_eq!(page_count(&pdf), 3, "Chrome: 3 pages");
}

/// K (guard): no offset at all, nothing moves: 1009 px hold 50 lines of 20 px.
#[test]
fn without_an_offset_a_page_holds_its_full_height() {
    let body: String = (1..=150).map(|i| format!("<p>Line{i:04}</p>")).collect();
    let pdf = render("k-no-offset", "", &body);
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=150));
    let pages = page_count(&pdf);
    assert_eq!(pages, 3, "Chrome: 3 pages");
    assert_eq!(per_page(&found, "Line", pages), vec![50, 50, 50]);
}
