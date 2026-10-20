//! F100: Blitz reports html5ever parse errors and dropped inline SVGs through `log`, never on stdout. A parse error
//! is a diagnostic (HTML LS 13.2.2), so it is a `debug` record from `blitz_html`; an SVG that usvg rejects is content
//! the author wrote and loses, so it is a `warn` record from `blitz_dom`. One test only: the logger is global to the
//! process.

use std::sync::Mutex;

use htmltopdf::{RenderOptions, Renderer};

static LOG: Mutex<Vec<(log::Level, String, String)>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.target().starts_with("blitz") {
            LOG.lock()
                .unwrap()
                .push((r.level(), r.target().to_string(), r.args().to_string()));
        }
    }
    fn flush(&self) {}
}

fn render(r: &Renderer, html: &str) -> Vec<(log::Level, String, String)> {
    std::mem::take(&mut *LOG.lock().unwrap());
    r.render(html, &RenderOptions::default()).unwrap();
    std::mem::take(&mut *LOG.lock().unwrap())
}

#[test]
fn parse_errors_are_debug_records_and_a_rejected_svg_is_a_warning() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    let r = Renderer::builder().strict_fonts(false).build();

    // No doctype: html5ever reports "Unexpected token".
    let recs = render(&r, "<html><body><p>Ahoj</p></body></html>");
    assert!(
        recs.iter().any(|(l, t, m)| *l == log::Level::Debug
            && t.starts_with("blitz_html")
            && m.contains("Unexpected token")),
        "{recs:?}"
    );
    assert!(
        recs.iter()
            .all(|(l, t, _)| !(t.starts_with("blitz_html") && *l <= log::Level::Warn)),
        "{recs:?}"
    );

    // A valid document is silent at warn and above.
    let recs = render(&r, "<!doctype html><html><body><p>Ahoj</p></body></html>");
    assert!(recs.iter().all(|(l, ..)| *l > log::Level::Warn), "{recs:?}");

    // usvg rejects a zero-sized SVG (`InvalidSize`); the SVG is dropped.
    let recs = render(
        &r,
        "<!doctype html><html><body><p>a</p><svg width=\"0\" height=\"0\"></svg></body></html>",
    );
    let warns: Vec<_> = recs
        .iter()
        .filter(|(l, t, _)| *l == log::Level::Warn && t.starts_with("blitz_dom"))
        .collect();
    assert_eq!(warns.len(), 1, "{recs:?}");
    assert!(warns[0].2.contains("SVG"), "{warns:?}");
}
