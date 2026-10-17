//! A paragraph split over pages is painted from the line ranges its fragmenter recorded (F78). When some other emitter
//! split it and no ranges exist, the renderer measures the lines from the fragment heights, which can skip or repeat a
//! line, and says so with a warning. That warning must not fire where the measuring cannot be wrong: a one-line
//! paragraph (or any whole paragraph) inside a block that the block fragmenter splits is simply drawn once (F78 review,
//! M2). A `break-inside: avoid` paragraph taller than a page used to be the real case that warned (it was cut into
//! strips by height and lost words); since F83 it is split between its line boxes like any paragraph and is silent.
//! The warn path itself is now covered only by the fork's unit test `paragraph_lines_for_page_slices_that_do_not_match_
//! the_lines_fall_back_to_measuring` (`render.rs`). One test only: the logger is global to the process.

use std::sync::Mutex;

use htmltopdf::{RenderOptions, Renderer};

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

/// The fallback warnings of one render, and the PDF.
fn fallback_warnings(r: &Renderer, body: &str) -> (Vec<String>, Vec<u8>) {
    std::mem::take(&mut *LOG.lock().unwrap());
    let pdf = r
        .render(
        &format!("<!doctype html><html><head><style>body{{font-family:Inter}}</style></head><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
    .expect("render");
    let warnings = std::mem::take(&mut *LOG.lock().unwrap())
        .into_iter()
        .filter(|m| m.contains("recorded no matching line ranges"))
        .collect();
    (warnings, pdf)
}

/// The `wordNNNNN` tokens of the PDF's text layer, in reading order.
fn printed_words(pdf: &[u8]) -> Vec<String> {
    let path = std::env::temp_dir().join(format!("htmltopdf-fw-{}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter(|w| w.starts_with("word"))
        .map(str::to_string)
        .collect()
}

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

#[test]
fn the_measuring_fallback_warns_only_where_it_can_be_wrong() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let r = Renderer::builder().font_bytes(inter()).unwrap().build();

    // A block taller than a page, split by the block fragmenter, whose text is one line: two fragments, one line.
    let (w, _) = fallback_warnings(
        &r,
        "<p>before</p><div style=\"height:1200px;background:#ccc\">text</div><p>after</p>",
    );
    assert!(w.is_empty(), "one line, nothing to skip or repeat: {w:?}");

    // The same with three lines: every page holds the whole paragraph or none of it.
    let (w, _) = fallback_warnings(
        &r,
        "<p>before</p><div style=\"height:1200px;background:#ccc\">a<br>b<br>c</div><p>after</p>",
    );
    assert!(w.is_empty(), "whole paragraph on one page: {w:?}");

    // The case that used to warn: a `break-inside: avoid` paragraph taller than a page (F83). `avoid` is relaxed
    // when the paragraph cannot fit a page (css-break-3 §4.4), so it is split between its line boxes by the line
    // fragmenter, which records the ranges: no warning, and every word is printed once, in order.
    let expected: Vec<String> = (0..2500).map(|i| format!("word{i:05}")).collect();
    let (w, pdf) = fallback_warnings(
        &r,
        &format!("<p style=\"break-inside:avoid\">{}</p>", expected.join(" ")),
    );
    assert!(w.is_empty(), "the line ranges are recorded: {w:?}");
    assert_eq!(printed_words(&pdf), expected, "all 2500 words, in order");
}
