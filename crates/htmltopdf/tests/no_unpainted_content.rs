//! No fixture loses content silently: every fixture renders with neither a "painted on no page" warning nor a
//! "content extends N pt past the bottom page edge" warning (F213d safety net, plan
//! docs/plans/2026-11-03-f213d-alone-inline-block-lost.md).
//!
//! The two warnings are the renderer's check that everything the fragmenter placed is painted on some page: the F111
//! bottom-edge check (text painted below the page) and the ledger of F213d (a paragraph's lines or an inline box that no
//! page paints). A fixture is a document we claim to render correctly, so a warning on one is a defect, not noise; an
//! entry in `ALLOWED` names the finding that owns it. The list is empty.
//!
//! Before the ledger exists the test is vacuous for the fixtures (nothing can say "painted on no page"), so it also
//! renders a document that is known to lose content (an inline-block alone on its line, taller than a page: Chrome
//! slices it, we print nothing of it) and requires that the net reports it. That sentinel is red until the net sees an
//! inline box's content, and keeps the test from passing for the wrong reason afterwards.
//!
//! One test only: the logger is global to the process.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use htmltopdf::{RenderOptions, Renderer};

/// Fixtures that are allowed to warn, with the finding that owns the loss. Empty on purpose.
const ALLOWED: &[(&str, &str)] = &[];

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

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn fixtures() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/invoices"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("index.html").exists())
        .collect();
    dirs.sort();
    dirs
}

/// The renderer `fixtures.rs` builds: Inter, the images next to each fixture, strict fonts and images; the compiled
/// Tailwind sheet only for a fixture that links it.
fn renderer(tailwind: bool) -> Renderer {
    let root = root();
    let mut b = Renderer::builder().strict_fonts(true).strict_images(true);
    if tailwind {
        b = b
            .css_file(root.join("fixtures/tailwind/tailwind.css"))
            .expect("tailwind.css");
    }
    let mut fonts: Vec<PathBuf> = std::fs::read_dir(root.join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    assert!(!fonts.is_empty(), "no fonts — run scripts/fetch-fonts.sh");
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    let mut images = BTreeMap::new();
    for dir in fixtures() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "png" || x == "jpg"))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            images.insert(name, std::fs::read(&f).unwrap());
        }
    }
    for (name, bytes) in images {
        b = b.image(name, bytes);
    }
    b.build()
}

fn net_warnings() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
        .into_iter()
        .filter(|m| m.contains("painted on no page") || m.contains("past the bottom page edge"))
        .collect()
}

#[test]
fn no_fixture_has_content_that_is_painted_on_no_page() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);

    let (with, without) = (renderer(true), renderer(false));
    let mut failures = Vec::new();
    for dir in fixtures() {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let r = if html.contains("tailwind.css") {
            &with
        } else {
            &without
        };
        std::mem::take(&mut *LOG.lock().unwrap());
        r.render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let w = net_warnings();
        if !w.is_empty() && !ALLOWED.iter().any(|(n, _)| *n == name) {
            failures.push(format!("{name}: {w:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "fixtures with content painted on no page or below the page edge:\n{}",
        failures.join("\n")
    );

    // The sentinel: a box alone on its line that is taller than a page is not printed (F213d residual; Chrome slices it).
    // The net must say so, or the loop above proves nothing about inline boxes.
    std::mem::take(&mut *LOG.lock().unwrap());
    let html = "<!doctype html><html><head><style>@page{size:A4;margin:15mm}\
        body{font:14px/20px Inter;margin:0}p{margin:0}</style></head><body>\
        <p><span style=\"display:inline-block;width:150px;height:1200px;border:1px solid #000\">\
        <span style=\"display:block\">BOXTOP</span></span></p><p>AFTER</p></body></html>";
    without
        .render(html, &RenderOptions::default())
        .expect("render");
    let w = net_warnings();
    assert!(
        !w.is_empty(),
        "an inline-block alone on its line, taller than a page, is printed nowhere and the net is silent"
    );
}
