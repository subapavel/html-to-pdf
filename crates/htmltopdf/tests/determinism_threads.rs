//! F99a: the output of a document must not depend on which documents the process rendered before it, nor on how
//! many threads shape its text (AGENTS.md goal 3, determinism).
//!
//! Blitz shapes inline layouts on rayon workers when its `parallel-construct` feature is on. Each worker keeps a
//! thread-local Parley `FontContext`; if that context outlives the document it was cloned from, the next document
//! the worker shapes sees the previous document's fonts and generic-family map. The references below therefore
//! come from child processes (this test binary re-run on one scenario), where "rendered first" is true by
//! construction and not by the order the test harness happens to run the tests in.
//!
//! Documents are large enough (hundreds of inline layouts) that every worker of a 2-4 thread pool takes part.
//!
//! F99b adds two `@page` margin-box scenarios (M1, M2). The margin-box documents of one render share a style and
//! font environment (the Blitz `SharedStyles` and a font template, both owned by that render's `MarginBoxRenderer`).
//! These tests are the guard that the environment of one render never reaches another render in the same process:
//! the two scenarios differ in the `--css` sheet, in the bundled fonts and in the margin boxes' own `font-family`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use htmltopdf::{GenericFamily, RenderOptions, Renderer};

const CHILD_ENV: &str = "HTMLTOPDF_F99A_CHILD";
const CHILD_TEST: &str = "zz_child_process";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn font(name: &str) -> Vec<u8> {
    let path = root().join("fixtures/fonts").join(name);
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// Inter Regular only; `monospace` is the built-in JetBrains Mono.
fn renderer_a() -> Renderer {
    Renderer::builder()
        .font_bytes(font("Inter-Regular.ttf"))
        .unwrap()
        .strict_fonts(true)
        .build()
}

/// Inter in two more weights than A has; `monospace` is the built-in JetBrains Mono.
fn renderer_b() -> Renderer {
    Renderer::builder()
        .font_bytes(font("Inter-Regular.ttf"))
        .unwrap()
        .font_bytes(font("Inter-Bold.ttf"))
        .unwrap()
        .font_bytes(font("Inter-SemiBold.ttf"))
        .unwrap()
        .strict_fonts(true)
        .build()
}

/// All Inter weights, `monospace` mapped to Inter (F70 A2).
fn renderer_g1() -> Renderer {
    all_inter()
        .generic_family(GenericFamily::Monospace, "Inter")
        .strict_fonts(true)
        .build()
}

/// All Inter weights, `monospace` left at the built-in JetBrains Mono.
fn renderer_g2() -> Renderer {
    all_inter().strict_fonts(true).build()
}

fn all_inter() -> htmltopdf::RendererBuilder {
    let mut b = Renderer::builder();
    for w in ["Regular", "Medium", "SemiBold", "Bold", "ExtraBold"] {
        b = b.font_bytes(font(&format!("Inter-{w}.ttf"))).unwrap();
    }
    b
}

/// 400 inline layouts: plain and bold prose. A face-less `<b>` in A is Regular; in B it is Inter Bold.
fn doc_a() -> String {
    let mut s = String::from("<!doctype html><html><body>");
    for i in 0..400 {
        s.push_str(&format!(
            "<p>Alfa {i} <b>tučný řádek číslo {i}</b> a obyčejný text faktury, Kč {}</p>",
            i * 7
        ));
    }
    s.push_str("</body></html>");
    s
}

/// 400 inline layouts of prose, bold prose, `<pre>` and `<code>` (the UA `monospace`).
fn doc_b() -> String {
    let mut s = String::from("<!doctype html><html><body>");
    for i in 0..400 {
        s.push_str(&format!(
            "<p>Bravo {i} <b>tučný řádek číslo {i}</b></p>\
             <pre>IBAN CZ65 0300 0000 0001 2345 6789 / {i}</pre>\
             <p>VS <code>2026{i:04}</code> a <span style=\"font-weight:600\">polotučný</span> text</p>"
        ));
    }
    s.push_str("</body></html>");
    s
}

/// M1: Inter Regular only; the `--css` sheet spaces letters of every `div`, margin boxes included.
fn renderer_m1() -> Renderer {
    Renderer::builder()
        .font_bytes(font("Inter-Regular.ttf"))
        .unwrap()
        .css("div { letter-spacing: .5px }")
        .strict_fonts(true)
        .build()
}

/// M2: Inter Regular and Bold; the `--css` sheet spaces words and makes every `div` bold.
fn renderer_m2() -> Renderer {
    Renderer::builder()
        .font_bytes(font("Inter-Regular.ttf"))
        .unwrap()
        .font_bytes(font("Inter-Bold.ttf"))
        .unwrap()
        .css("div { word-spacing: 4px; font-weight: 700 }")
        .strict_fonts(true)
        .build()
}

/// 36 pages, each with a running header (`element()`), a `counter(page)` / `counter(pages)` footer and a constant
/// footer box. `left_font` is the `font-family` of the `@bottom-left` box.
fn doc_margin(tag: &str, left_font: &str) -> String {
    let mut s = format!(
        "<!doctype html><html><head><style>\
         @page {{ size: A4; margin: 20mm 15mm 25mm 15mm; \
           @top-center {{ content: element(hdr); }} \
           @bottom-right {{ content: \"Strana \" counter(page) \" / \" counter(pages); font-size: 9pt; }} \
           @bottom-left {{ content: \"Faktura {tag} · Příkladná integrace s.r.o.\"; font-family: {left_font}; font-size: 8pt; }} \
         }} \
         .hdr {{ position: running(hdr); }} \
         .pg {{ break-after: page; }}\
         </style></head><body>\
         <div class=\"hdr\">Daňový doklad {tag} <b>č. 2026-001</b></div>"
    );
    for i in 0..36 {
        s.push_str(&format!(
            "<p class=\"pg\">Stránka {i}: položka {tag}-{i}, Kč {} včetně DPH</p>",
            i * 137
        ));
    }
    s.push_str("</body></html>");
    s
}

fn doc_m1() -> String {
    doc_margin("M1", "Inter")
}

fn doc_m2() -> String {
    doc_margin("M2", "\"JetBrains Mono\"")
}

fn render(r: &Renderer, html: &str) -> Vec<u8> {
    r.render(html, &RenderOptions::default()).expect("render")
}

fn has_jetbrains(pdf: &[u8]) -> bool {
    pdf.windows(13).any(|w| w == b"JetBrainsMono")
}

fn scenario(name: &str) -> (Renderer, String) {
    match name {
        "a" => (renderer_a(), doc_a()),
        "b" => (renderer_b(), doc_b()),
        "g1" => (renderer_g1(), doc_b()),
        "g2" => (renderer_g2(), doc_b()),
        "m1" => (renderer_m1(), doc_m1()),
        "m2" => (renderer_m2(), doc_m2()),
        other => panic!("unknown scenario {other}"),
    }
}

fn fixture_renderer() -> Renderer {
    let root = root();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .strict_images(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .unwrap();
    for w in ["Regular", "Medium", "SemiBold", "Bold", "ExtraBold"] {
        b = b.font_bytes(font(&format!("Inter-{w}.ttf"))).unwrap();
    }
    let mut images = BTreeMap::new();
    for dir in fixture_dirs() {
        for e in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "png" || x == "jpg") {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                images.insert(name, std::fs::read(&p).unwrap());
            }
        }
    }
    for (name, bytes) in images {
        b = b.image(name, bytes);
    }
    b.build()
}

fn fixture_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/invoices"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("index.html").is_file())
        .collect();
    v.sort();
    v
}

/// The child half: renders one scenario into the file named by the environment. A plain `cargo test` run
/// finds no variable and returns at once.
#[test]
fn zz_child_process() {
    let Ok(spec) = std::env::var(CHILD_ENV) else {
        return;
    };
    let (what, out) = spec.split_once('=').expect("scenario=path");
    if what == "fixtures" {
        let r = fixture_renderer();
        let opts = RenderOptions {
            lang: Some("cs".into()),
            ..Default::default()
        };
        std::fs::create_dir_all(out).unwrap();
        for dir in fixture_dirs() {
            let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
            let pdf = r.render(&html, &opts).expect("fixture render");
            let name = dir.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::write(Path::new(out).join(format!("{name}.pdf")), pdf).unwrap();
        }
    } else {
        let (r, html) = scenario(what);
        std::fs::write(out, render(&r, &html)).unwrap();
    }
}

fn run_child(what: &str, out: &Path, rayon_threads: Option<u32>) {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", CHILD_TEST, "--test-threads=1", "--nocapture"])
        .env(CHILD_ENV, format!("{what}={}", out.display()));
    if let Some(n) = rayon_threads {
        cmd.env("RAYON_NUM_THREADS", n.to_string());
    }
    let o = cmd.output().expect("spawn the test binary");
    assert!(
        o.status.success(),
        "child `{what}` failed:\n{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("htmltopdf-f99a-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// What each scenario renders to as the first and only document of a fresh process.
fn references() -> &'static BTreeMap<&'static str, Vec<u8>> {
    static REFS: OnceLock<BTreeMap<&'static str, Vec<u8>>> = OnceLock::new();
    REFS.get_or_init(|| {
        let dir = scratch("refs");
        let mut m = BTreeMap::new();
        for name in ["a", "b", "g1", "g2"] {
            let out = dir.join(format!("{name}.pdf"));
            run_child(name, &out, None);
            m.insert(name, std::fs::read(&out).unwrap());
        }
        let _ = std::fs::remove_dir_all(&dir);
        m
    })
}

/// Same for the margin-box scenarios; separate so the F99a tests do not pay for two more child processes.
fn margin_references() -> &'static BTreeMap<&'static str, Vec<u8>> {
    static REFS: OnceLock<BTreeMap<&'static str, Vec<u8>>> = OnceLock::new();
    REFS.get_or_init(|| {
        let dir = scratch("margin-refs");
        let mut m = BTreeMap::new();
        for name in ["m1", "m2"] {
            let out = dir.join(format!("{name}.pdf"));
            run_child(name, &out, None);
            m.insert(name, std::fs::read(&out).unwrap());
        }
        let _ = std::fs::remove_dir_all(&dir);
        m
    })
}

/// Fail with the first differing byte and, for the monospace scenarios, which mono face was used.
fn assert_same(what: &str, got: &[u8], want: &[u8]) {
    if got == want {
        return;
    }
    let at = got.iter().zip(want).position(|(a, b)| a != b);
    panic!(
        "{what}: differs from a fresh process ({} vs {} bytes, first difference at {at:?}; \
         JetBrainsMono embedded: got {}, want {})",
        got.len(),
        want.len(),
        has_jetbrains(got),
        has_jetbrains(want)
    );
}

/// Sanity: the scenarios must actually tell a contaminated render from a clean one, otherwise the tests below
/// prove nothing.
#[test]
fn scenarios_differ_from_each_other() {
    let r = references();
    assert_ne!(r["g1"], r["g2"], "the generic-family map changes no output");
    assert!(!has_jetbrains(&r["g1"]), "G1 maps monospace to Inter");
    assert!(has_jetbrains(&r["g2"]), "G2 uses the built-in mono face");
}

#[test]
fn document_does_not_see_the_fonts_of_earlier_documents() {
    let refs = references();
    let (ra, rb) = (renderer_a(), renderer_b());
    let (ha, hb) = (doc_a(), doc_b());
    assert_same("A first", &render(&ra, &ha), &refs["a"]);
    assert_same("B after A", &render(&rb, &hb), &refs["b"]);
    assert_same("B again", &render(&rb, &hb), &refs["b"]);
    assert_same("A after B", &render(&ra, &ha), &refs["a"]);
}

#[test]
fn document_does_not_see_the_generic_families_of_earlier_documents() {
    let refs = references();
    let (r1, r2) = (renderer_g1(), renderer_g2());
    let h = doc_b();
    assert_same("G1 first", &render(&r1, &h), &refs["g1"]);
    assert_same("G2 after G1", &render(&r2, &h), &refs["g2"]);
    assert_same("G1 after G2", &render(&r1, &h), &refs["g1"]);
    assert_same("G2 again", &render(&r2, &h), &refs["g2"]);
}

#[test]
fn concurrent_renders_are_identical() {
    let refs = references();
    let (ra, rb) = (renderer_a(), renderer_b());
    let (ha, hb) = (doc_a(), doc_b());
    // Four threads, all B.
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4).map(|_| s.spawn(|| render(&rb, &hb))).collect();
        for h in hs {
            assert_same("B on 4 threads", &h.join().unwrap(), &refs["b"]);
        }
    });
    // Two A and two B at the same time: the workers serve both documents.
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|i| {
                let (r, h, key) = if i % 2 == 0 {
                    (&ra, &ha, "a")
                } else {
                    (&rb, &hb, "b")
                };
                (key, s.spawn(move || render(r, h)))
            })
            .collect();
        for (key, h) in hs {
            assert_same(
                &format!("{key} next to the other"),
                &h.join().unwrap(),
                &refs[key],
            );
        }
    });
}

#[test]
fn fixtures_are_identical_at_one_and_four_rayon_threads() {
    let dir = scratch("fixtures");
    let (one, four) = (dir.join("t1"), dir.join("t4"));
    run_child("fixtures", &one, Some(1));
    run_child("fixtures", &four, Some(4));
    let mut n = 0;
    for e in std::fs::read_dir(&one).unwrap().filter_map(|e| e.ok()) {
        let name = e.file_name();
        let a = std::fs::read(e.path()).unwrap();
        let b = std::fs::read(four.join(&name)).unwrap();
        assert_same(&format!("{name:?} at 4 threads"), &b, &a);
        n += 1;
    }
    assert!(n >= 16, "expected the 16 fixtures, rendered {n}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// F99b: the margin-box documents of one render share one environment. A second render, with another `--css`
/// sheet, other fonts and another margin-box font, must not see any of it (and a repeat must not see its own
/// earlier self).
#[test]
fn margin_boxes_do_not_see_the_environment_of_earlier_renders() {
    let refs = margin_references();
    let (r1, r2) = (renderer_m1(), renderer_m2());
    let (h1, h2) = (doc_m1(), doc_m2());
    assert_same("M1 first", &render(&r1, &h1), &refs["m1"]);
    assert_same("M2 after M1", &render(&r2, &h2), &refs["m2"]);
    assert_same("M2 again", &render(&r2, &h2), &refs["m2"]);
    assert_same("M1 after M2", &render(&r1, &h1), &refs["m1"]);
}

#[test]
fn margin_boxes_are_identical_when_rendered_concurrently() {
    let refs = margin_references();
    let (r1, r2) = (renderer_m1(), renderer_m2());
    let (h1, h2) = (doc_m1(), doc_m2());
    // Two M1 and two M2 at the same time, from four threads.
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|i| {
                let (r, h, key) = if i % 2 == 0 {
                    (&r1, &h1, "m1")
                } else {
                    (&r2, &h2, "m2")
                };
                (key, s.spawn(move || render(r, h)))
            })
            .collect();
        for (key, h) in hs {
            assert_same(
                &format!("{key} next to the other"),
                &h.join().unwrap(),
                &refs[key],
            );
        }
    });
}

/// Sanity: the margin scenarios tell a leaked environment from a clean one, and they really have margin boxes
/// on every page.
#[test]
fn margin_scenarios_differ() {
    let r = margin_references();
    assert_ne!(r["m1"], r["m2"], "the margin scenarios render the same");
    for key in ["m1", "m2"] {
        let pages = r[key].windows(11).filter(|w| *w == b"/Type /Page").count();
        let pages_alt = r[key].windows(10).filter(|w| *w == b"/Type/Page").count();
        assert!(
            pages.max(pages_alt) >= 36,
            "{key}: expected 36 page objects, found {} / {}",
            pages,
            pages_alt
        );
    }
    assert!(
        has_jetbrains(&r["m2"]) && !has_jetbrains(&r["m1"]),
        "M2's @bottom-left uses JetBrains Mono, M1's does not"
    );
}
