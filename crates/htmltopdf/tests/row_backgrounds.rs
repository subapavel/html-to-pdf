//! Backgrounds of `<tr>` and row groups are painted behind the cells they cover (F19). CSS 2.1 §17.5.1
//! layers a table bottom to top as table, row groups, rows, cells; a row or row-group background shows
//! wherever the cells above it are transparent. Before F19 they were never painted, so the white-on-dark
//! total row of fixture 03 printed white text on a white page. Asserted on pixels via poppler's `pdftoppm`.

use std::path::{Path, PathBuf};

use htmltopdf::{Page, RenderOptions, Renderer};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

/// Max per-channel distance from the expected colour (conversion round-trips, anti-aliasing is avoided by
/// sampling cell centres).
const TOL: u8 = 4;

/// Cells are 100 × 40 CSS px with no padding; at 48 dpi that is 50 × 20 device pixels, so the centre of
/// column `c`, row `r` (both from 0) is device pixel `(50 * c + 25, 20 * r + 10)`.
const CELLS: &str = "table { border-collapse: separate; border-spacing: 0 } \
                     td, th { width: 100px; height: 40px; padding: 0 }";

fn at(c: usize, r: usize) -> (usize, usize) {
    (50 * c + 25, 20 * r + 10)
}

#[test]
fn row_paints_its_cells_only() {
    // Row 1 is red with a green second cell; row 2 is blue but has only one cell, so the area below the
    // green cell is covered by no cell: row backgrounds are painted per cell, not over the row's box.
    let pdf = render(
        CELLS,
        r#"<table>
  <tr style="background: red"><td></td><td style="background: green"></td></tr>
  <tr style="background: blue"><td></td></tr>
</table>"#,
    );
    let page = rasterise(&pdf, 1, 48);
    assert_pixel(&page, at(0, 0), RED, "row 1, cell without background");
    assert_pixel(
        &page,
        at(1, 0),
        GREEN,
        "row 1, cell with its own background",
    );
    assert_pixel(&page, at(0, 1), BLUE, "row 2, its only cell");
    assert_pixel(&page, at(1, 1), WHITE, "row 2, the cell it does not have");
}

#[test]
fn row_group_background_and_order() {
    // Row groups are the lowest layer: a plain row in the green `tbody` is green, a red row inside it is
    // red, and the `thead` above it (no background) stays white.
    let pdf = render(
        CELLS,
        r#"<table>
  <thead><tr><td></td></tr></thead>
  <tbody style="background: green">
    <tr><td></td></tr>
    <tr style="background: red"><td></td></tr>
  </tbody>
</table>"#,
    );
    let page = rasterise(&pdf, 1, 48);
    assert_pixel(&page, at(0, 0), WHITE, "thead without background");
    assert_pixel(&page, at(0, 1), GREEN, "plain row in the green tbody");
    assert_pixel(&page, at(0, 2), RED, "red row in the green tbody");
}

#[test]
fn backgrounds_on_every_page() {
    // 120 rows of 20 px: three or more pages. Every page starts with the blue header band (the first
    // page natively, the others through the F12 repeated copy) and carries green body rows below it.
    let rows: String = (0..120)
        .map(|_| r#"<tr><td style="height: 20px"></td></tr>"#)
        .collect();
    let pdf = render(
        CELLS,
        &format!(
            r#"<table>
  <thead style="background: blue"><tr><th style="height: 20px"></th></tr></thead>
  <tbody style="background: green">{rows}</tbody>
</table>"#
        ),
    );
    let pages = page_count(&pdf);
    assert!(
        pages >= 3,
        "expected a multi-page table, got {pages} page(s)"
    );
    for n in 1..=pages {
        let page = rasterise(&pdf, n, 48);
        // Device y 5 is CSS y 10: inside the 20 px header band. Device y 30 is CSS y 60: the third row.
        assert_pixel(&page, (25, 5), BLUE, &format!("page {n}, header band"));
        assert_pixel(&page, (25, 30), GREEN, &format!("page {n}, body row"));
    }
}

#[test]
fn fixture_03_total_is_dark() {
    // The total row `<tr class="bg-gray-900 text-white font-bold">` carries the background; its cells have
    // only a border. Poppler rasterises at 72 dpi here so that `pdftotext -bbox` coordinates (pt) are
    // device pixels.
    let root = root();
    let dir = root.join("fixtures/invoices/03-logo-qr");
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = fixture_renderer(&dir)
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("render");
    let (x0, y0, x1, y1) = word_bbox(&pdf, "úhradě");
    let page = rasterise(&pdf, 1, 72);
    let (mut dark, mut light, mut total) = (0, 0, 0);
    for y in y0.floor() as usize..y1.ceil() as usize {
        for x in x0.floor() as usize..x1.ceil() as usize {
            let p = page.pixel(x, y);
            total += 1;
            if p.iter().all(|&c| c < 80) {
                dark += 1;
            }
            if p.iter().all(|&c| c > 200) {
                light += 1;
            }
        }
    }
    assert!(
        dark * 2 > total,
        "the area behind `úhradě` is not dark: {dark} of {total} pixels"
    );
    assert!(
        light > 0,
        "no white text pixel inside the `úhradě` box ({total} pixels, {dark} dark)"
    );
    // The row's left edge, well outside the word, is part of the same bar.
    let left = page.pixel((x0 - 100.0) as usize, ((y0 + y1) / 2.0) as usize);
    assert!(
        left.iter().all(|&c| c < 80),
        "the total row is not dark at its left, got {left:?}"
    );
}

#[test]
#[ignore = "F21: border-spacing has no layout effect in Blitz"]
fn border_spacing_gap_is_not_painted() {
    // With `border-collapse: separate` the `border-spacing` area shows the table's own background, not the
    // row's (CSS 2.1 §17.5.1). Blitz ignores `border-spacing`, so the cells abut and there is no gap yet.
    let pdf = render(
        "td { width: 100px; height: 40px; padding: 0 }",
        r#"<table style="border-collapse: separate; border-spacing: 10px; background: yellow">
  <tr style="background: red"><td></td><td></td></tr>
</table>"#,
    );
    let page = rasterise(&pdf, 1, 48);
    // Cells span CSS x 10..110 and 120..220; x 115 (device 57) is the gap between them, y 30 (device 15)
    // is inside the row.
    assert_pixel(&page, (57, 15), [255, 255, 0], "gap between two cells");
}

// -- helpers --------------------------------------------------------------------------------------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A margin-less A4 page (`Page::margin_mm` wins over `@page`) with `css` and `body` inside.
fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(format!("body {{ margin: 0 }} {css}"))
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions {
                page: Page {
                    margin_mm: 0.0,
                    ..Page::A4
                },
                ..Default::default()
            },
        )
        .expect("render")
}

/// The fixture renderer of `fixtures.rs`: bundled Tailwind CSS, Inter fonts, the fixture's logo.
fn fixture_renderer(dir: &Path) -> Renderer {
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
    b.image_file("logo.png", dir.join("logo.png"))
        .expect("logo.png")
        .build()
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

/// Page `n` (from 1) at `dpi` via poppler's `pdftoppm`. Fails loudly when it is not installed: a silent
/// skip would hide wrong colours.
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

/// Bounding box `(xMin, yMin, xMax, yMax)` in pt of the first occurrence of `word` on page 1, from
/// `pdftotext -bbox`.
fn word_bbox(pdf: &[u8], word: &str) -> (f32, f32, f32, f32) {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftotext")
        .args(["-f", "1", "-l", "1", "-bbox"])
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let xml = String::from_utf8_lossy(&out.stdout);
    let line = xml
        .lines()
        .find(|l| l.contains(&format!(">{word}</word>")))
        .unwrap_or_else(|| panic!("`{word}` not found in the PDF text"));
    let attr = |name: &str| -> f32 {
        let key = format!("{name}=\"");
        let start = line.find(&key).unwrap() + key.len();
        let end = start + line[start..].find('"').unwrap();
        line[start..end].parse().unwrap()
    };
    (attr("xMin"), attr("yMin"), attr("xMax"), attr("yMax"))
}

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-rowbg-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
