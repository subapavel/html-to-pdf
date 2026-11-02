//! `visibility: hidden` on inline content paints nothing and keeps its space (F209, F209a).
//!
//! CSS 2.1 §11.2: a `hidden` box is invisible but still affects layout, and a `visibility: visible` descendant of it is
//! shown. fulgur read `visibility` per block and per paragraph only, so `<span style="visibility:hidden">hid</span>`
//! printed `hid`, the spacer idiom `.s::after{content:'0000';visibility:hidden}` printed `0000`, and a hidden `<div>` with a
//! visible `<span>` printed nothing. The hidden words were also in the text layer, in the link annotations and in the `/T` of
//! a tagged heading, which repeated the whole heading once per glyph run (F209a).
//!
//! Every Chrome number is m141 (A4, 15 mm, Inter 14px/20px; `out/scratch/f209/`); the text layer's coordinates are pt, so the
//! test converts no units (ink is read from a 72 dpi raster, where one pixel is one pt). Two oracles per case:
//! (a) the words in order with x and y within 1 pt of Chrome;
//! (b) the same body with the `visibility` removed has every printed word at the same x and y within 0.01 pt, which proves
//!     the advance is kept (a word that moved would have lost the hidden text's width).
//! Plan: `docs/plans/2026-11-02-f209-visibility-hidden.md`.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = concat!(
    "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p,ul{margin:0;padding:0}h2{margin:0;font:14px/20px Inter}",
    "table{margin:0;padding:0;border-spacing:0}td{padding:0}",
    ".vh{visibility:hidden}.vv{visibility:visible}.col{visibility:collapse}",
    ".t::before{content:'tx'}.hp::after{content:'ZZ';visibility:visible}",
    ".sp::before{content:'tx';visibility:hidden}.sp2::after{content:'0000';visibility:hidden}",
    ".ib{display:inline-block;width:30px;height:12px;background:#000;border:2px solid #f00}",
    ".ibt{display:inline-block;border:1px solid #00f}",
    ".pib::before{content:'';display:inline-block;width:20px;height:12px;background:#000}",
);

/// Appended for the oracle (b) render: nothing is hidden.
const SHOW_ALL: &str = ".vh,.col{visibility:visible}.sp::before,.sp2::after{visibility:visible}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn page(body: &str, show_all: bool) -> String {
    let extra = if show_all { SHOW_ALL } else { "" };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{HEAD}{extra}</style></head><body>{body}</body></html>"
    )
}

/// A 1x1 black PNG, registered as `dot.png` (a data URI is not a bundled image and draws nothing).
const DOT: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x7e, 0x9b,
    0x55, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x01, 0x48, 0xaf, 0xa4, 0x71, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn renderer() -> Renderer {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .image("dot.png", DOT.to_vec())
        .strict_fonts(true)
        .build()
}

fn render(body: &str) -> Vec<u8> {
    renderer()
        .render(&page(body, false), &RenderOptions::default())
        .unwrap_or_else(|e| panic!("render of {body:?}: {e}"))
}

fn render_tagged(body: &str) -> Vec<u8> {
    renderer()
        .render(
            &page(body, false),
            &RenderOptions {
                tagged: true,
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("tagged render of {body:?}: {e}"))
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    text: String,
}

fn tmp(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("htmltopdf-vh-{}-{n}-{name}", std::process::id()))
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = tmp("words.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .args(["-bbox"])
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(name))
            .and_then(|v| v.trim_start_matches('=').trim_matches('"').parse().ok())
            .unwrap_or_else(|| panic!("word without {name}"))
    };
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push(Word {
                page,
                x_min: attr(attrs, "xMin"),
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn words_of(body: &str) -> Vec<Word> {
    printed_words(&render(body))
}

/// The words of the same body with nothing hidden: oracle (b).
fn words_shown(body: &str) -> Vec<Word> {
    printed_words(
        &renderer()
            .render(&page(body, true), &RenderOptions::default())
            .unwrap_or_else(|e| panic!("render of {body:?}: {e}")),
    )
}

fn describe(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| format!("{}@{:.1},{:.1}", w.text, w.x_min, w.y_min))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One expected word: text, x, y (Chrome's).
type Expect = (&'static str, f32, f32);

/// Oracle (a): the words in order, x and y within 1 pt of Chrome's.
fn assert_like_chrome(what: &str, words: &[Word], expect: &[Expect]) {
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    let want: Vec<&str> = expect.iter().map(|e| e.0).collect();
    assert_eq!(
        texts,
        want,
        "{what}: the printed words, ours: {}",
        describe(words)
    );
    for (w, (text, x, y)) in words.iter().zip(expect) {
        assert_eq!(w.page, 1, "{what}: `{text}` is on page 1");
        assert!(
            (w.x_min - x).abs() <= 1.0 && (w.y_min - y).abs() <= 1.0,
            "{what}: `{text}` at {:.1},{:.1}, Chrome puts it at {x},{y} ({})",
            w.x_min,
            w.y_min,
            describe(words)
        );
    }
}

/// Oracle (b): every printed word is where the same word is when nothing is hidden, within 0.01 pt. A word may be a
/// prefix of the shown one (`q` of `q0000`: pdftotext joins adjacent runs).
fn assert_advance_kept(what: &str, printed: &[Word], shown: &[Word]) {
    for w in printed {
        assert!(
            shown.iter().any(|s| s.text.starts_with(&w.text)
                && s.page == w.page
                && (s.x_min - w.x_min).abs() <= 0.01
                && (s.y_min - w.y_min).abs() <= 0.01),
            "{what}: `{}` at {:.2},{:.2} is not where it is when nothing is hidden: {}",
            w.text,
            w.x_min,
            w.y_min,
            describe(shown)
        );
    }
}

/// Both oracles for one body.
fn check(what: &str, body: &str, chrome: &[Expect]) {
    let got = words_of(body);
    assert_like_chrome(what, &got, chrome);
    assert_advance_kept(what, &got, &words_shown(body));
}

/// Number of pixels darker than 200 (of 255) inside `[x0, x1) x [y0, y1)` of the first page, rasterised at 72 dpi, where one
/// pixel is one pt (so the rectangle is in pt and nothing is converted).
fn ink_in(pdf: &[u8], x0: usize, y0: usize, x1: usize, y1: usize) -> usize {
    let (input, out) = (tmp("ink.pdf"), tmp("ink"));
    std::fs::write(&input, pdf).unwrap();
    let status = std::process::Command::new("pdftoppm")
        .args(["-r", "72", "-gray", "-singlefile", "-f", "1", "-l", "1"])
        .arg(&input)
        .arg(&out)
        .status()
        .expect("pdftoppm (poppler-utils) is required for this test");
    assert!(status.success(), "pdftoppm failed");
    let pgm_path = out.with_file_name(format!(
        "{}.pgm",
        out.file_name().unwrap().to_string_lossy()
    ));
    let pgm = std::fs::read(&pgm_path).unwrap();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&pgm_path);
    // P5 <w> <h> 255 <bytes>
    let mut fields = Vec::new();
    let mut pos = 0;
    while fields.len() < 4 {
        while pgm[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        while !pgm[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(String::from_utf8_lossy(&pgm[start..pos]).into_owned());
    }
    pos += 1;
    assert_eq!(fields[0], "P5");
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let data = &pgm[pos..];
    let mut n = 0;
    for y in y0..y1.min(h) {
        for x in x0..x1.min(w) {
            if data[y * w + x] < 200 {
                n += 1;
            }
        }
    }
    n
}

fn tmp_pdf(pdf: &[u8]) -> std::path::PathBuf {
    let p = tmp("in.pdf");
    std::fs::write(&p, pdf).unwrap();
    p
}

/// `qpdf --check` is clean; returns the file in QDF form (objects uncompressed, one key per line).
fn check_and_qdf(pdf: &[u8]) -> String {
    let input = tmp_pdf(pdf);
    let check = std::process::Command::new("qpdf")
        .arg("--check")
        .arg(&input)
        .output()
        .expect("qpdf is required for this test and was not found in PATH");
    assert!(
        check.status.success(),
        "qpdf --check: {}{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let qdf = tmp("qdf.pdf");
    let status = std::process::Command::new("qpdf")
        .args(["--qdf", "--object-streams=disable"])
        .arg(&input)
        .arg(&qdf)
        .status()
        .expect("qpdf");
    assert!(status.success(), "qpdf --qdf failed");
    let text = String::from_utf8_lossy(&std::fs::read(&qdf).unwrap()).into_owned();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&qdf);
    text
}

/// The `/Rect` of every `/Link` annotation, as `[x0, y0, x1, y1]` in PDF pt.
fn link_rects(qdf: &str) -> Vec<[f32; 4]> {
    let mut found = Vec::new();
    for obj in qdf.split("\nendobj") {
        if !obj.contains("/Subtype /Link") {
            continue;
        }
        let at = obj.find("/Rect [").expect("a link without /Rect") + "/Rect [".len();
        let nums: Vec<f32> = obj[at..]
            .split(']')
            .next()
            .unwrap()
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        found.push([nums[0], nums[1], nums[2], nums[3]]);
    }
    found
}

/// Every `/T` string of the structure tree, decoded (a literal string or UTF-16BE hex).
fn titles(qdf: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in qdf.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix("/T ") else {
            continue;
        };
        if let Some(lit) = rest.strip_prefix('(') {
            found.push(lit.trim_end_matches(')').to_string());
        } else if let Some(hex) = rest.strip_prefix('<') {
            let hex = hex.trim_end_matches('>');
            let units: Vec<u16> = (0..hex.len() / 4)
                .map(|i| u16::from_str_radix(&hex[i * 4..i * 4 + 4], 16).unwrap())
                .collect();
            // Skip the byte order mark.
            found.push(String::from_utf16(&units[1..]).unwrap());
        }
    }
    found
}

const AFTER: Expect = ("AFTER", 42.7, 58.8);

#[test]
fn a_hidden_span_prints_nothing_and_keeps_its_space() {
    // (what, body, Chrome's words)
    let cases: [(&str, &str, Vec<Expect>); 4] = [
        (
            "span in a span, with a pseudo",
            "<div><span>a <span class='t vh'>hid</span> w</span></div><p>AFTER</p>",
            vec![("a", 42.7, 43.8), ("w", 78.9, 43.8), AFTER],
        ),
        (
            "hidden outer span",
            "<div><span class=vh>a <span class='t'>hid</span> w</span></div><p>AFTER</p>",
            vec![AFTER],
        ),
        (
            "plain hidden span",
            "<div>a <span class=vh>hid</span> w</div><p>AFTER</p>",
            vec![("a", 42.7, 43.8), ("w", 69.7, 43.8), AFTER],
        ),
        (
            "collapse on a span is hidden",
            "<div>a <span class=col>hid</span> w</div><p>AFTER</p>",
            vec![("a", 42.7, 43.8), ("w", 69.7, 43.8), AFTER],
        ),
    ];
    for (what, body, chrome) in cases {
        check(what, body, &chrome);
    }
}

#[test]
fn a_hidden_spacer_pseudo_keeps_its_space() {
    // `a` `w q` `e`, with gaps where `tx` and `0000` would be.
    let want = [
        ("a", 42.7, 43.8),
        ("w", 63.7, 43.8),
        ("q", 75.3, 43.8),
        ("e", 111.2, 43.8),
    ];
    check(
        "depth 1",
        "<div>a <span class=sp></span> w <span class=sp2>q</span> e</div>",
        &want,
    );
    check(
        "depth 2",
        "<div><span><span>a <span class=sp></span> w <span class=sp2>q</span> e</span></span></div>",
        &want,
    );
}

#[test]
fn a_visible_descendant_of_a_hidden_inline_is_printed() {
    let cases: [(&str, &str, Vec<Expect>, bool); 4] = [
        (
            "visible span with a pseudo in a hidden span",
            "<div><span class=vh>a <span class='t vv'>vis</span> w</span></div><p>AFTER</p>",
            vec![("txvis", 51.6, 43.8), AFTER],
            true,
        ),
        (
            "visible span in a hidden div (the inline root)",
            "<div class=vh>a <span class=vv>vis</span> w</div><p>AFTER</p>",
            vec![("vis", 51.6, 43.8), AFTER],
            true,
        ),
        (
            "visible ::after of a hidden p",
            "<p class='vh hp'>hidden text</p><p>AFTER</p>",
            vec![("ZZ", 98.2, 43.8), AFTER],
            // pdftotext joins `textZZ` into one word when nothing is hidden: no word to compare.
            false,
        ),
        (
            "visible block child of a hidden div with loose text",
            "<div class=vh>loose <p class=vv>blockvis</p> loose2</div><p>AFTER</p>",
            vec![("blockvis", 42.7, 58.8), ("AFTER", 42.7, 88.8)],
            true,
        ),
    ];
    for (what, body, chrome, advance) in cases {
        let got = words_of(body);
        assert_like_chrome(what, &got, &chrome);
        if advance {
            assert_advance_kept(what, &got, &words_shown(body));
        }
    }
}

#[test]
fn hidden_links_have_no_annotation() {
    // A hidden `<a>`: no text and no annotation (Chrome the same).
    let body = "<div>x <a class=vh href='https://e.cz/'>online</a> y</div><p>AFTER</p>";
    let pdf = render(body);
    assert_like_chrome(
        "hidden a",
        &printed_words(&pdf),
        &[("x", 42.7, 43.8), ("y", 84.3, 43.8), AFTER],
    );
    assert_eq!(link_rects(&check_and_qdf(&pdf)).len(), 0, "hidden a");
    assert_advance_kept("hidden a", &printed_words(&pdf), &words_shown(body));

    // A visible `<a>` whose last word is hidden: the annotation covers the visible run. Chrome writes two rectangles that
    // together cover the hidden word as well (a clickable blank), which css-display-3 §4 does not ask for: not copied.
    let body = "<div><a href='https://e.cz/'>Zobrazit <span class=vh>online</span></a> y</div><p>AFTER</p>";
    let pdf = render(body);
    assert_like_chrome(
        "partly hidden a",
        &printed_words(&pdf),
        &[("Zobrazit", 42.7, 43.8), AFTER, ("y", 119.5, 43.8)],
    );
    assert_advance_kept("partly hidden a", &printed_words(&pdf), &words_shown(body));
    let rects = link_rects(&check_and_qdf(&pdf));
    assert_eq!(rects.len(), 1, "one annotation: {rects:?}");
    // Chrome's first rectangle, `Zobrazit ` with its space, is 42.8 to 87.0.
    assert!(
        (rects[0][0] - 42.8).abs() <= 1.0 && (rects[0][2] - 87.0).abs() <= 1.0,
        "the annotation covers `Zobrazit ` only: {rects:?}"
    );

    // Only a hidden run in a visible `<a>`: nothing to click.
    let body =
        "<div>x <a href='https://e.cz/'><span class=vh>online</span></a> y</div><p>AFTER</p>";
    let pdf = render(body);
    assert_eq!(
        link_rects(&check_and_qdf(&pdf)).len(),
        0,
        "only hidden text"
    );
}

#[test]
fn boxes_in_a_hidden_span() {
    // (what, hidden content, the x range between `a` and the next word in which the box would be)
    let cases = [
        ("inline-block with background", "<span class=ib></span>", 79),
        (
            "inline-block with border and text",
            "<span class=ibt>boxtext</span>",
            93,
        ),
        ("img", "<img width=30 height=12 src='dot.png'>", 76),
        (
            "inline svg",
            "<svg width=30 height=12><rect width=30 height=12 fill='#000'/></svg>",
            76,
        ),
    ];
    for (what, hidden, w_x) in cases {
        let body = format!("<div>a <span class=vh>{hidden}</span> w</div>");
        let pdf = render(&body);
        // The box is where it is when shown (the control: the region is the right one), and absent when hidden.
        let shown = renderer()
            .render(&page(&body, true), &RenderOptions::default())
            .unwrap();
        assert!(
            ink_in(&shown, 50, 30, w_x - 1, 80) > 0,
            "{what}: no ink where the box is when it is shown"
        );
        assert_eq!(ink_in(&pdf, 50, 30, w_x - 1, 80), 0, "{what}: ink");
        let got = printed_words(&pdf);
        assert_eq!(
            got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
            ["a", "w"],
            "{what}: {}",
            describe(&got)
        );
        assert_advance_kept(what, &got, &words_shown(&body));
    }
    // A generated inline-block (`content:''`) in a hidden span.
    let body = "<div>a <span class='vh pib'></span> w</div>";
    let pdf = render(body);
    assert_eq!(ink_in(&pdf, 50, 30, 68, 80), 0, "pseudo inline-block: ink");
    assert_advance_kept(
        "pseudo inline-block",
        &printed_words(&pdf),
        &words_shown(body),
    );
    // A visible inline-block in a hidden span is shown (already so on main).
    let body = "<div>a <span class=vh><span class='ibt vv'>vischild</span></span> w</div>";
    let got = words_of(body);
    assert_eq!(
        got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        ["a", "vischild", "w"],
        "{}",
        describe(&got)
    );
    // A `visibility: visible` child of a hidden inline-block is printed (CSS 2.1 §11.2); Chrome: vischild at x 70.5. The
    // inline-block's own y differs from Chrome's in ours (a separate gap), so x and the order are compared.
    let body = "<div>a <span class=vh><span class=ibt>hid <span class=vv>vischild</span> hid2</span></span> w</div>";
    let got = words_of(body);
    assert_eq!(
        got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        ["a", "vischild", "w"],
        "{}",
        describe(&got)
    );
    assert!((got[1].x_min - 70.5).abs() <= 1.0, "{}", describe(&got));
    assert!((got[2].x_min - 136.5).abs() <= 1.0, "{}", describe(&got));
    assert_advance_kept(
        "visible child of a hidden inline-block",
        &got,
        &words_shown(body),
    );
}

#[test]
fn decorations_follow_the_text() {
    // Chrome draws no underline under hidden text, only under the visible `a ` and ` c` of the decorating box. Main already
    // has the gap here: its underline is per run and the hidden span's run has none of its own.
    let body = "<div><u>a <span class=vh>bb</span> c</u></div>";
    let pdf = render(body);
    assert_eq!(
        ink_in(&pdf, 52, 30, 64, 70),
        0,
        "nothing under the hidden word"
    );
    assert!(ink_in(&pdf, 42, 30, 49, 70) > 0, "`a` and its underline");
    // A hidden `<u>`: the glyphs are skipped and so is the underline that is drawn with them.
    let body = "<div>a <span class=vh><u>bb</u></span> c</div>";
    let pdf = render(body);
    assert_eq!(
        ink_in(&pdf, 52, 30, 64, 70),
        0,
        "no underline of a hidden u"
    );
    // A visible `<u>` in a hidden span keeps its underline.
    let body = "<div><span class=vh>a <u class=vv>bb</u> c</span></div>";
    let pdf = render(body);
    assert!(
        ink_in(&pdf, 50, 30, 66, 70) > 0,
        "the visible u's underline"
    );
}

#[test]
fn tagged_hidden_text() {
    // Tagged renders of the cases above are valid and hold none of the hidden words in the text layer.
    let cases = [
        (
            "<div>a <span class=vh>hid</span> w</div><p>AFTER</p>",
            vec!["hid"],
        ),
        (
            "<div class=vh>a <span class=vv>vis</span> w</div><p>AFTER</p>",
            vec!["a", "w"],
        ),
        (
            "<div>x <a class=vh href='https://e.cz/'>online</a> y</div><p>AFTER</p>",
            vec!["online"],
        ),
        (
            "<div><a href='https://e.cz/'>Zobrazit <span class=vh>online</span></a> y</div><p>AFTER</p>",
            vec!["online"],
        ),
    ];
    for (body, hidden) in cases {
        let pdf = render_tagged(body);
        check_and_qdf(&pdf);
        let words: Vec<String> = printed_words(&pdf).into_iter().map(|w| w.text).collect();
        for h in hidden {
            assert!(!words.iter().any(|w| w == h), "{body}: `{h}` in {words:?}");
        }
    }
    // The `/T` of a heading holds the visible words, once.
    let pdf = render_tagged("<h2>Faktura <span class=vh>KONCEPT</span></h2><p>AFTER</p>");
    let t = titles(&check_and_qdf(&pdf));
    assert_eq!(
        t.iter().map(|s| s.trim()).collect::<Vec<_>>(),
        ["Faktura"],
        "{t:?}"
    );
}

/// F209a: a heading with several glyph runs repeated its whole text once per run in `/T` (main: three runs, three copies).
#[test]
fn a_heading_title_holds_its_text_once() {
    let pdf = render_tagged(
        "<h2>Faktura <span style='color:#c00'>č. 42</span> za rok</h2><h2>Plain heading</h2><p>AFTER</p>",
    );
    let t = titles(&check_and_qdf(&pdf));
    assert_eq!(t, ["Faktura č. 42 za rok", "Plain heading"], "{t:?}");
}

#[test]
fn guards_that_already_hold() {
    // A hidden `li` prints no text and no marker; a visible `li` in a hidden `ul` prints both.
    let got = words_of("<ul><li class=vh>hidden li</li><li>shown li</li></ul>");
    assert_eq!(
        got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        ["•", "shown", "li"],
        "{}",
        describe(&got)
    );
    let got = words_of("<ul class=vh><li class=vv>shown li</li><li>hidden li</li></ul>");
    assert_eq!(
        got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        ["•", "shown", "li"],
        "{}",
        describe(&got)
    );
    // Hidden cells and rows print no text; a collapsed row neither (its space is F209b).
    for body in [
        "<table><tr><td class=vh>hid</td><td>shown</td></tr></table>",
        "<table><tr class=vh><td>hid</td><td>hid2</td></tr><tr><td>shown</td></tr></table>",
        "<table><tr class=col><td>hid</td><td>hid2</td></tr><tr><td>shown</td></tr></table>",
    ] {
        let got = words_of(body);
        assert_eq!(
            got.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
            ["shown"],
            "{body}: {}",
            describe(&got)
        );
    }
    // A hidden block paints no background, its visible block child does (the first band is the hidden div, the second
    // the visible child; both 15 pt high from y 42.5).
    let pdf = render(
        "<div class=vh style='background:#f88;height:20px'></div><div class=vh style='background:#f88;height:20px'>\
         <div class=vv style='background:#6c6;height:20px'>child</div></div>",
    );
    assert_eq!(ink_in(&pdf, 100, 43, 400, 57), 0, "hidden block background");
    assert!(
        ink_in(&pdf, 100, 58, 400, 72) > 0,
        "visible child of a hidden block"
    );
    // `opacity: 0` text stays in the text layer: a different rule (Chrome leaves it out; F211).
    let got = words_of("<div>a <span style='opacity:0'>zero</span> w</div>");
    assert!(got.iter().any(|w| w.text == "zero"), "{}", describe(&got));
}
