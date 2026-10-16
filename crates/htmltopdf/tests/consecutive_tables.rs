//! Consecutive tables (and other boxes whose first row sits at their top edge) paginate (F28): a block that does
//! not fit the rest of the page moves to the next page instead of hanging into the bottom margin and stacking
//! everything after it on page 1. Asserted on each page's words with their positions (`pdftotext -bbox`).
//!
//! Every `RadekNN` must appear exactly once, in page order, and no word may sit below the content box.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin the renderer applies.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt.
const SLACK_PT: f32 = 5.0;

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .css("body { margin: 0 } table { border-collapse: collapse } td, th { padding: 4px } .r { padding: 4px 0 }")
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn one_row_table(i: usize) -> String {
    format!("<table><tr><td>Radek{i:02}</td></tr></table>")
}

/// Every `Radek…` word as `(page, y_max_pt, text)`, in extraction order.
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
            && (word.starts_with("Radek") || word.starts_with("Odstavec"))
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

/// `expected` words, each exactly once, in this order, none below the content box. Returns the page count.
fn assert_paginated(pdf: &[u8], expected: &[String]) -> usize {
    let found = words(pdf);
    let seen: Vec<&str> = found.iter().map(|(_, _, w)| w.as_str()).collect();
    assert_eq!(
        seen,
        expected.iter().map(String::as_str).collect::<Vec<_>>(),
        "every word exactly once, in order (extraction is page by page, top to bottom)"
    );
    let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    for (page, y, word) in &found {
        assert!(
            *y <= limit,
            "{word} on page {page} ends at {y:.1}pt, below the content box ({limit:.1}pt)"
        );
    }
    let pages: Vec<usize> = found.iter().map(|(p, _, _)| *p).collect();
    assert!(pages.windows(2).all(|w| w[0] <= w[1]), "pages out of order");
    page_count(pdf)
}

fn radek(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|i| format!("Radek{i:02}")).collect()
}

#[test]
fn sixty_one_row_tables_paginate() {
    let body: String = (1..=60).map(one_row_table).collect();
    let pages = assert_paginated(&render(&body), &radek(1..=60));
    assert_eq!(pages, 2);
}

#[test]
fn sixty_one_row_tables_inside_a_div_paginate() {
    let body: String = (1..=60).map(one_row_table).collect();
    let pages = assert_paginated(&render(&format!("<div>{body}</div>")), &radek(1..=60));
    assert_eq!(pages, 2);
}

#[test]
fn tables_after_paragraphs_paginate() {
    let body: String = (1..=30)
        .map(|i| format!("<p class=\"r\">Odstavec{i:02}</p>{}", one_row_table(i)))
        .collect();
    let expected: Vec<String> = (1..=30)
        .flat_map(|i| [format!("Odstavec{i:02}"), format!("Radek{i:02}")])
        .collect();
    let pages = assert_paginated(&render(&body), &expected);
    assert!(pages >= 2, "{pages}");
}

#[test]
fn nested_divs_paginate() {
    let body: String = (1..=60)
        .map(|i| format!("<div><div><div class=\"r\">Radek{i:02}</div></div></div>"))
        .collect();
    assert_eq!(assert_paginated(&render(&body), &radek(1..=60)), 2);
}

#[test]
fn flex_and_grid_rows_paginate() {
    let flex: String = (1..=60)
        .map(|i| format!("<div style=\"display:flex\"><div class=\"r\">Radek{i:02}</div></div>"))
        .collect();
    assert_eq!(
        assert_paginated(&render(&format!("<div>{flex}</div>")), &radek(1..=60)),
        2
    );
    let grid: String = (1..=60)
        .map(|i| {
            format!(
                "<div style=\"display:grid;grid-template-columns:1fr\"><div class=\"r\">Radek{i:02}</div></div>"
            )
        })
        .collect();
    assert_eq!(
        assert_paginated(&render(&format!("<div>{grid}</div>")), &radek(1..=60)),
        2
    );
}

#[test]
fn a_gap_before_the_first_row_still_breaks_inside() {
    // Class C break point: the wrapper's padding is a gap, so the table may move alone.
    let body: String = (1..=60)
        .map(|i| format!("<div style=\"padding-top:6px\">{}</div>", one_row_table(i)))
        .collect();
    assert_eq!(assert_paginated(&render(&body), &radek(1..=60)), 2);
}

/// Filler so that exactly `room_px` of the first page is left for what follows.
fn filler(room_px: usize) -> String {
    // 1009 px of content height on A4 with 15 mm margins.
    let content_px = 1009;
    format!("<div style=\"height:{}px\"></div>", content_px - room_px)
}

fn two_row_table() -> &'static str {
    "<table><tr><td style=\"height:40px\">Radek01</td></tr><tr><td style=\"height:40px\">Radek02</td></tr></table>"
}

#[test]
fn a_table_with_room_for_one_row_splits_between_rows() {
    let body = format!("{}{}", filler(60), two_row_table());
    let pdf = render(&body);
    let found = words(&pdf);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].0, 1, "first row stays on page 1");
    assert_eq!(found[1].0, 2, "second row moves to page 2");
    assert_paginated(&pdf, &radek(1..=2));
}

#[test]
fn a_table_with_no_room_for_its_first_row_moves_whole() {
    let body = format!("{}{}", filler(30), two_row_table());
    let pdf = render(&body);
    let found = words(&pdf);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!((found[0].0, found[1].0), (2, 2), "both rows on page 2");
    assert_paginated(&pdf, &radek(1..=2));
}

#[test]
fn a_table_whose_header_does_not_fit_moves_whole_and_repeats_its_header() {
    let rows: String = (1..=80)
        .map(|i| format!("<tr><td>Radek{i:02}</td></tr>"))
        .collect();
    let table =
        format!("<table><thead><tr><th>Hlavicka</th></tr></thead><tbody>{rows}</tbody></table>");
    let pdf = render(&format!("{}{table}", filler(10)));
    let pages = assert_paginated(&pdf, &radek(1..=80));
    assert!(pages >= 2, "{pages}");
    // The header never stays alone at the bottom of page 1: it is printed once per page the table touches, and
    // page 1 carries only the filler.
    let text = |page: usize| {
        let path = temp(&pdf);
        let out = std::process::Command::new("pdftotext")
            .args(["-f", &page.to_string(), "-l", &page.to_string()])
            .arg(&path)
            .arg("-")
            .output()
            .expect("pdftotext");
        let _ = std::fs::remove_file(&path);
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert!(!text(1).contains("Hlavicka"), "header orphaned on page 1");
    for page in 2..=pages {
        assert_eq!(
            text(page).matches("Hlavicka").count(),
            1,
            "page {page} needs the header exactly once"
        );
    }
}

/// First page rasterised at 72 dpi (1 px = 1 pt) as `(width, height, 8-bit grey pixels)`.
fn page_one_pixels(pdf: &[u8]) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let prefix = path.with_extension("");
    let status = std::process::Command::new("pdftoppm")
        .args(["-gray", "-r", "72", "-f", "1", "-l", "1", "-singlefile"])
        .arg(&path)
        .arg(&prefix)
        .status()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(status.success(), "pdftoppm failed");
    let pgm = prefix.with_extension("pgm");
    let bytes = std::fs::read(&pgm).expect("pgm");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&pgm);
    // "P5\n<w> <h>\n255\n" followed by the raw pixels.
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
    let (w, h) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    (w, h, bytes[at + 1..].to_vec())
}

#[test]
fn a_bordered_table_that_moves_leaves_no_frame_on_the_page_it_left() {
    // The table's own border is not a gap a break may use (css-break-3 §4.1): the whole table moves, and
    // nothing of it (not even an empty bordered frame) is drawn on page 1 under the spacer.
    let rows: String = (1..=3)
        .map(|i| format!("<tr><td>Radek{i:02}</td></tr>"))
        .collect();
    let table = format!(
        "<table style=\"border-collapse:collapse;border:1px solid #333;width:100%\">{rows}</table>"
    );
    // 985 px of 1009 px: less than the table's first row below the spacer.
    let pdf = render(&format!("<div style=\"height:985px\"></div>{table}"));
    let found = words(&pdf);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found.iter().all(|(page, _, _)| *page == 2), "{found:?}");
    let (w, h, pixels) = page_one_pixels(&pdf);
    // Below the spacer (15 mm margin + 985 px = 781.3 pt) page 1 is blank.
    let first_row = (MARGIN_PT + 985.0 * 0.75).ceil() as usize + 1;
    for y in first_row..h {
        let row = &pixels[y * w..(y + 1) * w];
        assert!(
            row.iter().all(|&p| p == 255),
            "page 1 has ink at y={y} below the spacer"
        );
    }
}

#[test]
fn tables_under_display_contents_are_all_printed_once() {
    // Blitz lays these out side by side instead of stacking them; nothing may be dropped (it used to lose 20
    // of 60 rows to a second, blank page).
    let body: String = (1..=60).map(one_row_table).collect();
    let pdf = render(&format!("<div style=\"display:contents\">{body}</div>"));
    let found = words(&pdf);
    let seen: Vec<&str> = found.iter().map(|(_, _, w)| w.as_str()).collect();
    let mut expected: Vec<String> = radek(1..=60);
    expected.sort();
    let mut sorted: Vec<&str> = seen.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let limit = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    assert!(found.iter().all(|(_, y, _)| *y <= limit), "{found:?}");
}

#[test]
fn a_block_taller_than_a_page_does_not_waste_pages() {
    // Four blocks whose first row (1100 px) is taller than a page: moving one to a fresh page gains nothing.
    let paragraphs: String = (1..=20)
        .map(|i| format!("<p style=\"margin:0;height:24px\">Radek{i:02}</p>"))
        .collect();
    let blocks: String = (21..=24)
        .map(|i| {
            format!(
                "<div><div style=\"height:1100px\">Radek{i:02}</div><p style=\"margin:0\">Odstavec{i:02}</p></div>"
            )
        })
        .collect();
    let pdf = render(&format!("{paragraphs}{blocks}"));
    let found = words(&pdf);
    assert_eq!(found.len(), 28, "{found:?}");
    // 480 + 4 x (1100 + one line) is 5 pages of 1009 px; before the fix each block got a page of its own and
    // the document had 9.
    let pages = page_count(&pdf);
    assert!(pages <= 5, "{pages} pages");
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-ctab-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
