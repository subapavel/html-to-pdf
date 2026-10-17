//! A paragraph split over pages is painted from the line ranges its fragmenter recorded (F78). When some other emitter
//! split it and no ranges exist, the renderer measures the lines from the fragment heights, which can skip or repeat a
//! line, and says so with a warning. That warning must not fire where the measuring cannot be wrong: a one-line
//! paragraph (or any whole paragraph) inside a block that the block fragmenter splits is simply drawn once (F78 review,
//! M2). One test only: the logger is global to the process.

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

fn fallback_warnings(r: &Renderer, body: &str) -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap());
    r.render(
        &format!("<!doctype html><html><head><style>body{{font-family:Inter}}</style></head><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
    .expect("render");
    std::mem::take(&mut *LOG.lock().unwrap())
        .into_iter()
        .filter(|m| m.contains("recorded no matching line ranges"))
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
    let w = fallback_warnings(
        &r,
        "<p>before</p><div style=\"height:1200px;background:#ccc\">text</div><p>after</p>",
    );
    assert!(w.is_empty(), "one line, nothing to skip or repeat: {w:?}");

    // The same with three lines: every page holds the whole paragraph or none of it.
    let w = fallback_warnings(
        &r,
        "<p>before</p><div style=\"height:1200px;background:#ccc\">a<br>b<br>c</div><p>after</p>",
    );
    assert!(w.is_empty(), "whole paragraph on one page: {w:?}");

    // The real case: a `break-inside: avoid` paragraph taller than a page is split by the block fragmenter, so its
    // lines are measured, and some are lost (css-break-3 says `avoid` is relaxed here and the paragraph should be
    // split between lines; F83).
    let words: Vec<String> = (0..2500).map(|i| format!("word{i:05}")).collect();
    let w = fallback_warnings(
        &r,
        &format!("<p style=\"break-inside:avoid\">{}</p>", words.join(" ")),
    );
    assert!(!w.is_empty(), "lines spread over several pages by height");
    assert!(w[0].contains("word00000"), "names the paragraph: {w:?}");
}
