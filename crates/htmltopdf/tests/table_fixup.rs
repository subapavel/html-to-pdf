//! A table part whose `display` is not a table display (F50). `td { display: block | flex }` (Tailwind `block`,
//! `flex` on a `td`, the "stacked on mobile" template idiom `block md:table-cell` where `md:` does not match on A4),
//! `tr.block`, `tbody.block`: CSS 2.1 §17.2.1 wraps such a child in anonymous table objects (a run of consecutive
//! non-cells in a row becomes one anonymous cell, a run of non-rows in a row group or table becomes one anonymous
//! row), so the content is laid out and the row keeps its height. Before F50 the node was skipped: its content was
//! painted nowhere and the row collapsed.
//!
//! Expected positions are Chrome 141's (probes P1-P10 in `docs/plans/2026-10-16-f50-cell-display-override.md` §7).
//! Asserted on word positions (`pdftotext -bbox`); lines are forced with `<br>` (20 px = 15 pt each) so the numbers
//! do not depend on font metrics.

use htmltopdf::{Page, RenderOptions, Renderer};

/// Tolerance for "same position", in pt.
const TOL: f32 = 1.0;

const CSS: &str = "body { margin: 0 } p { margin: 0; line-height: 20px } \
    td { padding: 4px; vertical-align: top; line-height: 20px } table { width: 100%; border-spacing: 0 } \
    .block { display: block } .flex { display: flex } .hidden { display: none } .between { justify-content: space-between }";

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

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_max: f32,
    text: String,
}

fn attr(attrs: &str, name: &str) -> f32 {
    attrs
        .split_whitespace()
        .find_map(|a| a.strip_prefix(name))
        .and_then(|v| {
            v.trim_start_matches("=\"")
                .trim_end_matches('"')
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("word without {name}"))
}

/// Every word of the PDF as `(page, xMin, yMax, text)`, in extraction order (page by page, top to bottom).
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
        {
            found.push(Word {
                page,
                x_min: attr(attrs, "xMin"),
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
    let path = std::env::temp_dir().join(format!("htmltopdf-tfx-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// The first word `text`; panics with the words that were printed when it is missing.
fn word<'a>(found: &'a [Word], text: &str) -> &'a Word {
    found.iter().find(|w| w.text == text).unwrap_or_else(|| {
        let all: Vec<&str> = found.iter().map(|w| w.text.as_str()).collect();
        panic!("{text} is not printed; printed words: {all:?}")
    })
}

/// `text` on page `p` is within `TOL` of `y` pt below `from`.
fn assert_below_by(found: &[Word], text: &str, from: &str, pt: f32) {
    let (a, b) = (word(found, from), word(found, text));
    assert_eq!(a.page, b.page, "{text} and {from} on the same page");
    assert!(
        (b.y_max - a.y_max - pt).abs() <= TOL,
        "{text} is {:.1}pt below {from}, expected {pt:.1}pt",
        b.y_max - a.y_max
    );
}

fn assert_same_line(found: &[Word], a: &str, b: &str) {
    let (wa, wb) = (word(found, a), word(found, b));
    assert_eq!(wa.page, wb.page, "{a} and {b} on the same page");
    assert!(
        (wa.y_max - wb.y_max).abs() <= TOL,
        "{a} (y {:.1}) and {b} (y {:.1}) are not on one line",
        wa.y_max,
        wb.y_max
    );
}

fn assert_same_x(found: &[Word], a: &str, b: &str) {
    let (wa, wb) = (word(found, a), word(found, b));
    assert!(
        (wa.x_min - wb.x_min).abs() <= TOL,
        "{a} (x {:.1}) and {b} (x {:.1}) do not start at the same x",
        wa.x_min,
        wb.x_min
    );
}

fn assert_right_of(found: &[Word], right: &str, left: &str) {
    let (wr, wl) = (word(found, right), word(found, left));
    assert!(
        wr.x_min > wl.x_min + TOL,
        "{right} (x {:.1}) is not right of {left} (x {:.1})",
        wr.x_min,
        wl.x_min
    );
}

/// Every `texts` entry is below the previous one or on its line (page, then y), `strict` pairs strictly below.
fn assert_in_order(found: &[Word], texts: &[&str]) {
    let at: Vec<(usize, f32)> = texts
        .iter()
        .map(|t| {
            let w = word(found, t);
            (w.page, w.y_max)
        })
        .collect();
    for (i, pair) in at.windows(2).enumerate() {
        let ((p0, y0), (p1, y1)) = (pair[0], pair[1]);
        assert!(
            p1 > p0 || (p1 == p0 && y1 >= y0 - TOL),
            "{} (page {p1}, y {y1:.1}) is above {} (page {p0}, y {y0:.1})",
            texts[i + 1],
            texts[i]
        );
    }
}

fn assert_strictly_below(found: &[Word], later: &str, earlier: &str) {
    let (l, e) = (word(found, later), word(found, earlier));
    assert!(
        l.page > e.page || (l.page == e.page && l.y_max > e.y_max + TOL),
        "{later} (page {}, y {:.1}) is not below {earlier} (page {}, y {:.1})",
        l.page,
        l.y_max,
        e.page,
        e.y_max
    );
}

fn five(prefix: &str) -> String {
    (0..5)
        .map(|i| format!("{prefix}{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

// ---- 1 + 2: a cell with `display: block | flex` ---------------------------------------------------------------------

#[test]
fn a_block_cell_is_laid_out_and_keeps_its_row_height() {
    // tb_blk_noh, Chrome P1: the five lines are printed at the same x as `N`, the row is as tall as the block
    // (5 x 20 + 2 x 4 = 108 px), so `N` is 128 - 47 = 81 pt (108 px) below `A0000`.
    let pdf = render(&format!(
        "<table><tr><td style=\"display:block\"><p>{}</p></td></tr><tr><td>N</td></tr></table><p>AFTER</p>",
        five("A")
    ));
    let found = words(&pdf);
    for i in 0..5 {
        word(&found, &format!("A{i:04}"));
    }
    assert_below_by(&found, "N", "A0000", 81.0);
    assert_same_x(&found, "A0000", "N");
    assert_below_by(&found, "A0004", "A0000", 60.0);
    assert_strictly_below(&found, "AFTER", "N");
}

#[test]
fn a_flex_cell_with_a_height_is_a_real_height_and_its_content_overflows() {
    // cell_flex, Chrome P3: a `display:flex` td is no cell any more, its `height: 30px` is a height (content-box:
    // 30 + 2 x 4 = 38 px under this stylesheet, border-box: 30 px), the two `<p>` are flex items side by side and
    // overflow it over the next row.
    let table = "<table><tr><td class=\"flex\" style=\"height:30px\"><p>{A}</p><p>{B}</p></td></tr>\
                 <tr><td>N</td></tr></table><p>AFTER</p>";
    let table = table.replace("{A}", &five("A")).replace("{B}", &five("B"));
    let pdf = render(&table);
    let found = words(&pdf);
    for i in 0..5 {
        word(&found, &format!("A{i:04}"));
        word(&found, &format!("B{i:04}"));
    }
    assert_same_line(&found, "A0000", "B0000");
    assert_right_of(&found, "B0000", "A0000");
    assert_same_x(&found, "A0000", "N");
    assert_below_by(&found, "N", "A0000", 28.5);

    let pdf = render_with("td { box-sizing: border-box }", &table);
    let found = words(&pdf);
    assert_same_line(&found, "A0000", "B0000");
    assert_below_by(&found, "N", "A0000", 22.5);
}

// ---- 3: runs of misparented cells share one anonymous cell --------------------------------------------------------

#[test]
fn consecutive_block_cells_share_one_anonymous_cell_and_stack() {
    // P2: C1 | (X above Y) | C4, then N. `X` and `Y` are blocks of one anonymous cell: same x, one 21 pt line apart
    // (20 px line + 8 px padding = 28 px).
    let pdf = render(
        "<table><tr><td>C1</td><td class=\"block\">X</td><td class=\"block\">Y</td><td>C4</td></tr>\
         <tr><td>N</td></tr></table><p>AFTER</p>",
    );
    let found = words(&pdf);
    assert_same_x(&found, "X", "Y");
    assert_below_by(&found, "Y", "X", 21.0);
    assert_same_line(&found, "C1", "X");
    assert_same_line(&found, "C4", "X");
    assert_right_of(&found, "X", "C1");
    assert_right_of(&found, "C4", "X");
    assert_below_by(&found, "N", "Y", 21.0);
}

// ---- 4: every other display value ----------------------------------------------------------------------------------

#[test]
fn a_cell_with_any_other_display_is_still_printed() {
    // P4: grid, inline-block, list-item, contents, a nested `display: table`: each row has its first cell as the
    // marker, the second cell's content must be printed on the same line (an anonymous cell has no padding, so up
    // to 3 pt higher than the neighbouring td's text).
    let pdf = render(
        "<table>\
         <tr><td>R1a</td><td style=\"display:grid\">R1b</td></tr>\
         <tr><td>R2a</td><td style=\"display:inline-block\">R2b</td></tr>\
         <tr><td>R3a</td><td style=\"display:list-item;list-style:none\">R3b</td></tr>\
         <tr><td>R4a</td><td style=\"display:contents\">R4b</td></tr>\
         <tr><td>R5a</td><td style=\"display:table\"><div style=\"display:table-row\">\
         <div style=\"display:table-cell\">R5b</div></div></td></tr>\
         <tr><td>N</td></tr></table><p>AFTER</p>",
    );
    let found = words(&pdf);
    // Each `R{i}b` is right of `R{i}a` and inside its row: not above the row's text (an anonymous cell has no
    // padding, so up to 3 pt above it) and above the next row's text. (An inline-block sits 6.6 pt lower than
    // Chrome's, as it does in an ordinary cell: a separate baseline difference.)
    for r in 1..=5 {
        let (a, b) = (format!("R{r}a"), format!("R{r}b"));
        let next = if r < 5 {
            format!("R{}a", r + 1)
        } else {
            "N".to_string()
        };
        let (wa, wb, wn) = (word(&found, &a), word(&found, &b), word(&found, &next));
        assert!(
            wb.x_min > wa.x_min + TOL,
            "{b} is not right of {a} ({} vs {})",
            wb.x_min,
            wa.x_min
        );
        assert!(
            wb.y_max >= wa.y_max - 4.0 && wb.y_max < wn.y_max - TOL,
            "{b} (y {:.1}) is not inside the row of {a} (y {:.1}, next row at {:.1})",
            wb.y_max,
            wa.y_max,
            wn.y_max
        );
    }
    assert_in_order(&found, &["R1a", "R2a", "R3a", "R4a", "R5a", "N"]);
    assert_strictly_below(&found, "N", "R5a");
}

#[test]
fn inline_cells_share_a_line() {
    // P4: two `display: inline` cells are one run of an anonymous cell: `IA IB` on one line, left to right.
    let pdf = render(
        "<table><tr><td style=\"display:inline\">IA</td> <td style=\"display:inline\">IB</td></tr>\
         <tr><td>N</td></tr></table>",
    );
    let found = words(&pdf);
    assert_same_line(&found, "IA", "IB");
    assert_right_of(&found, "IB", "IA");
    assert_strictly_below(&found, "N", "IA");
}

// ---- 5 + 6: rows and row groups ------------------------------------------------------------------------------------

#[test]
fn a_row_with_display_block_or_flex_is_printed_between_its_neighbours() {
    // P5: `tr.block` and `tr.flex` with real cells are boxes inside an anonymous row. Horizontal placement of their
    // cells (Chrome puts U1 and U2 side by side in a nested anonymous table) is F50b; here every word is printed
    // and the row sits between the normal rows.
    for class in ["block", "flex"] {
        let pdf = render(&format!(
            "<table><tr><td>T1</td></tr><tr class=\"{class}\"><td>U1</td><td>U2</td></tr>\
             <tr><td>T2</td></tr></table><p>AFTER</p>"
        ));
        let found = words(&pdf);
        word(&found, "U1");
        word(&found, "U2");
        assert_strictly_below(&found, "U1", "T1");
        assert_strictly_below(&found, "T2", "U1");
        assert_strictly_below(&found, "T2", "U2");
        assert_strictly_below(&found, "AFTER", "T2");
    }
}

#[test]
fn a_row_group_with_display_block_is_printed_in_order() {
    // P6: `tbody.block` between two normal bodies; every word, DOM order, top to bottom.
    let pdf = render(
        "<table><tbody><tr><td>T1</td></tr></tbody>\
         <tbody class=\"block\"><tr><td>U1</td><td>U2</td></tr><tr><td>U3</td></tr></tbody>\
         <tbody><tr><td>T2</td></tr></tbody></table><p>AFTER</p>",
    );
    let found = words(&pdf);
    assert_in_order(&found, &["T1", "U1", "U2", "U3", "T2", "AFTER"]);
    assert_strictly_below(&found, "U1", "T1");
    assert_strictly_below(&found, "T2", "U3");
}

#[test]
fn a_stacked_card_row_prints_every_line_in_order() {
    // The card shape of fixture 10: `tr.block` holding `td.block` lines and a `td.flex` label/value line.
    let pdf = render(
        "<table><tr><td>T1</td></tr>\
         <tr class=\"block\"><td class=\"block\">Title</td><td class=\"block\">Desc</td>\
         <td class=\"flex between\"><span>Label</span><span>Value</span></td></tr>\
         <tr><td>T2</td></tr></table><p>AFTER</p>",
    );
    let found = words(&pdf);
    assert_in_order(&found, &["T1", "Title", "Desc", "Label", "T2", "AFTER"]);
    assert_strictly_below(&found, "Title", "T1");
    assert_strictly_below(&found, "Desc", "Title");
    assert_strictly_below(&found, "Label", "Desc");
    assert_same_line(&found, "Label", "Value");
    assert_strictly_below(&found, "T2", "Label");
}

// ---- 7: bare cells in a `display: table` div -----------------------------------------------------------------------

#[test]
fn bare_cells_before_a_row_form_their_own_anonymous_row() {
    // P7: `table-cell` K1, K2, then a `table-row` with K3: K1 and K2 share a row (anonymous), K3 is the next row.
    let pdf = render(
        "<div style=\"display:table;width:100%\"><div style=\"display:table-cell\">K1</div>\
         <div style=\"display:table-cell\">K2</div><div style=\"display:table-row\">\
         <div style=\"display:table-cell\">K3</div></div></div><p>AFTER</p>",
    );
    let found = words(&pdf);
    assert_same_line(&found, "K1", "K2");
    assert_right_of(&found, "K2", "K1");
    assert_below_by(&found, "K3", "K1", 15.0);
    assert_same_x(&found, "K3", "K1");
}

#[test]
fn a_cell_after_a_row_starts_a_new_anonymous_row() {
    // P7: a row with K1, then a bare cell K2: K2 is in a new row below, at the left edge.
    let pdf = render(
        "<div style=\"display:table;width:100%\"><div style=\"display:table-row\">\
         <div style=\"display:table-cell\">K1</div></div>\
         <div style=\"display:table-cell\">K2</div></div><p>AFTER</p>",
    );
    let found = words(&pdf);
    assert_below_by(&found, "K2", "K1", 15.0);
    assert_same_x(&found, "K2", "K1");
}

// ---- 8: pagination --------------------------------------------------------------------------------------------------

#[test]
fn a_long_table_of_block_cells_paginates_and_repeats_its_header() {
    // 120 rows, each a normal cell and a `td.block` description, under a normal `thead`: every row once, in order,
    // the header on every page.
    let rows: String = (1..=120)
        .map(|i| format!("<tr><td>Name{i:04}</td><td class=\"block\">Desc{i:04}</td></tr>"))
        .collect();
    let pdf = render(&format!(
        "<table><thead><tr><th>HEAD</th></tr></thead><tbody>{rows}</tbody></table><p>AFTER</p>"
    ));
    let found = words(&pdf);
    let pages = page_count(&pdf);
    assert!(pages >= 2, "expected a multi-page table, got {pages}");
    for prefix in ["Name", "Desc"] {
        let seen: Vec<&str> = found
            .iter()
            .filter(|w| w.text.starts_with(prefix))
            .map(|w| w.text.as_str())
            .collect();
        let want: Vec<String> = (1..=120).map(|i| format!("{prefix}{i:04}")).collect();
        assert_eq!(
            seen,
            want.iter().map(String::as_str).collect::<Vec<_>>(),
            "{prefix} words, once each and in order"
        );
    }
    let heads = found.iter().filter(|w| w.text == "HEAD").count();
    assert_eq!(heads, pages, "the header repeats on every page");
    assert_same_line(&found, "Name0001", "Desc0001");
}

// ---- 9: row backgrounds behind an anonymous cell --------------------------------------------------------------------

#[test]
fn a_row_background_shows_behind_a_flex_cell() {
    // P9: the `tr` background is painted behind its cells, the anonymous cell around a `td.flex` included
    // (CSS 2.1 §17.5.1). Cells are 100 x 40 CSS px at 48 dpi: the flex box is device x 50..100, y 0..20.
    let pdf = Renderer::builder()
        .css(
            "body { margin: 0 } table { border-collapse: separate; border-spacing: 0 } \
             td { width: 100px; height: 40px; padding: 0 } .flex { display: flex }"
                .to_string(),
        )
        .build()
        .render(
            "<!doctype html><html><body><table>\
             <tr style=\"background: #eee\"><td>Q1</td><td class=\"flex\">Q2</td></tr>\
             </table></body></html>",
            &RenderOptions {
                page: Page {
                    margin_mm: 0.0,
                    ..Page::A4
                },
                ..Default::default()
            },
        )
        .expect("render");
    let (r, g, b) = pixel(&pdf, 48, 90, 15);
    for c in [r, g, b] {
        assert!(
            c.abs_diff(0xee) <= 4,
            "inside the flex cell: ({r}, {g}, {b}), expected #eee"
        );
    }
    let (r, g, b) = pixel(&pdf, 48, 25, 15);
    for c in [r, g, b] {
        assert!(
            c.abs_diff(0xee) <= 4,
            "inside the first cell: ({r}, {g}, {b}), expected #eee"
        );
    }
}

/// The colour of device pixel `(x, y)` of page 1 at `dpi` via poppler's `pdftoppm`.
fn pixel(pdf: &[u8], dpi: u32, x: usize, y: usize) -> (u8, u8, u8) {
    let pdf_path = temp(pdf);
    let base = pdf_path.with_extension("");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-f", "1", "-l", "1", "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(out.status.success(), "pdftoppm failed");
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
    // Header: "P6\n<w> <h>\n<max>\n".
    assert!(ppm.starts_with(b"P6"), "not a binary PPM");
    let mut fields = Vec::new();
    let mut pos = 2;
    while fields.len() < 3 {
        while ppm[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        while !ppm[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(
            std::str::from_utf8(&ppm[start..pos])
                .unwrap()
                .parse::<usize>()
                .unwrap(),
        );
    }
    let data = &ppm[pos + 1..];
    let i = (y * fields[0] + x) * 3;
    (data[i], data[i + 1], data[i + 2])
}

// ---- 10: guards (green before and after) ----------------------------------------------------------------------------

#[test]
fn a_hidden_cell_leaves_no_gap() {
    let with = words(&render(
        "<table><tr><td>G1</td><td class=\"hidden\">GX</td><td>G2</td></tr><tr><td>N</td></tr></table>",
    ));
    let without = words(&render(
        "<table><tr><td>G1</td><td>G2</td></tr><tr><td>N</td></tr></table>",
    ));
    assert!(
        with.iter().all(|w| w.text != "GX"),
        "display: none is not printed"
    );
    for w in ["G1", "G2", "N"] {
        let (a, b) = (word(&with, w), word(&without, w));
        assert!(
            (a.x_min - b.x_min).abs() <= 0.1 && (a.y_max - b.y_max).abs() <= 0.1,
            "{w} moved: ({:.1}, {:.1}) vs ({:.1}, {:.1})",
            a.x_min,
            a.y_max,
            b.x_min,
            b.y_max
        );
    }
}

#[test]
fn whitespace_between_cells_creates_no_cell() {
    let compact = words(&render(
        "<table><tr><td>W1</td><td>W2</td></tr><tr><td>W3</td><td>W4</td></tr></table><p>AFTER</p>",
    ));
    let spaced = words(&render(
        "<table>\n <tbody>\n <tr>\n  <td>W1</td>\n  <td>W2</td>\n </tr>\n <tr>\n  <td>W3</td>\n  <td>W4</td>\n </tr>\n \
         </tbody>\n</table>\n<p>AFTER</p>",
    ));
    for w in ["W1", "W2", "W3", "W4", "AFTER"] {
        let (a, b) = (word(&compact, w), word(&spaced, w));
        assert!(
            (a.x_min - b.x_min).abs() <= 0.1 && (a.y_max - b.y_max).abs() <= 0.1,
            "{w} moved: ({:.1}, {:.1}) vs ({:.1}, {:.1})",
            a.x_min,
            a.y_max,
            b.x_min,
            b.y_max
        );
    }
}
