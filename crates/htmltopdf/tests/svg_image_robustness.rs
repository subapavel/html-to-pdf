//! Rasters inside an SVG are checked like a top-level `<img>` (F177).
//!
//! usvg resolves an SVG `<image href="data:…">` by its MIME type only and krilla-svg hands it to krilla, which decodes
//! the pixels in `Document::finish()` (on a rayon worker since F99e). F32a's checks were never consulted: a PNG whose
//! header claims 100000 x 100000 aborted the process (a 40 GB allocation, which `catch_unwind` cannot catch) and a
//! truncated one failed the whole document. Now a raster that does not decode is left out of its SVG with a warning
//! naming the SVG (SVG 2 §5.7: an image that cannot be loaded is not rendered, the rest is), `strict_images(true)`
//! refuses the document, and a damaged JPEG is embedded with a warning (F32a M1). Plan:
//! docs/plans/2026-10-26-f177-svg-image-checks.md.
//!
//! The SVG is 100 x 50 with a blue rect and the `<image>`; a 10 px green `div` after it marks where the next content
//! starts. Asserted on image XObjects (`pdfimages -list`) and pixels (`pdftoppm`, 96 dpi).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use htmltopdf::{Error, ImageProblemKind, RenderOptions, Renderer};

const RED_PNG: &[u8] = include_bytes!("data/red16.png");
const RED_GIF: &[u8] = include_bytes!("data/red16.gif");
const RED_WEBP: &[u8] = include_bytes!("data/red16.webp");
const RED_JPG: &[u8] = include_bytes!("data/red16.jpg");

// --- log capture (global to the process: every warning assertion is in one test) -----------------------

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.level() <= log::Level::Warn
            && (r.target().starts_with("fulgur") || r.target().starts_with("htmltopdf"))
        {
            LOG.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

// --- documents ------------------------------------------------------------------------------------------

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The SVG of every test: a blue rect and the raster `bytes` (`mime`) as an `<image>` over its left half.
fn svg(mime: &str, bytes: &[u8]) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\" viewBox=\"0 0 100 50\">\
         <rect x=\"50\" y=\"0\" width=\"50\" height=\"50\" fill=\"#0000ff\"/>\
         <image x=\"0\" y=\"0\" width=\"50\" height=\"50\" href=\"data:{mime};base64,{}\"/></svg>",
        b64(bytes)
    )
}

/// The three places an SVG can come from: inline, `<img src>`, `background-image`.
#[derive(Clone, Copy, Debug)]
enum Site {
    Inline,
    Img,
    Background,
}

const SITES: [Site; 3] = [Site::Inline, Site::Img, Site::Background];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn try_render(site: Site, svg: &str, strict: bool) -> htmltopdf::Result<Vec<u8>> {
    let mut b = Renderer::builder()
        .css("body { margin: 0; font: 16px/1.2 Inter } .m { height: 10px; background: #00ff00 } .bg { width: 100px; height: 50px; background: url(logo.svg) no-repeat }")
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .strict_images(strict);
    let body = match site {
        Site::Inline => format!("<div>{svg}</div><div class=\"m\"></div>"),
        Site::Img => {
            b = b.image("logo.svg", svg.as_bytes().to_vec());
            "<img src=\"logo.svg\"><div class=\"m\"></div>".to_string()
        }
        Site::Background => {
            b = b.image("logo.svg", svg.as_bytes().to_vec());
            "<div class=\"bg\"></div><div class=\"m\"></div>".to_string()
        }
    };
    b.build().render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

fn temp(bytes: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-svgimg-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Number of image XObjects (`pdfimages -list`).
fn xobjects(pdf: &[u8]) -> usize {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdfimages")
        .arg("-list")
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(2)
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Page 1 at 96 dpi: (width, height, RGB).
fn raster(pdf: &[u8]) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", "1", "-l", "1"])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
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

fn count(img: &(usize, usize, Vec<u8>), f: impl Fn([u8; 3]) -> bool) -> usize {
    img.2.chunks(3).filter(|p| f([p[0], p[1], p[2]])).count()
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}
fn is_green(p: [u8; 3]) -> bool {
    p[1] > 200 && p[0] < 60 && p[2] < 60
}

/// The first `fraction` of `bytes`.
fn cut(bytes: &[u8], fraction: f32) -> Vec<u8> {
    bytes[..((bytes.len() as f32 * fraction) as usize).max(1)].to_vec()
}

/// A PNG whose IHDR claims `w` x `h` RGBA and whose IDAT is a few bytes.
fn huge_png(w: u32, h: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    for (kind, data) in [
        (b"IHDR", ihdr),
        (b"IDAT", vec![0x78, 0x01, 0x01, 0x00, 0x00]),
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

// --- tests ----------------------------------------------------------------------------------------------

const CHILD: &str = "HTMLTOPDF_F177_CHILD";

/// Runs `test` of this binary in a child process with `CHILD` set: an abort kills the child, not the test run.
fn in_child(test: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(CHILD, "1")
        .output()
        .expect("re-run the test binary")
}

#[test]
fn huge_png_header_in_inline_svg_does_not_abort() {
    if std::env::var_os(CHILD).is_some() {
        let pdf = try_render(
            Site::Inline,
            &svg("image/png", &huge_png(100_000, 100_000)),
            false,
        )
        .expect("a raster over the pixel limit is left out, the document renders");
        assert_eq!(xobjects(&pdf), 0, "the huge raster is not embedded");
        let img = raster(&pdf);
        assert!(count(&img, is_blue) > 100, "the rest of the SVG is painted");
        assert!(
            count(&img, is_green) > 100,
            "the marker after the SVG is painted"
        );
        return;
    }
    let out = in_child("huge_png_header_in_inline_svg_does_not_abort");
    assert!(
        out.status.success(),
        "the child render failed or aborted ({:?}): {}{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn broken_rasters_in_svg_are_left_out_valid_ones_drawn_and_strict_mode_refuses() {
    let _ = log::set_boxed_logger(Box::new(Capture));
    log::set_max_level(log::LevelFilter::Warn);
    let broken: [(&str, Vec<u8>); 3] = [
        ("image/png", cut(RED_PNG, 0.6)),
        ("image/gif", cut(RED_GIF, 0.6)),
        ("image/webp", cut(RED_WEBP, 0.6)),
    ];
    for site in SITES {
        for (mime, bytes) in &broken {
            let what = format!("truncated {mime} in an SVG ({site:?})");
            take();
            let pdf = try_render(site, &svg(mime, bytes), false)
                .unwrap_or_else(|e| panic!("{what}: the document fails: {e}"));
            assert_eq!(xobjects(&pdf), 0, "{what}: embedded");
            let img = raster(&pdf);
            assert!(
                count(&img, is_blue) > 100,
                "{what}: the rest of the SVG is not painted"
            );
            assert!(
                count(&img, is_green) > 100,
                "{what}: the marker is not painted"
            );
            let warnings = take();
            assert_eq!(
                warnings
                    .iter()
                    .filter(|w| w.contains(&format!("data:{mime}")))
                    .count(),
                1,
                "{what}: one warning naming the image: {warnings:?}"
            );
            // Strict: an image problem in the SVG.
            match try_render(site, &svg(mime, bytes), true) {
                Err(Error::Images(list)) => assert!(
                    list.iter()
                        .any(|p| matches!(&p.kind, ImageProblemKind::InSvg { image, .. } if image.contains(mime))),
                    "{what}: strict error does not name the image: {list:?}"
                ),
                other => panic!("{what}: strict mode must refuse, got {:?}", other.map(|p| p.len())),
            }
        }
        // A damaged JPEG is embedded with a warning, in strict mode too (F32a M1).
        let what = format!("truncated JPEG in an SVG ({site:?})");
        take();
        let pdf = try_render(site, &svg("image/jpeg", &cut(RED_JPG, 0.7)), true)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(xobjects(&pdf), 1, "{what}: embedded as main and Chrome do");
        let warnings = take();
        assert!(
            warnings.iter().any(|w| w.contains("data:image/jpeg")),
            "{what}: a warning names the damaged JPEG: {warnings:?}"
        );
        // Valid rasters: drawn, no warning.
        for (mime, bytes) in [
            ("image/png", RED_PNG),
            ("image/gif", RED_GIF),
            ("image/webp", RED_WEBP),
            ("image/jpeg", RED_JPG),
        ] {
            let what = format!("valid {mime} in an SVG ({site:?})");
            take();
            let pdf =
                try_render(site, &svg(mime, bytes), true).unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_eq!(xobjects(&pdf), 1, "{what}");
            if mime != "image/jpeg" {
                assert!(count(&raster(&pdf), is_red) > 100, "{what}: not painted");
            }
            let warnings = take();
            assert!(warnings.is_empty(), "{what}: {warnings:?}");
        }
    }
}

#[test]
fn broken_rasters_in_a_pattern_or_mask_are_left_out() {
    let bad = format!("data:image/png;base64,{}", b64(&cut(RED_PNG, 0.6)));
    for (what, svg) in [
        (
            "pattern",
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\"><defs><pattern id=\"p\" \
                 width=\"16\" height=\"16\" patternUnits=\"userSpaceOnUse\"><image width=\"16\" height=\"16\" \
                 href=\"{bad}\"/></pattern></defs><rect width=\"50\" height=\"50\" fill=\"url(#p)\"/>\
                 <rect x=\"50\" width=\"50\" height=\"50\" fill=\"#0000ff\"/></svg>"
            ),
        ),
        (
            "mask",
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\"><defs><mask id=\"k\">\
                 <image width=\"100\" height=\"50\" href=\"{bad}\"/></mask></defs>\
                 <rect width=\"50\" height=\"50\" fill=\"#ff0000\" mask=\"url(#k)\"/>\
                 <rect x=\"50\" width=\"50\" height=\"50\" fill=\"#0000ff\"/></svg>"
            ),
        ),
    ] {
        let pdf = try_render(Site::Inline, &svg, false).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(
            count(&raster(&pdf), is_blue) > 100,
            "{what}: the rest of the SVG is not painted"
        );
    }
}

#[test]
fn a_nested_svgs_image_is_never_loaded() {
    // An SVG inside an SVG's `<image>` is processed without external resources (SVG Integration §3): its huge raster
    // is never decoded.
    let inner = svg("image/png", &huge_png(100_000, 100_000));
    let outer = svg("image/svg+xml", inner.as_bytes());
    let pdf = try_render(Site::Inline, &outer, true).expect("nested SVG");
    assert!(count(&raster(&pdf), is_blue) > 100);
}

#[test]
fn a_broken_raster_in_svg_renders_deterministically() {
    let s = svg("image/png", &cut(RED_PNG, 0.6));
    assert_eq!(
        try_render(Site::Inline, &s, false).unwrap(),
        try_render(Site::Inline, &s, false).unwrap()
    );
}
