//! A paragraph that spans dozens of pages prints every line once, where the paginator put it (F78).
//!
//! The paginator measures a line by its Parley line box; Blitz quantises the line box but advances by the
//! unrounded `line-height` (`normal` = 1.2 -> a 16 px Inter line advances 19.2 px and its box is 20 px), so
//! consecutive boxes overlap by 0.8 px. The renderer used to recompute the page's lines from the advance, drifted
//! by 0.8 px per page and, after about 25 pages, skipped a whole line (`word09095` ... `word09101`, a blank band
//! at the bottom of the page before it). Line ranges are now decided once, by the fragmenter.
//!
//! Spec: css-break-3 §4.1 (a break inside a paragraph falls between line boxes) and CSS 2.1 §13.3.3 (a line box is
//! never split), so every line box is painted in exactly one fragment.
//!
//! Asserted on each page's words with their boxes (`pdftotext -bbox`): every `wordNNNNN` exactly once, in order,
//! none below the content box, none shifted above its top (the blank band of the old bug sat at the bottom, the
//! shift at the top of the page after it), and the page count the paginator planned.

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

/// `n` words `word00000 word00001 …`.
fn words(n: usize) -> String {
    (0..n)
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

/// `n` words, each exactly once, in order, inside the content box on `pages` pages.
fn assert_every_word_once(what: &str, pdf: &[u8], n: usize, pages: usize) {
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
    assert_eq!(page_count(pdf), pages, "{what}: page count");
}

#[test]
fn a_body_paragraph_of_12000_words_loses_no_line() {
    let pdf = render(&format!("<p>{}</p>", words(12_000)));
    assert_every_word_once("body > p, 12 000 words", &pdf, 12_000, 33);
}

#[test]
fn a_body_paragraph_of_20000_words_loses_no_line() {
    let pdf = render(&format!("<p>{}</p>", words(20_000)));
    assert_every_word_once("body > p, 20 000 words", &pdf, 20_000, 55);
}

#[test]
fn a_paragraph_in_a_div_of_12000_words_loses_no_line() {
    // F30: a paragraph inside a block is split by `fragment_nested_inline_root`, not `fragment_inline_root`.
    let pdf = render(&format!("<div><p>{}</p></div>", words(12_000)));
    assert_every_word_once("div > p, 12 000 words", &pdf, 12_000, 33);
}

#[test]
fn a_paragraph_in_a_table_cell_of_12000_words_loses_no_line() {
    // F30 / F30b: a paragraph inside a table cell.
    let pdf = render(&format!(
        "<table><tr><td><p>{}</p></td></tr></table>",
        words(12_000)
    ));
    assert_every_word_once("td > p, 12 000 words", &pdf, 12_000, 33);
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-lpp-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
