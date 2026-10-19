//! Collapsed table borders (F22). Tailwind's preflight puts every table in `border-collapse: collapse`. CSS 2.1
//! §17.6.2: borders of cells, rows and row groups "are centered on the grid lines between the cells", so each
//! ruled row is taller by the rule's width and a border shared by two cells is one line, not two. Blitz used to lay
//! every table out as `separate`: row rules vanished, shared cell borders counted twice.
//!
//! F22a (layout, tests G1-G8) is Blitz: cells get half of the collapsed border on each side. F22b (painting, tests
//! P1-P13, `#[ignore]`d until it lands) draws the resolved lines.
//!
//! Expected numbers are Chrome 141's, from the step-0 probes of `docs/plans/2026-10-19-f22-row-borders.md` §7:
//! the same HTML printed with Chrome (Inter loaded from `fixtures/fonts`, `@page { margin: 0 }`) and read with the
//! same `pdftotext -bbox`. Geometry is asserted on word `yMin`, paint on pixels from `pdftoppm`.

use std::path::{Path, PathBuf};

use htmltopdf::{Margins, Page, RenderOptions, Renderer};

/// Pitch tolerance in pt (the plan's ±0.2): Taffy rounds 0.5 px halves, single rows may alternate by 0.375 pt.
const PITCH_TOL: f32 = 0.2;
/// Max per-channel distance from an expected colour when sampling pixels.
const TOL: u8 = 4;

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

/// Five one-cell rows `W1`..`W5`; `row` and `cell` are inline styles of every `<tr>` and `<td>`.
fn five(row: &str, cell: &str) -> String {
    let rows: String = (1..=5)
        .map(|i| format!("<tr style=\"{row}\"><td style=\"{cell}\">W{i}</td></tr>"))
        .collect();
    format!("<table>{rows}</table>")
}

// -- F22a: layout ---------------------------------------------------------------------------------

#[test]
fn row_rule_adds_its_width_to_the_pitch() {
    // G1. Chrome: pitch 29 px = 21.75 pt (28 px without the rule).
    let found = words(&render_g(&five("border-bottom: 1px solid", "")));
    assert_pitch(&found, 21.75, "tr border-bottom 1px");
    let found = words(&render_g(&five("border-bottom: 3px solid", "")));
    assert_pitch(&found, 23.25, "tr border-bottom 3px");
}

#[test]
fn shared_cell_border_counts_once() {
    // G2. `td { border: 1px solid }`: the line between two rows is one pixel, pitch 29 px = 21.75 pt. The
    // separated model (Blitz before F22a) made it 30 px = 22.5 pt.
    let found = words(&render_g(&five("", "border: 1px solid")));
    assert_pitch(&found, 21.75, "td border 1px");
    let found = words(&render_g(&five("", "border: 2px solid")));
    assert_pitch(&found, 22.5, "td border 2px");
}

#[test]
fn wider_border_sets_the_line() {
    // G3. A 4 px row rule against a 2 px cell border: the wider one wins, the pitch grows by 4 px = 3 pt
    // (Chrome 24 pt), not by 2 + 4.
    let found = words(&render_g(&five(
        "border-bottom: 4px solid",
        "border-bottom: 2px solid",
    )));
    assert_pitch(&found, 24.0, "row 4px against cell 2px");
}

#[test]
fn hidden_cell_border_suppresses_the_row_rule() {
    // G4, a guard: `hidden` wins over every other border of the line (§17.6.2.1), pitch stays 28 px = 21 pt.
    let found = words(&render_g(&five(
        "border-bottom: 3px solid",
        "border-bottom: hidden",
    )));
    assert_pitch(&found, 21.0, "cell hidden against row 3px");
}

#[test]
fn separate_model_is_unchanged() {
    // G5, a guard. `border-collapse: separate` keeps its geometry byte for byte: the numbers are the pitches of
    // main's binary (deacfe9) for these two documents (row rules dropped, `border-spacing` not implemented, F21,
    // cell borders 1 px on each side). Chrome's own `separate` pitches differ because of its 2 px spacing.
    let sep = "table { border-collapse: separate }";
    let rule = render_with(sep, &five("border-bottom: 1px solid", ""));
    assert_pitch(&words(&rule), 21.0, "separate, tr border-bottom 1px");
    let cell = render_with(sep, &five("", "border: 1px solid"));
    assert_pitch(&words(&cell), 22.5, "separate, td border 1px");
}

#[test]
fn table_holds_half_the_outer_border() {
    // G6. `width: 300px` with 10 px cell borders. Chrome: the table's border-box is 300 px wide and holds half
    // of the outer border (5 px) on each side, the cell half another 5 px, so the first word is at 5 + 5 + 4
    // px = 10.5 pt. The two columns have the same content, so they are 145 px wide each and the second word is at
    // 5 + 145 + 5 + 4 px = 119.25 pt. The right edge enters through that: a table that came out wider or narrower
    // than 300 px would shift it. (With different words Chrome divides the width in proportion to the contents and
    // Taffy equally, which is not what this test is about.)
    let html = "<table style=\"width: 300px\"><tr><td>Aa</td><td>Aa</td></tr>\
                <tr><td>Cc</td><td>Cc</td></tr></table>";
    let found = words(&render_g(&format!(
        "<style>td {{ border: 10px solid }}</style>{html}"
    )));
    for (text, xs) in [("Aa", [10.5, 119.25]), ("Cc", [10.5, 119.25])] {
        let mut at: Vec<&Word> = found.iter().filter(|w| w.text == text).collect();
        at.sort_by(|a, b| a.x_min.total_cmp(&b.x_min));
        assert_eq!(at.len(), 2, "`{text}` is printed {} times", at.len());
        for (w, x) in at.iter().zip(xs) {
            assert!(
                (w.x_min - x).abs() < PITCH_TOL,
                "`{text}` is at x {:.2} pt, Chrome has {x}",
                w.x_min
            );
        }
    }
    let rows = word(&found, "Cc").y_min - word(&found, "Aa").y_min;
    assert!(
        (rows - 28.5).abs() < PITCH_TOL,
        "row pitch {rows:.2} pt, Chrome has 28.5 (38 px: 28 + 5 + 5)"
    );
}

#[test]
fn fixture_02_page_1_ends_with_row_19() {
    // G7. Chrome fits rows 1-19 on page 1; main's binary fitted row 20 because it dropped every row rule.
    let pdf = render_fixture("02-long");
    let found = words(&pdf);
    assert_eq!(word(&found, "IF-1019").page, 1, "IF-1019 is not on page 1");
    assert_eq!(word(&found, "IF-1020").page, 2, "IF-1020 is not on page 2");
}

#[test]
fn fixture_words_match_chrome_below_tables() {
    // G8. A word under the ruled items table sits within 1 pt of the committed Chrome reference: 01 and 04 were
    // 4.7 pt too high (the row rules), 20 was 8.0 pt too low (every shared line counted twice).
    for (fixture, text) in [
        ("01-basic", "úhradě"),
        ("04-bilingual-vat", "Daň"),
        ("20-tfoot-first", "IBAN"),
    ] {
        let ours = word(&words(&render_fixture(fixture)), text).clone();
        let reference = std::fs::read(
            root()
                .join("fixtures/invoices")
                .join(fixture)
                .join("reference/chrome.pdf"),
        )
        .expect("reference PDF");
        let want = word(&words(&reference), text).clone();
        assert_eq!(ours.page, want.page, "{fixture}: `{text}` page");
        assert!(
            (ours.y_min - want.y_min).abs() <= 1.0,
            "{fixture}: `{text}` yMin is {:.2} pt, Chrome has {:.2}",
            ours.y_min,
            want.y_min
        );
    }
}

// -- F22b: painting -------------------------------------------------------------------------------

/// Cells hold a 40 px tall `<div>` with no padding; a table is 300 px wide with 100 px columns. Without borders row
/// `r` starts at CSS y `40 r`; every border adds half its width on each side of its grid line (§17.6.2). Pages are
/// rasterised at 96 dpi, so device pixels are CSS px.
const GRID: &str = "table { border-collapse: collapse; width: 300px } td { padding: 0; width: 100px } \
                    td > div { height: 40px }";

fn grid_rows(rows: usize, tr_style: &str, td_style: &str) -> String {
    let cell = format!("<td style=\"{td_style}\"><div></div></td>");
    let row = format!("<tr style=\"{tr_style}\">{}</tr>", cell.repeat(3));
    format!("<table>{}</table>", row.repeat(rows))
}

#[test]
#[ignore = "F22b"]
fn tr_border_bottom_is_painted() {
    // P1. A 2 px red rule under each row, across all three cells; the line between rows 1 and 2 is centred on
    // y = 41 (row 1 is 40 + 1 px high), so it covers device rows 40 and 41.
    let page = rasterise(
        &render_grid(&grid_rows(2, "border-bottom: 2px solid red", "")),
        1,
        96,
    );
    for x in [10, 150, 290] {
        assert_pixel(&page, (x, 40), RED, "rule, upper row");
        assert_pixel(&page, (x, 41), RED, "rule, lower row");
        assert_pixel(&page, (x, 20), WHITE, "cell interior, row 1");
        assert_pixel(&page, (x, 60), WHITE, "cell interior, row 2");
    }
}

#[test]
#[ignore = "F22b"]
fn divide_y_rows() {
    // P2. Tailwind's `divide-y` is `border-bottom-width: 1px` on every row but the last. Only `divide-gray-100`
    // (#f3f4f6) is in the compiled stylesheet, which is 12 off white: tolerance 4 still tells it from white.
    let pdf = render_tailwind(
        "<table class=\"w-full\"><tbody class=\"divide-y divide-gray-100\">\
         <tr><td><div style=\"height: 40px\"></div></td></tr>\
         <tr><td><div style=\"height: 40px\"></div></td></tr>\
         <tr><td><div style=\"height: 40px\"></div></td></tr></tbody></table>",
    );
    let page = rasterise(&pdf, 1, 96);
    // Rows: 40.5 | 1 px line | 41 | line | 40.5; the lines span [40, 41) and [81.5, 82.5).
    assert_pixel(
        &page,
        (200, 40),
        [243, 244, 246],
        "line between rows 1 and 2",
    );
    assert_pixel(&page, (200, 121), WHITE, "nothing under the last row");
    assert_pixel(&page, (200, 20), WHITE, "row interior");
}

#[test]
#[ignore = "F22b"]
fn row_group_border_top() {
    // P3. A 3 px blue line above the first body row only: the header row ends at 40 + 1.5 px.
    let html = format!(
        "<table><thead><tr>{c}</tr></thead>\
         <tbody style=\"border-top: 3px solid blue\"><tr>{c}</tr><tr>{c}</tr></tbody></table>",
        c = "<td><div></div></td>".repeat(3)
    );
    let page = rasterise(&render_grid(&html), 1, 96);
    for y in [40, 41, 42] {
        assert_pixel(&page, (150, y), BLUE, "line above the first body row");
    }
    assert_pixel(&page, (150, 100), WHITE, "no line between the body rows");
}

#[test]
#[ignore = "F22b"]
fn wider_wins() {
    // P4. Cell 4 px green against row 2 px red: green over its column, the row's red over the others. The row is
    // 40 + 2 px high (half of the widest segment), so the line is centred on y = 42.
    let html = "<table><tr style=\"border-bottom: 2px solid red\">\
                <td style=\"border-bottom: 4px solid green\"><div></div></td>\
                <td><div></div></td><td><div></div></td></tr>\
                <tr><td><div></div></td><td><div></div></td><td><div></div></td></tr></table>";
    let page = rasterise(&render_grid(html), 1, 96);
    assert_pixel(&page, (50, 40), GREEN, "cell border over the row rule");
    assert_pixel(&page, (50, 43), GREEN, "cell border, lower half");
    assert_pixel(&page, (150, 41), RED, "row rule where no cell has a border");
    assert_pixel(&page, (150, 40), WHITE, "the rule is only 2 px wide there");
}

#[test]
#[ignore = "F22b"]
fn cell_beats_row_at_equal_width() {
    // P5 (§17.6.2.1 rule 5): same width, same style: cell > row.
    let html = "<table><tr style=\"border-bottom: 2px solid red\">\
                <td style=\"border-bottom: 2px solid green\"><div></div></td>\
                <td><div></div></td></tr><tr><td><div></div></td><td><div></div></td></tr></table>";
    let page = rasterise(&render_grid(html), 1, 96);
    assert_pixel(&page, (50, 40), GREEN, "cell border over the row rule");
    assert_pixel(&page, (50, 41), GREEN, "cell border over the row rule");
    assert_pixel(&page, (150, 41), RED, "the row rule next to it");
}

#[test]
#[ignore = "F22b"]
fn solid_beats_dotted() {
    // P6 (rule 4): at equal width the style decides, `solid` over `dotted`, whatever the origin.
    let html = "<table><tr style=\"border-bottom: 2px solid blue\">\
                <td style=\"border-bottom: 2px dotted red\"><div></div></td></tr>\
                <tr><td><div></div></td></tr></table>";
    let page = rasterise(&render_grid(html), 1, 96);
    for x in 10..60 {
        assert_pixel(
            &page,
            (x, 40),
            BLUE,
            "solid row rule over the dotted cell border",
        );
    }
}

#[test]
#[ignore = "F22b"]
fn left_cell_wins() {
    // P7 (rule 6): two cells, equal borders on the line between them: the left one's colour.
    let html = "<table><tr>\
                <td style=\"border-right: 2px solid red\"><div></div></td>\
                <td style=\"border-left: 2px solid blue\"><div></div></td></tr></table>";
    let page = rasterise(&render_grid(html), 1, 96);
    // The line is centred on the column boundary at x = 150 (two 150 px columns).
    assert_pixel(&page, (149, 20), RED, "left cell's border");
    assert_pixel(&page, (150, 20), RED, "left cell's border");
}

#[test]
#[ignore = "F22b"]
fn shared_line_painted_once() {
    // P8. Translucent borders: one layer of black at 50 % is 128 grey; painted by both neighbours it would be 64.
    let page = rasterise(
        &render_grid(&grid_rows(2, "", "border: 2px solid rgba(0, 0, 0, 0.5)")),
        1,
        96,
    );
    assert_pixel(&page, (100, 20), [128, 128, 128], "line between two cells");
}

#[test]
#[ignore = "F22b"]
fn rule_below_every_repeated_header() {
    // P9. 120 rows, `thead` with a 3 px blue bottom border: Chrome prints the header on every page with its own
    // rule under it (probe brk_headonly: blue at CSS y 28-30 on pages 1 and 2).
    let rows: String = (0..120).map(|_| "<tr><td><div></div></td></tr>").collect();
    let html = format!(
        "<table><thead style=\"border-bottom: 3px solid blue\"><tr><td><div></div></td></tr></thead>\
         <tbody>{rows}</tbody></table>"
    );
    let pdf = render_grid(&html);
    let pages = page_count(&pdf);
    assert!(pages >= 3, "expected a multi-page table, got {pages}");
    for n in 1..=pages {
        let page = rasterise(&pdf, n, 96);
        assert_pixel(
            &page,
            (150, 41),
            BLUE,
            &format!("page {n}, rule under the header"),
        );
    }
}

#[test]
#[ignore = "F22b"]
fn rule_at_page_break() {
    // P10. A 3 px red rule under every row of a 60-row table, Chrome probe brk_rows: the line that falls on the
    // break is cut in two, its upper half (1.5 px) ends page 1 and its lower half starts page 2 (red at CSS y 0-1),
    // and the first rule of page 2 is centred 1.5 px lower than on page 1 (y 30-32 against 28-30).
    let pdf = render_grid(&grid_rows(60, "border-bottom: 3px solid red", ""));
    assert!(page_count(&pdf) >= 2);
    let second = rasterise(&pdf, 2, 96);
    assert_pixel(
        &second,
        (150, 0),
        RED,
        "lower half of the rule cut by the break",
    );
    assert_pixel(
        &second,
        (150, 1),
        RED,
        "lower half of the rule cut by the break",
    );
    assert_pixel(&second, (150, 30), RED, "first whole rule of page 2");
    assert_pixel(&second, (150, 10), WHITE, "first row of page 2");
}

#[test]
#[ignore = "F22b"]
fn separate_model_paints_no_row_borders() {
    // P11. `border-collapse: separate`: rows cannot have borders (§17.6.1), no red anywhere on the page.
    let html = format!(
        "<style>table {{ border-collapse: separate }}</style>{}",
        grid_rows(3, "border-bottom: 2px solid red", "")
    );
    let page = rasterise(&render_grid(&html), 1, 96);
    for y in 0..200 {
        for x in (0..300).step_by(7) {
            let p = page.pixel(x, y);
            assert!(
                !(p[0] > 200 && p[1] < 80 && p[2] < 80),
                "red pixel at ({x}, {y}) in the separated model"
            );
        }
    }
}

#[test]
#[ignore = "F22b"]
fn fixture_04_header_rule_is_indigo() {
    // P12. `tr.border-b-2.border-indigo-600` on the items header: Chrome's reference has a 2 px line at device
    // y 341-342, 680 px wide, rgb(79, 57, 246).
    let page = rasterise(&render_fixture("04-bilingual-vat"), 1, 96);
    let near = |p: [u8; 3]| {
        p.iter()
            .zip([79u8, 57, 246])
            .all(|(a, b)| a.abs_diff(b) <= 16)
    };
    let best = (336..348)
        .map(|y| (0..page.width).filter(|&x| near(page.pixel(x, y))).count())
        .max()
        .unwrap();
    assert!(
        best >= 600,
        "widest indigo run near y 341 is {best} px, Chrome has 680"
    );
}

#[test]
#[ignore = "F22b"]
fn table_border_collapses_with_cells() {
    // P13. `border: 4px solid blue` on the table against 2 px cell borders: the 4 px line wins on the outer
    // edge, centred on it, so it covers x 0-3 inside the table's border box (half of it is in the table's border).
    let html = "<table style=\"border: 4px solid blue\"><tr>\
                <td style=\"border: 2px solid black\"><div></div></td>\
                <td style=\"border: 2px solid black\"><div></div></td>\
                <td style=\"border: 2px solid black\"><div></div></td></tr></table>";
    let page = rasterise(&render_grid(html), 1, 96);
    for x in 0..4 {
        assert_pixel(&page, (x, 20), BLUE, "outer line, table border wins");
    }
    assert_pixel(&page, (5, 20), WHITE, "inside the line");
}

// -- helpers --------------------------------------------------------------------------------------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn font_files() -> Vec<PathBuf> {
    let mut fonts: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    assert!(
        !fonts.is_empty(),
        "no fonts in fixtures/fonts — run scripts/fetch-fonts.sh"
    );
    fonts
}

/// Strict, Inter bundled, no Tailwind, `css` appended after the common rules.
fn renderer(css: &str, tailwind: bool) -> Renderer {
    let mut b = Renderer::builder().strict_fonts(true);
    if tailwind {
        b = b
            .css_file(root().join("fixtures/tailwind/tailwind.css"))
            .expect("tailwind.css");
    }
    b = b.css(css);
    for f in font_files() {
        b = b.font_file(f).unwrap();
    }
    b.build()
}

/// A margin-less page (the caller's margin wins over `@page`).
fn options() -> RenderOptions {
    RenderOptions {
        lang: Some("cs".into()),
        page: Page {
            margin: Some(Margins::uniform_mm(0.0)),
            ..Page::default()
        },
        ..Default::default()
    }
}

fn render_with(css: &str, body: &str) -> Vec<u8> {
    let css = format!("{G_CSS} {css}");
    renderer(&css, false)
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &options(),
        )
        .expect("render")
}

/// The document of probes G1-G6: collapsed borders, 4 px padding, 20 px lines in Inter.
const G_CSS: &str = "body { margin: 0 } table { border-collapse: collapse } td { padding: 4px; font: 16px/20px Inter }";

fn render_g(body: &str) -> Vec<u8> {
    render_with("", body)
}

fn render_grid(body: &str) -> Vec<u8> {
    let css = format!("body {{ margin: 0 }} {GRID}");
    renderer(&css, false)
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &options(),
        )
        .expect("render")
}

fn render_tailwind(body: &str) -> Vec<u8> {
    renderer("body { margin: 0 }", true)
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &options(),
        )
        .expect("render")
}

/// A fixture with the bundled Tailwind CSS and Inter, as `scripts/render-fixtures.sh` renders it (these fixtures
/// have no images).
fn render_fixture(name: &str) -> Vec<u8> {
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    renderer("", true)
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("render")
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    text: String,
}

fn attr(attrs: &str, name: &str) -> f32 {
    let key = format!("{name}=\"");
    let start = attrs
        .find(&key)
        .unwrap_or_else(|| panic!("word without {name}"))
        + key.len();
    let end = start + attrs[start..].find('"').unwrap();
    attrs[start..end].parse().unwrap()
}

/// Every word of the PDF with its box, from `pdftotext -bbox` (pt, origin top left).
fn words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf, "pdf");
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
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

/// The first word `text`; panics with the words that were printed when it is missing.
fn word<'a>(found: &'a [Word], text: &str) -> &'a Word {
    found.iter().find(|w| w.text == text).unwrap_or_else(|| {
        let all: Vec<&str> = found.iter().map(|w| w.text.as_str()).collect();
        panic!("{text} is not printed; printed words: {all:?}")
    })
}

/// Mean pitch of `W1`..`W5` (four gaps, so rounding of single rows cancels) against `want` pt.
fn assert_pitch(found: &[Word], want: f32, what: &str) {
    let (first, last) = (word(found, "W1"), word(found, "W5"));
    assert_eq!(first.page, last.page, "{what}: the rows span two pages");
    let pitch = (last.y_min - first.y_min) / 4.0;
    assert!(
        (pitch - want).abs() <= PITCH_TOL,
        "{what}: row pitch is {pitch:.3} pt, Chrome has {want}"
    );
}

struct Raster {
    width: usize,
    rgb: Vec<u8>,
}

impl Raster {
    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

fn assert_pixel(page: &Raster, (x, y): (usize, usize), want: [u8; 3], what: &str) {
    let got = page.pixel(x, y);
    assert!(
        got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= TOL),
        "{what}: pixel ({x}, {y}) is {got:?}, expected {want:?}"
    );
}

/// Page `n` (from 1) at `dpi` via poppler's `pdftoppm`. Fails loudly when it is not installed.
fn rasterise(pdf: &[u8], n: usize, dpi: u32) -> Raster {
    let pdf_path = temp(pdf, "pdf");
    let base = pdf_path.with_extension("");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", &dpi.to_string()])
        .args(["-f", &n.to_string(), "-l", &n.to_string(), "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(
        out.status.success(),
        "pdftoppm failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
    // Header: "P6\n<w> <h>\n<max>\n".
    assert!(ppm.starts_with(b"P6"), "not a binary PPM");
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(
            std::str::from_utf8(&ppm[start..i])
                .unwrap()
                .parse::<usize>()
                .unwrap(),
        );
    }
    Raster {
        width: fields[0],
        rgb: ppm[i + 1..].to_vec(),
    }
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf, "pdf");
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

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-rowb-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
