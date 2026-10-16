//! Table cell content honours `vertical-align` (F13). A cell is as tall as its row, but its content sits at
//! the top, middle or bottom of that box according to `vertical-align` (CSS 2.1 §17.5.3; the HTML UA sheet,
//! §15.3.8, makes `middle` the default for row groups and rows, and `td`/`th` inherit it). `baseline` and
//! every other value act as `baseline`; for a cell whose first line is the row's tallest first line that is
//! the top. Before F13 every cell was top-aligned, so the numbers of a row whose description wrapped sat on
//! the first line. Asserted on word positions via poppler's `pdftotext -bbox`; the expectations were checked
//! against Chromium 141.
//!
//! Layout of the test tables: cell A is three lines (`Řa`, `Řb`, `Řc`) and cell B one line (`Bé`), line
//! height 20 CSS px = 15 pt. A line's `yMin` therefore differs by 15 pt from its neighbour's, and B's `yMin`
//! equals the `yMin` of the line of A it is aligned with.

use std::path::{Path, PathBuf};

use htmltopdf::{Page, RenderOptions, Renderer};

/// Tolerance for "same line", in pt.
const TOL: f32 = 1.0;

const CSS: &str = "table { border-collapse: collapse } \
                   td, th { padding: 4px 8px; line-height: 20px; text-align: left }";

const A: &str = "Řa<br>Řb<br>Řc";

#[test]
fn middle_is_default() {
    // No `vertical-align` anywhere: the UA sheet's `tbody { vertical-align: middle }` applies.
    let w = words(&render(&format!(
        "<table><tr><td>{A}</td><td>Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řb");
}

#[test]
fn explicit_middle_equals_default() {
    let w = words(&render(&format!(
        "<table><tr><td>{A}</td><td style=\"vertical-align: middle\">Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řb");
}

#[test]
fn align_top_keeps_top() {
    let w = words(&render(&format!(
        "<table><tr><td>{A}</td><td style=\"vertical-align: top\">Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řa");
}

#[test]
fn align_bottom() {
    let w = words(&render(&format!(
        "<table><tr><td>{A}</td><td style=\"vertical-align: bottom\">Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řc");
}

#[test]
fn row_vertical_align_is_inherited_by_cells() {
    // `td { vertical-align: inherit }` in the UA sheet: the row's value reaches a cell without its own.
    let w = words(&render(&format!(
        "<table><tr style=\"vertical-align: bottom\"><td>{A}</td><td>Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řc");
    let w = words(&render(&format!(
        "<table><tr style=\"vertical-align: top\"><td>{A}</td><td>Bé</td></tr></table>"
    )));
    assert_aligned(&w, "Bé", "Řa");
}

#[test]
fn baseline_acts_as_top() {
    // `baseline` aligns the first baselines of the row's cells; here A's first line is the tallest first
    // line, so B shares it. `sub` and the other values (CSS 2.1 §17.5.3) act as `baseline`.
    for value in ["baseline", "sub", "super", "text-top"] {
        let w = words(&render(&format!(
            "<table><tr><td>{A}</td><td style=\"vertical-align: {value}\">Bé</td></tr></table>"
        )));
        assert_aligned(&w, "Bé", "Řa");
    }
}

#[test]
fn single_line_row_does_not_move() {
    // Nothing to align: every `vertical-align` puts the one-line cells on the same line.
    for value in ["middle", "top", "bottom", "baseline"] {
        let w = words(&render(&format!(
            "<table><tr style=\"vertical-align: {value}\"><td>Řa</td><td>Bé</td></tr></table>"
        )));
        assert_aligned(&w, "Bé", "Řa");
    }
}

#[test]
fn block_children_cell() {
    // Cells whose content is block boxes (not an inline root) move as well, and so do the children of
    // the cell that is taller.
    let a = "<div>Řa</div><div>Řb</div><div>Řc</div>";
    for (value, line) in [("middle", "Řb"), ("top", "Řa"), ("bottom", "Řc")] {
        let w = words(&render(&format!(
            "<table><tr><td>{a}</td><td style=\"vertical-align: {value}\"><div>Bé</div></td></tr></table>"
        )));
        assert_aligned(&w, "Bé", line);
    }
}

#[test]
fn row_background_fills_row() {
    // The cell box keeps the row's height; only its content moves (F19 paints the cell's background).
    // Cells are 100 CSS px wide, rows 60 px tall; at 48 dpi that is 50 × 30 device pixels, so the second
    // cell covers device x 50..100, y 0..30.
    let pdf = render_at(
        "td { width: 100px; padding: 0; line-height: 20px; text-align: left } \
         table { border-collapse: separate; border-spacing: 0 }",
        &format!(
            "<table><tr><td style=\"background: red\">{A}</td><td style=\"background: rgb(0,128,0)\">Bé</td></tr></table>"
        ),
        0.0,
    );
    let page = rasterise(&pdf, 1, 48);
    for (x, y, what) in [
        (95, 2, "top of the shorter cell"),
        (95, 15, "middle of the shorter cell"),
        (95, 27, "bottom of the shorter cell"),
    ] {
        let got = page.pixel(x, y);
        assert!(
            got[0] < 10 && (got[1] as i32 - 128).abs() < 10 && got[2] < 10,
            "{what}: pixel ({x}, {y}) is {got:?}, expected green"
        );
    }
    let below = page.pixel(95, 33);
    assert!(
        below.iter().all(|&c| c > 245),
        "below the row: {below:?}, expected white"
    );
    // And the text really is in the middle of that box, not at its top.
    let w = words(&pdf);
    assert_aligned(&w, "Bé", "Řb");
}

#[test]
fn paginated_and_repeated() {
    // 120 two-line rows: more than two pages. Every row's `Bé<i>` is centred on its own two lines (7.5 pt
    // below the first), on every page, and the repeated thead is there too.
    let rows: String = (0..120)
        .map(|i| format!("<tr><td>Řa{i}<br>Řb{i}</td><td>Bé{i}</td></tr>"))
        .collect();
    let pdf = render(&format!(
        "<table><thead><tr><th>Hlavička</th><th>Cena</th></tr></thead><tbody>{rows}</tbody></table>"
    ));
    let pages = page_count(&pdf);
    assert!(
        pages >= 3,
        "expected a multi-page table, got {pages} page(s)"
    );
    let w = words(&pdf);
    let mut checked = 0;
    for page in 1..=pages {
        let on_page: Vec<&Word> = w.iter().filter(|x| x.page == page).collect();
        assert!(
            on_page.iter().any(|x| x.text == "Hlavička"),
            "page {page} has no header row"
        );
        for b in on_page.iter().filter(|x| x.text.starts_with("Bé")) {
            let i = &b.text["Bé".len()..];
            let a = on_page
                .iter()
                .find(|x| x.text == format!("Řa{i}"))
                .unwrap_or_else(|| panic!("row {i}: no `Řa{i}` on page {page}"));
            // Two 15 pt lines, one 15 pt line: centred is 7.5 pt below the first line.
            assert!(
                (b.y_min - (a.y_min + 7.5)).abs() <= TOL,
                "page {page}, row {i}: `{}` at {} but its row starts at {} (expected +7.5)",
                b.text,
                b.y_min,
                a.y_min
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 120, "every row checked exactly once");
}

#[test]
fn percentage_height_child_stays_inside_the_cell() {
    // `height: 100%` against a cell that is only as tall as the row is `auto` (CSS 2.1 §10.5: the
    // containing block's height is not specified explicitly), so the box is one line, centred, and
    // never pushed out of the four-line row. Chrome 141: `P100` at Řa + 22.5 pt.
    let d = "<td>Řa<br>Řb<br>Řc<br>Řd</td>";
    for (child, what) in [
        ("<div style=\"height: 100%\">P100</div>", "block"),
        (
            "<div style=\"height: 100%; display: flex; align-items: center\">P100</div>",
            "flex",
        ),
        ("<div style=\"min-height: 100%\">P100</div>", "min-height"),
    ] {
        let w = words(&render(&format!(
            "<table><tr>{d}<td>{child}</td></tr></table>"
        )));
        let find = |t: &str| w.iter().find(|x| x.text == t).unwrap();
        let (a, last, p) = (find("Řa"), find("Řd"), find("P100"));
        assert!(
            p.y_min >= a.y_min - TOL && p.y_max <= last.y_max + TOL,
            "{what}: `P100` ({}..{}) is outside the row ({}..{})",
            p.y_min,
            p.y_max,
            a.y_min,
            last.y_max
        );
        assert!(
            (p.y_min - (a.y_min + 22.5)).abs() <= TOL,
            "{what}: `P100` at {} is not centred (expected {})",
            p.y_min,
            a.y_min + 22.5
        );
    }
}

#[test]
fn cell_height_is_a_row_minimum_and_aligns() {
    // CSS 2.1 §17.5.3: a cell's `height` is the minimum height of its row. The row is 48 CSS px, both
    // cells are centred in it (Chrome 141: `Aa` and `Bé` at 71.06, next row at 102.56, i.e. 31.5 pt
    // below), and a `bottom` cell sits on the bottom edge.
    let table = |b: &str| {
        words(&render(&format!(
            "<table><tr><td style=\"height: 48px\">Aa</td><td{b}>Bé</td></tr>\
             <tr><td>Cc</td><td>Dd</td></tr></table>"
        )))
    };
    let w = table("");
    assert_aligned(&w, "Bé", "Aa");
    let (a, c) = (find(&w, "Aa"), find(&w, "Cc"));
    assert!(
        (c.y_min - a.y_min - 31.5).abs() <= TOL,
        "the second row starts {} pt below `Aa`, expected 31.5 (a 48 px first row)",
        c.y_min - a.y_min
    );
    let w = table(" style=\"vertical-align: bottom\"");
    let (a, b) = (find(&w, "Aa"), find(&w, "Bé"));
    assert!(
        (b.y_min - a.y_min - 10.5).abs() <= TOL,
        "bottom cell at {} but `Aa` at {}, expected 10.5 pt lower",
        b.y_min,
        a.y_min
    );
}

#[test]
fn row_height_is_a_row_minimum_and_aligns() {
    // `<tr style="height: 48px">` (Tailwind `h-12` on a `tr`): a 48 px row, so the next row starts 28.5 pt
    // below the centred text (Chrome 141: 68.06 and 96.56).
    let w = words(&render(
        "<table><tr style=\"height: 48px\"><td>Aa</td><td>Bé</td></tr><tr><td>Cc</td></tr></table>",
    ));
    assert_aligned(&w, "Bé", "Aa");
    let (a, c) = (find(&w, "Aa"), find(&w, "Cc"));
    assert!(
        (c.y_min - a.y_min - 28.5).abs() <= TOL,
        "the second row starts {} pt below `Aa`, expected 28.5",
        c.y_min - a.y_min
    );
}

#[test]
fn fixture_02_row_2_centred() {
    // Row 2 of the long invoice: the description wraps to two lines, `2` and `6 h` are centred on them.
    let root = root();
    let dir = root.join("fixtures/invoices/02-long");
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = fixture_renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("render");
    let w: Vec<Word> = words(&pdf).into_iter().filter(|x| x.page == 1).collect();
    let k = w
        .iter()
        .position(|x| x.text == "Konzultace")
        .expect("Konzultace");
    let first_line = w[k].y_min;
    let x = w[k..]
        .iter()
        .find(|x| x.text == "XSLT")
        .expect("XSLT after Konzultace");
    assert!(
        (x.y_min - first_line - 15.0).abs() < 2.0,
        "the description should wrap to two lines: {first_line} and {}",
        x.y_min
    );
    let mid = (first_line + x.y_min) / 2.0;
    let number = &w[k - 1];
    assert_eq!(number.text, "2", "row number");
    let qty = w[k..]
        .iter()
        .find(|x| x.text == "6")
        .expect("`6` of `6 h` after Konzultace");
    for (what, word) in [("row number", number), ("hours", qty)] {
        assert!(
            (word.y_min - mid).abs() <= TOL,
            "{what} `{}` at {} not centred between {first_line} and {} (middle {mid})",
            word.text,
            word.y_min,
            x.y_min
        );
    }
}

// -- helpers --------------------------------------------------------------------------------------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn render(body: &str) -> Vec<u8> {
    render_at(CSS, body, 20.0)
}

/// The page margin is explicit because `Page::margin_mm` wins over `@page`.
fn render_at(css: &str, body: &str, margin_mm: f32) -> Vec<u8> {
    Renderer::builder()
        .css(format!("body {{ margin: 0 }} {css}"))
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions {
                page: Page {
                    margin_mm,
                    ..Page::A4
                },
                ..Default::default()
            },
        )
        .expect("render")
}

/// The fixture renderer of `fixtures.rs`: bundled Tailwind CSS and Inter fonts.
fn fixture_renderer() -> Renderer {
    let root = root();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css");
    let mut fonts = std::fs::read_dir(root.join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect::<Vec<_>>();
    fonts.sort();
    assert!(
        !fonts.is_empty(),
        "no fonts in fixtures/fonts — run scripts/fetch-fonts.sh"
    );
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    b.build()
}

fn find<'a>(w: &'a [Word], t: &str) -> &'a Word {
    w.iter()
        .find(|x| x.text == t)
        .unwrap_or_else(|| panic!("`{t}` not found in the PDF text"))
}

/// `want` is on the same line as `line` (same `yMin` within `TOL`).
fn assert_aligned(w: &[Word], want: &str, line: &str) {
    let find = |t: &str| {
        w.iter()
            .find(|x| x.text == t)
            .unwrap_or_else(|| panic!("`{t}` not found in the PDF text"))
    };
    let (a, b) = (find(want), find(line));
    assert!(
        (a.y_min - b.y_min).abs() <= TOL,
        "`{want}` is at y {} but `{line}` is at y {}; expected them on one line (lines are 15 pt apart: {})",
        a.y_min,
        b.y_min,
        w.iter()
            .map(|x| format!("{}@{:.1}", x.text, x.y_min))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

struct Word {
    page: usize,
    text: String,
    y_min: f32,
    y_max: f32,
}

/// Every word of the PDF with its page and `yMin` (pt, from the top), in content order.
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
    let xml = String::from_utf8_lossy(&out.stdout);
    let mut page = 0;
    let mut v = Vec::new();
    for line in xml.lines() {
        let line = line.trim();
        if line.starts_with("<page ") {
            page += 1;
        } else if let Some(rest) = line.strip_prefix("<word ") {
            let attr = |name: &str| -> f32 {
                let key = format!("{name}=\"");
                let start = rest.find(&key).unwrap() + key.len();
                let end = start + rest[start..].find('"').unwrap();
                rest[start..end].parse().unwrap()
            };
            let text_start = rest.find('>').unwrap() + 1;
            let text_end = rest.rfind("</word>").unwrap();
            v.push(Word {
                page,
                text: rest[text_start..text_end].to_string(),
                y_min: attr("yMin"),
                y_max: attr("yMax"),
            });
        }
    }
    v
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

/// Page `n` (from 1) at `dpi` via poppler's `pdftoppm`.
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
        std::env::temp_dir().join(format!("htmltopdf-valign-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
