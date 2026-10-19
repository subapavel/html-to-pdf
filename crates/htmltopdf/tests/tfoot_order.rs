//! Table row groups are placed by their `display`, not by where they are written (F36). CSS 2.1 §17.2:
//! `table-header-group` is "always displayed before all other rows and row groups", `table-footer-group`
//! "always displayed after all other rows and row groups"; only the first of each kind counts, later ones are
//! treated as `table-row-group`. The HTML 4.01 pattern `<thead><tfoot><tbody>` (§11.2.3 required it) printed the
//! totals straight under the column headings, above the items, before F36.
//!
//! Expected orders are Chrome 141's (probes T1-T8, `docs/plans/2026-10-19-f36-tfoot-order.md` §7). Order is
//! asserted on `yMin` of `pdftotext -bbox` words, never on the order pdftotext lists them in.

use std::path::{Path, PathBuf};

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "<thead><tr><th>Hlavička</th><th>Cena</th></tr></thead>";
const FOOT: &str = "<tfoot><tr><td>Celkem</td><td>999</td></tr></tfoot>";
const BODY: &str = "<tbody><tr><td>Položka1</td><td>1</td></tr><tr><td>Položka2</td><td>2</td></tr>\
                    <tr><td>Položka3</td><td>3</td></tr></tbody>";

#[test]
fn tfoot_before_tbody_prints_after_the_body() {
    let found = words(&render(&table("", &[HEAD, FOOT, BODY])));
    assert_order(&found, &["Hlavička", "Položka1", "Položka3", "Celkem"]);
}

#[test]
fn tfoot_first_matches_tfoot_last() {
    // Every word of `thead, tfoot, tbody` is where it is in `thead, tbody, tfoot`, with automatic and with fixed
    // layout. In fixed layout the column widths come from the first row in visual order (Chrome, probes fx1-fx4):
    // here the header's three cells, not the footer's single `colspan=3` cell.
    let head = "<thead><tr><th>Hlavička</th><th>Sloupec A</th><th>Sloupec B</th></tr></thead>";
    let foot = "<tfoot><tr><td colspan=\"3\">Celkem</td></tr><tr><td>Fx</td><td>Fy</td><td>Fz</td></tr></tfoot>";
    let body = "<tbody><tr><td>P1a</td><td>P1b</td><td>P1c</td></tr><tr><td>P2a</td><td>P2b</td><td>P2c</td></tr></tbody>";
    for attrs in ["", "style=\"table-layout: fixed; width: 100%\""] {
        let first = words(&render(&table(attrs, &[head, foot, body])));
        let last = words(&render(&table(attrs, &[head, body, foot])));
        assert_eq!(first.len(), last.len(), "[{attrs}] number of words");
        for w in &last {
            let f = word(&first, &w.text);
            assert!(
                f.page == w.page
                    && (f.x_min - w.x_min).abs() < 0.1
                    && (f.y_min - w.y_min).abs() < 0.1
                    && (f.y_max - w.y_max).abs() < 0.1,
                "[{attrs}] `{}` is at ({:.1}, {:.1}) with the footer first, ({:.1}, {:.1}) with it last",
                w.text,
                f.x_min,
                f.y_min,
                w.x_min,
                w.y_min
            );
        }
    }
}

#[test]
fn tfoot_after_tbody_is_unchanged() {
    // The modern order, the guard: nothing moves.
    let found = words(&render(&table("", &[HEAD, BODY, FOOT])));
    assert_order(&found, &["Hlavička", "Položka1", "Položka3", "Celkem"]);
}

#[test]
fn thead_after_tbody_prints_first() {
    let found = words(&render(&table("", &[BODY, HEAD])));
    assert_order(&found, &["Hlavička", "Položka1", "Položka3"]);
}

#[test]
fn div_table_footer_group_first() {
    // `display: table-*` boxes follow the same rule as the elements (css-display-3 §2.4: the role comes from
    // the computed `display`).
    let cell = |t: &str| format!("<div style=\"display: table-cell\">{t}</div>");
    let row = |a: &str, b: &str| {
        format!(
            "<div style=\"display: table-row\">{}{}</div>",
            cell(a),
            cell(b)
        )
    };
    let html = format!(
        "<div style=\"display: table; width: 100%\">\
         <div style=\"display: table-footer-group\">{}</div>\
         <div style=\"display: table-row-group\">{}{}</div></div>",
        row("Celkem", "999"),
        row("Položka1", "1"),
        row("Položka2", "2"),
    );
    let found = words(&render(&html));
    assert_order(&found, &["Položka1", "Položka2", "Celkem"]);
}

#[test]
fn only_the_first_footer_group_moves() {
    // Chrome: the first footer group goes last, a second one is a plain row group and keeps its place.
    let foot1 = "<tfoot><tr><td>Celkem1</td><td>1</td></tr></tfoot>";
    let foot2 = "<tfoot><tr><td>Celkem2</td><td>2</td></tr></tfoot>";
    let found = words(&render(&table("", &[foot1, BODY, foot2])));
    assert_order(&found, &["Položka1", "Položka3", "Celkem2", "Celkem1"]);
}

#[test]
fn role_follows_display_not_tag() {
    // A `<tfoot>` made `table-row-group` is not a footer: it stays where it is written.
    let foot =
        "<tfoot style=\"display: table-row-group\"><tr><td>Celkem</td><td>999</td></tr></tfoot>";
    let found = words(&render(&table("", &[foot, BODY])));
    assert_order(&found, &["Celkem", "Položka1", "Položka3"]);
    // A `<tbody>` made `table-footer-group` is the footer, however it is written.
    let tail =
        "<tbody style=\"display: table-footer-group\"><tr><td>Součet</td><td>6</td></tr></tbody>";
    let found = words(&render(&table("", &[tail, BODY])));
    assert_order(&found, &["Položka1", "Položka3", "Součet"]);
}

#[test]
fn caption_stays_on_top() {
    let caption = "<caption>Titulek</caption>";
    let found = words(&render(&table("", &[caption, HEAD, FOOT, BODY])));
    assert_order(
        &found,
        &["Titulek", "Hlavička", "Položka1", "Položka3", "Celkem"],
    );
}

#[test]
fn long_table_tfoot_first() {
    let pdf = render(&table("", &[HEAD, FOOT, &rows(120)]));
    let found = words(&pdf);
    let pages = page_count(&pdf);
    assert!(pages >= 3, "expected a multi-page table, got {pages}");
    for i in 1..=120 {
        let label = format!("Řádek{i}");
        assert_eq!(
            found.iter().filter(|w| w.text == label).count(),
            1,
            "{label} is not printed exactly once"
        );
    }
    for page in 1..=pages {
        let heads = found
            .iter()
            .filter(|w| w.page == page && w.text == "Hlavička")
            .count();
        assert_eq!(heads, 1, "page {page} has {heads} header rows");
    }
    let totals: Vec<&Word> = found.iter().filter(|w| w.text == "Celkem").collect();
    assert_eq!(totals.len(), 1, "Celkem is printed {} times", totals.len());
    let last_row = word(&found, "Řádek120");
    assert_eq!(totals[0].page, pages, "Celkem is not on the last page");
    assert_eq!(last_row.page, pages, "Řádek120 is not on the last page");
    assert!(
        totals[0].y_min > last_row.y_min,
        "Celkem (y {:.1}) is not below Řádek120 (y {:.1})",
        totals[0].y_min,
        last_row.y_min
    );
}

#[test]
fn long_table_thead_after_tbody_repeats() {
    // F12 repeats the header group on every continuation page; once it is first, one written after the body
    // repeats as well (Chrome, probe t3long).
    let pdf = render(&table("", &[&rows(120), HEAD]));
    let found = words(&pdf);
    let pages = page_count(&pdf);
    assert!(pages >= 3, "expected a multi-page table, got {pages}");
    for i in 1..=120 {
        let label = format!("Řádek{i}");
        assert_eq!(
            found.iter().filter(|w| w.text == label).count(),
            1,
            "{label} is not printed exactly once"
        );
    }
    for page in 1..=pages {
        let heads: Vec<&Word> = found
            .iter()
            .filter(|w| w.page == page && w.text == "Hlavička")
            .collect();
        assert_eq!(
            heads.len(),
            1,
            "page {page} has {} header rows",
            heads.len()
        );
        let first_row = found
            .iter()
            .filter(|w| w.page == page && w.text.starts_with("Řádek"))
            .map(|w| w.y_min)
            .fold(f32::MAX, f32::min);
        assert!(
            heads[0].y_min < first_row,
            "page {page}: the header (y {:.1}) is not above the first row (y {first_row:.1})",
            heads[0].y_min
        );
    }
}

#[test]
fn footer_row_background_follows_the_row() {
    // The dark total row is painted where the footer is printed (last), and the first row under the header
    // is white. Probed at 72 dpi, where `pdftotext -bbox` pt are device pixels, left of the text inside the
    // cell's padding.
    let foot =
        "<tfoot><tr class=\"bg-gray-900 text-white\"><td>Celkem</td><td>999</td></tr></tfoot>";
    let pdf = render(&table("", &[HEAD, foot, BODY]));
    let found = words(&pdf);
    let page = rasterise(&pdf, 1, 72);
    let probe = |text: &str| {
        let w = word(&found, text);
        page.pixel(
            (w.x_min - 2.0) as usize,
            ((w.y_min + w.y_max) / 2.0) as usize,
        )
    };
    let last = probe("Celkem");
    assert!(
        last.iter().all(|&c| c < 80),
        "the last row is not dark: {last:?}"
    );
    assert_order(&found, &["Položka3", "Celkem"]);
    let first = probe("Položka1");
    assert!(
        first.iter().all(|&c| c > 240),
        "the first body row is not white: {first:?}"
    );
    // Just under the header row there is the first body row, not the dark one.
    let header = word(&found, "Hlavička");
    let under = page.pixel((header.x_min - 2.0) as usize, (header.y_max + 8.0) as usize);
    assert!(
        under.iter().all(|&c| c > 240),
        "the row under the header is not white: {under:?}"
    );
}

// -- helpers --------------------------------------------------------------------------------------

fn table(attrs: &str, parts: &[&str]) -> String {
    format!("<table {attrs}>{}</table>", parts.concat())
}

fn rows(n: usize) -> String {
    let rows: String = (1..=n)
        .map(|i| format!("<tr><td>Řádek{i}</td><td>{i},00</td></tr>"))
        .collect();
    format!("<tbody>{rows}</tbody>")
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A4 with the page margins of the renderer, strict, the bundled Tailwind CSS and Inter.
fn renderer() -> Renderer {
    let root = root();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css")
        .css("body { margin: 0 } table { width: 100%; border-collapse: collapse } td, th { padding: 4px }");
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

fn render(body: &str) -> Vec<u8> {
    renderer()
        .render(
            &format!("<!doctype html><html lang=\"cs\"><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    y_max: f32,
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
                y_max: attr(attrs, "yMax"),
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

/// The words are printed on one page, each strictly below the one before (by `yMin`).
fn assert_order(found: &[Word], texts: &[&str]) {
    for pair in texts.windows(2) {
        let (a, b) = (word(found, pair[0]), word(found, pair[1]));
        assert!(
            a.page < b.page || (a.page == b.page && a.y_min < b.y_min),
            "`{}` (page {}, y {:.1}) must be above `{}` (page {}, y {:.1})",
            pair[0],
            a.page,
            a.y_min,
            pair[1],
            b.page,
            b.y_min
        );
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

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tfo-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
