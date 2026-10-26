//! A table inside a multicol container is broken between its rows across the columns and pages (F174).
//!
//! The F30d flow placed a table whole in its column: a table taller than a page ran past the page bottom (130 rows
//! printed 52) and a short one was never split between the columns. Plan: docs/plans/2026-10-26-f174-table-in-columns.md.
//!
//! Spec (unverified): css-break-3 §2 (column boxes are fragmentainers), §4.1 (between sibling rows are class A break
//! points), css-multicol-1 §7.1 / §8.2, CSS 2.1 §17.2 (header rows repeat at the top of each fragment). The row ranges
//! per column come from Chrome 148 with the same CSS and Inter loaded through `@font-face` (A4, 15 mm margins); a range
//! may end one row earlier or later than Chrome's (row heights round differently).

use htmltopdf::{RenderOptions, Renderer};

const PAGE_W_PT: f32 = 595.28;
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A word left of this is in the first column.
const GUTTER_X_PT: f32 = PAGE_W_PT / 2.0;
const CSS: &str = "body{margin:0;font-family:Inter;font-size:16px;line-height:20px} p{margin:0} \
    table{border-spacing:0} td,th{padding:0 6px}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .css(format!("{CSS} {css}"))
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

#[derive(Clone, Debug)]
struct Word {
    page: usize,
    x0: f32,
    y0: f32,
    y1: f32,
    text: String,
}

impl Word {
    fn column(&self) -> usize {
        usize::from(self.x0 >= GUTTER_X_PT)
    }
}

fn temp(pdf: &[u8], ext: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-mct-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout);
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(&format!("{name}=\"")))
            .and_then(|v| v.trim_end_matches('"').parse().ok())
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
                x0: attr(attrs, "xMin"),
                y0: attr(attrs, "yMin"),
                y1: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(pdf: &[u8]) -> usize {
    words(pdf).iter().map(|w| w.page).max().unwrap_or(0)
}

/// `r1` … `r<n>` rows and `v1` … `v<n>` cells.
fn rows(from: usize, to: usize) -> String {
    (from..=to)
        .map(|i| format!("<tr><td>r{i}</td><td>v{i}</td></tr>"))
        .collect()
}

const HEAD: &str = "<thead><tr><th>Head1</th><th>Head2</th></tr></thead>";

fn row_number(w: &Word) -> Option<usize> {
    w.text.strip_prefix('r').and_then(|n| n.parse().ok())
}

/// Every `rN` and `vN` of `1..=n` once.
fn assert_rows_once(what: &str, found: &[Word], n: usize) {
    for i in 1..=n {
        for text in [format!("r{i}"), format!("v{i}")] {
            let count = found.iter().filter(|w| w.text == text).count();
            assert_eq!(count, 1, "{what}: {text} printed {count} times");
        }
    }
    let bottom = PAGE_H_PT - MARGIN_PT + 5.0;
    for w in found {
        assert!(
            w.y1 <= bottom,
            "{what}: {} on page {} ends at {:.1} pt, below the content box",
            w.text,
            w.page,
            w.y1
        );
    }
}

/// The rows in column `col` of `page` are `first..=last` as Chrome has them, each end within one row.
fn assert_piece(what: &str, found: &[Word], page: usize, col: usize, first: usize, last: usize) {
    let mut here: Vec<usize> = found
        .iter()
        .filter(|w| w.page == page && w.column() == col)
        .filter_map(row_number)
        .collect();
    here.sort_unstable();
    let (Some(&a), Some(&b)) = (here.first(), here.last()) else {
        panic!(
            "{what}: no row in column {} of page {page}; Chrome has r{first}-r{last}",
            col + 1
        );
    };
    assert!(
        a.abs_diff(first) <= 1 && b.abs_diff(last) <= 1,
        "{what}: column {} of page {page} has r{a}-r{b}, Chrome r{first}-r{last}",
        col + 1
    );
    assert_eq!(
        here.len(),
        b - a + 1,
        "{what}: rows of column {} of page {page} are not contiguous",
        col + 1
    );
}

/// `Head1` once at the top of every column piece of the table, above its first row.
fn assert_header_per_piece(what: &str, found: &[Word], pieces: &[(usize, usize)]) {
    for &(page, col) in pieces {
        let heads: Vec<&Word> = found
            .iter()
            .filter(|w| w.page == page && w.column() == col && w.text == "Head1")
            .collect();
        assert_eq!(
            heads.len(),
            1,
            "{what}: Head1 in column {} of page {page}",
            col + 1
        );
        let first_row = found
            .iter()
            .filter(|w| w.page == page && w.column() == col && row_number(w).is_some())
            .map(|w| w.y0)
            .reduce(f32::min)
            .unwrap();
        assert!(
            heads[0].y0 < first_row,
            "{what}: Head1 below the first row in column {} of page {page}",
            col + 1
        );
    }
}

const PIECES: [(usize, usize); 4] = [(1, 0), (1, 1), (2, 0), (2, 1)];

#[test]
fn a_a_long_table_in_two_columns_is_broken_between_rows_and_repeats_its_header() {
    let pdf = render(
        "",
        &format!(
            "<div style=\"columns:2\"><table>{HEAD}<tbody>{}</tbody></table></div><p>tail</p>",
            rows(1, 130)
        ),
    );
    let found = words(&pdf);
    assert_eq!(page_count(&pdf), 2);
    assert_rows_once("A", &found, 130);
    for (page, col, first, last) in [
        (1, 0, 1, 49),
        (1, 1, 50, 98),
        (2, 0, 99, 114),
        (2, 1, 115, 130),
    ] {
        assert_piece("A", &found, page, col, first, last);
    }
    assert_header_per_piece("A", &found, &PIECES);
    assert!(found.iter().any(|w| w.text == "tail"), "A: tail");
}

#[test]
fn b_a_long_table_without_a_header() {
    let pdf = render(
        "",
        &format!(
            "<div style=\"columns:2\"><table><tbody>{}</tbody></table></div><p>tail</p>",
            rows(1, 130)
        ),
    );
    let found = words(&pdf);
    assert_eq!(page_count(&pdf), 2);
    assert_rows_once("B", &found, 130);
    for (page, col, first, last) in [
        (1, 0, 1, 50),
        (1, 1, 51, 100),
        (2, 0, 101, 115),
        (2, 1, 116, 130),
    ] {
        assert_piece("B", &found, page, col, first, last);
    }
}

#[test]
fn c_a_short_table_is_split_between_the_balanced_columns() {
    let pdf = render(
        "",
        &format!(
            "<div style=\"columns:2\"><p>intro</p><table><tbody>{}</tbody></table><p>after</p></div><p>tail</p>",
            rows(1, 20)
        ),
    );
    let found = words(&pdf);
    assert_eq!(page_count(&pdf), 1);
    assert_rows_once("C", &found, 20);
    assert_piece("C", &found, 1, 0, 1, 10);
    assert_piece("C", &found, 1, 1, 11, 20);
    let after = found.iter().find(|w| w.text == "after").expect("after");
    assert_eq!(after.column(), 1, "C: after ends the second column");
    let tail = found.iter().find(|w| w.text == "tail").expect("tail");
    let lowest = found
        .iter()
        .filter(|w| w.text != "tail")
        .map(|w| w.y1)
        .fold(0.0, f32::max);
    assert!(tail.y0 >= lowest - 0.5, "C: tail is below the columns");
}

#[test]
fn d_a_table_after_paragraphs() {
    let paras: String = (0..30).map(|i| format!("<p>p{i}</p>")).collect();
    let pdf = render(
        "",
        &format!(
            "<div style=\"columns:2\">{paras}<table><tbody>{}</tbody></table></div><p>tail</p>",
            rows(1, 130)
        ),
    );
    let found = words(&pdf);
    assert_eq!(page_count(&pdf), 2);
    assert_rows_once("D", &found, 130);
    for (page, col, first, last) in [
        (1, 0, 1, 20),
        (1, 1, 21, 70),
        (2, 0, 71, 100),
        (2, 1, 101, 130),
    ] {
        assert_piece("D", &found, page, col, first, last);
    }
    for i in 0..30 {
        let p = found
            .iter()
            .find(|w| w.text == format!("p{i}"))
            .expect("paragraph");
        assert_eq!(
            (p.page, p.column()),
            (1, 0),
            "D: p{i} in the first column of page 1"
        );
    }
}

/// Page `page` of `pdf` rasterised at 96 dpi: (width, height, RGB bytes).
fn raster(pdf: &[u8], page: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "96", "-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
    // P6 header: "P6\n<w> <h>\n255\n".
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
    (w, h, data[at..at + w * h * 3].to_vec())
}

fn pixel(img: &(usize, usize, Vec<u8>), x_pt: f32, y_pt: f32) -> [u8; 3] {
    let (w, _, data) = img;
    let (x, y) = ((x_pt * 96.0 / 72.0) as usize, (y_pt * 96.0 / 72.0) as usize);
    let i = (y * w + x) * 3;
    [data[i], data[i + 1], data[i + 2]]
}

#[test]
fn e_collapsed_borders_are_drawn_in_each_column() {
    let css = "table{border-collapse:collapse} td,th{border:1px solid #000}";
    let pdf = render(
        css,
        &format!(
            "<div style=\"columns:2\"><table>{HEAD}<tbody>{}</tbody></table></div><p>tail</p>",
            rows(1, 130)
        ),
    );
    let found = words(&pdf);
    assert_eq!(page_count(&pdf), 2);
    assert_rows_once("E", &found, 130);
    for (page, col, first, last) in [
        (1, 0, 1, 47),
        (1, 1, 48, 94),
        (2, 0, 95, 112),
        (2, 1, 113, 130),
    ] {
        assert_piece("E", &found, page, col, first, last);
    }
    assert_header_per_piece("E", &found, &PIECES);
    // Column 2 of page 1 has its own left border, just left of its cells' text (6 px padding, 1 px border).
    let r60 = found.iter().find(|w| w.text == "r60").unwrap();
    let img = raster(&pdf, 1);
    let mid = (r60.y0 + r60.y1) / 2.0;
    let dark = (1..=8).any(|dx| {
        pixel(&img, r60.x0 - dx as f32, mid)
            .iter()
            .all(|&c| c < 128)
    });
    assert!(dark, "E: no border line left of r60 in column 2");
    // The frame of the table does not run on below the last row of column 1 on page 2.
    let img2 = raster(&pdf, 2);
    let last = found
        .iter()
        .filter(|w| w.page == 2 && w.column() == 0 && row_number(w).is_some())
        .max_by(|a, b| a.y1.total_cmp(&b.y1))
        .unwrap();
    let below = last.y1 + 40.0;
    let line_x = last.x0 - 4.5;
    let run_on = (0..6).any(|dx| {
        pixel(&img2, line_x - dx as f32 * 0.75, below)
            .iter()
            .all(|&c| c < 128)
    });
    assert!(
        !run_on,
        "E: the table's left border runs on below its last row on page 2"
    );
}

#[test]
fn f_row_backgrounds_are_painted_once() {
    let css = "tbody tr:nth-child(odd){background:rgba(37,99,235,.3)}";
    let body = format!("<table>{HEAD}<tbody>{}</tbody></table>", rows(1, 130));
    let pdf = render(
        css,
        &format!("<div style=\"columns:2\">{body}</div><p>tail</p>"),
    );
    let plain = render(css, &body);
    let (found, found_plain) = (words(&pdf), words(&plain));
    assert_rows_once("F", &found, 130);
    let sample = |pdf: &[u8], found: &[Word], text: &str| -> [u8; 3] {
        let w = found.iter().find(|w| w.text == text).unwrap();
        pixel(&raster(pdf, w.page), w.x0 - 3.0, (w.y0 + w.y1) / 2.0)
    };
    for text in ["r1", "r51", "r101"] {
        let (a, b) = (
            sample(&pdf, &found, text),
            sample(&plain, &found_plain, text),
        );
        assert!(
            a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2),
            "F: the background of {text} is {a:?} in columns, {b:?} in one column"
        );
    }
}

#[test]
fn g_guards() {
    // (2) `break-inside: avoid` on a table that fits a column: whole in one column.
    let pdf = render(
        "table{break-inside:avoid}",
        &format!(
            "<div style=\"columns:2\"><p>intro</p><table><tbody>{}</tbody></table><p>after</p></div>",
            rows(1, 20)
        ),
    );
    let found = words(&pdf);
    assert_rows_once("G2", &found, 20);
    let cols: std::collections::BTreeSet<usize> = found
        .iter()
        .filter(|w| row_number(w).is_some())
        .map(Word::column)
        .collect();
    assert_eq!(
        cols.len(),
        1,
        "G2: an avoid table is split between the columns"
    );
    // (3) one row taller than a page: placed alone, the rest is printed once.
    let tall = (0..120)
        .map(|i| format!("t{i}"))
        .collect::<Vec<_>>()
        .join("<br>");
    let pdf = render(
        "",
        &format!(
            "<div style=\"columns:2\"><table><tbody>{}<tr><td>{tall}</td><td>x</td></tr>{}</tbody></table></div>",
            rows(1, 5),
            rows(6, 10)
        ),
    );
    let found = words(&pdf);
    for i in 1..=10 {
        assert_eq!(
            found.iter().filter(|w| w.text == format!("r{i}")).count(),
            1,
            "G3: r{i}"
        );
    }
    // (5) the container in a bordered `<div>`, then a paragraph below the last row.
    let pdf = render(
        "",
        &format!(
            "<div style=\"border:1px solid #000\"><div style=\"columns:2\"><table><tbody>{}</tbody></table></div></div><p>below</p>",
            rows(1, 130)
        ),
    );
    let found = words(&pdf);
    assert_rows_once("G5", &found, 130);
    let below = found.iter().find(|w| w.text == "below").expect("below");
    let last_page_rows = found
        .iter()
        .filter(|w| w.page == below.page && row_number(w).is_some())
        .map(|w| w.y1)
        .fold(0.0, f32::max);
    assert!(
        below.y0 >= last_page_rows - 0.5,
        "G5: the paragraph after the container overlaps its rows"
    );
}
