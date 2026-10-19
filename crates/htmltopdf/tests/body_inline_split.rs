//! Text directly in `<body>` (no block around it) that is taller than a page splits at line boxes like the same text in
//! a `<div>` (F111). Blitz makes `<body>` itself the inline formatting root when all its in-flow children are
//! inline-level (CSS 2.1 §9.4.2), and the fragmenter used to look only at body's children: the whole text landed on
//! page 1, the lines below the page box were not printed, and nothing said so. Plan:
//! docs/plans/2026-10-19-f111-body-text-tail-lost.md.
//!
//! Every case: A4 with the default 15 mm margins, Inter bundled, strict fonts. For each shape of the plan (S1-S10):
//!
//! - every `LineNNN` / `wNNNN` is printed exactly once, in order;
//! - no word is below the content box, or more than 2 pt above its top;
//! - the page count equals the one of the same content wrapped in a `<div>` (the shape that always worked).
//!
//! Lines are forced with `<br>` and are 20 px high (`line-height`), so the number of lines per page does not depend on
//! font metrics. Positions come from `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt (see `nested_paragraph_split.rs`).
const SLACK_PT: f32 = 6.0;
/// A word may be this far above the content top (glyph box above the line box).
const ABOVE_TOP_PT: f32 = 2.0;

const CSS: &str = "body { margin: 0; font-family: Inter; line-height: 20px } p { margin: 0 } \
    td { padding: 0 } ul { margin: 0 } li { line-height: 20px }";

/// Where the content sits in the document.
#[derive(Clone, Copy)]
enum Form {
    /// `<html><body>content</body></html>`
    Body,
    /// `<html>content</html>`: html5ever makes the body.
    NoBodyTag,
    /// `content` alone, without a doctype.
    Bare,
}

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn renderer() -> htmltopdf::RendererBuilder {
    Renderer::builder()
        .font_bytes(inter())
        .expect("Inter")
        .strict_fonts(true)
}

fn html(form: Form, content: &str) -> String {
    match form {
        Form::Body => format!("<!doctype html><html><body>{content}</body></html>"),
        Form::NoBodyTag => format!("<!doctype html><html>{content}</html>"),
        Form::Bare => content.to_string(),
    }
}

fn render_html(extra_css: &str, document: &str) -> Vec<u8> {
    renderer()
        .css(format!("{CSS} {extra_css}"))
        .build()
        .render(document, &RenderOptions::default())
        .expect("render")
}

/// `Line001<br>Line002<br>…`: one line box per token.
fn lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("Line{i:03}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn expected_lines(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("Line{i:03}")).collect()
}

/// `w0001 w0002 …`: prose that wraps by itself, no `<br>`.
fn prose(n: usize) -> String {
    (1..=n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn expected_prose(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("w{i:04}")).collect()
}

/// A word printed in the PDF: 1-based page and its box in pt from the top left of the page.
#[derive(Debug, Clone)]
struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-bis-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> String {
    let path = temp(pdf);
    let mut cmd = std::process::Command::new(tool);
    cmd.args(args).arg(&path);
    if tool == "pdftotext" {
        cmd.arg("-");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{tool} (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every word of the PDF in extraction order (page by page, top to bottom).
fn all_words(pdf: &[u8]) -> Vec<Word> {
    let text = poppler("pdftotext", &["-bbox"], pdf);
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
                y_min: attr(attrs, "yMin"),
                y_max: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

/// The `LineNNN` and `wNNNN` tokens.
fn tokens(pdf: &[u8]) -> Vec<Word> {
    all_words(pdf)
        .into_iter()
        .filter(|w| {
            (w.text.starts_with("Line") && w.text.len() == 7)
                || (w.text.starts_with('w')
                    && w.text.len() == 5
                    && w.text[1..].chars().all(|c| c.is_ascii_digit()))
        })
        .collect()
}

fn page_count(pdf: &[u8]) -> usize {
    poppler("pdfinfo", &[], pdf)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// `expected` exactly once each, in this order, inside the content box. Returns the tokens.
fn assert_printed(pdf: &[u8], expected: &[String]) -> Vec<Word> {
    let found = tokens(pdf);
    let seen: Vec<&str> = found.iter().map(|w| w.text.as_str()).collect();
    let want: Vec<&str> = expected.iter().map(String::as_str).collect();
    let first_diff = seen.iter().zip(&want).position(|(a, b)| a != b);
    assert!(
        seen == want,
        "every token exactly once, in order: printed {} of {}, first difference at index {first_diff:?}, \
         last printed {:?}, {} pages",
        seen.len(),
        want.len(),
        seen.last(),
        page_count(pdf),
    );
    let bottom = PAGE_H_PT - MARGIN_PT + SLACK_PT;
    for w in &found {
        assert!(
            w.y_max <= bottom,
            "{} on page {} ends at {:.1} pt, below the content box ({bottom:.1} pt)",
            w.text,
            w.page,
            w.y_max
        );
        assert!(
            w.y_min >= MARGIN_PT - ABOVE_TOP_PT,
            "{} on page {} starts at {:.1} pt, above the content box ({MARGIN_PT:.1} pt)",
            w.text,
            w.page,
            w.y_min
        );
    }
    let pages: Vec<usize> = found.iter().map(|w| w.page).collect();
    assert!(pages.windows(2).all(|w| w[0] <= w[1]), "pages out of order");
    found
}

/// The shape of the plan: `content` in `form`, printed whole, and as many pages as the same content in a `<div>`.
/// Returns the PDF and the page count.
fn assert_like_a_div(
    form: Form,
    extra_css: &str,
    content: &str,
    expected: &[String],
) -> (Vec<u8>, usize) {
    let pdf = render_html(extra_css, &html(form, content));
    assert_printed(&pdf, expected);
    let pages = page_count(&pdf);
    let wrapped = render_html(extra_css, &html(form, &format!("<div>{content}</div>")));
    assert_printed(&wrapped, expected);
    assert_eq!(
        pages,
        page_count(&wrapped),
        "as many pages as the same content in a <div>"
    );
    (pdf, pages)
}

// --- S1-S4: the shapes that lost their tail ------------------------------------------------------------------------

#[test]
fn s1_br_lines_directly_in_body() {
    let (_, pages) = assert_like_a_div(Form::Body, "", &lines(200), &expected_lines(200));
    assert_eq!(pages, 4, "200 lines of 20 px on A4");
}

#[test]
fn s2_the_same_inside_an_inline_element() {
    for tag in ["span", "font", "b"] {
        let content = format!("<{tag}>{}</{tag}>", lines(200));
        let (_, pages) = assert_like_a_div(Form::Body, "", &content, &expected_lines(200));
        assert_eq!(pages, 4, "<{tag}>");
    }
}

#[test]
fn s3_no_body_tag() {
    for form in [Form::NoBodyTag, Form::Bare] {
        let (_, pages) = assert_like_a_div(form, "", &lines(200), &expected_lines(200));
        assert_eq!(pages, 4);
    }
}

#[test]
fn s4_prose_without_br() {
    let (_, pages) = assert_like_a_div(Form::Body, "", &prose(4000), &expected_prose(4000));
    assert!(pages >= 5, "{pages} pages for 4000 words");
}

// --- S5, S6: a body with a box of its own ---------------------------------------------------------------------------

#[test]
fn s5_body_padding_is_kept_on_every_page() {
    let css = "body { padding: 20px }";
    let (pdf, pages) = assert_like_a_div(Form::Body, css, &lines(200), &expected_lines(200));
    assert_eq!(pages, 5, "the padding takes 40 px of every page");
    // The first line of every page starts 20 px (15 pt) below the content top, like the first page's.
    let found = tokens(&pdf);
    let first_y = found[0].y_min;
    assert!(
        first_y >= MARGIN_PT + 15.0 - ABOVE_TOP_PT,
        "page 1 starts at {first_y:.1} pt, the padding is 15 pt"
    );
    for page in 2..=pages {
        let top = found
            .iter()
            .find(|w| w.page == page)
            .unwrap_or_else(|| panic!("no token on page {page}"))
            .y_min;
        assert!(
            (top - first_y).abs() <= 1.5,
            "page {page} starts at {top:.1} pt, page 1 at {first_y:.1} pt"
        );
    }
}

#[test]
fn s5_a_short_padded_body_prints_its_text() {
    let pdf = render_html("body { padding: 24px }", &html(Form::Body, "Dobrý den"));
    let text = all_words(&pdf)
        .into_iter()
        .map(|w| w.text)
        .collect::<Vec<_>>();
    assert_eq!(text, ["Dobrý", "den"]);
    assert_eq!(page_count(&pdf), 1);
}

#[test]
fn s6_body_background_keeps_the_text() {
    let css = "body { background: #fafafa }";
    let (_, pages) = assert_like_a_div(Form::Body, css, &lines(200), &expected_lines(200));
    assert_eq!(pages, 4);
    // Even a short body.
    let pdf = render_html(css, &html(Form::Body, "Dobrý den"));
    assert_eq!(all_words(&pdf).len(), 2, "both words of a short body");
}

// --- S9: a running footer before the text -----------------------------------------------------------------------

#[test]
fn s9_a_running_footer_stays_on_every_page() {
    let css =
        ".foot { position: running(foot) } @page { @bottom-center { content: element(foot) } }";
    let content = format!("<div class=\"foot\">FOOTERTXT</div>{}", lines(200));
    let pdf = render_html(css, &html(Form::Body, &content));
    assert_printed(&pdf, &expected_lines(200));
    let pages = page_count(&pdf);
    assert_eq!(pages, 4);
    let footers: Vec<usize> = all_words(&pdf)
        .iter()
        .filter(|w| w.text == "FOOTERTXT")
        .map(|w| w.page)
        .collect();
    assert_eq!(
        footers,
        (1..=pages).collect::<Vec<_>>(),
        "the footer once on every page"
    );
}

// --- an image on a line ---------------------------------------------------------------------------------------------

/// An opaque `w` × `h` RGB PNG of one colour (stored deflate blocks).
fn solid_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let row: Vec<u8> = std::iter::once(0)
        .chain(rgb.iter().copied().cycle().take(3 * w as usize))
        .collect();
    let raw = row.repeat(h as usize);
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// Images drawn on each page (`pdfimages -list`), index 0 = page 1.
fn images_per_page(pdf: &[u8], pages: usize) -> Vec<usize> {
    let text = poppler("pdfimages", &["-list"], pdf);
    let mut per = vec![0; pages];
    for line in text.lines().skip(2) {
        if let Some(p) = line
            .split_whitespace()
            .next()
            .and_then(|p| p.parse::<usize>().ok())
            && (1..=pages).contains(&p)
        {
            per[p - 1] += 1;
        }
    }
    per
}

#[test]
fn an_inline_image_lands_on_the_page_of_its_line() {
    let marker = "<img src=\"red.png\" width=\"10\" height=\"10\">";
    let content = (1..=200)
        .map(|i| {
            if i == 150 {
                format!("Line{i:03}{marker}")
            } else {
                format!("Line{i:03}")
            }
        })
        .collect::<Vec<_>>()
        .join("<br>");
    let pdf = renderer()
        .css(CSS)
        .image("red.png", solid_png(10, 10, [255, 0, 0]))
        .build()
        .render(&html(Form::Body, &content), &RenderOptions::default())
        .expect("render");
    let found = assert_printed(&pdf, &expected_lines(200));
    let pages = page_count(&pdf);
    let line_page = found.iter().find(|w| w.text == "Line150").unwrap().page;
    let mut want = vec![0; pages];
    want[line_page - 1] = 1;
    assert_eq!(
        images_per_page(&pdf, pages),
        want,
        "the image is on page {line_page}, with Line150"
    );
}

// --- guards: shapes that always worked, and must stay as they are -----------------------------------------------

#[test]
fn s7_text_block_text_makes_anonymous_blocks() {
    // Inline runs around a block are wrapped in anonymous blocks (CSS 2.1 §9.2.1.1): not an inline-root body.
    let content = format!("{}<p>x</p>{}", lines(120), {
        (121..=240)
            .map(|i| format!("Line{i:03}"))
            .collect::<Vec<_>>()
            .join("<br>")
    });
    let pdf = render_html("", &html(Form::Body, &content));
    let mut want = expected_lines(240);
    want.insert(120, "x".to_string());
    let seen: Vec<String> = all_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("Line") || w.text == "x")
        .map(|w| w.text)
        .collect();
    assert_eq!(seen, want);
}

#[test]
fn s8_direct_text_in_other_boxes() {
    let wrappers = [
        ("td", "<table><tr><td>{}</td></tr></table>"),
        ("li", "<ul><li>{}</li></ul>"),
        (
            "flex item",
            "<div style=\"display:flex\"><div>{}</div></div>",
        ),
        (
            "abspos",
            "<div style=\"position:absolute;left:0;top:0;width:100%\">{}</div>",
        ),
        ("nested div", "<div><div>{}</div></div>"),
    ];
    for (name, wrapper) in wrappers {
        let content = wrapper.replace("{}", &lines(200));
        let pdf = render_html("", &html(Form::Body, &content));
        let seen: Vec<String> = tokens(&pdf).into_iter().map(|w| w.text).collect();
        assert_eq!(seen, expected_lines(200), "{name}");
    }
    // A flex body: the text is an anonymous flex item.
    let pdf = render_html(
        "body { display: flex; flex-direction: column }",
        &html(Form::Body, &lines(200)),
    );
    let seen: Vec<String> = tokens(&pdf).into_iter().map(|w| w.text).collect();
    assert_eq!(seen, expected_lines(200), "flex body");
}

#[test]
fn s10_a_short_bare_body_is_one_page() {
    let pdf = render_html("", &html(Form::Body, &lines(10)));
    assert_printed(&pdf, &expected_lines(10));
    assert_eq!(page_count(&pdf), 1);
}
