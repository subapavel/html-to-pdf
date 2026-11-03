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

/// F213: the page of the probes (A4, 15 mm margins: content box 1009.13 px), the block that leaves 200 px below it and a
/// 300 px inline-block with a one-line block child.
const F213_PAGE: &str = "@page { size: A4; margin: 15mm }";
const F213_ROOM: &str = "<div style=\"height:809px\"></div>";
const F213_BOX: &str = "<span style=\"display:inline-block;width:150px;height:300px;border:1px solid #000\">\
    <span style=\"display:block\">BOXTOP</span></span>";

/// F213e: a 950 px inline-block with a block child (952 px with its border): it fits a fresh page, not the 809 px below
/// a 200 px body margin.
const F213E_BOX: &str = "<span style=\"display:inline-block;width:150px;height:950px;border:1px solid #000\">\
    <span style=\"display:block\">BOXTOP</span></span>";

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
        // F213: a line with an inline-block taller than the room moves to the next page whole; it used to be left on
        // page 1 with its baseline below the edge (plan docs/plans/2026-11-03-f213-text-beside-tall-inline-block.md).
        (
            "F213 P1: text, a tall inline-block with a block child, text",
            F213_PAGE.into(),
            format!("{F213_ROOM}<p>START {F213_BOX} TAIL</p><p>AFTER</p>"),
        ),
        (
            "F213 P5: the same in a <section>",
            F213_PAGE.into(),
            format!("{F213_ROOM}<section><p>START {F213_BOX} TAIL</p></section><p>AFTER</p>"),
        ),
        // F213e: the same first block of the body under `body { margin-top: 200px }` (page 1 starts 200 px down): the line moves to
        // page 2 like Chrome's instead of being printed on no page ("content extends 63.1 pt past the bottom page edge").
        // Plan docs/plans/2026-11-03-f213e-first-block-below-top-margin.md.
        (
            "F213e Q1: a first line with a tall inline-block under a body margin of 200px",
            format!("{F213_PAGE} body {{ margin-top: 200px }}"),
            format!("<p>START {F213E_BOX} TAIL</p><p>AFTER</p>"),
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

    // F213b, a known residual loss: a line taller than a page (the inline-block is 1200 px) is not split, so its baseline
    // words are painted below the edge of page 1. Chrome slices the box over pages 2 and 3 and prints the words on page 2.
    // Slicing a line shared with text is an ADR-0004 decision that is still open; the net says so until it is made.
    let w = page_warnings(
        F213_PAGE,
        &format!(
            "{F213_ROOM}<p>START <span style=\"display:inline-block;width:150px;height:1200px;border:1px solid #000\">\
             <span style=\"display:block\">BOXTOP</span></span> TAIL</p><p>AFTER</p>"
        ),
    );
    assert!(
        w.iter()
            .any(|m| m.contains("past the bottom page edge on page 1")),
        "F213b, a line taller than a page: {w:?}"
    );

    // A transformed body is a split barrier: kept whole, and the pagination warning names the box.
    let w = page_warnings("body { transform: translateX(0) }", &lines(200));
    assert!(
        w.iter().any(|m| m.contains("in a transformed box")),
        "transformed body: {w:?}"
    );
}
