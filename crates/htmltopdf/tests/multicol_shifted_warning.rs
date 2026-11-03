//! A box in columns that the planner cannot cut still says so (F198j guard). A paragraph pulled 40 px up, above the last
//! line of the one before it, is an overlap `mc_box` refuses: the card goes the old way, placed whole in its column,
//! and the engine warns that it is taller than a page. One test only: the logger is global to the process.

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
    let body: String = (0..25)
        .map(|i| {
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

#[test]
fn a_child_pulled_above_the_previous_line_is_warned_about_and_a_plain_card_is_not() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let font = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/Inter-Regular.ttf"),
    )
    .expect("fixtures/fonts/Inter-Regular.ttf: run scripts/fetch-fonts.sh");
    let r = Renderer::builder()
        .font_bytes(font)
        .unwrap()
        .strict_fonts(true)
        .build();
    let warned = |msgs: &[String]| {
        msgs.iter()
            .filter(|m| m.contains("taller than a page"))
            .count()
    };
    let msgs = render(&r, &paragraphs("margin-top:-40px;"));
    assert_eq!(warned(&msgs), 1, "{msgs:?}");
    let msgs = render(&r, &paragraphs(""));
    assert_eq!(
        warned(&msgs),
        0,
        "a card the planner cuts is not warned about: {msgs:?}"
    );
}
