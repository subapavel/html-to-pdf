//! `<img>` without `width`/`height` attributes takes its natural size (F9). CSS 2.1 §10.3.2 / §10.6.2: an
//! `auto` size of a replaced element is its intrinsic size, and with one dimension given the other follows the
//! intrinsic ratio; css-images-3 §5: one image pixel is one CSS px. Before F9 the image bytes were visible
//! only to fulgur's asset bundle, never to layout, so every `<img>` that was not sized on both axes by
//! attributes was laid out 0 wide and not painted at all (Tailwind's `h-12 w-auto` logo of fixture 03).
//!
//! Asserted on pixels via poppler's `pdftoppm`: at 96 dpi one device pixel is one CSS px, the page is
//! margin-less, and the checked box is the bounding box of the non-white pixels of a solid red image, so it
//! is the image's border box to within anti-aliasing (`TOL` px). The images are inline (the default), like
//! the ones in an unstyled document; the `block_level_*` tests use Tailwind's `display: block` preflight.

use std::path::{Path, PathBuf};

use htmltopdf::{Margins, Page, RenderOptions, Renderer};

/// Pixel tolerance of a bounding-box edge (anti-aliasing at the image border).
const TOL: i64 = 1;

/// Natural size of the generated test image, CSS px.
const W: u32 = 80;
const H: u32 = 40;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><rect width="100" height="50" fill="#f00"/></svg>"##;

#[test]
fn bare_img() {
    // No attributes, no CSS: the natural size.
    let page = rasterise(&render("", r#"<img src="red.png">"#));
    assert_bbox(&page, (0, 0, W as i64, H as i64), "bare <img>");
}

#[test]
fn height_only_keeps_ratio() {
    // `h-12 w-auto` of fixture 03: height given, width follows the 2:1 ratio.
    let page = rasterise(&render(
        "",
        r#"<img src="red.png" style="height: 60px; width: auto">"#,
    ));
    assert_bbox(&page, (0, 0, 120, 60), "height: 60px; width: auto");
    let page = rasterise(&render("", r#"<img src="red.png" style="height: 60px">"#));
    assert_bbox(&page, (0, 0, 120, 60), "height: 60px alone");
}

#[test]
fn percent_width() {
    // 50 % of a 400 px container is 200 px; the height follows the ratio.
    let page = rasterise(&render(
        "",
        r#"<div style="width: 400px"><img src="red.png" style="width: 50%"></div>"#,
    ));
    assert_bbox(&page, (0, 0, 200, 100), "width: 50% of 400px");
}

#[test]
fn width_attr_only() {
    // The `width` attribute alone is a presentational hint (HTML §15.4.3); the height follows the ratio.
    let page = rasterise(&render("", r#"<img src="red.png" width="100">"#));
    assert_bbox(&page, (0, 0, 100, 50), "width=100 only");
}

#[test]
fn table_cell() {
    // An unsized image in a table cell sizes the cell: the cell has no padding, the table no border-spacing.
    let page = rasterise(&render(
        "table { border-collapse: separate; border-spacing: 0 } td { padding: 0 }",
        r#"<table><tr><td><img src="red.png"></td></tr></table>"#,
    ));
    assert_bbox(&page, (0, 0, W as i64, H as i64), "<img> in a <td>");
}

#[test]
fn svg_img() {
    // An SVG image's natural size is the root `width`/`height`.
    let page = rasterise(&render("", r#"<img src="red.svg">"#));
    assert_bbox(&page, (0, 0, 100, 50), "bare <img> of an SVG");
    let page = rasterise(&render("", r#"<img src="red.svg" style="height: 100px">"#));
    assert_bbox(&page, (0, 0, 200, 100), "SVG <img> with height only");
}

/// Tailwind's preflight rules for images (`img { display: block; max-width: 100%; height: auto }`).
const PREFLIGHT: &str =
    "img { display: block; vertical-align: middle; max-width: 100%; height: auto }";

#[test]
fn block_level_img_is_not_stretched() {
    // Taffy's block layout hands every block child the container width; a replaced element with
    // `width: auto` is sized by the inline-replaced rules instead (CSS 2.1 §10.3.4). Chrome 141 gives the
    // same boxes.
    let page = rasterise(&render(PREFLIGHT, r#"<img src="red.png">"#));
    assert_bbox(&page, (0, 0, W as i64, H as i64), "preflight, bare <img>");
    // Tailwind `h-12 w-auto`.
    let page = rasterise(&render(
        PREFLIGHT,
        r#"<img src="red.png" style="height: 48px; width: auto">"#,
    ));
    assert_bbox(&page, (0, 0, 96, 48), "preflight, h-12 w-auto");
    let page = rasterise(&render(PREFLIGHT, r#"<img src="red.png" width="100">"#));
    assert_bbox(&page, (0, 0, 100, 50), "preflight, width=100 only");
    let page = rasterise(&render(
        "img { display: block }",
        r#"<img src="red.png" width="120" height="36">"#,
    ));
    assert_bbox(&page, (0, 0, 120, 36), "display: block, both attributes");
    let page = rasterise(&render(PREFLIGHT, r#"<img src="red.svg">"#));
    assert_bbox(&page, (0, 0, 100, 50), "preflight, bare SVG <img>");
}

#[test]
fn block_level_img_wider_than_its_container_is_clamped() {
    // 600 x 300 natural size in a 400 px box: `max-width: 100%` clamps it and the ratio is kept.
    let page = rasterise(&render(
        PREFLIGHT,
        r#"<div style="width: 400px"><img src="wide.png"></div>"#,
    ));
    assert_bbox(&page, (0, 0, 400, 200), "600x300 image in a 400 px box");
}

#[test]
fn block_level_img_with_width_100_percent_fills_its_container() {
    // The intended stretch: `w-full`.
    let page = rasterise(&render(
        PREFLIGHT,
        r#"<div style="width: 400px"><img src="red.png" style="width: 100%"></div>"#,
    ));
    assert_bbox(&page, (0, 0, 400, 200), "width: 100% of 400 px");
}

#[test]
fn block_level_img_keeps_auto_margins() {
    // `margin: 0 auto` centres the image at its natural width: (794 - 80) / 2 = 357 px from the left
    // (Chrome 141: 356, the page is 793.7 px wide).
    let page = rasterise(&render(
        PREFLIGHT,
        r#"<img src="red.png" style="margin: 0 auto">"#,
    ));
    let (x0, _, x1, _) = page.bbox().expect("nothing was painted");
    assert!(
        (x0 - 356).abs() <= 2 && (x1 + 1 - x0 - W as i64).abs() <= TOL,
        "centred image spans {x0}..{x1}, expected 356..436"
    );
}

#[test]
fn flex_item_img_keeps_flex_sizing() {
    // Flex items follow flexbox, not the block rule: a row item keeps its natural size, a column item
    // stretches across the cross axis and the ratio fills the height (Chrome 141: 300 x 150).
    let css = "img { display: block }";
    let page = rasterise(&render(
        css,
        r#"<div style="display: flex"><img src="red.png"></div>"#,
    ));
    assert_bbox(&page, (0, 0, W as i64, H as i64), "image in a flex row");
    let page = rasterise(&render(
        css,
        r#"<div style="display: flex; flex-direction: column; width: 300px"><img src="red.png"></div>"#,
    ));
    assert_bbox(&page, (0, 0, 300, 150), "image in a flex column");
}

#[test]
fn flex_row_of_fixed_height_stretches_only_what_has_an_auto_size() {
    // css-flexbox §9.4: `align-self: stretch` applies to an item whose cross size is `auto`. The
    // `width`/`height` attributes are presentational hints (HTML §15.4.3), so they make a size definite.
    // Chrome 141, same markup: 400 x 200, 120 x 60, 100 x 200, 80 x 40.
    let row = |img: &str| {
        rasterise(&render(
            "img { display: block }",
            &format!(r#"<div style="display: flex; height: 200px">{img}</div>"#),
        ))
    };
    assert_bbox(
        &row(r#"<img src="red.png">"#),
        (0, 0, 400, 200),
        "bare: stretched, the ratio gives the width",
    );
    assert_bbox(
        &row(r#"<img src="red.png" width="120" height="60">"#),
        (0, 0, 120, 60),
        "both attributes: definite, not stretched",
    );
    assert_bbox(
        &row(r#"<img src="red.png" width="100">"#),
        (0, 0, 100, 200),
        "width attribute: the height still stretches",
    );
    assert_bbox(
        &row(r#"<img src="red.png" height="40">"#),
        (0, 0, 80, 40),
        "height attribute: definite, the ratio gives the width",
    );
}

#[test]
fn grid_item_img_is_not_stretched() {
    // css-grid-1 §6.2 / css-align-3: `normal` self-alignment of an item with a preferred aspect ratio
    // (every replaced element) behaves as `start`, so a grid item takes its natural size in both axes.
    // Chrome 141, same markup: 80 x 40, 96 x 48, 120 x 36, 80 x 40.
    let grid = |css: &str, img: &str| {
        rasterise(&render(
            &format!("img {{ display: block }} {css}"),
            &format!(
                r#"<div style="display: grid; grid-template-columns: 1fr 1fr; {css}">{img}<div></div></div>"#
            ),
        ))
    };
    assert_bbox(
        &grid("", r#"<img src="red.png">"#),
        (0, 0, W as i64, H as i64),
        "bare image in a grid",
    );
    assert_bbox(
        &grid(
            "",
            r#"<img src="red.png" style="height: 48px; width: auto">"#,
        ),
        (0, 0, 96, 48),
        "h-12 w-auto in a grid",
    );
    assert_bbox(
        &grid("", r#"<img src="red.png" width="120" height="36">"#),
        (0, 0, 120, 36),
        "both attributes in a grid",
    );
    assert_bbox(
        &grid("height: 300px", r#"<img src="red.png">"#),
        (0, 0, W as i64, H as i64),
        "bare image in a grid row taller than the image",
    );
}

#[test]
fn grid_item_img_stretches_when_asked_to() {
    // An explicit `stretch` is honoured. Chrome 141: `justify-self: stretch` alone gives
    // the 397 px column and the ratio height (397 x 199); `justify-items`/`align-items: stretch` on a
    // 300 px tall grid fill the 600 x 300 cell.
    let page = rasterise(&render(
        "img { display: block }",
        r#"<div style="display: grid; grid-template-columns: 1fr 1fr"><img src="red.png" style="justify-self: stretch"><div></div></div>"#,
    ));
    assert_bbox(&page, (0, 0, 397, 199), "justify-self: stretch");
    let page = rasterise(&render(
        "img { display: block }",
        r#"<div style="display: grid; grid-template-columns: 1fr 1fr; height: 300px; justify-items: stretch; align-items: stretch"><img src="red.png"><div></div></div>"#,
    ));
    assert_bbox(
        &page,
        (0, 0, 600, 300),
        "justify-items/align-items: stretch",
    );
}

#[test]
fn block_svg_with_only_a_view_box_fills_its_container() {
    // CSS 2.1 §10.3.4 / §10.3.2: a replaced element with a ratio but no natural width is as wide as its
    // containing block (Chrome 141: 794 x 397 at 96 dpi for a 20 x 10 viewBox). Tailwind: `svg { display: block }`.
    let css = "svg { display: block }";
    let fill = |svg: &str| rasterise(&render(css, svg));
    let rect = r#"<rect width="20" height="10" fill="red"/>"#;
    let page = fill(&format!(r#"<svg viewBox="0 0 20 10">{rect}</svg>"#));
    assert_bbox(&page, (0, 0, 794, 397), "viewBox only");
    // With a natural width or a definite height the viewBox only gives the ratio (Chrome: 40 x 20, 60 x 30).
    let page = fill(&format!(
        r#"<svg width="40" height="20" viewBox="0 0 20 10">{rect}</svg>"#
    ));
    assert_bbox(&page, (0, 0, 40, 20), "width and height");
    let page = fill(&format!(
        r#"<svg viewBox="0 0 20 10" style="height: 30px">{rect}</svg>"#
    ));
    assert_bbox(&page, (0, 0, 60, 30), "viewBox and a height");
}

#[test]
fn both_attrs_unchanged() {
    // Both attributes: they win over the natural size, as before F9.
    let page = rasterise(&render(
        "",
        r#"<img src="red.png" width="120" height="36">"#,
    ));
    assert_bbox(&page, (0, 0, 120, 36), "width=120 height=36");
}

#[test]
fn missing_src_renders_nothing() {
    // `src` names no registered image: nothing is painted, and the render does not fail.
    let page = rasterise(&render(
        "",
        r#"<img src="nope.png"><img src="nope.png" height="30"><img>"#,
    ));
    assert!(
        page.bbox().is_none(),
        "an <img> with an unregistered src painted {:?}",
        page.bbox()
    );
}

#[test]
fn fixture_03_logo_matches_chrome() {
    // `logo.png` is 240 × 72 px (IHDR, no pHYs). The element is `h-12 w-auto`: 48 px tall, so 160 px wide
    // (Chrome embeds it at 144 ppi = 240 px over 160 CSS px). The 150 × 37 px bounding box seen in the
    // reference is not the element but the opaque part of the logo: the PNG has an 8 px transparent frame
    // (alpha bounding box (8, 8)-(232, 64)), which scales by 2/3 to (5.3, 5.3)-(154.7, 42.7) in the box.
    // Chrome 141's non-white pixels in the header (96 dpi): x 62..212, y 67..104.
    let root = root();
    let dir = root.join("fixtures/invoices/03-logo-qr");
    let (iw, ih) = png_size(&std::fs::read(dir.join("logo.png")).unwrap());
    assert_eq!((iw, ih), (240, 72), "fixtures/invoices/03-logo-qr/logo.png");

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
    let page = rasterise(&pdf);
    // The logo is the only thing left of x = 300 above y = 115 (the title block is right-aligned).
    let (x0, y0, x1, y1) = page
        .bbox_within(300, 115)
        .expect("no logo painted in the top-left corner of fixture 03");
    // Chrome: 62, 67, 212, 104 (right and bottom edges exclusive here, hence +1).
    for (got, want, what) in [
        (x0, 62, "left"),
        (y0, 67, "top"),
        (x1 + 1, 212, "right"),
        (y1 + 1, 104, "bottom"),
    ] {
        assert!(
            (got - want).abs() <= 2,
            "logo {what} edge is at {got} px, Chrome has {want} px"
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

/// A margin-less A4 page with `css` and `body` inside; `red.png` (80 × 40), `wide.png` (600 × 300) and
/// `red.svg` (100 × 50) are registered.
fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .css(format!("body {{ margin: 0 }} {css}"))
        .image("red.png", solid_png(W, H, [255, 0, 0]))
        .image("red.svg", SVG.as_bytes().to_vec())
        .image("wide.png", solid_png(600, 300, [255, 0, 0]))
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions {
                page: Page {
                    margin: Some(Margins::uniform_mm(0.0)),
                    ..Page::default()
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

/// Width and height from a PNG's IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n") && &png[12..16] == b"IHDR");
    let be = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    (be(16), be(20))
}

/// An opaque `w` × `h` RGB PNG of one colour (stored deflate blocks, no compression: the test needs no
/// image crate).
fn solid_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    // Each scanline: filter byte 0 (none), then `w` pixels.
    let row: Vec<u8> = std::iter::once(0)
        .chain(rgb.iter().copied().cycle().take(3 * w as usize))
        .collect();
    let raw = row.repeat(h as usize);
    // zlib stream: header, stored blocks of at most 65535 bytes, Adler-32.
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit, RGB, deflate, no filter, no interlace
    for (kind, data) in [(b"IHDR", ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(&data);
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    png
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

struct Raster {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Raster {
    fn is_white(&self, x: usize, y: usize) -> bool {
        let i = (y * self.width + x) * 3;
        self.rgb[i..i + 3].iter().all(|&c| c >= 250)
    }

    /// Bounding box `(x0, y0, x1, y1)` of the non-white pixels inside `[0, w) × [0, h)`, edges inclusive.
    fn bbox_within(&self, w: usize, h: usize) -> Option<(i64, i64, i64, i64)> {
        let mut bb: Option<(i64, i64, i64, i64)> = None;
        for y in 0..h.min(self.height) {
            for x in 0..w.min(self.width) {
                if !self.is_white(x, y) {
                    let (x, y) = (x as i64, y as i64);
                    bb = Some(match bb {
                        None => (x, y, x, y),
                        Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                    });
                }
            }
        }
        bb
    }

    fn bbox(&self) -> Option<(i64, i64, i64, i64)> {
        self.bbox_within(self.width, self.height)
    }
}

/// The non-white bounding box is `want = (x, y, width, height)` to within `TOL` px.
fn assert_bbox(page: &Raster, want: (i64, i64, i64, i64), what: &str) {
    let (x0, y0, x1, y1) = page
        .bbox()
        .unwrap_or_else(|| panic!("{what}: nothing was painted"));
    let got = (x0, y0, x1 + 1 - x0, y1 + 1 - y0);
    let ok = [
        (got.0, want.0),
        (got.1, want.1),
        (got.2, want.2),
        (got.3, want.3),
    ]
    .iter()
    .all(|(g, w)| (g - w).abs() <= TOL);
    assert!(
        ok,
        "{what}: painted box (x, y, w, h) is {got:?}, expected {want:?} ± {TOL} px"
    );
}

/// Page 1 at 96 dpi via poppler's `pdftoppm`. Fails loudly when it is not installed: a silent skip would hide
/// a missing image.
fn rasterise(pdf: &[u8]) -> Raster {
    let pdf_path = temp(pdf, "pdf");
    let base = pdf_path.with_extension("");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "96", "-f", "1", "-l", "1", "-singlefile"])
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
        height: fields[1],
        rgb: ppm[i + 1..].to_vec(),
    }
}

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-imgsize-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}
