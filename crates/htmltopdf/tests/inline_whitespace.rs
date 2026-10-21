//! Whitespace at the inner edges of inline elements (F154, F159f). Plan:
//! docs/plans/2026-10-21-f154-span-edge-whitespace.md.
//!
//! CSS Text 3 §4.1.1 collapses a collapsible space that follows another one *across* element boundaries, so the first
//! space survives in the font and decoration of its own element: `a <span>b </span>d` prints `a b d`. Parley trimmed
//! the start and the end of every style span, so the space of `<b>Splatnost: </b>2. 11. 2026` was deleted and the
//! words were glued (`Splatnost:2.`). Spaces at the start and the end of a line, and around a forced break, are
//! removed (§4.1.3); NBSP is never collapsible.
//!
//! Every case renders A4 with the default 15 mm margins, Inter Regular and Bold bundled, strict fonts, `line-height:
//! 20px`. Two oracles:
//!
//! - flat: an unstyled element must not move anything, the boxes of the words equal those of the same text written
//!   without the edge whitespace in the element (`FLAT_PT`);
//! - Chrome 148 with the same two Inter files loaded through `@font-face` (the probe files of plan §7): the gaps
//!   between words (the next `xMin` minus the previous `xMax`, `CHROME_PT`) and, for alignment, the absolute `x`.
//!
//! Positions come from `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

/// An element without edge whitespace of its own against the flat text.
const FLAT_PT: f32 = 0.05;
/// Against Chrome: its layout units are 1/64 px, ours are floats.
const CHROME_PT: f32 = 0.3;

const CSS: &str = "body { margin: 0; font-family: Inter; line-height: 20px } p { margin: 0 } \
    ul, ol { margin: 0 } li { line-height: 20px }";

fn font(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(name);
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn renderer(css: &str) -> Renderer {
    Renderer::builder()
        .font_bytes(font("Inter-Regular.ttf"))
        .expect("Inter Regular")
        .font_bytes(font("Inter-Bold.ttf"))
        .expect("Inter Bold")
        .strict_fonts(true)
        .css(format!("{CSS} {css}"))
        .build()
}

fn try_render(css: &str, body: &str) -> htmltopdf::Result<Vec<u8>> {
    let html = format!("<!doctype html><html><body>{body}</body></html>");
    renderer(css).render(&html, &RenderOptions::default())
}

fn render(css: &str, body: &str) -> Vec<u8> {
    try_render(css, body).expect("render")
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-iw-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn pdftotext_bbox(pdf: &[u8]) -> String {
    let path = temp(pdf);
    let out = std::process::Command::new("pdftotext")
        .args(["-bbox"])
        .arg(&path)
        .arg("-")
        .output()
        .unwrap_or_else(|e| panic!("pdftotext (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every word of the PDF in extraction order (page by page, top to bottom).
fn words(pdf: &[u8]) -> Vec<Word> {
    let text = pdftotext_bbox(pdf);
    let mut page = 0;
    let mut found = Vec::new();
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(&format!("{name}=\"")))
            .and_then(|v| v.trim_end_matches('"').parse().ok())
            .unwrap_or_else(|| panic!("word without {name}"))
    };
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push(Word {
                page,
                x0: attr(attrs, "xMin"),
                y0: attr(attrs, "yMin"),
                x1: attr(attrs, "xMax"),
                y1: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn texts(words: &[Word]) -> Vec<&str> {
    words.iter().map(|w| w.text.as_str()).collect()
}

/// The words grouped into lines: a new line starts when `y0` moves by more than 2 pt (page changes start one too).
fn line_texts(words: &[Word]) -> Vec<String> {
    let mut out: Vec<Vec<&Word>> = Vec::new();
    for w in words {
        match out.last_mut() {
            Some(line) if line[0].page == w.page && (line[0].y0 - w.y0).abs() <= 2.0 => {
                line.push(w)
            }
            _ => out.push(vec![w]),
        }
    }
    out.iter()
        .map(|l| {
            l.iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// The flat oracle: `pdf` has the same words as `flat`, in the same boxes (`FLAT_PT`).
fn assert_same_boxes(what: &str, pdf: &[u8], flat: &[u8]) {
    let (wa, wb) = (words(pdf), words(flat));
    assert_eq!(texts(&wa), texts(&wb), "{what}: the words differ");
    for (x, y) in wa.iter().zip(&wb) {
        let near = |p: f32, q: f32| (p - q).abs() <= FLAT_PT;
        assert!(
            x.page == y.page
                && near(x.x0, y.x0)
                && near(x.y0, y.y0)
                && near(x.x1, y.x1)
                && near(x.y1, y.y1),
            "{what}: {} moved: with the element {x:?}, flat {y:?}",
            x.text
        );
    }
}

/// Renders `body` and `flat` (the same text without the edge whitespace in the element) and compares them.
fn assert_flat(what: &str, css: &str, body: &str, flat: &str) -> Vec<u8> {
    let pdf = render(css, body);
    assert_same_boxes(what, &pdf, &render(css, flat));
    pdf
}

/// The words of one line are `expected`, the gap before each word after the first is `gaps[i]` (Chrome).
fn assert_gaps(what: &str, pdf: &[u8], expected: &[&str], gaps: &[f32]) {
    let found = words(pdf);
    assert_eq!(texts(&found), expected, "{what}: words printed");
    assert_eq!(expected.len(), gaps.len() + 1, "{what}: test data");
    assert_eq!(line_texts(&found).len(), 1, "{what}: one line");
    for (i, gap) in gaps.iter().enumerate() {
        let got = found[i + 1].x0 - found[i].x1;
        assert!(
            (got - gap).abs() <= CHROME_PT,
            "{what}: the gap between {} and {} is {got:.2} pt, Chrome has {gap:.2} pt ({found:?})",
            found[i].text,
            found[i + 1].text
        );
    }
}

fn near_chrome(what: &str, got: f32, chrome: f32) {
    assert!(
        (got - chrome).abs() <= CHROME_PT,
        "{what}: {got:.2} pt, Chrome has {chrome:.2} pt"
    );
}

// --- W1-W3: the space inside the element, outside it ----------------------------------------------------------------

#[test]
fn w1_trailing_space_inside_a_span() {
    let pdf = assert_flat("W1", "", "<p>a <span>b </span>d</p>", "<p>a b d</p>");
    assert_gaps("W1", &pdf, &["a", "b", "d"], &[3.38, 3.38]);
}

#[test]
fn w2_leading_space_inside_a_span() {
    let pdf = assert_flat("W2", "", "<p>a<span> b</span> d</p>", "<p>a b d</p>");
    assert_gaps("W2", &pdf, &["a", "b", "d"], &[3.38, 3.38]);
}

#[test]
fn w3_nested_spans() {
    let pdf = assert_flat(
        "W3",
        "",
        "<p>a <span>b <i>c </i></span>d</p>",
        "<p>a b <i>c</i> d</p>",
    );
    assert_gaps("W3", &pdf, &["a", "b", "c", "d"], &[3.38, 3.38, 3.38]);
}

// --- W4: labels of an invoice ---------------------------------------------------------------------------------------

#[test]
fn w4_bold_label_with_its_space() {
    // The space belongs to the bold element: Chrome's gap is the bold space (2.85 pt), not the regular one.
    let pdf = render("", "<p><b>Splatnost: </b>2. 11. 2026</p>");
    assert_gaps(
        "W4a",
        &pdf,
        &["Splatnost:", "2.", "11.", "2026"],
        &[2.85, 3.38, 3.37],
    );
    let pdf = render("", "<p><strong>Celkem k úhradě: </strong>12 100 Kč</p>");
    assert_gaps(
        "W4b",
        &pdf,
        &["Celkem", "k", "úhradě:", "12", "100", "Kč"],
        &[2.84, 2.84, 2.84, 3.38, 3.37],
    );
}

// --- W5: two spaces become one --------------------------------------------------------------------------------------

#[test]
fn w5_a_space_after_a_space_collapses_across_the_boundary() {
    let pdf = assert_flat("W5a", "", "<p>a <span> b</span></p>", "<p>a b</p>");
    assert_gaps("W5a", &pdf, &["a", "b"], &[3.38]);
    // The first space survives, with the font size of its own element: a 30 px space.
    let pdf = render("", "<p>a<span style=\"font-size:30px\"> </span> b</p>");
    assert_gaps("W5b", &pdf, &["a", "b"], &[6.33]);
}

// --- W6: spaces at the start and the end of the line ----------------------------------------------------------------

#[test]
fn w6_start_and_end_of_the_line_do_not_take_part_in_alignment() {
    for (align, chrome_x) in [("right", 165.06), ("center", 103.90)] {
        let what = format!("W6 {align}");
        let css = format!("p {{ width: 200px; text-align: {align} }}");
        let pdf = assert_flat(
            &what,
            &css,
            "<p><span> a</span> b <span>c </span></p>",
            "<p>a b c</p>",
        );
        assert_gaps(&what, &pdf, &["a", "b", "c"], &[3.38, 3.38]);
        near_chrome(&what, words(&pdf)[0].x0, chrome_x);
    }
}

// --- W7: forced breaks ----------------------------------------------------------------------------------------------

#[test]
fn w7_spaces_around_a_forced_break_are_removed() {
    let pdf = render("", "<p>a <br> <span> b</span></p>");
    let found = words(&pdf);
    assert_eq!(line_texts(&found), ["a", "b"]);
    assert!((found[0].x0 - found[1].x0).abs() <= 0.01, "{found:?}");

    let pdf = render(
        "",
        "<p style=\"white-space:pre-line\">a <span>b </span>\n  c</p>",
    );
    let found = words(&pdf);
    assert_eq!(line_texts(&found), ["a b", "c"]);
    assert!((found[0].x0 - found[2].x0).abs() <= 0.01, "{found:?}");
    // The space before `b ` is Chrome's 3.38 pt; the one before the newline is gone.
    near_chrome("W7b", found[1].x0 - found[0].x1, 3.38);
}

#[test]
fn w7_the_space_before_a_br_does_not_move_centred_text() {
    // Chrome: `b` is 0.30 pt left of `a` (it is 0.61 pt wider).
    let css = "p { width: 200px; text-align: center }";
    let pdf = render(css, "<p>a <br>b</p>");
    let found = words(&pdf);
    assert_eq!(line_texts(&found), ["a", "b"]);
    near_chrome("X4", found[1].x0 - found[0].x0, -0.30);
    // The same when the space is the end of a span.
    let pdf = assert_flat("X2", "", "<p>a <span>b </span><br>c</p>", "<p>a b<br>c</p>");
    assert_eq!(line_texts(&words(&pdf)), ["a b", "c"]);
}

// --- W8: preserved spaces -------------------------------------------------------------------------------------------

#[test]
fn w8_a_collapsible_space_after_a_preserved_one_is_kept() {
    // `a ` + `  b  ` (pre) + ` c`: three spaces on each side of `b`.
    let pdf = render(
        "",
        "<p>a <span style=\"white-space:pre\">  b  </span> c</p>",
    );
    assert_gaps("W8", &pdf, &["a", "b", "c"], &[10.12, 10.12]);
}

// --- W9: inline-block -----------------------------------------------------------------------------------------------

#[test]
fn w9_inline_block_guard() {
    let pdf = render(
        "",
        "<p>a <span style=\"display:inline-block\">X</span> b</p>",
    );
    assert_gaps("W9", &pdf, &["a", "X", "b"], &[3.38, 3.38]);
}

// --- W10: generated content (F159f) ---------------------------------------------------------------------------------

#[test]
fn w10_spaces_in_generated_content() {
    let css = ".x::before { content: \"Pred \" } .x::after { content: \" Po\" }";
    let pdf = render(css, "<p class=\"x\">mid</p>");
    assert_gaps("W10a", &pdf, &["Pred", "mid", "Po"], &[3.38, 3.38]);
    let pdf = render(css, "<p>m <span class=\"x\">i</span> n</p>");
    assert_gaps(
        "W10b",
        &pdf,
        &["m", "Pred", "i", "Po", "n"],
        &[3.37, 3.38, 3.38, 3.38],
    );
}

// --- W11-W13: other roots of inline content -------------------------------------------------------------------------

#[test]
fn w11_display_contents_span_with_a_style() {
    let pdf = assert_flat(
        "W11",
        "",
        "<p>a <span style=\"display:contents;color:red\">b </span>d</p>",
        "<p>a b d</p>",
    );
    assert_gaps("W11", &pdf, &["a", "b", "d"], &[3.38, 3.38]);
}

#[test]
fn w12_link_text_keeps_its_space_and_the_annotation() {
    let pdf = assert_flat(
        "W12",
        "",
        "<p>Viz <a href=\"https://x.cz\">web </a>firmy</p>",
        "<p>Viz web firmy</p>",
    );
    assert_gaps("W12", &pdf, &["Viz", "web", "firmy"], &[3.38, 3.38]);
    assert!(
        pdf.windows(5).any(|w| w == b"/Link"),
        "no link annotation in the PDF"
    );
}

#[test]
fn w13_body_as_the_inline_root() {
    // F111: text directly in the body, no paragraph around it.
    let pdf = render("", "Faktura <b>č. </b>2026");
    assert_gaps("W13", &pdf, &["Faktura", "č.", "2026"], &[3.38, 2.48]);
}

// --- W14, W15: modes that look at the text --------------------------------------------------------------------------

#[test]
fn w14_nowrap() {
    let pdf = assert_flat(
        "W14",
        "",
        "<p style=\"white-space:nowrap\">a <span>b </span>d</p>",
        "<p style=\"white-space:nowrap\">a b d</p>",
    );
    assert_gaps("W14", &pdf, &["a", "b", "d"], &[3.38, 3.38]);
}

#[test]
fn w15_capitalize_after_collapsing() {
    let pdf = assert_flat(
        "W15",
        "",
        "<p style=\"text-transform:capitalize\">a <span>b </span>d</p>",
        "<p style=\"text-transform:capitalize\">a b d</p>",
    );
    assert_gaps("W15", &pdf, &["A", "B", "D"], &[3.38, 3.38]);
}

// --- W16: inside list marker ----------------------------------------------------------------------------------------

#[test]
fn w16_inside_marker_swallows_the_leading_spaces() {
    // Chrome draws the bullet as a shape and prints `x` at the same place with and without the leading spaces.
    let css = "ul { list-style-position: inside }";
    assert_flat(
        "W16",
        css,
        "<ul><li> <span> x</span></li></ul>",
        "<ul><li>x</li></ul>",
    );
}

// --- W17: NBSP is never collapsible ---------------------------------------------------------------------------------

#[test]
fn w17_no_break_spaces_survive() {
    // Two NBSPs, one of them at the end of the bold element: Chrome's gap is 6.22 pt. Parley's `trim_end` is a
    // Unicode trim and deleted the first.
    let pdf = render("", "<p><b>12&nbsp;</b>&nbsp;000</p>");
    assert_gaps("W17", &pdf, &["12", "000"], &[6.22]);
}

// --- decoration over a hanging space --------------------------------------------------------------------------------

#[test]
fn x3_wrapped_underlined_text_with_a_trailing_space() {
    // `bbb ` ends the second line and its space hangs. Chrome prints three lines.
    let pdf = render("", "<p style=\"width:60px\"><u>aaa bbb </u>ccc</p>");
    assert_eq!(line_texts(&words(&pdf)), ["aaa", "bbb", "ccc"]);
}
