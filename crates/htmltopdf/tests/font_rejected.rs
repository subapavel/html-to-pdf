//! A bundled font the PDF writer cannot embed is left out, with a warning (F178); `strict_fonts` refuses it.
//!
//! Parley (fontique, skrifa) and krilla parse a font separately and they disagree. A face without a `head` table is
//! shaped with by Parley, but krilla returns `None` for it at draw time and the run was skipped silently: exit 0
//! even with `--strict-fonts`, the bold lines missing from the PDF. With `glyf` renamed krilla accepts the face and
//! embeds it as a Type 3 font whose glyph procedures are empty: `pdftotext` finds the text and nothing is painted.
//! The fix checks every font when it is registered, with krilla's own parser, and leaves a font it cannot embed out
//! of the bundle whole (css-fonts-4 §4.3: an invalid font resource is treated as if it failed to load; Chrome's OTS
//! says "head: missing required table" / "no supported glyph data table(s) present"). Plan:
//! docs/plans/2026-10-27-f178-krilla-rejected-faces.md.
//!
//! Font names come from `pdffonts` (subset prefix stripped), text from `pdftotext`. Every test takes `serial()`:
//! the logger is global to the process and `take()` must see only the warnings of its own test.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};

use htmltopdf::{RenderOptions, Renderer, RendererBuilder};

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn font_path(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(name);
    assert!(
        p.is_file(),
        "{} missing: run scripts/fetch-fonts.sh",
        p.display()
    );
    p
}

fn font(name: &str) -> Vec<u8> {
    std::fs::read(font_path(name)).unwrap()
}

/// `font` with the table `from` renamed to `to` in the table directory. The directory stays sorted by tag, so the
/// file is still a well-formed sfnt: only the table that the readers look for by name is gone.
fn rename_table(font: &[u8], from: &[u8; 4], to: &[u8; 4]) -> Vec<u8> {
    let mut out = font.to_vec();
    let n = u16::from_be_bytes([font[4], font[5]]) as usize;
    let mut found = false;
    let mut records: Vec<[u8; 16]> = (0..n)
        .map(|i| {
            let mut r: [u8; 16] = font[12 + 16 * i..28 + 16 * i].try_into().unwrap();
            if &r[..4] == from {
                r[..4].copy_from_slice(to);
                found = true;
            }
            r
        })
        .collect();
    assert!(found, "no `{}` table", String::from_utf8_lossy(from));
    records.sort_by(|a, b| a[..4].cmp(&b[..4]));
    for (i, r) in records.iter().enumerate() {
        out[12 + 16 * i..28 + 16 * i].copy_from_slice(r);
    }
    out
}

/// A TrueType collection of `fonts` (each a single sfnt): `ttcf` header, 4-byte aligned copies, table offsets
/// rebased to the start of the file.
fn ttc_of(fonts: &[&[u8]]) -> Vec<u8> {
    let header = 12 + 4 * fonts.len();
    let mut out = vec![0u8; header];
    out[..4].copy_from_slice(b"ttcf");
    out[4..8].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    out[8..12].copy_from_slice(&(fonts.len() as u32).to_be_bytes());
    for (i, f) in fonts.iter().enumerate() {
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let base = out.len();
        out[12 + 4 * i..16 + 4 * i].copy_from_slice(&(base as u32).to_be_bytes());
        out.extend_from_slice(f);
        let n = u16::from_be_bytes([f[4], f[5]]) as usize;
        for t in 0..n {
            let at = base + 12 + 16 * t + 8;
            let off = u32::from_be_bytes(out[at..at + 4].try_into().unwrap()) + base as u32;
            out[at..at + 4].copy_from_slice(&off.to_be_bytes());
        }
    }
    out
}

fn temp(bytes: &[u8], ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-fontrej-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn pdffonts(pdf: &[u8]) -> String {
    let path = temp(pdf, "pdf");
    let out = Command::new("pdffonts")
        .arg(&path)
        .output()
        .expect("pdffonts (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdffonts failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The distinct font names of the PDF (subset prefix `ABCDEF+` stripped).
fn fonts(pdf: &[u8]) -> BTreeSet<String> {
    pdffonts(pdf)
        .lines()
        .skip(2)
        .filter_map(|l| l.split_whitespace().next())
        .map(|n| match n.split_once('+') {
            Some((prefix, rest)) if prefix.len() == 6 => rest.to_string(),
            _ => n.to_string(),
        })
        .collect()
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

static SERIAL: Mutex<()> = Mutex::new(());

/// Holds the process-wide logger for one test and empties what earlier tests left in it.
fn serial() -> MutexGuard<'static, ()> {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::set_boxed_logger(Box::new(Capture)).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    let guard = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    take();
    guard
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

const CSS: &str = ".font-bold{font-weight:700} .font-semibold{font-weight:600}";

const BODY: &str = "<!doctype html><html><body><p>Normální</p>\
                    <p class=\"font-bold\">TUČNÝ ŘÁDEK</p></body></html>";

fn render(b: RendererBuilder, html: &str) -> htmltopdf::Result<Vec<u8>> {
    b.css(CSS).build().render(html, &RenderOptions::default())
}

/// Inter-Regular, then the font(s) under test, each added with `font_bytes` (so the first is `font_bytes #2`).
/// `strict_first` puts `strict_fonts` before the fonts, otherwise after them.
fn builder(extra: &[Vec<u8>], strict: bool, strict_first: bool) -> RendererBuilder {
    let mut b = Renderer::builder();
    if strict_first {
        b = b.strict_fonts(strict);
    }
    b = b.font_bytes(font("Inter-Regular.ttf")).unwrap();
    for f in extra {
        b = b.font_bytes(f.clone()).unwrap();
    }
    if !strict_first {
        b = b.strict_fonts(strict);
    }
    b
}

/// The error of a strict render: its variant (Debug) and its message.
fn refused(r: htmltopdf::Result<Vec<u8>>, what: &str) -> (String, String) {
    match r {
        Err(e) => (format!("{e:?}"), e.to_string()),
        Ok(pdf) => panic!(
            "{what}: strict mode must refuse the font, got a PDF of {} bytes",
            pdf.len()
        ),
    }
}

/// The strict-mode contract of a rejected Bold, whichever way the switch is ordered.
fn assert_strict_refuses(extra: &[Vec<u8>], face: &str, why: &str) {
    for strict_first in [true, false] {
        let (variant, msg) = refused(
            render(builder(extra, true, strict_first), BODY),
            &format!("strict_first={strict_first}"),
        );
        assert!(
            variant.starts_with("RejectedFonts"),
            "strict_first={strict_first}: wrong error {variant}: {msg}"
        );
        for want in [face, "font_bytes #2", why] {
            assert!(
                msg.contains(want),
                "strict_first={strict_first}: `{want}` is missing from {msg:?}"
            );
        }
    }
}

/// A non-strict build + render: the PDF and the warnings it logged.
fn lenient(extra: &[Vec<u8>], html: &str) -> (Vec<u8>, Vec<String>) {
    take();
    let pdf = render(builder(extra, false, false), html).expect("non-strict mode renders");
    (pdf, take())
}

fn assert_one_warning(log: &[String], face: &str, why: &str) {
    let named: Vec<&String> = log.iter().filter(|m| m.contains(face)).collect();
    assert_eq!(named.len(), 1, "exactly one warning names {face}: {log:?}");
    for want in ["font_bytes #2", why] {
        assert!(
            named[0].contains(want),
            "`{want}` is missing from the warning {:?}",
            named[0]
        );
    }
    assert!(
        !log.iter().any(|m| m.contains("no font face")),
        "the run is not dropped any more, so no dropped-text warning: {log:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 1. `head` renamed: krilla returns None

#[test]
fn a_bold_without_head_is_left_out_and_its_text_is_drawn_in_the_other_face() {
    let _g = serial();
    let bad = rename_table(&font("Inter-Bold.ttf"), b"head", b"hexd");
    let (pdf, log) = lenient(std::slice::from_ref(&bad), BODY);
    let t = text(&pdf);
    assert!(
        t.contains("Normální") && t.contains("TUČNÝ ŘÁDEK"),
        "text is missing from {t:?}"
    );
    assert_eq!(fonts(&pdf), names(&["Inter-Regular"]));
    assert_one_warning(&log, "face 0 (Inter Bold)", "head");

    assert_strict_refuses(&[bad], "face 0 (Inter Bold)", "head");
}

// ---------------------------------------------------------------------------------------------------------------
// 3. `glyf` renamed: krilla accepts it and would embed Type 3 with empty glyphs

#[test]
fn a_bold_without_outlines_is_left_out_and_never_embedded_as_invisible_type_3() {
    let _g = serial();
    let bad = rename_table(&font("Inter-Bold.ttf"), b"glyf", b"glyx");
    let (pdf, log) = lenient(std::slice::from_ref(&bad), BODY);
    let t = text(&pdf);
    assert!(
        t.contains("Normální") && t.contains("TUČNÝ ŘÁDEK"),
        "text is missing from {t:?}"
    );
    let listing = pdffonts(&pdf);
    assert!(
        !listing.contains("Type 3"),
        "an invisible Type 3 font is embedded:\n{listing}"
    );
    assert_eq!(fonts(&pdf), names(&["Inter-Regular"]), "{listing}");
    assert_one_warning(&log, "face 0 (Inter Bold)", "outline");

    assert_strict_refuses(&[bad], "face 0 (Inter Bold)", "outline");
}

// ---------------------------------------------------------------------------------------------------------------
// 3b. Faces krilla opens but cannot write (F178 reviews): a panic, a failed document, invisible colour-table glyphs

/// `font` with the big-endian u16 at offset `at` of table `table` set to `value`.
fn set_u16(font: &[u8], table: &[u8; 4], at: usize, value: u16) -> Vec<u8> {
    let n = u16::from_be_bytes([font[4], font[5]]) as usize;
    let record = (0..n)
        .map(|i| 12 + 16 * i)
        .find(|r| &font[*r..*r + 4] == table)
        .expect("no such table");
    let off = u32::from_be_bytes(font[record + 8..record + 12].try_into().unwrap()) as usize;
    let mut out = font.to_vec();
    out[off + at..off + at + 2].copy_from_slice(&value.to_be_bytes());
    out
}

#[test]
fn a_bold_the_pdf_writer_cannot_write_is_left_out_instead_of_failing_the_document() {
    let _g = serial();
    let bold = font("Inter-Bold.ttf");
    for (what, bad, why) in [
        // Before: krilla panicked in the CID font descriptor (`cid.rs` unwrap), no PDF.
        ("unitsPerEm 0", set_u16(&bold, b"head", 18, 0), "unitsPerEm"),
        // Before: "failed to subset font: malformed font", no PDF, even without strict mode.
        (
            "numberOfHMetrics 0",
            set_u16(&bold, b"hhea", 34, 0),
            "subset",
        ),
        // Before: accepted for its colour table, embedded as Type 3 with empty glyphs.
        (
            "glyf renamed COLR",
            rename_table(&bold, b"glyf", b"COLR"),
            "outline",
        ),
    ] {
        let (pdf, log) = lenient(std::slice::from_ref(&bad), BODY);
        let t = text(&pdf);
        assert!(
            t.contains("Normální") && t.contains("TUČNÝ ŘÁDEK"),
            "{what}: text is missing from {t:?}"
        );
        assert_eq!(fonts(&pdf), names(&["Inter-Regular"]), "{what}");
        assert_one_warning(&log, "face 0 (Inter Bold)", why);
        assert_strict_refuses(&[bad], "face 0 (Inter Bold)", why);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 4. A collection with one bad face is left out whole

#[test]
fn a_collection_with_one_bad_face_is_left_out_whole() {
    let _g = serial();
    let bad = ttc_of(&[
        &font("Inter-SemiBold.ttf"),
        &rename_table(&font("Inter-Bold.ttf"), b"head", b"hexd"),
    ]);
    let html = "<!doctype html><html><body><p>Normální</p>\
                <p class=\"font-semibold\">POLOTUČNÝ ŘÁDEK</p></body></html>";
    let (pdf, log) = lenient(std::slice::from_ref(&bad), html);
    let t = text(&pdf);
    assert!(
        t.contains("Normální") && t.contains("POLOTUČNÝ ŘÁDEK"),
        "text is missing from {t:?}"
    );
    assert_eq!(
        fonts(&pdf),
        names(&["Inter-Regular"]),
        "the good SemiBold of the collection is left out with its bad neighbour"
    );
    let named: Vec<&String> = log.iter().filter(|m| m.contains("face 1")).collect();
    assert_eq!(named.len(), 1, "one warning names face 1: {log:?}");
    assert!(named[0].contains("head"), "{named:?}");

    for strict_first in [true, false] {
        let (variant, msg) = refused(
            render(
                builder(std::slice::from_ref(&bad), true, strict_first),
                html,
            ),
            "collection",
        );
        assert!(variant.starts_with("RejectedFonts"), "{variant}: {msg}");
        assert!(msg.contains("face 1") && msg.contains("head"), "{msg}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Bytes that are not a font

#[test]
fn bytes_that_are_not_a_font_are_left_out_and_never_become_the_default_family() {
    let _g = serial();
    // Junk first: it must not be the document's default family, nor stop Inter from being it.
    let build = |strict: bool| {
        Renderer::builder()
            .strict_fonts(strict)
            .font_bytes(b"junk".to_vec())
            .unwrap()
            .font_bytes(font("Inter-Regular.ttf"))
            .unwrap()
            .css(CSS)
            .build()
    };
    take();
    let pdf = build(false)
        .render("<p>Prostý text</p>", &RenderOptions::default())
        .unwrap();
    let log = take();
    assert!(text(&pdf).contains("Prostý text"));
    assert_eq!(fonts(&pdf), names(&["Inter-Regular"]), "default family");
    let named: Vec<&String> = log
        .iter()
        .filter(|m| m.contains("not an OpenType font"))
        .collect();
    assert_eq!(named.len(), 1, "one warning for the junk: {log:?}");
    assert!(named[0].contains("font_bytes #1"), "{named:?}");

    let (variant, msg) = refused(
        build(true).render("<p>Prostý text</p>", &RenderOptions::default()),
        "junk",
    );
    assert!(variant.starts_with("RejectedFonts"), "{variant}: {msg}");
    assert!(msg.contains("not an OpenType font"), "{msg}");
}

// ---------------------------------------------------------------------------------------------------------------
// 6. SVG text (F152 path)

#[test]
fn svg_text_never_uses_a_font_without_outlines() {
    let _g = serial();
    let bad = rename_table(&font("Inter-Bold.ttf"), b"glyf", b"glyx");
    let html = "<!doctype html><html><body><p>Před</p>\
        <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"300\" height=\"60\" viewBox=\"0 0 300 60\">\
        <text x=\"10\" y=\"35\" font-size=\"20\" font-family=\"Inter\" font-weight=\"bold\">TUČNĚ</text></svg>\
        </body></html>";
    let (pdf, _log) = lenient(&[bad], html);
    let t = text(&pdf);
    assert!(t.contains("TUČNĚ"), "the SVG text is not drawn: {t:?}");
    let listing = pdffonts(&pdf);
    assert!(
        !listing.contains("Type 3"),
        "an invisible Type 3 font is embedded:\n{listing}"
    );
    assert_eq!(fonts(&pdf), names(&["Inter-Regular"]), "{listing}");
}
