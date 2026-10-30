//! `break-inside: avoid` holds while the box fits a page; past that it is relaxed and the box breaks between its
//! line boxes like an `auto` one (F83).
//!
//! Spec: css-break-3 §4.4 (unforced breaks: when the avoid rules leave no way to keep the content from overflowing
//! the fragmentainer, the `avoid` of Rules 1, 2 and 4 is dropped first, orphans / widows last) and §4.1 (a break
//! inside a paragraph falls between line boxes). CSS 2.1 §13.3.3 has the same order.
//!
//! A `<p style="break-inside:avoid">` of 2500 words used to print 2478 of them: no fragmenter gave it line ranges, so
//! the strip slicer cut it at fixed pixel heights and the renderer measured the lines from those heights, which
//! dropped the line that straddled each cut. Asserted with `pdftotext -bbox`: every `wordNNNNN` exactly once, in
//! order, inside the content box; and a relaxed `avoid` paragraph pages exactly like the same paragraph without the
//! property. The guards pin the paths that were already right (nested boxes, `page-break-inside`, a wrapper, a box
//! that fits a page).

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the default 15 mm page margin the renderer applies.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_BELOW_PT: f32 = 5.0;
/// A word whose top is higher than this above the content top was drawn too high.
const SLACK_ABOVE_PT: f32 = 2.0;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// Words `word<from> … word<to - 1>`.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>body{{font-family:Inter}}</style></head>\
                 <body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// One printed `wordNNNNN`: its 1-based page, the top and bottom of its box in pt (from the page top) and its text.
struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let text = String::from_utf8_lossy(&out.stdout);
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
            && word.starts_with("word")
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

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// `n` words, each exactly once, in order, inside the content box.
fn assert_every_word_once(what: &str, pdf: &[u8], n: usize) {
    let found = printed_words(pdf);
    let seen: Vec<&str> = found.iter().map(|w| w.text.as_str()).collect();
    let missing: Vec<usize> = (0..n)
        .filter(|i| !seen.contains(&format!("word{i:05}").as_str()))
        .collect();
    let first_diff = seen
        .iter()
        .enumerate()
        .position(|(i, w)| *w != format!("word{i:05}"));
    assert!(
        seen.len() == n && first_diff.is_none(),
        "{what}: every word exactly once, in order: printed {} of {n}, first difference at index {first_diff:?}, \
         {} missing (first {:?})",
        seen.len(),
        missing.len(),
        missing.first()
    );
    let bottom = PAGE_H_PT - MARGIN_PT + SLACK_BELOW_PT;
    let top = MARGIN_PT - SLACK_ABOVE_PT;
    for w in &found {
        assert!(
            w.y_max <= bottom,
            "{what}: {} on page {} ends at {:.1}pt, below the content box ({bottom:.1}pt)",
            w.text,
            w.page,
            w.y_max
        );
        assert!(
            w.y_min >= top,
            "{what}: {} on page {} starts at {:.1}pt, above the content box ({top:.1}pt): the page is drawn too high",
            w.text,
            w.page,
            w.y_min
        );
    }
    assert!(
        found.windows(2).all(|w| w[0].page <= w[1].page),
        "{what}: pages out of order"
    );
}

/// The first `wordNNNNN` of every page that has one, in page order.
fn first_word_per_page(pdf: &[u8]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for w in printed_words(pdf) {
        if out.last().is_none_or(|(p, _)| *p != w.page) {
            out.push((w.page, w.text));
        }
    }
    out
}

/// The page (1-based) `word` is printed on.
fn page_of(pdf: &[u8], word: &str) -> usize {
    printed_words(pdf)
        .iter()
        .find(|w| w.text == word)
        .unwrap_or_else(|| panic!("{word} is not printed"))
        .page
}

/// A relaxed `avoid` box pages exactly like the same box without the property: the page count and the first word of
/// every page. `avoid` and `auto` are the two bodies.
fn assert_pages_like_auto(what: &str, avoid: &str, auto: &str, n: usize) {
    let pdf = render(avoid);
    let reference = render(auto);
    assert_every_word_once(&format!("{what} (auto)"), &reference, n);
    assert_every_word_once(what, &pdf, n);
    assert_eq!(
        page_count(&pdf),
        page_count(&reference),
        "{what}: page count differs from the same box without break-inside"
    );
    assert_eq!(
        first_word_per_page(&pdf),
        first_word_per_page(&reference),
        "{what}: first word of each page differs from the same box without break-inside"
    );
}

#[test]
fn a_body_paragraph_taller_than_a_page_loses_no_word() {
    assert_pages_like_auto(
        "body > p{avoid}, 2500 words",
        &format!("<p style=\"break-inside:avoid\">{}</p>", words(0, 2500)),
        &format!("<p>{}</p>", words(0, 2500)),
        2500,
    );
}

#[test]
fn a_paragraph_taller_than_a_page_after_content_starts_on_the_next_page() {
    // The box does not fit the rest of the page and is not first on its page, so it starts on a fresh page (Chrome
    // does the same); it does not fit there either, so it breaks between lines.
    let avoid = format!(
        "<p>before</p><p style=\"break-inside:avoid\">{}</p>",
        words(0, 2500)
    );
    let pdf = render(&avoid);
    assert_eq!(page_of(&pdf, "word00000"), 2, "starts on page 2");
    // F30c: an `auto` paragraph after `before` starts below it on page 1 (Chrome 6 pages, ours 7), so the relaxed `avoid`
    // one no longer pages like it. It pages like the same paragraph on a fresh page (Chrome 7 pages for both, ours 8).
    assert_pages_like_auto(
        "p + p{avoid}, 2500 words",
        &avoid,
        &format!(
            "<p>before</p><p style=\"break-before:page\">{}</p>",
            words(0, 2500)
        ),
        2500,
    );
}

#[test]
fn a_body_div_with_direct_text_taller_than_a_page_loses_no_word() {
    // An inline root that is not a `<p>`.
    assert_pages_like_auto(
        "body > div{avoid} with direct text, 2500 words",
        &format!("<div style=\"break-inside:avoid\">{}</div>", words(0, 2500)),
        &format!("<div>{}</div>", words(0, 2500)),
        2500,
    );
}

#[test]
fn page_break_inside_avoid_loses_no_word() {
    // The legacy alias is not read yet (F83a): the paragraph is an ordinary one. Green before and after.
    let pdf = render(&format!(
        "<p style=\"page-break-inside:avoid\">{}</p>",
        words(0, 2500)
    ));
    assert_every_word_once("p{page-break-inside:avoid}", &pdf, 2500);
}

#[test]
fn an_avoid_paragraph_in_a_div_loses_no_word() {
    let pdf = render(&format!(
        "<div><p>before</p><p style=\"break-inside:avoid\">{}</p></div>",
        words(0, 2500)
    ));
    assert_every_word_once("div > p + p{avoid}", &pdf, 2500);
}

#[test]
fn an_avoid_div_around_a_paragraph_loses_no_word() {
    let pdf = render(&format!(
        "<div style=\"break-inside:avoid\"><p>{}</p></div>",
        words(0, 2500)
    ));
    assert_every_word_once("div{avoid} > p", &pdf, 2500);
}

#[test]
fn an_avoid_wrapper_of_four_long_paragraphs_loses_no_word() {
    // Four paragraphs of 700 words, each a few pages long, in one `avoid` box.
    let paragraphs: String = (0..4)
        .map(|i| format!("<p>{}</p>", words(i * 700, (i + 1) * 700)))
        .collect();
    let pdf = render(&format!(
        "<div style=\"break-inside:avoid\">{paragraphs}</div>"
    ));
    assert_every_word_once("div{avoid} > 4 x p", &pdf, 2800);
}

#[test]
fn an_avoid_paragraph_that_fits_a_page_still_moves_whole() {
    // 900 px of spacer leave about 110 px of the 1009 px page: the 200 words (about 30 lines) do not fit below it,
    // fit a fresh page, and `avoid` holds: every word is on page 2, none on page 1.
    let spacer = "<div style=\"height:900px\"></div>";
    let body = format!(
        "{spacer}<p style=\"break-inside:avoid\">{}</p>",
        words(0, 200)
    );
    let pdf = render(&body);
    assert_every_word_once("spacer + p{avoid}, 200 words", &pdf, 200);
    assert!(
        printed_words(&pdf).iter().all(|w| w.page == 2),
        "every word on page 2: {:?}",
        first_word_per_page(&pdf)
    );
    // The same inside a box.
    let body = format!(
        "{spacer}<div><p style=\"break-inside:avoid\">{}</p></div>",
        words(0, 200)
    );
    let pdf = render(&body);
    assert_every_word_once("spacer + div > p{avoid}, 200 words", &pdf, 200);
    assert!(
        printed_words(&pdf).iter().all(|w| w.page == 2),
        "every word on page 2 (in a div): {:?}",
        first_word_per_page(&pdf)
    );
}

#[test]
fn an_avoid_box_taller_than_a_page_only_by_height_still_spans_pages_two_and_three() {
    // F83c: three lines in a 1500 px box. The lines fit a page, so `avoid` holds (the box is not relaxed to `auto`):
    // it moves to page 2 and its frame spans pages 2 and 3, as on main and in Chrome (3 pages, the words on page 2 at
    // the top of the content box).
    let body = format!(
        "<p>before</p><div style=\"height:1500px;background:#eee;break-inside:avoid\">{}</div>",
        "word00000<br>word00001<br>word00002"
    );
    let pdf = render(&body);
    assert_eq!(page_count(&pdf), 3, "pages");
    let found = printed_words(&pdf);
    assert_eq!(found.len(), 3);
    assert!(found.iter().all(|w| w.page == 2), "all on page 2");
    assert!(
        (found[0].y_min - MARGIN_PT).abs() < 6.0,
        "{} starts at {:.1}pt, at the top of the content box ({MARGIN_PT:.1}pt)",
        found[0].text,
        found[0].y_min
    );
    // Without anything before it the box starts on page 1 and spans pages 1 and 2.
    let pdf = render(
        "<div style=\"height:1500px;background:#eee;break-inside:avoid\">word00000<br>word00001<br>word00002</div>",
    );
    assert_eq!(page_count(&pdf), 2, "pages");
    assert!(printed_words(&pdf).iter().all(|w| w.page == 1));
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-bia-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
