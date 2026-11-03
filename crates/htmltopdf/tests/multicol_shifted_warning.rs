//! A box in columns that the planner cannot cut still says so (F198j guard). A paragraph pulled 40 px up, above the last
//! line of the one before it, is an overlap `mc_box` refuses: the card goes the old way, placed whole in its column,
//! and the engine warns that it is taller than a page. F198j-f: an offset that leaves the sheet is warned about on both
//! paths of the fallback. The logger is global to the process: it is installed once and the tests run one at a time.

use std::sync::{Mutex, Once};

use htmltopdf::{RenderOptions, Renderer};

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static SERIAL: Mutex<()> = Mutex::new(());
static INSTALL: Once = Once::new();

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

fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render(r: &Renderer, body: &str) -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap());
    let html = format!(
        "<!doctype html><html><head><meta charset=utf-8><style>\
         @page{{size:A4;margin:15mm}}body{{margin:0;font:14px/1.2 Inter}}p{{margin:0}}\
         </style></head><body>{body}</body></html>"
    );
    r.render(&html, &RenderOptions::default()).expect("render");
    std::mem::take(&mut *LOG.lock().unwrap())
}

fn paragraphs(extra: &str) -> String {
    paragraphs_every(extra, 1)
}

/// The 25 paragraphs of the card; `extra` is the style of every `every / 2`-th of each run of `every` (all for 1).
fn paragraphs_every(extra: &str, every: usize) -> String {
    let body: String = (0..25)
        .map(|i| {
            let extra = if i % every == every / 2 { extra } else { "" };
            format!(
                "<p style='padding:4px 12px;{extra}'>{}</p>",
                words(i * 100, i * 100 + 100)
            )
        })
        .collect();
    format!(
        "<div style='column-count:2;column-gap:20px'><div style='border:1px solid #888;border-radius:12px;\
         overflow:hidden'>{body}</div></div><p>AFTER</p>"
    )
}

fn renderer() -> Renderer {
    INSTALL.call_once(|| {
        log::set_boxed_logger(Box::new(Capture)).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    let font = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/Inter-Regular.ttf"),
    )
    .expect("fixtures/fonts/Inter-Regular.ttf: run scripts/fetch-fonts.sh");
    Renderer::builder()
        .font_bytes(font)
        .unwrap()
        .strict_fonts(true)
        .build()
}

fn count(msgs: &[String], what: &str) -> usize {
    msgs.iter().filter(|m| m.contains(what)).count()
}

#[test]
fn a_child_pulled_above_the_previous_line_is_warned_about_and_a_plain_card_is_not() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let r = renderer();
    let warned = |msgs: &[String]| count(msgs, "taller than a page");
    let msgs = render(&r, &paragraphs("margin-top:-40px;"));
    assert_eq!(warned(&msgs), 1, "{msgs:?}");
    let msgs = render(&r, &paragraphs(""));
    assert_eq!(
        warned(&msgs),
        0,
        "a card the planner cuts is not warned about: {msgs:?}"
    );
}

#[test]
#[ignore = "F198j-f: red until the fork fix"]
fn an_offset_off_the_sheet_is_warned_about_on_both_paths() {
    // F198j-f. A card the fallback can bake keeps its offsets in the flow and says so ("part of the flow"); a card it cannot
    // bake keeps its painted pieces, and since a line is painted past the edge of the page the engine says that, once. On round 2
    // the second card is placed whole instead and warned about as "taller than a page" (and "part of the flow").
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let r = renderer();
    // Paragraphs 3, 8, ... `top: 60px` (45 pt, more than the 42.5 pt margin): no baked description.
    let msgs = render(&r, &paragraphs_every("position:relative;top:60px;", 5));
    assert_eq!(count(&msgs, "past the edge of the page"), 1, "{msgs:?}");
    assert_eq!(count(&msgs, "taller than a page"), 0, "{msgs:?}");
    assert_eq!(count(&msgs, "part of the flow"), 0, "{msgs:?}");
    // Every paragraph `top: 100px`: baked, as before.
    let msgs = render(&r, &paragraphs("position:relative;top:100px;"));
    assert_eq!(count(&msgs, "part of the flow"), 1, "{msgs:?}");
    assert_eq!(count(&msgs, "past the edge of the page"), 0, "{msgs:?}");
    // `top: 20px` stays in the margin: painted, nothing to say.
    let msgs = render(&r, &paragraphs("position:relative;top:20px;"));
    assert!(msgs.is_empty(), "{msgs:?}");
}
