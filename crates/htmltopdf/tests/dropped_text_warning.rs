//! Text that ends up in no glyph run is reported (F70). The static `font-family` check cannot see through a cascade
//! (a custom property that is `'Nope'` where it is used and `Inter` somewhere else passes it), so the engine says what
//! it actually dropped: one `log::warn!` per piece of text, or `Error::TextWithoutFont` with `strict_fonts(true)`.
//! One test only: the logger is global to the process.

use std::sync::Mutex;

use htmltopdf::{Error, RenderOptions, Renderer};

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

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// `--f` is `'Nope'` on `:root` and `Inter` only under `.x`: the static check accepts the union, but `<p>` is not
/// under `.x`, so its text has no face.
const LOST: &str = "<!doctype html><html><head><style>\
    :root{--f:'Nope'} .x{--f:Inter} p{font-family:var(--f)}\
    </style></head><body><p>LOST7</p><p class=\"x\">KEPT8</p></body></html>";

fn renderer(strict: bool) -> Renderer {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(strict)
        .build()
}

#[test]
fn text_that_no_face_covers_is_reported_once_and_is_an_error_in_strict_mode() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);

    // Strict: an error that names the text.
    take();
    match renderer(true).render(LOST, &RenderOptions::default()) {
        Err(Error::TextWithoutFont(found)) => {
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].contains("LOST7"), "{found:?}");
            assert!(!found[0].contains("KEPT8"), "{found:?}");
        }
        other => panic!("expected TextWithoutFont, got {:?}", other.map(|p| p.len())),
    }

    // Not strict: a PDF, and exactly one warning naming the text.
    take();
    let pdf = renderer(false)
        .render(LOST, &RenderOptions::default())
        .expect("a dropped text is a warning unless strict");
    assert!(pdf.starts_with(b"%PDF-"));
    let w: Vec<String> = take().into_iter().filter(|m| m.contains("LOST7")).collect();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(take().is_empty());

    // Nothing to say when every piece of text has a face, including whitespace-only runs, a collapsed space, a
    // zero-width space and an inline box.
    take();
    renderer(true)
        .render(
            "<!doctype html><body><p>Faktura\u{200b}2026 <b> </b>  <span style=\"display:inline-block;width:10px;height:10px\"></span>\n</p><p> </p><div>\n</div></body>",
            &RenderOptions::default(),
        )
        .expect("nothing is dropped here");
    let w = take();
    assert!(w.is_empty(), "{w:?}");
}
