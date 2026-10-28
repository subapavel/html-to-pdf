//! A clip box shorter than its content shows what it should hide, or writes it into the PDF, or loses what it should
//! show (F30f-b, the clip part).
//!
//! A box clips its block axis when its computed `overflow-y` is not `visible` (css-overflow-3 §3). Given a `height` or
//! `max-height` below its content, main did one of three things: it revealed the hidden lines (a body-level box with
//! direct text: 12 pages, 4000 words), it kept the `<p>` whole and wrote every glyph of it although the clip hides most
//! of them (372 words in the text layer, `pdftotext`, copy and search read text nobody sees), and it never advanced a
//! page for a `max-height` box taller than a page (701 words in Chrome, 372 here), nor put what follows the box at the
//! box's bottom.
//!
//! Spec: css-overflow-3 §3 (the clip is the padding box), css-break-3 §2 and §4.4 (a UA may treat `overflow: hidden` with a
//! definite height as monolithic; `max-height` must fragment; breaks between line boxes, orphans and widows 2),
//! CSS 2.1 §11.1.1. No spec says what a clipped line does to a PDF's text layer: Chrome writes none for a line box wholly
//! outside the clip and ours does the same (ADR-0009). Every number below is Chrome's (m141, A4, 15 mm, Inter, 20 px
//! lines, `W` = 4000 words of 7 per line), measured with `out/scratch/f30f-b/probe.mjs` (F30f-b plan §7).
//!
//! Every case puts `<p>AFTER</p>` after the box. AFTER is printed once, at the bottom of the box, within 2 pt of
//! Chrome's y.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}p{margin:0;line-height:20px}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_with(body: &str, options: RenderOptions) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &options,
        )
        .expect("render")
}

fn render(body: &str) -> Vec<u8> {
    render_with(body, RenderOptions::default())
}

/// Words `word<from> … word<to - 1>`.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The 4000 words.
fn w() -> String {
    words(0, 4000)
}

const AFTER: &str = "<p>AFTER</p>";

/// One printed word: its 1-based page, the top of its box in pt from the page top, and its text.
struct Word {
    page: usize,
    y_min: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-cb-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> String {
    let path = temp(pdf);
    let out = std::process::Command::new(tool)
        .args(args)
        .arg(&path)
        .arg("-")
        .output()
        .unwrap_or_else(|_| panic!("{tool} (poppler-utils) is required for this test"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let text = poppler("pdftotext", &["-bbox"], pdf);
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
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// The URLs of the link annotations, from `pdfinfo -url`.
fn link_urls(pdf: &[u8]) -> Vec<String> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfinfo")
        .arg("-url")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().last())
        .filter(|u| u.starts_with("http"))
        .map(str::to_string)
        .collect()
}

/// How many `wordNNNNN` are printed on each page, from page 1.
fn words_per_page(pdf: &[u8]) -> Vec<usize> {
    let mut per = vec![0; page_count(pdf)];
    for w in printed_words(pdf) {
        if w.text.starts_with("word") {
            per[w.page - 1] += 1;
        }
    }
    per
}

/// Chrome's numbers: `pages`, the words on each page (±7 each, one line), and where AFTER is printed (page, y in pt, ±2).
fn like_chrome(what: &str, pdf: &[u8], per_page: &[usize], after: (usize, f32)) {
    assert_eq!(
        words_per_page(pdf).len(),
        per_page.len(),
        "{what}: pages (Chrome {})",
        per_page.len()
    );
    let got = words_per_page(pdf);
    for (k, (g, c)) in got.iter().zip(per_page).enumerate() {
        assert!(
            g.abs_diff(*c) <= 7,
            "{what}: words on page {}: {g}, Chrome {c} (all pages {got:?} vs {per_page:?})",
            k + 1
        );
    }
    let afters: Vec<Word> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text == "AFTER")
        .collect();
    assert_eq!(afters.len(), 1, "{what}: AFTER is printed once");
    assert_eq!(afters[0].page, after.0, "{what}: AFTER on page (Chrome)");
    assert!(
        (afters[0].y_min - after.1).abs() <= 2.0,
        "{what}: AFTER at {:.1} pt, Chrome {:.1}",
        afters[0].y_min,
        after.1
    );
}

/// A box of `style` holding the 4000 words directly, then AFTER.
fn direct(style: &str) -> String {
    format!("<div style=\"{style}\">{}</div>{AFTER}", w())
}

/// A box of `style` holding one `<p>` of the 4000 words, then AFTER.
fn with_p(style: &str) -> String {
    format!("<div style=\"{style}\"><p>{}</p></div>{AFTER}", w())
}

// ── C1: R1, a clip box shorter than its content ─────────────────────────────────────────────────────────────────────

#[test]
fn a_body_level_clip_box_with_direct_text_shows_only_what_its_height_shows() {
    // R1: 12 pages and 4000 words before (the clip was cut at every page's own fragment, so it hid nothing). Chrome: 15
    // lines of 7 words, one page.
    let pdf = render(&direct("overflow:hidden;height:300px"));
    like_chrome("C1", &pdf, &[105], (1, 268.1));
}

#[test]
fn a_clip_box_with_a_paragraph_shows_only_what_its_height_shows() {
    // C1p: the text layer held 372 words and AFTER stood on page 2.
    let pdf = render(&with_p("overflow:hidden;height:300px"));
    like_chrome("C1p", &pdf, &[105], (1, 268.1));
}

#[test]
fn a_clip_box_with_a_nested_paragraph_shows_only_what_its_height_shows() {
    // C1n: box > div > p. Main revealed all 4000 words on 12 pages.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden;height:300px\"><div><p>{}</p></div></div>{AFTER}",
        w()
    ));
    like_chrome("C1n", &pdf, &[105], (1, 268.1));
}

// ── C2: R2, a flex clip box ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_flex_clip_box_writes_only_the_visible_lines() {
    // R2: the picture was right, the text layer held 372 words.
    let pdf = render(&with_p("display:flex;overflow:hidden;height:300px"));
    like_chrome("C2", &pdf, &[105], (1, 268.1));
}

// ── C3: R3, a clip box taller than a few lines but shorter than the page ────────────────────────────────────────────

#[test]
fn a_clip_box_of_600px_writes_only_the_visible_lines() {
    // R3: 372 words in the text layer and AFTER on page 2. Chrome: 30 lines (211 words; the 30th is cut by the box
    // edge: 600 / 20 = 30 lines of 7 words, one of them short).
    let pdf = render(&with_p("overflow:hidden;height:600px"));
    like_chrome("C3", &pdf, &[211], (1, 493.1));
}

// ── C4: R4, max-height ──────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_max_height_clip_box_taller_than_a_page_spans_two_pages() {
    // R4: 372 words on one page and AFTER on page 2 before. Chrome fragments a `max-height` box (css-break-3 §2) and
    // clips each fragment: 2000 px = 1009 + 991.
    let pdf = render(&with_p("overflow:hidden;max-height:2000px"));
    like_chrome("C4", &pdf, &[351, 350], (2, 785.6));
}

#[test]
fn a_max_height_clip_box_with_direct_text_spans_two_pages() {
    // C4t: main revealed all 4000 words on 12 pages.
    let pdf = render(&direct("overflow:hidden;max-height:2000px"));
    like_chrome("C4t", &pdf, &[351, 350], (2, 785.6));
}

// ── C5, C6: a box taller than a page, a box that straddles the page bottom ──────────────────────────────────────────

#[test]
fn a_clip_box_of_3000px_spans_three_pages() {
    let pdf = render(&direct("overflow:hidden;height:3000px"));
    like_chrome("C5 direct", &pdf, &[351, 350, 344], (3, 778.1));
    let pdf = render(&with_p("overflow:hidden;height:3000px"));
    like_chrome("C5 p", &pdf, &[351, 350, 344], (3, 778.1));
}

#[test]
fn a_clip_box_that_straddles_the_page_bottom_is_fragmented() {
    // The box starts 900 px down: 5 lines on page 1 (109 px), 10 on page 2 (the box's remaining 191 px, the last line cut).
    let spacer = "<div style=\"height:900px\"></div>";
    let pdf = render(&format!(
        "{spacer}{}",
        direct("overflow:hidden;height:300px")
    ));
    like_chrome("C6 direct", &pdf, &[35, 70], (2, 185.6));
    let pdf = render(&format!(
        "{spacer}{}",
        with_p("overflow:hidden;height:300px")
    ));
    like_chrome("C6 p", &pdf, &[35, 70], (2, 185.6));
}

// ── C7: the other clipping values ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn clip_auto_and_scroll_boxes_behave_like_hidden_ones() {
    // Chrome fragments `auto` and `scroll` boxes in print like `hidden` ones and writes the same words.
    for value in ["clip", "auto", "scroll"] {
        let pdf = render(&direct(&format!("overflow:{value};height:300px")));
        like_chrome(&format!("C7 {value} direct"), &pdf, &[105], (1, 268.1));
        let pdf = render(&with_p(&format!("overflow:{value};height:600px")));
        like_chrome(&format!("C7 {value} p"), &pdf, &[211], (1, 493.1));
        let pdf = render(&with_p(&format!("overflow:{value};max-height:2000px")));
        like_chrome(&format!("C7 {value} max"), &pdf, &[351, 350], (2, 785.6));
    }
}

// ── C8: which axis counts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn overflow_y_alone_clips_the_block_axis() {
    for style in ["overflow-y:hidden", "overflow-y:clip", "overflow-x:hidden"] {
        // `overflow-x: hidden` computes `overflow-y: visible` to `auto` (css-overflow-3 §3).
        let pdf = render(&direct(&format!("{style};height:300px")));
        like_chrome(&format!("C8 {style}"), &pdf, &[105], (1, 268.1));
    }
}

#[test]
fn overflow_x_clip_alone_hides_nothing_in_the_block_axis() {
    // `overflow-x: clip` leaves `overflow-y` visible: the text runs on over 12 pages, every word once. (Chrome puts AFTER
    // under the 300 px box, on top of the overflowing text of page 1; ours puts it below the text.)
    let pdf = render(&direct("overflow-x:clip;height:300px"));
    assert_eq!(page_count(&pdf), 12, "Chrome: 12 pages");
    let printed: Vec<String> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .map(|w| w.text)
        .collect();
    let expected: Vec<String> = (0..4000).map(|i| format!("word{i:05}")).collect();
    assert_eq!(printed, expected, "every word once, in order");
}

// ── C9: a partly visible line ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_partly_visible_line_is_written() {
    // 310 px: the 16th line starts at 300 px and 10 px of it show. Chrome writes it (112 words = 16 lines).
    let pdf = render(&direct("overflow:hidden;height:310px"));
    like_chrome("C9 direct", &pdf, &[112], (1, 275.6));
    let pdf = render(&with_p("overflow:hidden;height:310px"));
    like_chrome("C9 p", &pdf, &[112], (1, 275.6));
}

// ── C10: a link in the hidden part ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_link_in_the_hidden_part_is_not_written() {
    let body = format!(
        "<div style=\"overflow:hidden;height:300px\"><p>{} <a href=\"https://example.com/visible\">VISLINK</a> {} \
         <a href=\"https://example.com/hidden\">HIDLINK</a> {}</p></div>{AFTER}",
        words(0, 56),
        words(56, 200),
        words(200, 300)
    );
    let pdf = render(&body);
    let printed: Vec<String> = printed_words(&pdf).into_iter().map(|w| w.text).collect();
    assert!(
        printed.iter().any(|t| t == "VISLINK"),
        "the visible link text is written"
    );
    assert!(
        !printed.iter().any(|t| t == "HIDLINK"),
        "the hidden link text is not written"
    );
    // Chrome: one annotation.
    assert_eq!(
        link_urls(&pdf),
        vec!["https://example.com/visible".to_string()],
        "Chrome: only the visible link has an annotation"
    );
}

// ── Guards ──────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn tagged_renders_of_clip_boxes_succeed() {
    // A paragraph clipped out completely leaves a structure element without content; it must not fail the render.
    let tagged = || RenderOptions {
        tagged: true,
        lang: Some("cs".into()),
        ..Default::default()
    };
    for (what, body) in [
        ("C1", direct("overflow:hidden;height:300px")),
        ("C3", with_p("overflow:hidden;height:600px")),
        ("C4", with_p("overflow:hidden;max-height:2000px")),
    ] {
        let pdf = render_with(&body, tagged());
        assert!(pdf.starts_with(b"%PDF"), "{what}: tagged render");
    }
}

// ── Fixture 28 ──────────────────────────────────────────────────────────────────────────────────────────────────────

fn fixture_28() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("fixtures/invoices/28-notes-box-clip");
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .unwrap();
    for face in ["Regular", "Medium", "SemiBold", "Bold", "ExtraBold"] {
        b = b
            .font_file(root.join(format!("fixtures/fonts/Inter-{face}.ttf")))
            .unwrap();
    }
    b.build()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("fixture 28")
}

#[test]
fn fixture_28_writes_the_words_chrome_writes() {
    // The note is longer than its 112 px box and a body-direct child: main moved the box to page 2 and wrote (and showed)
    // all of it, 491 words on 2 pages.
    let ours = fixture_28();
    let reference = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/invoices/28-notes-box-clip/reference/chrome.pdf"),
    )
    .unwrap();
    assert_eq!(page_count(&ours), 1, "Chrome: 1 page");
    let words =
        |pdf: &[u8]| -> Vec<String> { printed_words(pdf).into_iter().map(|w| w.text).collect() };
    let (ours, chrome) = (words(&ours), words(&reference));
    // A one-line wrapping difference would move a word at most; the words themselves are the same.
    assert_eq!(
        ours.len(),
        chrome.len(),
        "words in the text layer: {} vs Chrome {}",
        ours.len(),
        chrome.len()
    );
    assert_eq!(ours, chrome);
}
