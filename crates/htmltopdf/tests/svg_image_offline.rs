//! An SVG's `<image href>` never reads the host's filesystem (F177b).
//!
//! usvg's default resolver read every non-`data:` href from the host disk, relative to the process's working directory
//! or absolute: a document could embed any image file the process can read, and `href="/dev/zero"` was an unbounded
//! read. Now an inline `<svg>` looks the href up by name among the bundled images (the `<img src>` rule), an SVG used
//! as an image (`<img>`, `background-image`, `list-style-image`) loads no external file at all (SVG Integration §3,
//! secure mode), and anything else is left out with a warning (`strict_images(true)` refuses the document,
//! `ImageProblemKind::UnregisteredInSvg`). Plan: docs/plans/2026-10-26-f177b-svg-href-offline.md.
//!
//! Every case that reads the working directory or could hang runs in a child process whose working directory holds a
//! red `logo.png`, so the old resolver really finds it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use htmltopdf::{Error, ImageProblemKind, RenderOptions, Renderer};

const RED_PNG: &[u8] = include_bytes!("data/red16.png");

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

/// A 100 x 50 SVG: a blue rect on the right half and `<image href="{href}">` on the left half.
fn svg(href: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"s\" width=\"100\" height=\"50\" viewBox=\"0 0 100 50\">\
         <rect x=\"50\" y=\"0\" width=\"50\" height=\"50\" fill=\"#0000ff\"/>\
         <image x=\"0\" y=\"0\" width=\"50\" height=\"50\" href=\"{href}\"/></svg>"
    )
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Renders `body` with the bundled `images` and Inter.
fn try_render(images: &[(&str, Vec<u8>)], body: &str, strict: bool) -> htmltopdf::Result<Vec<u8>> {
    let mut b = Renderer::builder()
        .css("body { margin: 0; font: 16px/1.2 Inter } .bg { width: 100px; height: 50px; background: url(card.svg) no-repeat } .li { list-style-image: url(card.svg) }")
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .strict_images(strict);
    for (name, bytes) in images {
        b = b.image(*name, bytes.clone());
    }
    b.build().render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

fn temp(bytes: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-svgoff-{}-{n}.{ext}", std::process::id()));
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

/// Pixels of page 1 at 96 dpi for which `f` holds.
fn count(pdf: &[u8], f: impl Fn([u8; 3]) -> bool) -> usize {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", "1", "-l", "1"])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
    let mut fields = 0;
    let mut at = 0;
    while fields < 4 {
        while data[at].is_ascii_whitespace() {
            at += 1;
        }
        while !data[at].is_ascii_whitespace() {
            at += 1;
        }
        fields += 1;
    }
    data[at + 1..]
        .chunks(3)
        .filter(|p| p.len() == 3 && f([p[0], p[1], p[2]]))
        .count()
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60
}

/// The strict error lists an `UnregisteredInSvg` naming `href`.
fn assert_refused(r: htmltopdf::Result<Vec<u8>>, href: &str, what: &str) {
    match r {
        Err(Error::Images(list)) => assert!(
            list.iter().any(
                |p| matches!(&p.kind, ImageProblemKind::UnregisteredInSvg { image, .. } if image == href)
            ),
            "{what}: strict error does not name {href}: {list:?}"
        ),
        other => panic!("{what}: strict mode must refuse, got {:?}", other.map(|p| p.len())),
    }
}

const CHILD: &str = "HTMLTOPDF_F177B_CHILD";

/// Runs `test` of this binary in a child process with `CHILD` set, its working directory a temp dir that holds a red
/// `logo.png`; fails if the child does not finish within `limit`.
fn in_child(test: &str, limit: Duration) {
    let dir = std::env::temp_dir().join(format!(
        "htmltopdf-svgoff-cwd-{}-{test}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("logo.png"), RED_PNG).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(CHILD, dir.as_os_str())
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("re-run the test binary");
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let out = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{test}: the child failed ({status:?}): {}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            return;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            panic!("{test}: the child did not finish within {limit:?} (an unbounded read?)");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn relative_absolute_file_and_http_hrefs_are_never_read() {
    let Some(dir) = std::env::var_os(CHILD) else {
        in_child(
            "relative_absolute_file_and_http_hrefs_are_never_read",
            Duration::from_secs(60),
        );
        return;
    };
    let abs = Path::new(&dir).join("logo.png").display().to_string();
    for href in [
        "logo.png".to_string(),
        abs.clone(),
        format!("file://{abs}"),
        "http://example.com/logo.png".to_string(),
    ] {
        let what = format!("inline <svg> with href {href}");
        let pdf = try_render(&[], &svg(&href), false).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(xobjects(&pdf), 0, "{what}: the host file was read");
        assert!(
            count(&pdf, is_blue) > 100,
            "{what}: the rest of the SVG is not painted"
        );
        assert_refused(try_render(&[], &svg(&href), true), &href, &what);
    }
}

#[test]
fn a_device_href_does_not_hang() {
    if std::env::var_os(CHILD).is_none() {
        in_child("a_device_href_does_not_hang", Duration::from_secs(60));
        return;
    }
    let card = svg("/dev/zero").into_bytes();
    for (what, images, body) in [
        ("inline <svg>", vec![], svg("/dev/zero")),
        (
            "<img src=card.svg>",
            vec![("card.svg", card.clone())],
            "<img src=\"card.svg\">".to_string(),
        ),
        (
            "background-image",
            vec![("card.svg", card.clone())],
            "<div class=\"bg\"></div>".to_string(),
        ),
    ] {
        let started = Instant::now();
        let pdf = try_render(&images, &body, false).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{what}: {:?}",
            started.elapsed()
        );
        assert_eq!(xobjects(&pdf), 0, "{what}");
    }
}

#[test]
fn a_bundled_href_in_an_inline_svg_is_drawn() {
    for href in ["logo.png", "./logo.png"] {
        let what = format!("inline <svg> with bundled href {href}");
        let pdf = try_render(&[("logo.png", RED_PNG.to_vec())], &svg(href), true)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(xobjects(&pdf), 1, "{what}");
        assert!(count(&pdf, is_red) > 100, "{what}: not painted");
    }
}

#[test]
fn an_svg_used_as_an_image_never_loads_external_hrefs() {
    let images = |href: &str| {
        vec![
            ("card.svg", svg(href).into_bytes()),
            ("logo.png", RED_PNG.to_vec()),
        ]
    };
    for (what, body) in [
        ("<img src=card.svg>", "<img src=\"card.svg\">"),
        ("background-image", "<div class=\"bg\"></div>"),
        ("list-style-image", "<ul><li class=\"li\">x</li></ul>"),
    ] {
        let pdf =
            try_render(&images("logo.png"), body, false).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(
            xobjects(&pdf),
            0,
            "{what}: the bundled logo.png was loaded by an SVG used as an image"
        );
        assert_refused(
            try_render(&images("logo.png"), body, true),
            "logo.png",
            what,
        );
        // Its `data:` twin draws.
        let data = format!("data:image/png;base64,{}", b64(RED_PNG));
        let pdf = try_render(&images(&data), body, true)
            .unwrap_or_else(|e| panic!("{what} (data:): {e}"));
        assert_eq!(xobjects(&pdf), 1, "{what}: the data: raster is not drawn");
    }
}

#[test]
fn a_render_with_an_unloaded_href_is_deterministic() {
    let s = svg("missing.png");
    assert_eq!(
        try_render(&[], &s, false).unwrap(),
        try_render(&[], &s, false).unwrap()
    );
}
