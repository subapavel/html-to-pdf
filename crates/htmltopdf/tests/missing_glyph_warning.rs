//! A character that its font has no glyph for is drawn as `.notdef` and is missing from the text layer (F101, F44).
//! The engine reports it: one `log::warn!` per (character, font family), or `Error::MissingGlyphs` with
//! `strict_fonts(true)`. Plan: docs/plans/2026-11-04-f101-missing-glyph-warning.md (tests G1-G7, fixture gaps).
//!
//! Step 1 of the plan: the tests marked `ignore` are red until the fulgur fork reports `.notdef` (run them with
//! `--ignored` to see them fail); the others pass today and guard against false positives and byte changes.
//!
//! The logger is global to the process, so every test that renders holds `SERIAL`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once};

use htmltopdf::{Error, MissingGlyph, RenderOptions, Renderer};

#[path = "common/glyph_gaps.rs"]
mod glyph_gaps;

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static SERIAL: Mutex<()> = Mutex::new(());
static INIT: Once = Once::new();

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

/// Serialises the tests of this binary, installs the logger once and empties the log.
fn serial() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    INIT.call_once(|| {
        log::set_boxed_logger(Box::new(Capture)).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    take();
    guard
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

/// The warnings about a character without a glyph (the plan's text: `font family "Inter" has no glyph for U+21C4 "⇄"; ...`
/// and `... and N more characters with no glyph in their font`).
fn glyph_warnings() -> Vec<String> {
    take()
        .into_iter()
        .filter(|m| m.contains("no glyph"))
        .collect()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn inter(weight: &str) -> Vec<u8> {
    let path = root().join(format!("fixtures/fonts/Inter-{weight}.ttf"));
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// Inter Regular and Bold, as the plan's tests use.
fn renderer(strict: bool) -> Renderer {
    Renderer::builder()
        .font_bytes(inter("Regular"))
        .unwrap()
        .font_bytes(inter("Bold"))
        .unwrap()
        .strict_fonts(strict)
        .build()
}

fn render(html: &str, strict: bool) -> Result<Vec<u8>, Error> {
    renderer(strict).render(html, &RenderOptions::default())
}

fn entry(c: char) -> MissingGlyph {
    MissingGlyph {
        character: c,
        font_family: "Inter".into(),
    }
}

/// The entries of `Err(MissingGlyphs)`; anything else fails the test.
fn missing(r: Result<Vec<u8>, Error>) -> Vec<MissingGlyph> {
    match r {
        Err(Error::MissingGlyphs(v)) => v,
        other => panic!(
            "expected Error::MissingGlyphs, got {:?}",
            other.map(|pdf| format!("a PDF of {} bytes", pdf.len()))
        ),
    }
}

fn code_point(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

/// Seven CJK letters, an emoji and `⇄` three times (once in `<b>`); every Latin letter is in Inter.
const G1: &str = "<!doctype html><html><body>\
    <p>Odběratel: 株式会社サクラ, Ōsaka</p>\
    <table><tr><td>Cestovné – Trutnov ⇄ Berlin</td></tr>\
    <tr><td>Zpět ⇄ <b>Berlín ⇄ Trutnov</b></td></tr></table>\
    <p>Děkujeme za nákup 🙂</p></body></html>";

/// What G1 must report, in drawing order.
const G1_CHARS: [char; 9] = [
    '\u{682A}',
    '\u{5F0F}',
    '\u{4F1A}',
    '\u{793E}',
    '\u{30B5}',
    '\u{30AF}',
    '\u{30E9}',
    '\u{21C4}',
    '\u{1F642}',
];

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn g1_strict_names_every_missing_character_once_in_drawing_order() {
    let _g = serial();
    let found = missing(render(G1, true));
    let want: Vec<MissingGlyph> = G1_CHARS.iter().copied().map(entry).collect();
    assert_eq!(found, want);
    // No Czech letter and no `Ō`: Inter maps them.
    assert!(found.iter().all(|m| m.character as u32 > 0x200));
    assert!(found.iter().all(|m| m.character != '\u{14C}'));
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn g2_without_strict_it_is_one_warning_per_character() {
    let _g = serial();
    let pdf = render(G1, false).expect("a missing glyph is a warning unless strict");
    assert!(pdf.starts_with(b"%PDF-"));
    let w = glyph_warnings();
    assert_eq!(w.len(), G1_CHARS.len(), "{w:#?}");
    for (msg, c) in w.iter().zip(G1_CHARS) {
        for want in [code_point(c), format!("\"{c}\""), "Inter".to_string()] {
            assert!(msg.contains(&want), "`{want}` is missing from {msg:?}");
        }
    }
}

/// Everything here is drawn without a glyph of its own or not drawn at all, and none of it is a lost character.
/// Green today: the false-positive guard for step 2. (A tab in `<pre>` is drawn as `.notdef` today, F71; reporting it
/// would fail every strict `<pre>`, so it is a non-goal of F101.)
#[test]
fn g3_whitespace_ignorables_hidden_and_clipped_text_report_nothing() {
    let _g = serial();
    let words = (0..60)
        .map(|i| format!("word{i:02}"))
        .collect::<Vec<_>>()
        .join(" ");
    let html = format!(
        "<!doctype html><html><head><style>body{{font:16px/20px Inter}}p,pre{{margin:0}}</style></head><body>\
         <p>Faktura 2026 &nbsp;nbsp, zero&#8203;width, soft&#173;hyphen</p>\
         <p>line one<br>line two<br><br>line four</p>\
         <pre>first\n\ttabbed\nlast</pre>\
         <p>visible <span style=\"visibility:hidden\">株</span> text</p>\
         <div style=\"width:200px;height:20px;overflow:hidden\"><p>{words} 株</p></div>\
         </body></html>"
    );
    render(&html, true).expect("nothing is reported here, strict or not");
    assert!(glyph_warnings().is_empty());
    render(&html, false).unwrap();
    assert!(glyph_warnings().is_empty());
}

/// The control of G3: the same clipped paragraph without `overflow:hidden` does draw `株`, so G3 proves the clip.
#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn g3_control_the_same_line_unclipped_is_reported() {
    let _g = serial();
    let words = (0..60)
        .map(|i| format!("word{i:02}"))
        .collect::<Vec<_>>()
        .join(" ");
    let html = format!(
        "<!doctype html><html><head><style>body{{font:16px/20px Inter}}p{{margin:0}}</style></head><body>\
         <div style=\"width:200px\"><p>{words} 株</p></div></body></html>"
    );
    assert_eq!(missing(render(&html, true)), vec![entry('株')]);
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn g4_margin_box_text_is_reported() {
    // F70's convert-time check does not see margin boxes; the draw-time check does.
    let _g = serial();
    let html = "<!doctype html><html><head><style>@page { @bottom-center { content: \"株\" } }</style></head>\
                <body><p>Faktura</p></body></html>";
    assert_eq!(missing(render(html, true)), vec![entry('株')]);
}

/// The plan's G5 was `list-style-type: "株 "` on an inside marker. Blitz ignores a string `list-style-type` (the item
/// gets its disc), and every marker fulgur can emit (disc, circle, square, disclosure-*, the numbering systems and
/// the `□` fallback of the ones it does not know) is mapped by Inter or the built-in JetBrains Mono, so with the
/// bundled faces no marker reaches `.notdef`. What remains to guard is the false positive: markers report nothing.
/// Green today. The F44 marker case is not reproducible until a marker can name a character (see the postscript of
/// the plan's §4).
#[test]
fn g5_inside_markers_report_nothing() {
    let _g = serial();
    for style in [
        "disc",
        "circle",
        "square",
        "disclosure-open",
        "disclosure-closed",
        "decimal",
        "lower-greek",
        "cjk-decimal",
        "thai",
        "\"株 \"",
    ] {
        let html = format!(
            "<!doctype html><html><head><style>li{{list-style-position:inside;list-style-type:{style}}}\
             </style></head><body><ul><li>item</li></ul><ol><li>x</li><li>y</li></ol></body></html>"
        );
        render(&html, true).unwrap_or_else(|e| panic!("list-style-type: {style}: {e}"));
        assert!(glyph_warnings().is_empty(), "list-style-type: {style}");
    }
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn g6_twenty_five_characters_are_twenty_warnings_and_a_summary() {
    let _g = serial();
    let chars: String = (0..25u32)
        .map(|i| char::from_u32(0x4E00 + i).unwrap())
        .collect();
    let html = format!("<!doctype html><html><body><p>{chars}</p></body></html>");
    let found = missing(render(&html, true));
    assert_eq!(found.len(), 25, "the vector holds all of them");
    let msg = Error::MissingGlyphs(found).to_string();
    assert!(
        msg.contains("U+4E13 ") && !msg.contains("U+4E14 "),
        "the message names 20: {msg}"
    );
    assert!(msg.ends_with("; ... and 5 more"), "{msg}");

    take();
    render(&html, false).expect("a warning unless strict");
    let w = glyph_warnings();
    assert_eq!(w.len(), 21, "{w:#?}");
    assert!(
        w[..20].iter().all(|m| m.contains("has no glyph for U+")),
        "{w:#?}"
    );
    assert!(
        w[20].contains("and 5 more characters with no glyph in their font"),
        "{:?}",
        w[20]
    );
}

/// Text in a family nothing bundled matches gets no glyph run at all, so it is `TextWithoutFont` (F70), never
/// `MissingGlyphs`. `--f` is `'Nope'` where `<p>` is, so the static `font-family` scan cannot refuse it first.
/// Green today and after.
#[test]
fn g7_text_without_a_run_is_text_without_font_not_a_missing_glyph() {
    let _g = serial();
    let html = "<!doctype html><html><head><style>\
        :root{--f:'Nope'} .x{--f:Inter} p{font-family:var(--f)}\
        </style></head><body><p>株式会社</p><p class=\"x\">Faktura</p></body></html>";
    match render(html, true) {
        Err(Error::TextWithoutFont(found)) => {
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].contains("株式会社"), "{found:?}");
        }
        other => panic!(
            "expected TextWithoutFont, got {:?}",
            other.map(|pdf| pdf.len())
        ),
    }
    assert!(glyph_warnings().is_empty());
}

// -- fixtures/glyph-gaps.txt ---------------------------------------------------------------------

/// A fixture rendered as `scripts/render-fixtures.sh` does: its Tailwind sheet if it links one, every Inter weight,
/// the images next to its `index.html`.
fn fixture_renderer(dir: &Path, html: &str, strict: bool) -> Renderer {
    let root = root();
    let mut b = Renderer::builder().strict_fonts(strict).strict_images(true);
    if html.contains("tailwind.css") {
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
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    let mut images: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png" || x == "jpg"))
        .collect();
    images.sort();
    for f in images {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        b = b.image(name, std::fs::read(&f).unwrap());
    }
    b.build()
}

fn render_fixture(name: &str, strict: bool) -> Result<Vec<u8>, Error> {
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    fixture_renderer(&dir, &html, strict).render(
        &html,
        &RenderOptions {
            lang: Some("cs".into()),
            ..Default::default()
        },
    )
}

/// Every entry names an existing fixture whose `index.html` contains the character: a stale line fails.
#[test]
fn every_listed_gap_is_in_its_fixture() {
    let listed = glyph_gaps::listed();
    assert!(!listed.is_empty());
    for (name, chars) in listed {
        let html = std::fs::read_to_string(
            root()
                .join("fixtures/invoices")
                .join(&name)
                .join("index.html"),
        )
        .unwrap_or_else(|e| panic!("glyph-gaps.txt lists `{name}`: {e}"));
        for c in chars {
            assert!(
                html.contains(c),
                "{name}: index.html does not contain {}",
                code_point(c)
            );
        }
    }
}

/// The byte gate of the plan: reporting a gap never changes the PDF, so a listed fixture renders to the same bytes
/// with and without `strict_fonts`. Green today (strict has nothing to say) and after (strict says it after drawing).
/// With strict reporting on, the strict render is an `Err`, so this compares the lenient render with the CLI's
/// stored output instead: see step 4 of the plan; here it holds while strict still returns `Ok`.
#[test]
fn a_listed_fixture_renders_the_same_bytes_strict_or_not_while_strict_passes() {
    let _g = serial();
    for name in glyph_gaps::listed().keys() {
        let lenient = render_fixture(name, false).unwrap();
        if let Ok(strict) = render_fixture(name, true) {
            assert_eq!(strict, lenient, "{name}");
        }
        assert_eq!(lenient, render_fixture(name, false).unwrap(), "{name}");
    }
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn fixture_04_strict_render_returns_exactly_u21c4_in_inter() {
    // Also proves 04 passes every earlier strict check: the one error is the missing glyph.
    let _g = serial();
    assert_eq!(
        missing(render_fixture("04-bilingual-vat", true)),
        vec![entry('\u{21C4}')]
    );
}

/// A listed fixture is rendered without `strict_fonts` and must warn about exactly its listed code points, so a new
/// gap in it still fails.
#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn listed_fixtures_warn_about_exactly_their_listed_code_points() {
    let _g = serial();
    for (name, chars) in glyph_gaps::listed() {
        take();
        render_fixture(&name, false).unwrap_or_else(|e| panic!("{name}: {e}"));
        let warned: BTreeSet<String> = glyph_warnings()
            .iter()
            .flat_map(|m| {
                m.match_indices("U+")
                    .map(|(i, _)| m[i..i + 6].to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        let listed: BTreeSet<String> = chars.iter().copied().map(code_point).collect();
        assert_eq!(warned, listed, "{name}");
    }
}

/// The bench corpus documents that scripts/bench.py gates stay strict and report nothing. Green today; it is the
/// guard that no later change makes them lose a glyph.
#[test]
fn corpus_documents_have_no_missing_glyph() {
    let _g = serial();
    let root = root();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .unwrap();
    let mut fonts: Vec<PathBuf> = std::fs::read_dir(root.join("fixtures/fonts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    let r = b.build();
    for doc in ["b200-items", "b200-text", "b200-mixed"] {
        let html = std::fs::read_to_string(root.join(format!("bench/corpus/{doc}.html"))).unwrap();
        r.render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{doc}: {e}"));
        assert!(glyph_warnings().is_empty(), "{doc}");
    }
}
