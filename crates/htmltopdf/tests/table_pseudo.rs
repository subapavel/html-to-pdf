//! `::before` / `::after` on a table box (F163). Plan: docs/plans/2026-10-21-f163-table-pseudo-elements.md.
//!
//! The pseudo-element boxes of a table box (`<table>`, `display: table`, `inline-table`) are its first and last child
//! (css-pseudo-4 §3.1) and take part in the table's fixup (CSS 2.1 §17.2.1): a block or inline one sits in an anonymous
//! row and cell, a `table-cell` one is a cell of an anonymous row, a `table-row` one is a row. Blitz created them but never
//! put them in the grid: with fewer rows than pseudo-elements Taffy indexed past the grid items and the render panicked
//! (`EnginePanic`); with enough rows the generated text was not painted.
//!
//! Every case renders A4 with the default 15 mm margins, `border-spacing: 0` (F21), Inter bundled, strict fonts.
//! Positions are offsets `(dx, dy)` in pt from the first word, measured in Chrome 148 with the same Inter loaded through
//! `@font-face` (tolerance 1.5 pt; the absolute offset differs by a constant because Chrome uses the host's Inter in
//! the fixtures, we the bundled one). Positions come from `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

/// Against Chrome.
const CHROME_PT: f32 = 1.5;

const CSS: &str = "body { margin: 0; font-family: Inter; line-height: 20px } p { margin: 0 } \
    table { border-spacing: 0 }";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn try_render(css: &str, body: &str) -> htmltopdf::Result<Vec<u8>> {
    let html = format!("<!doctype html><html><body>{body}</body></html>");
    Renderer::builder()
        .font_bytes(inter())
        .expect("Inter")
        .strict_fonts(true)
        .css(format!("{CSS} {css}"))
        .build()
        .render(&html, &RenderOptions::default())
}

fn render(css: &str, body: &str) -> Vec<u8> {
    try_render(css, body).expect("render")
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x0: f32,
    y0: f32,
    text: String,
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> Vec<u8> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tp-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let mut cmd = std::process::Command::new(tool);
    cmd.args(args).arg(&path);
    if tool == "pdftotext" {
        cmd.arg("-");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{tool} (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    out.stdout
}

/// Every word of the PDF in extraction order (page by page, top to bottom).
fn words(pdf: &[u8]) -> Vec<Word> {
    let text = String::from_utf8_lossy(&poppler("pdftotext", &["-bbox"], pdf)).into_owned();
    let mut page = 0;
    let mut found = Vec::new();
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(&format!("{name}=\"")))
            .and_then(|v| v.trim_end_matches('"').parse().ok())
            .unwrap_or_else(|| panic!("word without {name}"))
    };
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
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(&poppler("pdfinfo", &[], pdf))
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

fn texts(words: &[Word]) -> Vec<&str> {
    words.iter().map(|w| w.text.as_str()).collect()
}

/// The words of `pdf` are `expected` in order, at the offsets `(dx, dy)` from the first word that Chrome has.
fn assert_like_chrome(what: &str, pdf: &[u8], expected: &[(&str, f32, f32)]) {
    let found = words(pdf);
    assert_eq!(
        texts(&found),
        expected.iter().map(|e| e.0).collect::<Vec<_>>(),
        "{what}: words"
    );
    for (w, (_, dx, dy)) in found.iter().zip(expected) {
        let (gx, gy) = (w.x0 - found[0].x0, w.y0 - found[0].y0);
        assert!(
            (gx - dx).abs() <= CHROME_PT && (gy - dy).abs() <= CHROME_PT,
            "{what}: {} is at ({gx:.1}, {gy:.1}) from the first word, Chrome has ({dx:.1}, {dy:.1})",
            w.text
        );
    }
}

fn word<'a>(found: &'a [Word], text: &str) -> &'a Word {
    found
        .iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("{text} is not printed: {:?}", texts(found)))
}

const B: &str = "table::before { content: \"B\" }";
const A: &str = "table::after { content: \"A\" }";

// --- T1-T4: the crash ------------------------------------------------------------------------------------------------

#[test]
fn t1_a_table_without_rows_and_a_before_prints_it() {
    // Main: `EnginePanic` (index out of bounds in Blitz's table wrapper).
    let pdf = render(B, "<table></table><p>z</p>");
    assert_like_chrome("T1", &pdf, &[("B", 0.0, 0.0), ("z", 0.0, 15.0)]);
}

#[test]
fn t2_before_and_after_around_one_row() {
    let css = format!("{B} {A}");
    let pdf = render(&css, "<table><tr><td>b</td></tr></table><p>z</p>");
    // Chrome: the anonymous row, the row and the anonymous row, 15 pt apart; `b` is indented by the cell's padding.
    assert_like_chrome(
        "T2",
        &pdf,
        &[
            ("B", 0.0, 0.0),
            ("b", 0.75, 15.8),
            ("A", 0.0, 31.5),
            ("z", 0.0, 46.5),
        ],
    );
}

#[test]
fn t3_a_display_table_box_puts_its_after_in_the_row_of_its_bare_cell() {
    let css = "#t { display: table } #t::after { content: \"A\" }";
    let body = "<div id=\"t\"><div style=\"display:table-cell\">x</div></div><p>z</p>";
    let pdf = render(css, body);
    // Chrome: one anonymous row, `x` and `A` on one line, A right of x (it prints "xA", the cells touch).
    assert_like_chrome(
        "T3",
        &pdf,
        &[("x", 0.0, 0.0), ("A", 6.55, 0.0), ("z", 0.0, 15.0)],
    );
}

#[test]
fn t4_a_before_and_one_row() {
    // Main: no panic, but `B` was not printed.
    let pdf = render(B, "<table><tr><td>b</td></tr></table><p>z</p>");
    assert_like_chrome(
        "T4",
        &pdf,
        &[("B", 0.0, 0.0), ("b", 0.75, 15.8), ("z", 0.0, 31.5)],
    );
}

// --- T5-T8: the display of the pseudo-element ---------------------------------------------------------------------------

#[test]
fn t5_to_t7_a_block_cell_or_row_pseudo_is_a_row_above_the_first_one() {
    for display in ["block", "table-cell", "table-row"] {
        let css = format!("table::before {{ content: \"B\"; display: {display} }}");
        let pdf = render(
            &css,
            "<table><tr><td>a1</td><td>a2</td></tr></table><p>z</p>",
        );
        assert_like_chrome(
            display,
            &pdf,
            &[
                ("B", 0.0, 0.0),
                ("a1", 0.75, 15.8),
                ("a2", 13.4, 15.8),
                ("z", 0.0, 31.5),
            ],
        );
    }
}

#[test]
fn t8_a_caption_pseudo_does_not_crash() {
    let css = "table::before { content: \"C\"; display: table-caption }";
    let pdf = render(css, "<table><tr><td>a1</td></tr></table><p>z</p>");
    assert_eq!(texts(&words(&pdf)), ["a1", "z"]);
}

#[test]
#[ignore = "F30e-b: table-caption boxes are not laid out"]
fn t8_a_caption_pseudo_is_printed_above_the_table() {
    let css = "table::before { content: \"C\"; display: table-caption }";
    let pdf = render(css, "<table><tr><td>a1</td></tr></table><p>z</p>");
    assert_like_chrome(
        "T8",
        &pdf,
        &[("C", 0.0, 0.0), ("a1", 0.75, 15.8), ("z", 0.0, 31.5)],
    );
}

// --- T9: the order of header and footer groups (F36) ------------------------------------------------------------------

#[test]
fn t9_the_header_comes_first_and_the_footer_last_around_the_pseudo_rows() {
    let css = format!("{B} {A}");
    let body = "<table><tfoot><tr><td>F</td></tr></tfoot><tbody><tr><td>body</td></tr></tbody>\
        <thead><tr><td>H</td></tr></thead></table><p>z</p>";
    let pdf = render(&css, body);
    // Chrome: H, the `::before` row, the body, the `::after` row, F (CSS 2.1 §17.2).
    assert_like_chrome(
        "T9",
        &pdf,
        &[
            ("H", 0.0, 0.0),
            ("B", -0.75, 15.8),
            ("body", 0.0, 31.5),
            ("A", -0.75, 47.2),
            ("F", 0.0, 63.0),
            ("z", -0.75, 78.7),
        ],
    );
}

// --- T10, T11: inline-table and collapsed borders (F22) ----------------------------------------------------------------

#[test]
fn t10_an_inline_table_with_a_before() {
    let css = format!("{B} table {{ display: inline-table }}");
    let pdf = render(
        &css,
        "<p>x <table><tr><td>b</td></tr></table> y</p><p>z</p>",
    );
    let found = words(&pdf);
    let mut printed = texts(&found);
    printed.sort_unstable();
    assert_eq!(printed, ["B", "b", "x", "y", "z"]);
    // Chrome: the generated row is above the cell and in its column. (Where the line `y` goes is the inline-table
    // baseline, which is not about pseudo-elements.)
    let (pseudo, cell) = (word(&found, "B"), word(&found, "b"));
    assert!(pseudo.y0 < cell.y0, "B is above b");
    assert!(
        (pseudo.x0 + 0.75 - cell.x0).abs() <= CHROME_PT,
        "same column"
    );
}

#[test]
fn t11_collapsed_borders_with_pseudo_rows() {
    let css =
        format!("{B} {A} table {{ border-collapse: collapse }} td {{ border: 2px solid #000 }}");
    let pdf = render(
        &css,
        "<table><tr><td>a1</td><td>a2</td></tr></table><p>z</p>",
    );
    assert_like_chrome(
        "T11",
        &pdf,
        &[
            ("B", 0.0, 0.0),
            ("a1", 1.5, 17.2),
            ("a2", 15.64, 17.2),
            ("A", 0.0, 34.5),
            ("z", -0.75, 49.5),
        ],
    );
}

// --- T12: header repetition and pagination (F12) ---------------------------------------------------------------------

#[test]
fn t12_the_header_repeats_and_each_pseudo_row_is_printed_once() {
    let css = format!("{B} {A} td {{ padding: 6px }}");
    let rows: String = (1..=60)
        .map(|i| format!("<tr><td>r{i}</td><td>r{i}1</td></tr>"))
        .collect();
    let body = format!(
        "<table><thead><tr><th>H1</th><th>H2</th></tr></thead><tbody>{rows}</tbody></table>"
    );
    let pdf = render(&css, &body);
    // Chrome: 2 pages, `B` on the first, `A` on the last, the header on both.
    assert_eq!(page_count(&pdf), 2);
    let found = words(&pdf);
    let on = |text: &str| -> Vec<usize> {
        found
            .iter()
            .filter(|w| w.text == text)
            .map(|w| w.page)
            .collect()
    };
    assert_eq!(on("H1"), [1, 2]);
    assert_eq!(on("B"), [1]);
    assert_eq!(on("A"), [2]);
}

// --- T13: column widths with a pseudo row (CSS 2.1 §17.5.2) -----------------------------------------------------------

/// `b.x0 - a.x0`: where the second column starts, from the first word of the first column.
fn column_gap(pdf: &[u8], first: &str, second: &str) -> f32 {
    let found = words(pdf);
    word(&found, second).x0 - word(&found, first).x0
}

#[test]
fn t13_a_pseudo_row_does_not_move_the_columns_of_an_automatic_table() {
    for (what, rows) in [
        (
            "widths in row 1",
            "<tr><td style=\"width:100px\">a</td><td style=\"width:200px\">b</td></tr>\
             <tr><td>c</td><td>d</td></tr>",
        ),
        (
            "widths in row 2",
            "<tr><td>a</td><td>b</td></tr>\
             <tr><td style=\"width:150px\">c</td><td style=\"width:200px\">dd</td></tr>",
        ),
    ] {
        let body = format!("<table>{rows}</table>");
        let with = column_gap(&render(B, &body), "a", "b");
        let without = column_gap(&render("", &body), "a", "b");
        assert!((with - without).abs() <= 0.5, "{what}: {with} / {without}");
    }
}

#[test]
fn t13_in_a_fixed_table_the_pseudo_row_is_the_first_row() {
    let css = format!("{B} table {{ table-layout: fixed; width: 400px }}");
    let body = "<table><tr><td style=\"width:100px\">a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>";
    // Chrome: the first row, which is the generated one, decides the columns: the `width` of `a` is not used, the
    // second column starts at 150 pt (200 px, an even split of 400 px is 300 pt / 2).
    let with = column_gap(&render(&css, body), "a", "b");
    assert!((with - 150.0).abs() <= CHROME_PT, "{with}");
    let without = column_gap(
        &render("table { table-layout: fixed; width: 400px }", body),
        "a",
        "b",
    );
    assert!((without - 75.75).abs() <= CHROME_PT, "{without}");
}

// --- T14: the other table parts ------------------------------------------------------------------------------------------

#[test]
fn t14_a_cell_pseudo_is_the_start_of_the_cell() {
    let pdf = render(
        "td::before { content: \"C\" }",
        "<table><tr><td>a</td></tr></table><p>z</p>",
    );
    assert_eq!(texts(&words(&pdf)), ["Ca", "z"]);
}

#[test]
#[ignore = "F163b: the pseudo-elements of rows and row groups are not constructed"]
fn t14_the_pseudo_elements_of_rows_and_groups() {
    for (css, body, first) in [
        (
            "tr::before { content: \"R\" }",
            "<table><tr><td>a</td></tr></table><p>z</p>",
            "Ra",
        ),
        (
            "tbody::before { content: \"G\" }",
            "<table><tbody><tr><td>a</td></tr></tbody></table><p>z</p>",
            "G",
        ),
    ] {
        let pdf = render(css, body);
        assert_eq!(words(&pdf)[0].text, first, "{css}");
    }
}

// --- T15: content that makes no row, and out-of-flow pseudo-elements ---------------------------------------------------------

#[test]
fn t15_an_empty_or_counter_pseudo() {
    let body = "<table><tr><td>a</td></tr></table><p>z</p>";
    // `content: ""` makes an empty row: no height, nothing printed.
    let pdf = render("table::before { content: \"\" }", body);
    assert_like_chrome("T15 empty", &pdf, &[("a", 0.0, 0.0), ("z", -0.75, 15.8)]);
    // Blitz flushes strings only (construct.rs); a counter prints `0`, as in Chrome.
    let pdf = render("table::before { content: counter(c) }", body);
    assert_like_chrome(
        "T15 counter",
        &pdf,
        &[("0", 0.0, 0.0), ("a", 0.75, 15.8), ("z", 0.0, 31.5)],
    );
}

#[test]
fn t15_a_floating_pseudo() {
    let css = "table::before { content: \"B\"; float: left }";
    let pdf = render(css, "<table><tr><td>a</td></tr></table><p>z</p>");
    assert_like_chrome(
        "T15 float",
        &pdf,
        &[("B", 0.0, 0.0), ("a", 0.75, 15.8), ("z", 0.0, 31.5)],
    );
}

#[test]
#[ignore = "F170: a position: absolute pseudo-element of a table box is not printed"]
fn t15_an_absolutely_positioned_pseudo() {
    let css = "table::before { content: \"B\"; position: absolute }";
    let pdf = render(css, "<table><tr><td>a</td></tr></table><p>z</p>");
    assert_like_chrome(
        "T15 absolute",
        &pdf,
        &[("B", 0.0, 0.0), ("a", 0.75, 0.8), ("z", 0.0, 16.5)],
    );
}

// --- Bytes ---------------------------------------------------------------------------------------------------------------------

#[test]
fn rendering_twice_gives_the_same_bytes() {
    let css = format!("{B} {A}");
    let body = "<table><tr><td>b</td></tr></table><div id=\"t\" style=\"display:table\"><div style=\"display:table-cell\">x</div></div>";
    assert_eq!(render(&css, body), render(&css, body));
}

#[test]
fn t17_empty_nested_and_row_less_tables_around_each_other() {
    // An empty table, a table with a nested empty one, a table with an empty row: Chrome prints the pseudo-elements
    // of each (pdftotext joins the two of an empty table into "BA").
    let css = format!("{B} {A}");
    let body = "<table></table><table><tr><td><table></table></td></tr></table>\
        <table><tr></tr></table><p>z</p>";
    let pdf = try_render(&css, body).expect("render");
    assert_like_chrome(
        "T17",
        &pdf,
        &[
            ("BA", 0.0, 0.0),
            ("B", 0.0, 15.0),
            ("BA", 0.75, 30.7),
            ("A", 0.0, 46.5),
            ("B", 0.0, 61.5),
            ("A", 0.0, 76.5),
            ("z", 0.0, 91.5),
        ],
    );
}

// --- T18: a table-cell pseudo, or `order` on a cell, keeps the header repeating (F163c) -----------------------------

/// Cells have 6 px of horizontal padding so that `pdftotext` keeps neighbouring cells apart.
/// A table of a two-cell `thead` (`Head1`, `Head2`) and 130 body rows `rN` / `vN`, plus `extra` after the body.
fn long_table(extra: &str) -> String {
    let rows: String = (1..=130)
        .map(|i| format!("<tr><td>r{i}</td><td>v{i}</td></tr>"))
        .collect();
    format!(
        "<table><thead><tr><th>Head1</th><th>Head2</th></tr></thead><tbody>{rows}</tbody>{extra}</table>"
    )
}

/// The pages `text` is printed on.
fn pages_of(found: &[Word], text: &str) -> Vec<usize> {
    found
        .iter()
        .filter(|w| w.text == text)
        .map(|w| w.page)
        .collect()
}

/// Every body word of [`long_table`] exactly once, and the header at the top of every page.
fn assert_rows_once_and_header_on_every_page(what: &str, pdf: &[u8], pages: usize) {
    let found = words(pdf);
    assert_eq!(page_count(pdf), pages, "{what}: pages (Chrome)");
    for i in 1..=130 {
        for text in [format!("r{i}"), format!("v{i}")] {
            assert_eq!(pages_of(&found, &text).len(), 1, "{what}: {text} once");
        }
    }
    let all: Vec<usize> = (1..=pages).collect();
    assert_eq!(
        pages_of(&found, "Head1"),
        all,
        "{what}: the header on every page"
    );
    for page in 1..=pages {
        let first = found.iter().find(|w| w.page == page).unwrap();
        assert_eq!(
            first.text, "Head1",
            "{what}: page {page} starts with the header"
        );
    }
}

#[test]
fn t18a_a_table_cell_before_keeps_the_header_repeating() {
    for (what, css) in [
        ("separate", ""),
        (
            "collapse",
            "table { border-collapse: collapse } td, th { border: 1px solid #000 }",
        ),
    ] {
        let css = format!(
            "table::before {{ content: \"BEFORE-ONCE\"; display: table-cell }} td, th {{ padding: 0 6px }} {css}"
        );
        let pdf = render(&css, &long_table(""));
        // Chrome 148: 3 pages, the header on each, the pseudo once, in an anonymous row under the header, in the
        // first column.
        assert_rows_once_and_header_on_every_page(what, &pdf, 3);
        let found = words(&pdf);
        assert_eq!(pages_of(&found, "BEFORE-ONCE"), [1], "{what}");
        let (head, before, r1) = (
            word(&found, "Head1"),
            word(&found, "BEFORE-ONCE"),
            word(&found, "r1"),
        );
        assert!(
            head.y0 < before.y0 && before.y0 < r1.y0,
            "{what}: the pseudo row is between the header and the first row"
        );
        // Chrome: 4.5 pt (separate) and 4.9 pt (collapse) left of `r1`: the pseudo cell is not a `td` and has no
        // padding.
        assert!(
            (before.x0 - (r1.x0 - 4.5)).abs() <= CHROME_PT,
            "{what}: in the first column, {:.1} pt left of r1",
            r1.x0 - before.x0
        );
    }
}

#[test]
fn t18c_order_on_a_cell_does_not_move_it() {
    // `order` does not apply to table cells (css-flexbox-1 §5.4 / css-display-3): Chrome prints the table as without
    // the rule. Before F163c the cell was walked first by the fragmenter: 8 of 266 words on 2 pages.
    let css = "td, th { padding: 0 6px } tbody tr:last-child td:first-child { order: -1 }";
    let pdf = render(css, &long_table(""));
    assert_rows_once_and_header_on_every_page("order", &pdf, 3);
    let plain = render("td, th { padding: 0 6px }", &long_table(""));
    assert_eq!(
        texts(&words(&pdf)),
        texts(&words(&plain)),
        "the same words in the same order as without the rule"
    );
}

#[test]
fn t18d_a_table_cell_after_sits_above_the_footer() {
    let css = "table::after { content: \"AFT\"; display: table-cell } td, th { padding: 0 6px }";
    let pdf = render(
        css,
        &long_table("<tfoot><tr><td>Foot</td><td>F2</td></tr></tfoot>"),
    );
    let found = words(&pdf);
    let pages = page_count(&pdf);
    // Chrome 148: 3 pages, `AFT` once on the last page, directly above the footer (it also repeats the footer on
    // every page, which fulgur does not do yet, F12b).
    assert_eq!(pages, 3);
    let all: Vec<usize> = (1..=pages).collect();
    assert_eq!(pages_of(&found, "Head1"), all);
    assert_eq!(pages_of(&found, "AFT"), [pages]);
    let (aft, last_row) = (word(&found, "AFT"), word(&found, "r130"));
    let foot = found.iter().rfind(|w| w.text == "Foot").expect("Foot");
    assert_eq!(foot.page, pages);
    assert!(
        last_row.y0 < aft.y0 && aft.y0 < foot.y0,
        "AFT between the last row and the footer"
    );
}
