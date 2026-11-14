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

/// The 60 px red square the F213e cases stretch to a tall `<img>` (registered as `red60.png`).
const RED60: &[u8] = include_bytes!("data/red60.png");

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
        .image("red60.png", RED60.to_vec())
        .strict_images(true)
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

// --- F213e: the first block of the body does not fit page 1, which starts below the content top ----------------------
//
// Plan docs/plans/2026-11-03-f213e-first-block-below-top-margin.md. The fragmenter moved a box whole to the next page
// only when something precedes it (`cursor_y > 0`); for the first child of the body the offset of page 0 (body margin,
// a margin collapsed through `<body>`, `html` padding or border) did not count, so a first block that does not fit
// page 1 but fits a fresh one hung below the content box or was lost. Chrome (m153) numbers are in each comment; the
// `f213e_*` tests are red before the fix, `f213e_g*` are guards that hold before and after.
//
// What Chrome does, measured (plan section 7), and what the cases below therefore assert:
// - a monolithic first block (`<img>`, `inline-block`) that does not fit moves to page 2 whole (C4, C10, C11);
// - a *breakable* first block, even one with `break-inside: avoid`, is NOT moved: with no sibling before it there is no
//   Class A break point in front of it (css-break-3 section 4.1), so the avoid is not honoured and it splits in place,
//   40 lines / 5 lines (C5, C6). With any sibling before it, even a zero-height one, it moves whole.

/// The pages (1-based) that hold a drawing of an image, from `pdfimages -list`.
fn image_pages(pdf: &[u8]) -> Vec<usize> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfimages")
        .arg("-list")
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(2)
        .filter_map(|l| l.split_whitespace().next()?.parse().ok())
        .collect()
}

/// A block `<img>` of `h` px (and 100 px wide) of the bundled red square.
fn img(h: u32) -> String {
    format!("<img src=\"red60.png\" style=\"display:block;width:100px;height:{h}px\">")
}

/// A `break-inside: avoid` box of 45 lines (900 px) of inline text; `extra_style` is more style.
fn card(extra_style: &str) -> String {
    card_of_lines(45, extra_style)
}

/// The same box with `n` lines of exactly 20 px (the body's `line-height` is `normal`, about 19.4 px; 30 lines are 600 px
/// + 2 px of border).
fn card_of_lines(n: usize, extra_style: &str) -> String {
    format!(
        "<div style=\"break-inside:avoid;border:1px solid #000;line-height:20px;{extra_style}\">{}</div>",
        lines(1..=n)
    )
}

/// The same 45 lines as one block child (`<p>`, 20 px) per line.
fn card_of_blocks() -> String {
    let rows: String = (1..=45).map(|i| format!("<p>Line{i:04}</p>")).collect();
    format!("<div style=\"break-inside:avoid;border:1px solid #000\">{rows}</div>")
}

/// An image first in the body, then `AFTER`, under `css`: Chrome puts page 1 empty, the image on page 2 and `AFTER` below it.
fn assert_first_image_on_page_two(name: &str, css: &str, h: u32) {
    let pdf = render(name, css, &format!("{}<p>AFTER</p>", img(h)));
    let found = words(&pdf);
    assert_eq!(
        image_pages(&pdf),
        vec![2],
        "{name}: the image is on page 2 in Chrome (page 1 empty)"
    );
    assert_eq!(page_of(&found, "AFTER"), 2, "{name}: AFTER");
    assert_inside(&found);
    assert_eq!(page_count(&pdf), 2, "{name}: Chrome: 2 pages");
}

#[test]
fn f213e_c4_a_first_tall_image_under_a_body_margin_moves_to_page_two() {
    // C4: `body { margin-top: M }`, a 950 px block `<img>`, `AFTER`. Chrome (M = 100 and 200): page 1 empty, the image at
    // 42 pt on page 2, AFTER at 753.4 pt. Before F213e: the image hung below the content box on page 1 (M = 100: from
    // 117 pt to 830 pt, the box ends at 800 pt) and AFTER was alone on page 2.
    for m in [100, 200] {
        assert_first_image_on_page_two(
            &format!("f213e-c4-m{m}"),
            &format!("body {{ margin-top: {m}px }}"),
            950,
        );
    }
}

#[test]
fn f213e_c11_a_first_tall_image_under_html_padding_or_border_moves_to_page_two() {
    // C11: the offset is `html { padding-top: 100px }` (or `border-top: 100px solid`, which is the same to the
    // fragmenter). Chrome: page 1 empty, the image and AFTER on page 2.
    assert_first_image_on_page_two("f213e-c11-html-padding", "html { padding-top: 100px }", 950);
    assert_first_image_on_page_two(
        "f213e-c11-html-border",
        "html { border-top: 100px solid #ccc }",
        950,
    );
}

#[test]
fn f213e_c6_a_first_tall_image_whose_margin_collapses_through_body_moves_to_page_two() {
    // C6 (image): no body margin; the image's own `margin-top: 100px` collapses through `<body>` and is page 0's offset.
    // Chrome: page 1 empty, the image on page 2.
    let pdf = render(
        "f213e-c6-collapsed-image",
        "",
        &format!(
            "{}<p>AFTER</p>",
            img(950).replace("display:block", "display:block;margin-top:100px")
        ),
    );
    assert_eq!(image_pages(&pdf), vec![2], "image on page 2");
    assert_eq!(page_of(&words(&pdf), "AFTER"), 2);
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
}

#[test]
fn f213e_c8_a_first_image_that_overflows_page_one_only_by_the_ua_body_margin_moves() {
    // C8: `body { margin: 8px }` (the UA default, which every fulgur test and golden has). The room on page 1 is
    // 1001.13 px; Chrome's fragmentainer is 1010 px (F30f-b), so its images of 1001 and 1002 px stay on page 1 (guards
    // below) and one of 1003 px or more moves to page 2 and leaves page 1 empty; AFTER (20 px) then does not fit under
    // it and goes to page 3. Chrome, 1003 / 1004 / 1008 px: image on page 2, AFTER on page 3, 3 pages. Before F213e the
    // image stayed on page 1, 2 to 7 px below the content box.
    for h in [1003, 1004, 1008] {
        let name = format!("f213e-c8-img{h}");
        let pdf = render(
            &name,
            "body { margin: 8px }",
            &format!("{}<p>AFTER</p>", img(h)),
        );
        assert_eq!(image_pages(&pdf), vec![2], "{name}: image on page 2");
        assert_eq!(page_of(&words(&pdf), "AFTER"), 3, "{name}: AFTER on page 3");
        assert_eq!(page_count(&pdf), 3, "{name}: Chrome: 3 pages");
    }
}

#[test]
fn f213e_g_c8_a_first_image_within_chromes_1010px_fragmentainer_stays_on_page_one() {
    // C8 guard: 8 + 1001 and 8 + 1002 px are within Chrome's 1010 px (`chrome_bottom(0)`): image on page 1, AFTER on
    // page 2 at 40.9 pt, 2 pages. A fix that compares with `bottom(0)` = 1001.13 px moves the 1002 px image: wrong.
    for h in [1001, 1002] {
        let name = format!("f213e-gc8-img{h}");
        let pdf = render(
            &name,
            "body { margin: 8px }",
            &format!("{}<p>AFTER</p>", img(h)),
        );
        assert_eq!(image_pages(&pdf), vec![1], "{name}: image on page 1");
        assert_eq!(page_of(&words(&pdf), "AFTER"), 2, "{name}: AFTER on page 2");
        assert_eq!(page_count(&pdf), 2, "{name}: Chrome: 2 pages");
    }
}

/// Chrome's split of the 45-line avoid card (900 px) that has nothing before it, with the offset 200 px: 40 lines fit the
/// 809 px of page 1 (first line at 191.7 pt), 5 go to page 2 with `AFTER`. 2 pages.
fn assert_card_splits_40_5(name: &str, css: &str, card_html: &str) {
    let pdf = render(name, css, &format!("{card_html}<p>AFTER</p>"));
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=45));
    assert_eq!(page_count(&pdf), 2, "{name}: Chrome: 2 pages");
    assert_eq!(
        per_page(&found, "Line", 2),
        vec![40, 5],
        "{name}: Chrome splits the card 40 / 5 (an avoid box with nothing before it is not moved)"
    );
    assert_eq!(page_of(&found, "AFTER"), 2, "{name}: AFTER");
}

#[test]
#[ignore = "F213e-b: a first avoid card of inline text only hangs below the content box and loses lines"]
fn f213e_c5_a_first_avoid_card_of_text_lines_under_a_body_margin_splits_in_place() {
    // C5: `body { margin-top: 200px }`, `<div style="break-inside:avoid">` of 45 `<br>`-separated lines, AFTER. Chrome does not
    // move the card (nothing precedes it, so no break point is in front of it; css-break-3 section 4.1): page 1 holds
    // lines 1-40, page 2 lines 41-45 and AFTER. Before F213e: lines 1-43 on page 1, the last 3 below the content box
    // and the last 2 below the page edge, printed on no page ("content extends 22.6 pt past the bottom page edge").
    // The plan predicted "moved whole" here; Chrome does not, and a card moved whole is a different page 1 than Chrome's.
    assert_card_splits_40_5("f213e-c5-card", "body { margin-top: 200px }", &card(""));
}

#[test]
#[ignore = "F213e-b: a first avoid card of inline text only hangs below the content box and loses lines"]
fn f213e_c6_a_first_avoid_card_whose_margin_collapses_through_body_splits_in_place() {
    // C6: no body margin; the card's own `margin-top: 200px` collapses through `<body>`. Chrome: the same 40 / 5 split.
    assert_card_splits_40_5("f213e-c6-card", "", &card("margin-top:200px"));
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213e_c5z_an_avoid_card_after_a_zero_height_box_moves_whole() {
    // C5z: a zero-height `<div>` before the card. It is a sibling, so the break in front of the card is a real Class A
    // break point and Chrome honours the avoid: page 1 empty, the whole card from the content top (first line at 41.7
    // pt) and AFTER on page 2. Before F213e: the same hang as C5 (43 lines, 2 lost); `cursor_y` is still 0 after the
    // empty box. With a 2 px box before it, `cursor_y > 0` and the card already moves (guard below).
    let pdf = render(
        "f213e-c5z-zero-before",
        "body { margin-top: 200px }",
        &format!("<div style=\"height:0\"></div>{}<p>AFTER</p>", card("")),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=45));
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
    assert_eq!(per_page(&found, "Line", 2), vec![0, 45]);
    assert_eq!(page_of(&found, "AFTER"), 2);
}

#[test]
fn f213e_g_c5_a_card_after_a_2px_box_moves_whole() {
    // Guard: with something (2 px) before the card, `cursor_y > 0` and the card moves whole, as in Chrome (page 1 empty).
    let pdf = render(
        "f213e-gc5-2px-before",
        "body { margin-top: 200px }",
        &format!("<div style=\"height:2px\"></div>{}<p>AFTER</p>", card("")),
    );
    let found = words(&pdf);
    assert_paginated(&found, "Line", &names("Line", 4, 1..=45));
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
    assert_eq!(per_page(&found, "Line", 2), vec![0, 45]);
}

#[test]
fn f213e_g_c5_a_first_avoid_card_of_blocks_splits_in_place_like_chrome() {
    // Guard (green before): the card of 45 block children, bare and in a `<section>`. Chrome splits both 40 / 5; they
    // already do. A fix that moves every first avoid box whole when page 0 has an offset (the plan's `keeps_whole_at`
    // change) breaks this: page 1 would be empty, Chrome's is not.
    assert_card_splits_40_5(
        "f213e-gc5-blocks",
        "body { margin-top: 200px }",
        &card_of_blocks(),
    );
    assert_card_splits_40_5(
        "f213e-gc5-section",
        "body { margin-top: 200px }",
        &format!("<section>{}</section>", card_of_blocks()),
    );
}

#[test]
fn f213e_g_a_first_paragraph_taller_than_the_room_splits_under_every_offset_source() {
    // Guard: a 60-line paragraph, offset 200 px from each source (body margin, collapsed child margin, html padding, html
    // border, body margin + body border): Chrome puts 40 lines on page 1 (809 px) for all of them; so do we.
    for (name, css, style) in [
        ("margin", "body { margin-top: 200px }", ""),
        ("collapsed", "", "margin-top:200px"),
        ("html-padding", "html { padding-top: 200px }", ""),
        ("html-border", "html { border-top: 200px solid #ccc }", ""),
        (
            "margin-border",
            "body { margin-top: 195px; border-top: 5px solid #00f }",
            "",
        ),
    ] {
        let pdf = render(
            &format!("f213e-gpar-{name}"),
            css,
            &format!("<p style=\"{style}\">{}</p>", lines(1..=60)),
        );
        let found = words(&pdf);
        assert_paginated(&found, "Line", &names("Line", 4, 1..=60));
        assert_eq!(per_page(&found, "Line", 2), vec![40, 20], "{name}");
    }
}

#[test]
fn f213e_g_a_first_image_that_fits_stays_on_page_one() {
    // Guard: M = 100, a 300 px image. Chrome: one page, the image at 117 pt, AFTER at 340.9 pt.
    let pdf = render(
        "f213e-g-fits",
        "body { margin-top: 100px }",
        &format!("{}<p>AFTER</p>", img(300)),
    );
    assert_eq!(image_pages(&pdf), vec![1]);
    assert_eq!(page_count(&pdf), 1, "Chrome: 1 page");
}

#[test]
fn f213e_g_body_padding_and_border_already_count_as_content_above() {
    // Guard (green before): the offset is inside the body (`padding-top`, `border-top`), so the first child starts below
    // the body's content top and the gap makes `cursor_y > 0`. Chrome, same as for a body margin: page 1 empty, image on
    // page 2, 2 pages (C9: the body's border and background are on page 1).
    assert_first_image_on_page_two(
        "f213e-g-body-padding",
        "body { margin-top: 100px; padding-top: 100px }",
        950,
    );
    assert_first_image_on_page_two(
        "f213e-g-body-border",
        "body { margin-top: 95px; border-top: 5px solid #00f }",
        950,
    );
}

#[test]
#[ignore = "F213b / F29b: a first image taller than a page"]
fn f213e_c12_a_first_image_taller_than_a_page_under_a_body_margin_starts_on_page_two() {
    // C12: `body { margin-top: 100px }` and a 1200 px image. Unlike the plan's guess (a block taller than a page is not
    // moved), Chrome moves it: page 1 empty, the image from the content top of page 2 over to page 3 (42 pt to the page
    // bottom, then 42-185 pt), AFTER at 183.4 pt on page 3, 3 pages. With no body margin it starts on page 1 and
    // splits. We draw it on pages 1 and 2 (clipped) and print 2 pages. Splitting images is F213b / F29b.
    let pdf = render(
        "f213e-c12-img1200",
        "body { margin-top: 100px }",
        &format!("{}<p>AFTER</p>", img(1200)),
    );
    assert_eq!(
        image_pages(&pdf),
        vec![2, 3],
        "Chrome: the image is on pages 2 and 3"
    );
    assert_eq!(page_of(&words(&pdf), "AFTER"), 3);
    assert_eq!(page_count(&pdf), 3, "Chrome: 3 pages");
}

// --- F213e-c: a zero-height sibling is a break point in front of an avoid card ----------------------------------------
//
// Plan docs/plans/2026-11-09-f213e-b-avoid-card-under-letterhead.md (PR A), Chrome m153 numbers from its section 7.
// A zero-height in-flow box before a `break-inside: avoid` card leaves `cursor_y` at 0, so the fragmenter read "nothing
// precedes the card" and let it hang below the content box (css-break-3 section 4.1: a break between in-flow siblings is
// a Class A break point whatever their height). Chrome moves the card whole to page 2 (0 / 45). The tests marked
// `F213e-c` are red until the fork fix; the `_g_` ones are green on main and must stay green.
//
// Every case below is `body { margin-top: ... }`, an optional box before the card, the card, then `AFTER`. A case lists
// the `Line` words per page that Chrome prints and the page count; `AFTER` must be on the last page.

/// One case: name, extra CSS, the body, the `Line` words Chrome prints on each page.
type Case = (String, String, String, Vec<usize>);

/// Renders each case and reports every one that differs from Chrome (lost or doubled words, words below the content
/// box, other split, other page count, `AFTER` not on the last page), so one run names all red cases of a test.
fn assert_cases(cases: &[Case]) {
    let mut bad = Vec::new();
    for (name, css, body, want) in cases {
        let pdf = render(name, css, body);
        let found = words(&pdf);
        let got = per_page(&found, "Line", want.len());
        let total: usize = tokens(&found, "Line").len();
        let pages = page_count(&pdf);
        let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
        let below = found.iter().filter(|w| w.y_max > limit).count();
        let after_ok = !body.contains("AFTER") || page_of(&found, "AFTER") == want.len();
        if &got != want
            || total != want.iter().sum::<usize>()
            || pages != want.len()
            || below > 0
            || !after_ok
        {
            bad.push(format!(
                "{name}: Chrome {want:?}; printed {got:?} ({total} words), {pages} pages, {below} words below the \
                 content box, AFTER on the last page: {after_ok}"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ from Chrome:\n{}",
        bad.len(),
        cases.len(),
        bad.join("\n")
    );
}

/// A card of `n` lines after `before`, then `AFTER`, under `css`.
fn case(name: &str, css: &str, before: &str, card_html: &str, want: &[usize]) -> Case {
    (
        name.to_string(),
        css.to_string(),
        format!("{before}{card_html}<p>AFTER</p>"),
        want.to_vec(),
    )
}

const Z: &str = "<div style=\"height:0\"></div>";
const M200: &str = "body { margin-top: 200px }";

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_card_of_blocks_after_a_zero_height_box_moves_whole() {
    // P2, block-children twin of C5z: Chrome 0 / 45 (page 1 empty, the card from the content top, AFTER below it); main
    // splits it 40 / 5 because `content_above` reads "nothing above". Without the box the same card splits 40 / 5 in
    // both (guard `f213e_g_c5_a_first_avoid_card_of_blocks_splits_in_place_like_chrome`). Also with the box's
    // `margin-bottom: 10px` and a 0.4 px one (Taffy rounds it to 0).
    assert_cases(&[
        case("f213ec-blocks-z", M200, Z, &card_of_blocks(), &[0, 45]),
        case(
            "f213ec-blocks-z-mb10",
            M200,
            "<div style=\"height:0;margin-bottom:10px\"></div>",
            &card_of_blocks(),
            &[0, 45],
        ),
        case(
            "f213ec-blocks-0.4px",
            M200,
            "<div style=\"height:0.4px\"></div>",
            &card_of_blocks(),
            &[0, 45],
        ),
        case(
            "f213ec-blocks-empty-span",
            M200,
            "<span></span>",
            &card_of_blocks(),
            &[0, 45],
        ),
    ]);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_every_kind_of_zero_height_box_is_a_sibling_before_an_inline_card() {
    // P2 / P7 (plan 7.2, 7.5): all of these put the 45-line card on page 2 whole in Chrome (0 / 45). Main hangs it for
    // each (43 lines on page 1, 2 lost). `<span></span>` is an anonymous block with an inline element and no line box: it
    // counts (the plan's first guess, "not an anonymous block", was wrong). 0.4 px rounds to 0 in Taffy; 0.5 px and
    // larger already work (guard `f213e_g_c5_a_card_after_a_2px_box_moves_whole`).
    let befores = [
        ("z-z", format!("{Z}{Z}")),
        ("empty-p", "<p></p>".to_string()),
        (
            "relative",
            "<div style=\"position:relative;height:0\"></div>".to_string(),
        ),
        (
            "sticky",
            "<div style=\"position:sticky;height:0\"></div>".to_string(),
        ),
        (
            "clear",
            "<div style=\"clear:both;height:0\"></div>".to_string(),
        ),
        ("flex", "<div style=\"display:flex\"></div>".to_string()),
        (
            "empty-table",
            "<table style=\"height:0\"></table>".to_string(),
        ),
        (
            "img0",
            "<img src=\"red60.png\" style=\"display:block;width:100px;height:0\">".to_string(),
        ),
        (
            "mb10",
            "<div style=\"height:0;margin-bottom:10px\"></div>".to_string(),
        ),
        ("0.4px", "<div style=\"height:0.4px\"></div>".to_string()),
        ("empty-span", "<span></span>".to_string()),
        ("empty-span-spaces", " <span> </span> ".to_string()),
    ];
    let cases: Vec<Case> = befores
        .iter()
        .map(|(n, b)| case(&format!("f213ec-inline-{n}"), M200, b, &card(""), &[0, 45]))
        .collect();
    assert_cases(&cases);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_counter_reset_div_is_a_sibling() {
    // The shape the finding came from: `<div style="counter-reset:polozka">` is a zero-height div. Chrome 0 / 45.
    assert_cases(&[case(
        "f213ec-counter-reset",
        M200,
        "<div style=\"counter-reset:polozka\"></div>",
        &card(""),
        &[0, 45],
    )]);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_hidden_or_out_of_flow_box_does_not_stop_a_zero_height_box_from_counting() {
    // P5 / P6: `display:none` and `position:absolute` boxes are not siblings, but a zero-height box after them still is.
    // Chrome 0 / 45 for each; main hangs. (That the hidden / abspos box alone does not count is F213e-b's, PR B.)
    let befores = [
        ("none-z", format!("<div style=\"display:none\">x</div>{Z}")),
        (
            "abspos-z",
            format!("<div style=\"position:absolute;height:0\"></div>{Z}"),
        ),
        ("hidden-attr-z", format!("<div hidden>x</div>{Z}")),
    ];
    let cases: Vec<Case> = befores
        .iter()
        .map(|(n, b)| case(&format!("f213ec-{n}"), M200, b, &card(""), &[0, 45]))
        .collect();
    assert_cases(&cases);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_padded_card_after_a_zero_height_box_moves_whole() {
    // P8: 16 px of padding (934 px in all). Chrome 0 / 45 after the box (first line at 56.6 pt); main hangs it (42 lines
    // on page 1). Without the box Chrome splits it 39 / 6 and main hangs: that is F213e-b, PR B.
    assert_cases(&[case(
        "f213ec-padded-z",
        M200,
        Z,
        &card("padding:16px"),
        &[0, 45],
    )]);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_card_with_room_for_44_lines_after_a_zero_height_box_moves_whole() {
    // P8: `body { margin-top: M }`, M = 126 .. 130 leaves room for 44 of the 45 lines. After the box Chrome moves the card
    // whole at every M (0 / 45); main hangs it (all 45 on page 1 at M = 128, the last 2 off the content box).
    let cases: Vec<Case> = [126, 128, 130]
        .iter()
        .map(|m| {
            case(
                &format!("f213ec-room44-m{m}"),
                &format!("body {{ margin-top: {m}px }}"),
                Z,
                &card(""),
                &[0, 45],
            )
        })
        .collect();
    assert_cases(&cases);
}

#[test]
#[ignore = "F213e-c: red until the fork fix"]
fn f213ec_a_card_ending_past_chromes_1010px_moves_whole_after_a_zero_height_box() {
    // P8, the 1 px band: a 30-line card (602 px) ends at M + 602. Past Chrome's 1010 px (M = 408.5 and above) Chrome puts
    // the whole card on page 2 after the box (0 / 30, first line at 44.6 pt); main keeps it on page 1, and from M = 480
    // loses the lines past the page edge. The room is `chrome_bottom(0) - cursor`, not `bottom(0) - cursor`.
    let cases: Vec<Case> = ["408.5", "450", "560"]
        .iter()
        .map(|m| {
            (
                format!("f213ec-band-m{m}"),
                format!("body {{ margin-top: {m}px }}"),
                format!("{Z}{}", card_of_lines(30, "")),
                vec![0, 30],
            )
        })
        .collect();
    assert_cases(&cases);
}

#[test]
fn f213ec_g_a_card_that_fits_stays_on_page_one_with_or_without_a_zero_height_box() {
    // Guard (P2): 30 lines (602 px) under a 200 px offset fit page 1. Chrome: whole on page 1, first line at 194.6 pt,
    // AFTER below it, 1 page. A fix that moves a card after the box without asking whether it overflows breaks this.
    assert_cases(&[
        case("f213ec-g-fit30", M200, "", &card_of_lines(30, ""), &[30]),
        case("f213ec-g-fit30-z", M200, Z, &card_of_lines(30, ""), &[30]),
    ]);
}

#[test]
fn f213ec_g_a_card_ending_within_chromes_1010px_stays_on_page_one() {
    // Guard (P8, the 1 px band): a 30-line card ending at 1008 to 1010 px (M = 406 .. 408) stays whole on page 1 in Chrome,
    // with or without a zero-height box before it; a card ending in (1009.13, 1010] is within `chrome_bottom(0)` though
    // past `bottom(0)`. No `AFTER`: the page count is the card's.
    let mut cases: Vec<Case> = Vec::new();
    for m in ["406", "407.5", "408"] {
        for (tag, before) in [("", ""), ("-z", Z)] {
            cases.push((
                format!("f213ec-g-band-m{m}{tag}"),
                format!("body {{ margin-top: {m}px }}"),
                format!("{before}{}", card_of_lines(30, "")),
                vec![30],
            ));
        }
    }
    assert_cases(&cases);
}

#[test]
fn f213ec_g_an_inline_card_in_a_section_or_a_wrapper_splits_in_place() {
    // Guard (P3, plan 7.3): with nothing before it, a `<section>` or a bare `<div>` around the inline card does not
    // change the split: Chrome 40 / 5, main the same (the plan expected this to be red; it is not). The twin with
    // block children is `f213e_g_c5_a_first_avoid_card_of_blocks_splits_in_place_like_chrome`.
    assert_cases(&[
        case(
            "f213ec-g-section-inline",
            M200,
            "",
            &format!("<section>{}</section>", card("")),
            &[40, 5],
        ),
        case(
            "f213ec-g-wrapper-inline",
            M200,
            "",
            &format!("<div>{}</div>", card("")),
            &[40, 5],
        ),
    ]);
}

#[test]
#[ignore = "F213e-d: a zero-height sibling around a <section> is not seen by the nested walk"]
fn f213ed_a_zero_height_box_around_a_section_moves_the_card_whole() {
    // P3 (plan 7.3), not covered by PR A's three call sites (`content_above` passed to `fragment_block_subtree` and
    // the nested walk are left unchanged): Chrome moves the card whole (0 / 45) when a zero-height box is before the
    // `<section>`, or inside it before the card, for both card kinds; main splits it 40 / 5.
    let ic = card("");
    let bc = card_of_blocks();
    assert_cases(&[
        case(
            "f213ed-z-section-inline",
            M200,
            Z,
            &format!("<section>{ic}</section>"),
            &[0, 45],
        ),
        case(
            "f213ed-z-section-blocks",
            M200,
            Z,
            &format!("<section>{bc}</section>"),
            &[0, 45],
        ),
        case(
            "f213ed-section-z-inline",
            M200,
            "",
            &format!("<section>{Z}{ic}</section>"),
            &[0, 45],
        ),
        case(
            "f213ed-section-z-blocks",
            M200,
            "",
            &format!("<section>{Z}{bc}</section>"),
            &[0, 45],
        ),
    ]);
}

#[test]
fn f213ec_g_a_box_that_fits_chromes_room_stays_on_page_one_after_a_zero_height_box() {
    // Guard for `chrome_bottom` (PR A, C8): `body { margin: 8px }`, a zero-height box, a block image of 1000 / 1001 /
    // 1002 px. The room on page 1 is 1001.13 px but Chrome's fragmentainer is 1010 px, so the image stays on page 1 and
    // AFTER (20 px) goes to page 2 at 43.8 pt. 1003 px and above move (`f213e_c8_*`, without the box).
    for h in [1000, 1001, 1002] {
        let name = format!("f213ec-g-img{h}-z");
        let pdf = render(
            &name,
            "body { margin: 8px }",
            &format!("{Z}{}<p>AFTER</p>", img(h)),
        );
        assert_eq!(image_pages(&pdf), vec![1], "{name}: image on page 1");
        assert_eq!(page_of(&words(&pdf), "AFTER"), 2, "{name}: AFTER on page 2");
        assert_eq!(page_count(&pdf), 2, "{name}: Chrome: 2 pages");
    }
}

#[test]
fn f213ec_g_page_one_that_is_merely_smaller_takes_no_break_in_front_of_the_card() {
    // Guard (P4, plan 7.4, corrected): `@page :first { margin-top: 60mm }` makes page 1 smaller (839 px, 41 lines of the
    // card), it is not an offset that was consumed above the card. Chrome takes no break in front of the card after a
    // zero-height box either: 41 / 4 with and without it (first line at 172.1 pt), and the 30-line card stays whole.
    let first = "@page :first { margin-top: 60mm }";
    assert_cases(&[
        case(
            "f213ec-g-first-30",
            first,
            "",
            &card_of_lines(30, ""),
            &[30],
        ),
        case(
            "f213ec-g-first-30-z",
            first,
            Z,
            &card_of_lines(30, ""),
            &[30],
        ),
        case("f213ec-g-first-45", first, "", &card(""), &[41, 4]),
        case("f213ec-g-first-45-z", first, Z, &card(""), &[41, 4]),
    ]);
}

#[test]
#[ignore = "F109: page 2 is given page 1's height"]
fn f109_a_2px_box_before_a_card_on_a_smaller_first_page_moves_the_card_whole() {
    // P4: with a 2 px box before the 45-line card Chrome puts page 1 empty and the whole card on page 2 (0 / 45, first
    // line at 44.6 pt, 2 pages). Main moves it too, but paginates page 2 against page 1's smaller box: 41 lines on page
    // 2, 4 on page 3 (3 pages).
    assert_cases(&[case(
        "f109-first-2px",
        "@page :first { margin-top: 60mm }",
        "<div style=\"height:2px\"></div>",
        &card(""),
        &[0, 45],
    )]);
}
