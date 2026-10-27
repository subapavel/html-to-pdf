//! SVG text is shaped with the bundled fonts only (F152).
//!
//! Blitz parsed every inline `<svg>` with a process-wide font database that scanned the host's font directories, so
//! the PDF depended on the machine (a host `Inter-Regular` won over the bundled one) and the first SVG paid for the
//! scan. An SVG used as an image (`<img src>`, `background-image`, `list-style-image`) had the opposite fault: usvg's
//! default `Options` have an empty database, so its text was not drawn at all. Now every SVG parse gets a database of
//! the bundled faces, the generic families map like they do for HTML (F70), and a family list that matches nothing
//! falls back to the serif mapping with a warning (css-fonts-4 §5); `strict_fonts` refuses it
//! (`Origin::Svg`). Plan: docs/plans/2026-10-26-f152-svg-text-bundled-fonts.md.
//!
//! Font names come from `pdffonts` (subset prefix stripped), text from `pdftotext`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use htmltopdf::{Error, Origin, RenderOptions, Renderer};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn font(name: &str) -> PathBuf {
    let p = root().join("fixtures/fonts").join(name);
    assert!(
        p.is_file(),
        "{} missing: run scripts/fetch-fonts.sh",
        p.display()
    );
    p
}

fn temp(bytes: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-svgfont-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}

/// The distinct font names of the PDF (`pdffonts`, subset prefix `ABCDEF+` stripped), one entry per font object.
fn font_list(pdf: &[u8]) -> Vec<String> {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdffonts")
        .arg(&path)
        .output()
        .expect("pdffonts (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdffonts failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(2)
        .filter_map(|l| l.split_whitespace().next())
        .map(|n| match n.split_once('+') {
            Some((prefix, rest)) if prefix.len() == 6 => rest.to_string(),
            _ => n.to_string(),
        })
        .collect()
}

fn fonts(pdf: &[u8]) -> BTreeSet<String> {
    font_list(pdf).into_iter().collect()
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn text(pdf: &[u8]) -> String {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

const CSS: &str = "body { margin: 0; font: 16px/1.2 Inter } .bg { width: 400px; height: 140px; \
                   background: url(card.svg) no-repeat } .li { list-style-image: url(card.svg) } \
                   .run { position: running(hdr) } \
                   @page { @top-center { content: element(hdr) } }";

fn render_with(
    bundled: &[&str],
    images: &[(&str, String)],
    body: &str,
    strict: bool,
) -> htmltopdf::Result<Vec<u8>> {
    let mut b = Renderer::builder().css(CSS).strict_fonts(strict);
    for f in bundled {
        b = b.font_file(font(f)).unwrap();
    }
    for (name, svg) in images {
        b = b.image(*name, svg.clone().into_bytes());
    }
    b.build().render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

/// An SVG with four texts: no family, `sans-serif`, `Inter` and `monospace`.
fn four_texts(inline_id: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\"{inline_id} width=\"400\" height=\"140\" viewBox=\"0 0 400 140\">\
         <rect width=\"400\" height=\"140\" fill=\"#eef\"/>\
         <text x=\"10\" y=\"30\" font-size=\"20\">Default Faktura</text>\
         <text x=\"10\" y=\"60\" font-size=\"20\" font-family=\"sans-serif\">Sans Faktura</text>\
         <text x=\"10\" y=\"90\" font-size=\"20\" font-family=\"Inter\">Inter Faktura</text>\
         <text x=\"10\" y=\"120\" font-size=\"20\" font-family=\"monospace\">Mono Faktura</text></svg>"
    )
}

/// A one-text SVG.
fn one_text(attrs: &str, content: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"300\" height=\"60\" viewBox=\"0 0 300 60\">\
         <text x=\"10\" y=\"35\" font-size=\"20\" {attrs}>{content}</text></svg>"
    )
}

// ---------------------------------------------------------------------------------------------------------------
// 1. Host isolation (child process)

const CHILD: &str = "F152_CHILD";

/// Re-runs `test` of this binary in a child that renders into the path in `CHILD`, with `HOME` a temp dir and
/// `FONTCONFIG_FILE` a fonts.conf; `host_bold` puts a copy of `Inter-Bold.ttf` in the directories both name.
fn child_render(test: &str, tag: &str, host_bold: bool) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!("htmltopdf-f152-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = dir.join("home");
    let fc = dir.join("fc-fonts");
    std::fs::create_dir_all(home.join(".fonts")).unwrap();
    std::fs::create_dir_all(&fc).unwrap();
    if host_bold {
        let bold = std::fs::read(font("Inter-Bold.ttf")).unwrap();
        std::fs::write(home.join(".fonts/Inter-Bold.ttf"), &bold).unwrap();
        std::fs::write(fc.join("Inter-Bold.ttf"), &bold).unwrap();
    }
    let conf = dir.join("fonts.conf");
    std::fs::write(
        &conf,
        format!(
            "<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"fonts.dtd\"><fontconfig><dir>{}</dir></fontconfig>",
            fc.display()
        ),
    )
    .unwrap();
    let out = dir.join("out.pdf");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(CHILD, &out)
        .env("HOME", &home)
        .env("FONTCONFIG_FILE", &conf)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("re-run the test binary");
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let o = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{test}: the child failed ({status:?}): {}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "{test}: the child did not finish"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let pdf = std::fs::read(&out).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    pdf
}

#[test]
fn a_host_font_never_reaches_svg_text() {
    const NAME: &str = "a_host_font_never_reaches_svg_text";
    let body = format!(
        "<p>Before</p>{}",
        one_text("font-weight=\"bold\" font-family=\"Inter\"", "Bold Faktura")
    );
    if let Some(out) = std::env::var_os(CHILD) {
        let pdf = render_with(&["Inter-Regular.ttf"], &[], &body, false).unwrap();
        std::fs::write(out, pdf).unwrap();
        return;
    }
    let without = child_render(NAME, "without", false);
    let with = child_render(NAME, "with", true);
    assert!(
        !fonts(&with).iter().any(|f| f.contains("Bold")),
        "a host Inter-Bold is embedded: {:?}",
        fonts(&with)
    );
    assert!(
        text(&with).contains("Bold Faktura"),
        "the SVG text is not drawn"
    );
    assert!(
        with == without,
        "the host's font directory changed the PDF ({} vs {} bytes)",
        with.len(),
        without.len()
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 2. Only bundled faces, at every site

#[test]
fn every_svg_site_uses_only_the_bundled_faces() {
    let card = || vec![("card.svg", four_texts(""))];
    let inline = format!("<p>Before</p>{}", four_texts(" id=\"s\""));
    let allowed = names(&["Inter-Regular", "JetBrainsMono-Regular"]);
    for (site, images, body, drawn) in [
        ("inline <svg>", vec![], inline.as_str(), true),
        (
            "<img src>",
            card(),
            "<p>Before</p><img src=\"card.svg\">",
            true,
        ),
        (
            "background-image",
            card(),
            "<p>Before</p><div class=\"bg\"></div>",
            true,
        ),
        // The marker's text is not extractable, but its glyphs are still embedded.
        (
            "list-style-image",
            card(),
            "<ul><li class=\"li\">Before</li></ul>",
            false,
        ),
    ] {
        let pdf = render_with(&["Inter-Regular.ttf"], &images, body, false)
            .unwrap_or_else(|e| panic!("{site}: {e}"));
        let used = fonts(&pdf);
        assert!(
            used.is_subset(&allowed),
            "{site}: fonts outside the bundle: {used:?}"
        );
        assert!(
            used.contains("JetBrainsMono-Regular"),
            "{site}: the monospace text is not drawn: {used:?}"
        );
        if drawn {
            let t = text(&pdf);
            for want in [
                "Default Faktura",
                "Sans Faktura",
                "Inter Faktura",
                "Mono Faktura",
            ] {
                assert!(t.contains(want), "{site}: `{want}` is missing from {t:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 3. Generic families map like they do for HTML

#[test]
fn generic_families_in_svg_text_map_like_html() {
    let bundled = ["Inter-Regular.ttf", "Inter-Bold.ttf"];
    for (attrs, want) in [
        ("font-family=\"monospace\"", "JetBrainsMono-Regular"),
        ("font-family=\"ui-monospace\"", "JetBrainsMono-Regular"),
        ("font-family=\"system-ui, monospace\"", "Inter-Regular"),
        ("", "Inter-Regular"),
        ("font-family=\"serif\"", "Inter-Regular"),
        ("font-family=\"sans-serif\"", "Inter-Regular"),
        ("font-family=\"Inter\" font-weight=\"700\"", "Inter-Bold"),
        (
            "font-family=\"sans-serif\" font-weight=\"bold\"",
            "Inter-Bold",
        ),
    ] {
        let pdf = render_with(&bundled, &[], &one_text(attrs, "Mapped Faktura"), false)
            .unwrap_or_else(|e| panic!("{attrs}: {e}"));
        assert_eq!(fonts(&pdf), names(&[want]), "{attrs:?}");
        assert!(
            text(&pdf).contains("Mapped Faktura"),
            "{attrs:?}: not drawn"
        );
    }
}

#[test]
fn css_in_an_inline_svg_maps_like_its_presentation_attributes() {
    // The HTML generic rewrite must leave CSS inside an inline `<svg>` alone: usvg does not read the CSS escape it
    // writes (`JetBrains\000020Mono` would be the family `JetBrains`), and strict mode would refuse a valid document.
    for (what, svg) in [
        (
            "style attribute",
            one_text("style=\"font-family:monospace\"", "Styled Faktura"),
        ),
        (
            "<style> element",
            one_text("", "Styled Faktura").replace(
                "<text ",
                "<style>text{font-family:ui-monospace}</style><text ",
            ),
        ),
    ] {
        let pdf = render_with(&["Inter-Regular.ttf"], &[], &svg, true)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(fonts(&pdf), names(&["JetBrainsMono-Regular"]), "{what}");
        assert!(text(&pdf).contains("Styled Faktura"), "{what}: not drawn");
    }
    // An unbundled family in an SVG's CSS is reported once, as SVG text (which is drawn), not as HTML CSS whose text
    // would be dropped.
    let svg = one_text("style=\"font-family:Georgia\"", "Styled Faktura");
    match render_with(&["Inter-Regular.ttf"], &[], &svg, true) {
        Err(Error::UnresolvedFontFamilies(list)) => assert!(
            list.iter().all(|u| u.origin == Origin::Svg) && list.len() == 1,
            "{list:?}"
        ),
        other => panic!("strict mode must refuse, got {:?}", other.map(|p| p.len())),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 4. An unbundled family

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.level() <= log::Level::Warn {
            LOG.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

fn capture_warnings() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::set_boxed_logger(Box::new(Capture)).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
}

#[test]
fn an_unbundled_family_falls_back_with_a_warning_and_strict_refuses_it() {
    capture_warnings();
    let svg = one_text("font-family=\"Georgia, Palatino\"", "Georgia Faktura");
    let body = format!("<p>Before</p>{svg}");

    let pdf = render_with(&["Inter-Regular.ttf"], &[], &body, false).unwrap();
    assert_eq!(
        fonts(&pdf),
        names(&["Inter-Regular"]),
        "drawn in the default family"
    );
    assert!(text(&pdf).contains("Georgia Faktura"), "not drawn");
    let warned: Vec<String> = LOG
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m.contains("Georgia"))
        .cloned()
        .collect();
    assert_eq!(warned.len(), 1, "exactly one warning: {warned:?}");
    assert!(
        warned[0].contains("SVG") || warned[0].contains("svg"),
        "the warning names the SVG: {warned:?}"
    );
    assert!(warned[0].contains("Georgia, Palatino"), "{warned:?}");

    match render_with(&["Inter-Regular.ttf"], &[], &body, true) {
        Err(Error::UnresolvedFontFamilies(list)) => assert!(
            list.iter()
                .any(|u| u.origin == Origin::Svg && u.font_family.contains("Georgia")),
            "{list:?}"
        ),
        other => panic!("strict mode must refuse, got {:?}", other.map(|p| p.len())),
    }

    // The same switch accepts a list with a bundled entry, in an SVG used as an image as well.
    let ok = one_text("font-family=\"Georgia, Inter\"", "Listed Faktura");
    render_with(&["Inter-Regular.ttf"], &[], &ok, true).expect("a bundled entry resolves the list");
    match render_with(
        &["Inter-Regular.ttf"],
        &[("card.svg", svg)],
        "<img src=\"card.svg\">",
        true,
    ) {
        Err(Error::UnresolvedFontFamilies(list)) => {
            assert!(list.iter().any(|u| u.origin == Origin::Svg), "{list:?}")
        }
        other => panic!(
            "<img> SVG: strict must refuse, got {:?}",
            other.map(|p| p.len())
        ),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Fonts are shared between HTML and SVG text

#[test]
fn a_face_used_by_html_and_svg_text_is_embedded_once() {
    let body = format!(
        "<p style=\"font-family: Inter\">Html Faktura</p>{}",
        one_text("font-family=\"Inter\"", "Svg Faktura")
    );
    let pdf = render_with(&["Inter-Regular.ttf"], &[], &body, false).unwrap();
    let list = font_list(&pdf);
    assert_eq!(
        list.iter().filter(|f| *f == "Inter-Regular").count(),
        1,
        "{list:?}"
    );
    let t = text(&pdf);
    assert!(
        t.contains("Html Faktura") && t.contains("Svg Faktura"),
        "{t:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 6. Margin box

#[test]
fn svg_text_in_a_margin_box_uses_the_bundled_faces() {
    let body = format!(
        "<div class=\"run\">{}</div><p>Before</p>",
        one_text("font-family=\"monospace\"", "Margin Faktura")
    );
    let pdf = render_with(&["Inter-Regular.ttf"], &[], &body, false).unwrap();
    let used = fonts(&pdf);
    assert!(
        used.is_subset(&names(&["Inter-Regular", "JetBrainsMono-Regular"])),
        "{used:?}"
    );
    assert!(used.contains("JetBrainsMono-Regular"), "{used:?}");
    assert!(text(&pdf).contains("Margin Faktura"), "not drawn");
}

// ---------------------------------------------------------------------------------------------------------------
// 7. Determinism

#[test]
fn a_render_with_svg_text_is_deterministic() {
    let body = format!(
        "<p>Before</p>{}<img src=\"card.svg\">",
        four_texts(" id=\"s\"")
    );
    let images = [("card.svg", four_texts(""))];
    let bundled = ["Inter-Regular.ttf", "Inter-Bold.ttf"];
    let a = render_with(&bundled, &images, &body, false).unwrap();
    let b = render_with(&bundled, &images, &body, false).unwrap();
    assert!(
        a == b,
        "two renders differ ({} vs {} bytes)",
        a.len(),
        b.len()
    );
}
