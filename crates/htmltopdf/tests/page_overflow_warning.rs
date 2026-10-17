//! Content that runs past the right edge of the page is clipped by the page box; `nowrap` / `pre` text and wide boxes
//! make that possible (F66). Chrome shrinks the page to fit, which no specification asks for; we follow CSS and say so:
//! one `log::warn!` per page (F66g). One test only: the logger is global to the process.

use std::sync::Mutex;

use htmltopdf::{RenderOptions, Renderer};

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        // The warning comes from the engine (`fulgur`), the other messages from `htmltopdf`.
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

fn overflow_warnings(r: &Renderer, body: &str) -> Vec<String> {
    take();
    r.render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
    .expect("an overflow is a warning, never an error");
    take()
        .into_iter()
        .filter(|m| m.contains("page edge"))
        .collect()
}

const WORDS: &str = "Dodavatel zboží a služeb poskytuje plnění podle objednávky";

#[test]
fn content_past_the_page_edge_is_reported_once_per_page() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    // Strict modes only affect fonts and images; they must not turn the warning into an error.
    let r = Renderer::builder()
        .css("body { font-family: JetBrains Mono, monospace }")
        .strict_fonts(false)
        .strict_images(true)
        .build();

    // A `nowrap` line of 2000 px (1500 pt) on an A4 page (595 pt) with 15 mm margins: it ends about 960 pt past the
    // right edge of the page.
    let text = WORDS.repeat(8);
    let w = overflow_warnings(
        &r,
        &format!(r#"<p style="white-space:nowrap">{text} {text}</p>"#),
    );
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].starts_with("content extends ")
            && w[0].contains("pt past the right page edge on page 1 and is clipped"),
        "{w:?}"
    );
    let pt: f32 = w[0]
        .trim_start_matches("content extends ")
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(pt > 100.0, "{w:?}");

    // Identical input, identical message.
    let again = overflow_warnings(
        &r,
        &format!(r#"<p style="white-space:nowrap">{text} {text}</p>"#),
    );
    assert_eq!(w, again);

    // A box with a background wider than the page.
    let w = overflow_warnings(
        &r,
        r#"<div style="width:1200px;height:20px;background:rgb(200,0,0)"></div>"#,
    );
    assert_eq!(w.len(), 1, "{w:?}");

    // Only the page that overflows is named: two pages of text, then a wide `pre` line.
    let filler = format!("<p>{WORDS}</p>").repeat(60);
    let w = overflow_warnings(
        &r,
        &format!(r#"{filler}<p style="white-space:pre;margin-top:30px">{text}{text}{text}</p>"#),
    );
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(!w[0].contains("on page 1 "), "{w:?}");

    // Nothing to say when the content fits, wraps, or is clipped by its own box.
    for (what, body) in [
        ("wrapping text", format!("<p>{text} {text}</p>")),
        (
            "nowrap that fits",
            "<p style=\"white-space:nowrap\">short</p>".into(),
        ),
        (
            "a clipped line",
            format!(
                r#"<div style="width:200px;overflow:hidden;white-space:nowrap">{text} {text}</div>"#
            ),
        ),
        (
            "a wide box inside a clip",
            r#"<div style="width:200px;overflow:hidden"><div style="width:1200px;height:20px;background:rgb(200,0,0)"></div></div>"#.into(),
        ),
        (
            "a line that overflows the margin but not the page",
            format!(r#"<div style="width:300px;white-space:nowrap">{WORDS}</div>"#),
        ),
        (
            "a transparent wide box",
            r#"<div style="width:1200px;height:20px"></div>"#.into(),
        ),
    ] {
        let w = overflow_warnings(&r, &body);
        assert!(w.is_empty(), "{what}: {w:?}");
    }
}
