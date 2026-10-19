//! Content that is painted below the bottom edge of its page is not printed, and the renderer says so (F111 safety
//! net): `content extends N pt past the bottom page edge on page K and is not printed`. Every line of text is painted
//! on exactly one page (F78), so a line below the edge of any page is printed nowhere. The warning is the right-edge
//! check's counterpart (F66g) and records text only: backgrounds repeat on every page, text under a clip or a
//! transform is where its box puts it, margin boxes have their own canvas.
//!
//! The pagination warning for a paragraph the fragmenter has to keep whole is the older sibling: "placed whole and
//! ends N px below the content box". Body text taller than a page used to give neither (plan
//! docs/plans/2026-10-19-f111-body-text-tail-lost.md), and a body with a barrier now gets that one, naming the barrier.
//!
//! One test only: the logger is global to the process.

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

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("Line{i:03}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn prose(n: usize) -> String {
    (1..=n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

const BASE_CSS: &str = "body { margin: 0; font-family: Inter; line-height: 20px } p { margin: 0 } \
    table { border-collapse: collapse; width: 100% } td { padding: 4px 8px; border-bottom: 1px solid #ccc }";

/// The pagination warnings of one render: the new bottom-edge one and the F30 "placed whole" one.
fn page_warnings(extra_css: &str, body: &str) -> Vec<String> {
    let r = Renderer::builder()
        .font_bytes(inter())
        .expect("Inter")
        .strict_fonts(true)
        .css(format!("{BASE_CSS} {extra_css}"))
        .build();
    std::mem::take(&mut *LOG.lock().unwrap());
    let pdf = r
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    assert!(pdf.starts_with(b"%PDF"));
    std::mem::take(&mut *LOG.lock().unwrap())
        .into_iter()
        .filter(|m| m.contains("past the bottom page edge") || m.contains("is placed whole"))
        .collect()
}

#[test]
fn warns_about_content_below_the_page_only() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);

    // --- must not warn ---------------------------------------------------------------------------------------------
    let silent: Vec<(&str, String, String)> = vec![
        ("S1: 200 lines in body", String::new(), lines(200)),
        ("S4: 4000 words in body", String::new(), prose(4000)),
        (
            "S5: padded body",
            "body { padding: 20px }".into(),
            lines(200),
        ),
        ("S10: a short body", String::new(), lines(10)),
        (
            "a body-level <p> of 12000 words",
            String::new(),
            format!("<p>{}</p>", prose(12000)),
        ),
        (
            "a table over 3 pages",
            String::new(),
            format!(
                "<table>{}</table>",
                (1..=150)
                    .map(|i| format!(
                        "<tr><td>Item {i}</td><td>Popis položky {i}</td><td>{i},00 Kč</td></tr>"
                    ))
                    .collect::<String>()
            ),
        ),
        (
            "margin 0, 3 pages of <p>",
            "@page { margin: 0 }".into(),
            (1..=160)
                .map(|i| format!("<p>Řádek {i}</p>"))
                .collect::<String>(),
        ),
        (
            "an empty div of 300vh with a background",
            String::new(),
            "<div style=\"height:300vh;background:#eef\"></div>".into(),
        ),
        (
            "overflow:hidden around 4000 words",
            String::new(),
            format!(
                "<div style=\"overflow:hidden;height:300px\">{}</div>",
                prose(4000)
            ),
        ),
    ];
    for (name, css, body) in &silent {
        let w = page_warnings(css, body);
        assert!(w.is_empty(), "{name}: no warning expected, got {w:?}");
    }

    // --- must warn -------------------------------------------------------------------------------------------------
    // A box the fragmenter cannot move: an absolutely positioned box whose text starts at the bottom edge of a
    // 100 pt page is painted below it on page 1 and is on no page (a known loss, F119). The net sees it.
    let w = page_warnings(
        "@page { size: 100pt 100pt; margin: 0 }",
        "<div style=\"position:absolute;top:100vh\">outer on page 2\
         <div style=\"position:absolute;top:300vh\">inner on page 5</div></div>",
    );
    assert!(
        w.iter()
            .any(|m| m.contains("past the bottom page edge on page 1")),
        "abspos box at the page edge: {w:?}"
    );

    // F30f-b, a known residual loss: an inline-flex box is kept whole. Its text is painted inside the inline box,
    // under a translation, which the net does not follow; the pagination warning is what says so.
    let w = page_warnings(
        "",
        &format!(
            "<div style=\"display:inline-flex;width:100%\"><div>{}</div></div>",
            prose(4000)
        ),
    );
    assert!(
        w.iter()
            .any(|m| m.contains("in an inline-flex or inline-grid box is placed whole")),
        "inline-flex around 4000 words: {w:?}"
    );

    // A transformed body is a split barrier: kept whole, and the pagination warning names the box.
    let w = page_warnings("body { transform: translateX(0) }", &lines(200));
    assert!(
        w.iter().any(|m| m.contains("in a transformed box")),
        "transformed body: {w:?}"
    );
}
