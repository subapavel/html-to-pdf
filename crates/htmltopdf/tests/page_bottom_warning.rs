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
//! F213d adds the "painted on no page" warning of the same net (a paragraph's lines or an inline box that no page
//! paints) and the cases of an inline-block alone on its line (plan
//! docs/plans/2026-11-03-f213d-alone-inline-block-lost.md).
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
const F213_BOXTOP: &str = "<span style=\"display:block\">BOXTOP</span>";
const F213_BOX: &str = "<span style=\"display:inline-block;width:150px;height:300px;border:1px solid #000\">\
    <span style=\"display:block\">BOXTOP</span></span>";

/// F213d: the same page with an inline-block alone on its line. `f213d_room(r)` leaves `r` px below it; the box is `h` px
/// tall with the block children `inner`.
fn f213d_room(r: u32) -> String {
    format!("<div style=\"height:{}px\"></div>", 1009 - r)
}

fn f213d_box(h: u32, inner: &str) -> String {
    format!(
        "<p><span style=\"display:inline-block;width:150px;height:{h}px;border:1px solid #000\">{inner}</span></p><p>AFTER</p>"
    )
}

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
        .filter(|m| {
            m.contains("past the bottom page edge")
                || m.contains("is placed whole")
                || m.contains("painted on no page")
        })
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
        // F213d: an inline-block alone on its line, with a short block child, moves to the next page whole; it used to be left
        // on page 1 with the box and its text painted below the edge, printed nowhere, with no warning. Each of these is
        // green on main only because the net does not see the box (its text is painted under a local canvas); once the net
        // does, they stay silent only because the fragmenter moves the box.
        (
            "F213d Q1: a 300 px box with a block child, room 120",
            F213_PAGE.into(),
            format!("{}{}", f213d_room(120), f213d_box(300, F213_BOXTOP)),
        ),
        (
            "F213d Q1: room 60, then room 100 on the next page",
            F213_PAGE.into(),
            format!(
                "{}{}{}{}",
                f213d_room(60),
                f213d_box(300, F213_BOXTOP),
                f213d_room(100),
                f213d_box(300, F213_BOXTOP)
            ),
        ),
        (
            "F213d Q2: a 400 px box, room 120",
            F213_PAGE.into(),
            format!("{}{}", f213d_room(120), f213d_box(400, F213_BOXTOP)),
        ),
        (
            "F213d Q3: an auto-height box of 15 block children, room 120",
            F213_PAGE.into(),
            format!(
                "{}<p><span style=\"display:inline-block;width:150px;border:1px solid #000\">{}</span></p><p>AFTER</p>",
                f213d_room(120),
                (1..=15)
                    .map(|i| format!("<span style=\"display:block\">L{i:02}</span>"))
                    .collect::<String>()
            ),
        ),
        (
            "F213d Q4: a 196 px box with a caption, an svg and a name, room 130",
            F213_PAGE.into(),
            format!(
                "{}{}",
                f213d_room(130),
                f213d_box(
                    196,
                    "<span style=\"display:block\">CAPTION</span>\
                     <svg style=\"display:block;margin:8px 0\" width=\"72\" height=\"72\" viewBox=\"0 0 100 100\">\
                     <circle cx=\"50\" cy=\"50\" r=\"40\" fill=\"none\" stroke=\"#000\"/></svg>\
                     <span style=\"display:block\">NAME</span>"
                )
            ),
        ),
        (
            "F213d Q6: the box in a <section>, room 120",
            F213_PAGE.into(),
            format!(
                "{}<section>{}</section>",
                f213d_room(120),
                f213d_box(300, F213_BOXTOP)
            ),
        ),
        (
            "F213d Q6: the box in a bordered card with padding, room 120",
            F213_PAGE.into(),
            format!(
                "{}<div style=\"border:1px solid #ccc;padding:16px;border-radius:8px\">{}</div>",
                f213d_room(120),
                f213d_box(300, F213_BOXTOP)
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

    // F213d Q10, a known residual loss: an inline-block alone on its line that is taller than a page (1200 px, short block
    // child) is not split: its box and its text are painted below the edge of page 1, even with no room made at all.
    // Chrome slices it over pages 1 and 2. Main says nothing today (red until the safety net of F213d sees the box); the
    // fix of F213d moves only boxes that fit a page, so this one must keep warning (the residual is F213b's family).
    for (name, room) in [
        ("with 120 px of room", f213d_room(120)),
        ("at the top of page 1", String::new()),
    ] {
        let w = page_warnings(
            F213_PAGE,
            &format!("{room}{}", f213d_box(1200, F213_BOXTOP)),
        );
        assert!(
            !w.is_empty(),
            "F213d Q10, a box alone on its line taller than a page, {name}: no warning"
        );
    }

    // F183c, a known residual loss the net now sees: a fixed-height box with in-flow content is painted hanging below its line
    // by about its free space (ours seats an inline box bottom-on-baseline, Blitz places the box by its last line). A box of 600 px
    // moved to page 2 by F213d (Chrome: the whole box at the top of page 2) is painted down to 90.9 pt below the edge of page 2,
    // so the bottom of the box is printed on no page. The fix is F183c, not F213d; the net says so until then.
    let w = page_warnings(
        F213_PAGE,
        &format!("{}{}", f213d_room(120), f213d_box(600, F213_BOXTOP)),
    );
    assert!(
        w.iter()
            .any(|m| m.contains("past the bottom page edge on page 2")),
        "F183c, a fixed 600 px box moved to page 2: {w:?}"
    );

    // A transformed body is a split barrier: kept whole, and the pagination warning names the box.
    let w = page_warnings("body { transform: translateX(0) }", &lines(200));
    assert!(
        w.iter().any(|m| m.contains("in a transformed box")),
        "transformed body: {w:?}"
    );
}
