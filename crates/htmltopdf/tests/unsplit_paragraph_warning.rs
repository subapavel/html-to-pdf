//! The warning for a paragraph the fragmenter places whole names the box that kept it whole (F30f).
//!
//! `warn_unprinted_inline_lines` used to list every wrapper "paragraphs are not split in" (clipping, multicol,
//! transformed, inline-block / inline-flex, `display: table`), which became wrong once clipping wrappers and an
//! inline-block alone on its line were split. It now says which barrier applies, and says nothing where the clip hides
//! the lines anyway (Chrome hides them too, so "not printed" would blame pagination for a clip). One test only: the
//! logger is global to the process.

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

fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The "placed whole" warnings of one render.
fn unsplit_warnings(r: &Renderer, body: &str) -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap());
    r.render(
        &format!(
            "<!doctype html><html><head><style>@page{{size:A4;margin:15mm}}\
             body{{font-family:Inter;line-height:20px;margin:0}}p{{margin:0}}</style></head><body>{body}</body></html>"
        ),
        &RenderOptions::default(),
    )
    .expect("render");
    std::mem::take(&mut *LOG.lock().unwrap())
        .into_iter()
        .filter(|m| m.contains("is placed whole"))
        .collect()
}

#[test]
fn the_unsplit_paragraph_warning_names_its_barrier() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let r = Renderer::builder().font_bytes(inter()).unwrap().build();
    let w = words(4000);

    // An inline-block that shares its line with text is not split (Chrome slices it, F30f-b): the lines below the page
    // are lost, and the warning says why.
    let shared = unsplit_warnings(
        &r,
        &format!("<p>Úvod <span style=\"display:inline-block;width:60%\">{w}</span> konec</p>"),
    );
    assert_eq!(shared.len(), 1, "{shared:?}");
    assert!(
        shared[0].contains("an inline-block that shares its line"),
        "{}",
        shared[0]
    );

    // A clip box shorter than its paragraph hides the lines below it: nothing is unprinted that Chrome prints, no
    // warning.
    let clipped = unsplit_warnings(
        &r,
        &format!("<div style=\"overflow:hidden;height:600px\"><p>{w}</p></div>"),
    );
    assert!(clipped.is_empty(), "{clipped:?}");

    // Wrappers that are split now say nothing.
    let alone = unsplit_warnings(
        &r,
        &format!("<div style=\"display:inline-block;width:100%\"><p>{w}</p></div>"),
    );
    assert!(alone.is_empty(), "{alone:?}");
    let hidden = unsplit_warnings(
        &r,
        &format!("<div style=\"overflow:hidden\"><p>{w}</p></div>"),
    );
    assert!(hidden.is_empty(), "{hidden:?}");
}
