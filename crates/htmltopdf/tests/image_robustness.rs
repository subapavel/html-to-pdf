//! Oversized and broken images (F32a).
//!
//! 1. An `<img>` wider than its containing block is one unbreakable box that overflows the line box
//!    (CSS 2.1 §9.4.2) and is painted over the edge (CSS 2.1 §11.1.1, `overflow: visible`; css-page-3 §3.2:
//!    content in the page margin is printed, what leaves the page box is lost). Before F32a the box was
//!    placed on a first line and a second, empty line was added after it; the two-line paragraph took
//!    the multi-line fragmentation path, which recorded no geometry for the image, so it was never drawn.
//!    Chrome is not the reference here: its print path shrinks the whole page to fit instead
//!    (`out/scratch/f32a/p1.chrome-1.png`), which CSS does not ask for.
//! 2. A raster whose pixels cannot be decoded (HTML §4.8.4.1: "corrupted in some fatal way") made Krilla fail
//!    the whole document in `Document::finish()`. The image is now skipped; the layout box stays.
//!
//! Asserted on pixels via poppler's `pdftoppm` at 96 dpi (one device pixel is one CSS px) and on image
//! XObjects via `pdfimages -list`. Images are solid colours; a blue 10 px `div` after the image marks
//! where the next content starts.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use htmltopdf::{Page, RenderOptions, Renderer};

const TOL: i64 = 1;

/// Left edge of the content box of A4 with 15 mm margins, CSS px (15 mm = 56.69 px).
const CONTENT_LEFT: i64 = 57;

/// A real JPEG (64 × 64 gradient, baseline) for truncation tests.
const JPEG: &[u8] = include_bytes!("data/gradient64.jpg");

// ---------------------------------------------------------------------------------------------------------
// 1. Wider than the containing block
// ---------------------------------------------------------------------------------------------------------

#[test]
fn inline_img_wider_than_content_box_is_painted() {
    // 700 px image in a 680.3 px content box.
    let page = rasterise(&render(
        &[("wide.png", solid_png(700, 100, [255, 0, 0]))],
        r#"<img src="wide.png"><div class="m"></div>"#,
    ));
    assert_box(
        page.bbox(is_red),
        (CONTENT_LEFT, 57, 700, 100),
        "700x100 inline image in a 680 px content box",
    );
}

#[test]
fn inline_img_wider_than_content_box_takes_one_line() {
    // The image is the only thing on its line: the next block starts right below it (the strut's
    // descender at most), not a whole empty line further down.
    let page = rasterise(&render(
        &[("wide.png", solid_png(700, 100, [255, 0, 0]))],
        r#"<img src="wide.png"><div class="m"></div>"#,
    ));
    let red = page.bbox(is_red).expect("the image is painted");
    let blue = page.bbox(is_blue).expect("the marker is painted");
    let gap = blue.1 - (red.1 + red.3);
    assert!(
        (0..=8).contains(&gap),
        "marker starts {gap} px below the image, expected 0..=8 (one line box)"
    );
    // The same image where it fits sets the reference.
    let fits = rasterise(&render(
        &[("fit.png", solid_png(600, 100, [255, 0, 0]))],
        r#"<img src="fit.png"><div class="m"></div>"#,
    ));
    let (r, b) = (fits.bbox(is_red).unwrap(), fits.bbox(is_blue).unwrap());
    assert_eq!(
        gap,
        b.1 - (r.1 + r.3),
        "an image that overflows takes the line box of one that fits"
    );
}

#[test]
fn img_wider_than_page_is_painted_to_the_page_edge() {
    // 1000 px: past the content box and past the page; the part inside the page box is printed.
    let page = rasterise(&render(
        &[("huge.png", solid_png(1000, 100, [255, 0, 0]))],
        r#"<img src="huge.png"><div class="m"></div>"#,
    ));
    let red = page.bbox(is_red).expect("nothing painted");
    assert!(
        (red.0 - CONTENT_LEFT).abs() <= TOL,
        "left edge at {}, expected {CONTENT_LEFT}",
        red.0
    );
    let right = red.0 + red.2;
    assert!(
        right >= page.width as i64 - 2,
        "painted only up to x = {right}, the page is {} px wide",
        page.width
    );
}

#[test]
fn block_img_wider_than_content_box_is_painted() {
    // Tailwind's preflight `display: block`, without its `max-width: 100%`: a block-level replaced
    // element is as wide as its natural size (CSS 2.1 §10.3.4) and overflows its container.
    let page = rasterise(&render(
        &[("wide.png", solid_png(700, 100, [255, 0, 0]))],
        r#"<img src="wide.png" style="display:block"><div class="m"></div>"#,
    ));
    assert_box(
        page.bbox(is_red),
        (CONTENT_LEFT, 57, 700, 100),
        "700x100 block image",
    );
}

#[test]
fn text_beside_a_wide_img_wraps_at_the_content_box() {
    // The line breaker must not widen the text lines to the image: only the image overflows.
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit ".repeat(4);
    let html = format!(r#"<p>{words}<img src="wide.png"> {words}</p>"#);
    let page = rasterise(&render(
        &[("wide.png", solid_png(700, 100, [255, 0, 0]))],
        &html,
    ));
    let text = page.bbox(is_dark).expect("no text painted");
    let right = text.0 + text.2;
    assert!(
        right <= 738,
        "text reaches x = {right}, the content box ends at 737"
    );
}

#[test]
fn inline_img_on_a_later_line_of_a_paragraph_is_painted() {
    // The cause of the vanishing image was not its width: an image or inline-block in a paragraph of
    // two or more lines got no geometry on the multi-line path. Here the image does not fit beside the
    // text and wraps onto its own line.
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit ".repeat(6);
    let html = format!(r#"<p>{words}<img src="a.png"></p>"#);
    let page = rasterise(&render(
        &[("a.png", solid_png(300, 40, [255, 0, 0]))],
        &html,
    ));
    assert_box(
        page.bbox(is_red),
        (CONTENT_LEFT, 0, 300, 40),
        "wrapped image (y not checked)",
    );
}

// ---------------------------------------------------------------------------------------------------------
// 2. Images that do not decode
// ---------------------------------------------------------------------------------------------------------

#[test]
fn truncated_png_does_not_fail_the_render() {
    for (what, png) in [
        ("half the IDAT data", png_half_idat(120, 60)),
        (
            "file cut in half",
            cut(&solid_png(120, 60, [255, 0, 0]), 0.5),
        ),
    ] {
        let pdf = render(
            &[("t.png", png)],
            r#"<img src="t.png" width="120" height="60"><div class="m"></div>"#,
        );
        assert_eq!(
            xobjects(&pdf),
            0,
            "{what}: an undecodable image is not embedded"
        );
        assert!(
            rasterise(&pdf).bbox(is_blue).is_some(),
            "{what}: the content after it is painted"
        );
    }
}

#[test]
fn truncated_png_keeps_its_layout_box() {
    // The header is intact, so the box has its natural size, as in Chrome.
    let doc = |png: Vec<u8>| {
        let page = rasterise(&render(
            &[("a.png", png)],
            r#"<img src="a.png"><div class="m"></div>"#,
        ));
        page.bbox(is_blue).expect("marker").1
    };
    let intact = doc(solid_png(120, 60, [255, 0, 0]));
    let truncated = doc(png_half_idat(120, 60));
    assert!(
        (intact - truncated).abs() <= TOL,
        "marker at y = {truncated} after a truncated image, {intact} after an intact one"
    );
}

#[test]
fn truncated_jpeg_is_embedded_as_main_and_chrome_do() {
    // Krilla never decodes a JPEG and Chrome draws what is there (40-99 % of a truncated file), so a
    // damaged JPEG that still decodes leniently is embedded, with a warning (`image_warnings.rs`).
    // Only one that cannot be decoded at all is refused.
    let sos = JPEG.windows(2).position(|w| w == [0xff, 0xda]).unwrap();
    for (what, bytes) in [
        (
            "cut in the scan",
            JPEG[..sos + (JPEG.len() - sos) / 2].to_vec(),
        ),
        ("cut near the end", JPEG[..JPEG.len() - 30].to_vec()),
        ("no EOI", JPEG[..JPEG.len() - 2].to_vec()),
        ("intact", JPEG.to_vec()),
    ] {
        let pdf = render(
            &[("t.jpg", bytes)],
            r#"<img src="t.jpg"><div class="m"></div>"#,
        );
        assert_eq!(xobjects(&pdf), 1, "{what}");
    }
}

#[test]
fn jpeg_over_the_pixel_cap_is_embedded_by_header_check_only() {
    // 12000 x 12000 (144 Mpx, over the 128 Mi limit that bounds our own decode) in the SOF header. Krilla
    // passes a JPEG through undecoded, so main embedded it; a PNG of that size is refused.
    let mut jpeg = JPEG.to_vec();
    let sof = jpeg.windows(2).position(|w| w == [0xff, 0xc0]).unwrap();
    jpeg[sof + 5..sof + 9].copy_from_slice(&[0x2e, 0xe0, 0x2e, 0xe0]);
    let pdf = render(
        &[("big.jpg", jpeg)],
        r#"<img src="big.jpg" style="width:100px"><div class="m"></div>"#,
    );
    assert_eq!(xobjects(&pdf), 1);
}

/// `JPEG` with `n` zero bytes between its first (APP0) segment and the next marker: stray bytes between
/// header segments, which libjpeg skips with a warning and every viewer draws.
fn jpeg_with_stray_bytes(n: usize) -> Vec<u8> {
    let app0_end = 2 + 2 + usize::from(u16::from_be_bytes([JPEG[4], JPEG[5]]));
    let mut out = JPEG[..app0_end].to_vec();
    out.extend(std::iter::repeat_n(0u8, n));
    out.extend_from_slice(&JPEG[app0_end..]);
    out
}

#[test]
fn jpeg_with_stray_bytes_between_headers_is_still_embedded() {
    // zune-jpeg's strict mode refuses these; main embedded them and viewers draw them, so they are.
    for n in [3, 6] {
        let pdf = render(
            &[("s.jpg", jpeg_with_stray_bytes(n))],
            r#"<img src="s.jpg"><div class="m"></div>"#,
        );
        assert_eq!(xobjects(&pdf), 1, "{n} stray bytes");
    }
}

#[test]
fn header_only_garbage_and_empty_images_render_nothing() {
    let header_only = solid_png(120, 60, [255, 0, 0])[..40].to_vec();
    for (what, bytes) in [
        ("header only", header_only),
        ("garbage", b"not an image at all".repeat(10)),
        ("empty", Vec::new()),
        ("10-byte JPEG", JPEG[..10].to_vec()),
        ("JPEG with a broken header", {
            let mut j = JPEG.to_vec();
            j[2..30].fill(0x11);
            j
        }),
    ] {
        let name = if what.contains("JPEG") {
            "x.jpg"
        } else {
            "x.png"
        };
        let html = format!(r#"<img src="{name}"><div class="m"></div>"#);
        let pdf = render(&[(name, bytes)], &html);
        assert_eq!(xobjects(&pdf), 0, "{what}");
        let page = rasterise(&pdf);
        assert!(page.bbox(is_red).is_none(), "{what}: something was painted");
        assert!(page.bbox(is_blue).is_some(), "{what}: marker missing");
    }
}

#[test]
fn huge_declared_png_is_refused_without_decoding() {
    // IHDR says 20000 x 20000 (1.2 GB of RGB); the IDAT is a few bytes.
    let huge = png_with_idat(20000, 20000, &zlib_stored(&[0u8; 100]));
    let started = Instant::now();
    let pdf = render(
        &[("huge.png", huge)],
        r#"<img src="huge.png" style="width:100px"><div class="m"></div>"#,
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "took {:?}",
        started.elapsed()
    );
    assert_eq!(xobjects(&pdf), 0);
    assert!(rasterise(&pdf).bbox(is_blue).is_some());
}

#[test]
fn broken_img_with_size_attributes_keeps_its_box() {
    // HTML §15.4.2: a broken image with dimensions from attributes keeps a replaced box of that size.
    let page = rasterise(&render(
        &[],
        r#"<img src="nope.png" width="120" height="36"><div class="m"></div>"#,
    ));
    let blue = page.bbox(is_blue).expect("marker");
    assert!(blue.1 - CONTENT_LEFT >= 36 - 1, "marker at y = {}", blue.1);
}

#[test]
fn render_with_a_broken_image_is_deterministic() {
    let html = r#"<img src="t.png"><img src="g.png"><img src="nope.png"><div class="m"></div>"#;
    let images = [
        ("t.png", png_half_idat(120, 60)),
        ("g.png", b"garbage".to_vec()),
    ];
    assert_eq!(render(&images, html), render(&images, html));
}

// ---------------------------------------------------------------------------------------------------------
// 3. Fill bytes before a marker (F60)
// ---------------------------------------------------------------------------------------------------------
//
// ITU-T T.81 §B.1.1.2: "Any marker may optionally be preceded by any number of fill bytes, which are
// bytes assigned code X'FF'." Krilla's header reader (zune-jpeg) skips them and so do Chrome and poppler;
// fulgur's own scan for the natural size took the first fill byte for the marker and the next two for a
// segment length, so the image had no size and was not painted, without a warning.

/// `JPEG` with `n` fill bytes (`0xFF`) in front of the first occurrence of each marker in `markers`
/// (second byte of the marker, e.g. `0xDB`). Markers are found by walking the segments up to and including
/// SOS, never by searching the bytes, so table contents and entropy-coded data are not mistaken for markers.
fn jpeg_with_fill_bytes(markers: &[u8], n: usize) -> Vec<u8> {
    let mut at = Vec::new();
    let mut seen = Vec::new();
    let mut i = 2; // after SOI
    loop {
        assert_eq!(JPEG[i], 0xFF, "segment walk lost the marker at {i}");
        let m = JPEG[i + 1];
        if markers.contains(&m) && !seen.contains(&m) {
            seen.push(m);
            at.push(i);
        }
        if m == 0xDA {
            break;
        }
        i += 2 + usize::from(u16::from_be_bytes([JPEG[i + 2], JPEG[i + 3]]));
    }
    assert_eq!(
        seen.len(),
        markers.len(),
        "not every marker is in the header"
    );
    let mut out = JPEG.to_vec();
    for &p in at.iter().rev() {
        out.splice(p..p, std::iter::repeat_n(0xFFu8, n));
    }
    out
}

#[test]
fn jpeg_with_fill_bytes_is_painted_at_its_natural_size() {
    let html = r#"<img src="f.jpg"><div class="m"></div>"#;
    let intact = rasterise(&render(&[("f.jpg", JPEG.to_vec())], html));
    let marker_top = intact.bbox(is_blue).expect("marker").1;
    assert_box(
        intact.bbox(is_not_white),
        (CONTENT_LEFT, CONTENT_LEFT, 64, 64),
        "the intact JPEG",
    );
    for (what, markers) in [
        ("DQT", &[0xDB][..]),
        ("SOF0", &[0xC0]),
        ("DHT", &[0xC4]),
        ("SOS", &[0xDA]),
        ("DQT, SOF0, DHT and SOS", &[0xDB, 0xC0, 0xC4, 0xDA]),
    ] {
        let jpeg = jpeg_with_fill_bytes(markers, 7);
        assert!(jpeg.len() > JPEG.len(), "{what}: no fill bytes inserted");
        let pdf = render(&[("f.jpg", jpeg)], html);
        assert_eq!(xobjects(&pdf), 1, "{what}");
        let page = rasterise(&pdf);
        assert_box(
            page.bbox(is_not_white),
            (CONTENT_LEFT, CONTENT_LEFT, 64, 64),
            &format!("fill bytes before {what}"),
        );
        let top = page.bbox(is_blue).expect("marker").1;
        assert!(
            (top - marker_top).abs() <= TOL,
            "{what}: marker at y = {top}, {marker_top} after the intact JPEG"
        );
    }
}

#[test]
fn fill_byte_jpeg_with_css_width_keeps_its_ratio() {
    // `width: 100px` and no height: the height comes from the natural ratio, which needs the size.
    let page = rasterise(&render(
        &[("f.jpg", jpeg_with_fill_bytes(&[0xDB], 3))],
        r#"<img src="f.jpg" style="width:100px"><div class="m"></div>"#,
    ));
    assert_box(
        page.bbox(is_not_white),
        (CONTENT_LEFT, CONTENT_LEFT, 100, 100),
        "fill-byte JPEG at width 100px",
    );
}

#[test]
fn fill_byte_jpeg_as_background_image_is_drawn_at_natural_size() {
    // `no-repeat` at `auto` size paints the natural size (css-images-3 §5.3); a lost size fell back to 1 x 1.
    let page = rasterise(&render(
        &[("f.jpg", jpeg_with_fill_bytes(&[0xDB, 0xC0], 5))],
        r#"<div style="width:64px;height:64px;background:url(f.jpg) no-repeat"></div>"#,
    ));
    assert_box(
        page.bbox(is_not_white),
        (CONTENT_LEFT, CONTENT_LEFT, 64, 64),
        "fill-byte JPEG as background-image",
    );
}

// ---------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A4 with the default 15 mm margins; Inter is bundled. `.m` is the 10 px blue marker.
fn render(images: &[(&str, Vec<u8>)], body: &str) -> Vec<u8> {
    let fonts = root().join("fixtures/fonts");
    let mut b = Renderer::builder().css(
        "body { margin: 0; font: 16px/1.2 Inter } p { margin: 0 } .m { height: 10px; background: #00f }",
    );
    let mut files: Vec<_> = std::fs::read_dir(&fonts)
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fonts — run scripts/fetch-fonts.sh");
    for f in files {
        b = b.font_file(f).unwrap();
    }
    for (name, bytes) in images {
        b = b.image(*name, bytes.clone());
    }
    b.build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions {
                page: Page::A4,
                ..Default::default()
            },
        )
        .expect("render")
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}

/// Anything but white and the blue marker: the gradient JPEG.
fn is_not_white(p: [u8; 3]) -> bool {
    p.iter().any(|&c| c < 250) && !is_blue(p)
}

fn is_dark(p: [u8; 3]) -> bool {
    p.iter().all(|&c| c < 100)
}

/// `(x, y, w, h)` of `got` equals `want` within `TOL`; `want.1 == 0` leaves y unchecked.
fn assert_box(got: Option<(i64, i64, i64, i64)>, want: (i64, i64, i64, i64), what: &str) {
    let got = got.unwrap_or_else(|| panic!("{what}: nothing was painted"));
    let ok = (got.0 - want.0).abs() <= TOL
        && (want.1 == 0 || (got.1 - want.1).abs() <= TOL)
        && (got.2 - want.2).abs() <= TOL
        && (want.3 == 0 || (got.3 - want.3).abs() <= TOL);
    assert!(
        ok,
        "{what}: painted box (x, y, w, h) is {got:?}, expected {want:?} ± {TOL} px"
    );
}

/// Number of image XObjects in the PDF (poppler's `pdfimages -list`).
fn xobjects(pdf: &[u8]) -> usize {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdfimages")
        .arg("-list")
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "pdfimages failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Two header lines, then one line per image.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(2)
        .filter(|l| !l.trim().is_empty())
        .count()
}

// --- PNG construction (no image crate: stored deflate blocks) ------------------------------------------

fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    z
}

/// A PNG with the given IHDR size (8-bit RGB) and IDAT payload.
fn png_with_idat(w: u32, h: u32, idat: &[u8]) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    for (kind, data) in [
        (b"IHDR", ihdr),
        (b"IDAT", idat.to_vec()),
        (b"IEND", Vec::new()),
    ] {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(&data);
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    png
}

fn solid_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let row: Vec<u8> = std::iter::once(0)
        .chain(rgb.iter().copied().cycle().take(3 * w as usize))
        .collect();
    png_with_idat(w, h, &zlib_stored(&row.repeat(h as usize)))
}

/// A valid PNG container whose zlib stream ends halfway: well-formed chunks, too few pixels.
fn png_half_idat(w: u32, h: u32) -> Vec<u8> {
    let row: Vec<u8> = std::iter::once(0)
        .chain([255u8, 0, 0].iter().copied().cycle().take(3 * w as usize))
        .collect();
    let z = zlib_stored(&row.repeat(h as usize));
    png_with_idat(w, h, &z[..z.len() / 2])
}

fn cut(bytes: &[u8], fraction: f32) -> Vec<u8> {
    bytes[..(bytes.len() as f32 * fraction) as usize].to_vec()
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

// --- Raster ----------------------------------------------------------------------------------------------

struct Raster {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Raster {
    /// `(x, y, w, h)` of the pixels satisfying `pred`.
    fn bbox(&self, pred: impl Fn([u8; 3]) -> bool) -> Option<(i64, i64, i64, i64)> {
        let mut bb: Option<(i64, i64, i64, i64)> = None;
        for y in 0..self.height {
            for x in 0..self.width {
                let i = (y * self.width + x) * 3;
                if pred([self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]) {
                    let (x, y) = (x as i64, y as i64);
                    bb = Some(match bb {
                        None => (x, y, x, y),
                        Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                    });
                }
            }
        }
        bb.map(|(x0, y0, x1, y1)| (x0, y0, x1 + 1 - x0, y1 + 1 - y0))
    }
}

/// Page 1 at 96 dpi via `pdftoppm`.
fn rasterise(pdf: &[u8]) -> Raster {
    let pdf_path = temp(pdf, "pdf");
    let base = pdf_path.with_extension("");
    let out = Command::new("pdftoppm")
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

fn temp(bytes: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-imgrobust-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}
