//! An absolutely positioned paragraph, or a wrapper of one, keeps all of its text (F30g).
//!
//! Text inside an absolutely positioned box was printed only where it happened to fit the page its box starts on: the
//! fragmenter never placed such a box (out-of-flow children are skipped), so a box under a zero-height or an entered
//! parent was not drawn at all, and the body-direct pass gave it one full-height window per page, which the renderer
//! can only cut at the page edge. Now a box whose text crosses a page is fragmented from its own start like content
//! in the normal flow.
//!
//! Spec: CSS 2.1 §10.1 (containing block), §10.6.4 (vertical position), §13.3 (breaks are defined for the normal
//! flow only); css-break-3 §2 (a box laid out against a stitched containing block is then fragmented like other
//! content of the context), §4.1, §4.4 (breaks between line boxes, orphans and widows). Every expectation was
//! measured with headless Chromium (m141) first; the numbers are in the F30g plan §7.
//!
//! The page is A4 with 15 mm margins (content box 1009.1 px) and 20 px lines unless a test says otherwise. Words are
//! found with `pdftotext -bbox`. Positions are compared with an in-flow twin of the same text rather than with an
//! absolute number, because the glyph box of a line starts a little above or below its line box depending on the
//! renderer.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the 15 mm page margin.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_BELOW_PT: f32 = 5.0;
const SLACK_ABOVE_PT: f32 = 2.0;

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}p{margin:0;line-height:20px}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    render_with(HEAD, body)
}

fn render_with(head_style: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head_style}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Words `word<from> … word<to - 1>`.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `height: <px>px` and nothing in it.
fn spacer(px: u32) -> String {
    format!("<div style=\"height:{px}px\"></div>")
}

/// One printed word: its 1-based page, the top and bottom of its box in pt (from the page top) and its text.
#[derive(Clone)]
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

/// The word `text` (first occurrence).
fn find(pdf: &[u8], text: &str) -> Word {
    printed_words(pdf)
        .into_iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("{text} is not printed"))
}

/// How many times `text` is printed.
fn count_of(pdf: &[u8], text: &str) -> usize {
    printed_words(pdf)
        .into_iter()
        .filter(|w| w.text == text)
        .count()
}

/// How many `wordNNNNN` are printed on `page`.
fn words_on_page(pdf: &[u8], page: usize) -> usize {
    printed_words(pdf)
        .into_iter()
        .filter(|w| w.page == page && w.text.starts_with("word"))
        .count()
}

/// `n` words `wordNNNNN`, each exactly once, in order, inside the content box.
fn assert_every_word_once(what: &str, pdf: &[u8], n: usize) {
    let found: Vec<Word> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .collect();
    let first_diff = found
        .iter()
        .enumerate()
        .position(|(i, w)| w.text != format!("word{i:05}"));
    assert!(
        found.len() == n && first_diff.is_none(),
        "{what}: every word exactly once, in order: printed {} of {n}, first difference at index {first_diff:?}",
        found.len()
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
            "{what}: {} on page {} starts at {:.1}pt, above the content box ({top:.1}pt)",
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

/// `y_min` of the first line of text at the top of a page: what a word on the content top looks like.
fn content_top_glyph_y() -> f32 {
    let pdf = render("<p>probe</p>");
    find(&pdf, "probe").y_min
}

// ── A1: R1, the F83 validator's case ─────────────────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_paragraph_in_a_zero_height_relative_parent_is_printed() {
    // R1: the `<div>` has no height, so the fragmenter recorded it alone and the box inside it never got geometry:
    // 0 of 200 words, with `avoid` and with `auto` alike. Chrome: 200 on page 1 at the top.
    let top = content_top_glyph_y();
    for avoid in ["break-inside:avoid;", ""] {
        let pdf = render(&format!(
            "<div style=\"position:relative\"><p style=\"{avoid}position:absolute;top:0;left:0;width:400px\">{}</p></div>",
            words(0, 200)
        ));
        assert_every_word_once(&format!("R1 {avoid:?}"), &pdf, 200);
        assert_eq!(page_count(&pdf), 1, "{avoid:?}");
        let first = find(&pdf, "word00000");
        assert!(
            (first.y_min - top).abs() < 1.0,
            "{avoid:?}: word00000 at {:.1}pt, a first line is at {top:.1}pt",
            first.y_min
        );
    }
}

#[test]
fn the_same_with_line_height_normal() {
    // The validator's own shape: `line-height: normal` (19.2 px advance, 20 px boxes in Blitz, F78).
    let pdf = render_with(
        "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:normal;margin:0}",
        &format!(
            "<div style=\"position:relative\"><p style=\"break-inside:avoid;position:absolute;top:0;left:0;width:400px;margin:0\">{}</p></div>",
            words(0, 200)
        ),
    );
    assert_every_word_once("R1 normal", &pdf, 200);
}

// ── A2: an abspos paragraph in the body ───────────────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_paragraph_taller_than_a_page_is_split_between_lines() {
    // 2000 words directly in the body: the window pass gave it one full-height window per page and the renderer cut
    // it at the page edge (372 words, page 1 only). Chrome: all of them on 6 pages.
    for avoid in ["break-inside:avoid;", ""] {
        let pdf = render(&format!(
            "<p style=\"{avoid}position:absolute;top:0;left:0\">{}</p>",
            words(0, 2000)
        ));
        assert_every_word_once(&format!("A2 {avoid:?}"), &pdf, 2000);
        assert_eq!(page_count(&pdf), 6, "{avoid:?} (Chrome: 6)");
    }
}

#[test]
fn an_abspos_paragraph_with_a_body_margin_stays_in_the_page_area() {
    // `top:0` is the top of the page area, not of the body: a body margin moves neither the box nor the pages after
    // the first (Chrome: the same words on the same pages as without the margin). The window pass put the box 8 px
    // too far left on every page and 8 px too high on the pages after the first (F92).
    let plain = render(&format!(
        "<p style=\"position:absolute;top:0;left:0\">{}</p>",
        words(0, 2000)
    ));
    let pdf = render_with(
        &format!("{HEAD}body{{margin:8px}}"),
        &format!(
            "<p style=\"position:absolute;top:0;left:0\">{}</p>",
            words(0, 2000)
        ),
    );
    assert_every_word_once("A2 body margin", &pdf, 2000);
    // The box is as wide as the body (Taffy lays it out against its parent, 16 px narrower than the page area, a
    // non-goal of F30g), so lines wrap differently from the plain case and a page holds a few words fewer; the pages
    // and the first line do not move.
    assert_eq!(page_count(&pdf), page_count(&plain));
    let (a, b) = (find(&plain, "word00000"), find(&pdf, "word00000"));
    assert!(
        (a.y_min - b.y_min).abs() < 1.0,
        "word00000 moved with the body margin: {:.1} -> {:.1}",
        a.y_min,
        b.y_min
    );
}

// ── A3: R2, a wrapper ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_wrapper_of_a_long_paragraph_keeps_every_word() {
    // R2: `<div style="position:absolute;top:0;left:0;right:0"><p>` 5000 words. 379 words on 14 pages before; Chrome:
    // all of them on 15 pages.
    let body = format!(
        "<div style=\"position:absolute;top:0;left:0;right:0\"><p>{}</p></div>",
        words(0, 5000)
    );
    let pdf = render(&body);
    assert_every_word_once("R2", &pdf, 5000);
    assert_eq!(page_count(&pdf), 15, "Chrome: 15");
    // The same with `line-height: normal`.
    let pdf = render_with(
        "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:normal;margin:0}",
        &body,
    );
    assert_every_word_once("R2 normal", &pdf, 5000);
}

#[test]
fn an_abspos_wrapper_in_a_static_div_or_in_a_relative_div_keeps_every_word() {
    // (a) abspos > static > p: printed 379 words. (b) relative > abspos > p: printed none (the relative `div` has no
    // height). Chrome: 5000 words on 15 pages for both.
    let inner = format!("<p>{}</p>", words(0, 5000));
    let a = render(&format!(
        "<div style=\"position:absolute;top:0;left:0;right:0\"><div>{inner}</div></div>"
    ));
    assert_every_word_once("(a) abs > static > p", &a, 5000);
    assert_eq!(page_count(&a), 15);
    let b = render(&format!(
        "<div style=\"position:relative\"><div style=\"position:absolute;top:0;left:0;right:0\">{inner}</div></div>"
    ));
    assert_every_word_once("(b) relative > abs > p", &b, 5000);
    assert_eq!(page_count(&b), 15);
}

// ── A4: an abspos wrapper of several paragraphs ───────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_wrapper_of_ten_paragraphs_is_fragmented_from_its_start() {
    // Ten paragraphs of 300 words with a 20 px gap. 1821 of 3000 words printed; Chrome: 9 pages. Short paragraphs
    // that fit a page move whole in the normal flow (F30c), so the page count may be a little higher.
    let paragraphs: String = (0..10)
        .map(|i| {
            format!(
                "<p style=\"margin-bottom:20px\">{}</p>",
                words(i * 300, i * 300 + 300)
            )
        })
        .collect();
    let pdf = render(&format!(
        "<div style=\"position:absolute;top:0;left:0;right:0\">{paragraphs}</div>"
    ));
    assert_every_word_once("A4", &pdf, 3000);
    let pages = page_count(&pdf);
    assert!(
        (9..=11).contains(&pages),
        "{pages} pages, Chrome 9 (+2 for F30c)"
    );
}

// ── A5: a box under a relative parent that starts below content ───────────────────────────────────────────────────

#[test]
fn an_abspos_paragraph_in_a_relative_parent_below_content_starts_at_its_parent() {
    // 600 px of spacer, then a zero-height relative `div` with an abspos paragraph of 1000 words. Chrome: it starts
    // 600 px down page 1 and runs over 4 pages.
    let top = content_top_glyph_y();
    let pdf = render(&format!(
        "{}<div style=\"position:relative\"><p style=\"position:absolute;top:0\">{}</p></div>",
        spacer(600),
        words(0, 1000)
    ));
    assert_every_word_once("A5", &pdf, 1000);
    let first = find(&pdf, "word00000");
    assert_eq!(first.page, 1);
    assert!(
        (first.y_min - (top + 600.0 * 0.75)).abs() < 1.0,
        "word00000 at {:.1}pt, expected {:.1}pt",
        first.y_min,
        top + 450.0
    );
    assert_eq!(page_count(&pdf), 4, "Chrome: 4");
}

#[test]
fn an_abspos_paragraph_crossing_the_page_in_a_whole_emitted_parent_is_split() {
    // A5b: the relative parent (60 px at 900 px) fits page 1 and is emitted whole, which put the whole paragraph on
    // page 1 and lost what falls below it (56 of 300 words). Chrome: 35 words on page 1, the rest on page 2.
    let pdf = render(&format!(
        "{}<div style=\"position:relative;height:60px\"><p style=\"position:absolute;top:0\">{}</p></div>",
        spacer(900),
        words(0, 300)
    ));
    assert_every_word_once("A5b", &pdf, 300);
    assert_eq!(page_count(&pdf), 2, "Chrome: 2");
    let on_first = words_on_page(&pdf, 1);
    assert!(
        (30..=40).contains(&on_first),
        "{on_first} words on page 1, Chrome 35"
    );
}

// ── A6: a stamp in an entered parent ──────────────────────────────────────────────────────────────────────────────

/// 60 paragraphs of 12 words: two lines each, 2400 px in all, three pages.
fn sixty_paragraphs() -> String {
    (0..60)
        .map(|i| format!("<p>{}</p>", words(i * 12, i * 12 + 12)))
        .collect()
}

#[test]
fn a_stamp_in_a_relative_parent_that_spans_pages_is_printed_at_its_top() {
    // The parent is entered by the fragmenter, which skips its out-of-flow children: the stamp was never drawn.
    // Chrome: once, on page 1, level with the first line.
    let pdf = render(&format!(
        "<div style=\"position:relative\">{}<div style=\"position:absolute;top:0;right:0\">ZAPLACENO</div></div>",
        sixty_paragraphs()
    ));
    assert_every_word_once("A6 text", &pdf, 720);
    assert_eq!(count_of(&pdf, "ZAPLACENO"), 1);
    let stamp = find(&pdf, "ZAPLACENO");
    assert_eq!(stamp.page, 1);
    assert!(
        (stamp.y_min - find(&pdf, "word00000").y_min).abs() < 2.0,
        "ZAPLACENO at {:.1}pt, the first line at {:.1}pt",
        stamp.y_min,
        find(&pdf, "word00000").y_min
    );
}

#[test]
fn a_bottom_anchored_stamp_in_a_relative_parent_lands_on_the_last_page() {
    // `bottom:0` is the bottom of the parent's last fragment: page 3, next to the last line (Chrome: 324.8pt against
    // a last line at 325pt).
    let pdf = render(&format!(
        "<div style=\"position:relative\">{}<div style=\"position:absolute;bottom:0;right:0\">ZAPLACENO</div></div>",
        sixty_paragraphs()
    ));
    assert_every_word_once("A6b text", &pdf, 720);
    assert_eq!(count_of(&pdf, "ZAPLACENO"), 1);
    let stamp = find(&pdf, "ZAPLACENO");
    assert_eq!(stamp.page, 3, "Chrome: page 3");
    let last = find(&pdf, "word00719");
    assert!(
        (stamp.y_min - last.y_min).abs() < 2.0,
        "ZAPLACENO at {:.1}pt, the last line at {:.1}pt",
        stamp.y_min,
        last.y_min
    );
}

// ── A7: a guard ───────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_note_in_a_short_relative_parent_stays_where_it_was() {
    // Guard: the box fits, so its geometry is not touched (the PDF is byte-identical to main's, checked by hand).
    let top = content_top_glyph_y();
    let pdf = render(
        "<div style=\"position:relative;height:200px\"><div style=\"position:absolute;top:20px;left:20px\">Poznámka</div></div>",
    );
    assert_eq!(count_of(&pdf, "Poznámka"), 1);
    let note = find(&pdf, "Poznámka");
    assert_eq!(note.page, 1);
    // 20 px = 15 pt below the content top (Chrome: 54.8pt against a content-top glyph at 39.8pt).
    assert!(
        (note.y_min - (top + 15.0)).abs() < 1.0,
        "Poznámka at {:.1}pt, expected {:.1}pt",
        note.y_min,
        top + 15.0
    );
}

// ── A8: top, bottom and height ────────────────────────────────────────────────────────────────────────────────────

fn assert_words_per_page(what: &str, pdf: &[u8], expected: &[usize]) {
    for (i, want) in expected.iter().enumerate() {
        let got = words_on_page(pdf, i + 1);
        assert!(
            got.abs_diff(*want) <= 8,
            "{what}: page {} holds {got} words, Chrome {want} (one line is about 7)",
            i + 1
        );
    }
    assert_eq!(page_count(pdf), expected.len(), "{what}: pages");
}

#[test]
fn an_abspos_box_with_top_and_height_flows_from_its_top() {
    // (a) `top:300px; height:1500px` with 1000 words of text: Chrome 4 pages, 246 / 350 / 350 / 54 words.
    let pdf = render(&format!(
        "<div style=\"position:absolute;top:300px;height:1500px\">{}</div>",
        words(0, 1000)
    ));
    assert_every_word_once("A8a", &pdf, 1000);
    assert_words_per_page("A8a", &pdf, &[246, 350, 350, 54]);
}

#[test]
fn an_abspos_box_with_top_and_bottom_continues_past_the_box() {
    // (b) `top:0; bottom:0`: the box is one page tall and its text goes on: Chrome 3 pages, 351 / 350 / 299 words.
    let pdf = render(&format!(
        "<div style=\"position:absolute;top:0;bottom:0\">{}</div>",
        words(0, 1000)
    ));
    assert_every_word_once("A8b", &pdf, 1000);
    assert_words_per_page("A8b", &pdf, &[351, 350, 299]);
}

#[test]
fn a_bottom_anchored_abspos_box_keeps_its_end_at_the_bottom_of_the_page() {
    // (c) `bottom:0`: the box grows upwards from the bottom of the page, and what is above the top of the page is not
    // printed. Chrome prints 355 words (645..=999); the part in the margin is not clipped by either renderer, so
    // ours prints a few lines more (guard: this is the window path, unchanged by F30g).
    let pdf = render(&format!(
        "<div style=\"position:absolute;bottom:0\">{}</div>",
        words(0, 1000)
    ));
    assert_eq!(page_count(&pdf), 1);
    let found: Vec<Word> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .collect();
    assert_eq!(found.last().map(|w| w.text.as_str()), Some("word00999"));
    let n = found.len();
    assert!((340..=390).contains(&n), "{n} words, Chrome 355");
    let last = found.last().unwrap();
    assert!(
        last.y_max <= PAGE_H_PT - MARGIN_PT + SLACK_BELOW_PT,
        "the last line ends at {:.1}pt, below the content box",
        last.y_max
    );
}

// ── A9: the static position ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_box_without_insets_starts_after_the_previous_sibling() {
    // `top:auto`: the static position, after `before`. Chrome: `word00000` one line (15pt) below `before`, 3 pages.
    let pdf = render(&format!(
        "<p>before</p><div style=\"position:absolute\"><p>{}</p></div>",
        words(0, 1000)
    ));
    assert_every_word_once("A9", &pdf, 1000);
    assert_eq!(page_count(&pdf), 3, "Chrome: 3");
    let (before, first) = (find(&pdf, "before"), find(&pdf, "word00000"));
    assert_eq!(first.page, 1);
    assert!(
        (first.y_min - before.y_min - 15.0).abs() < 1.0,
        "word00000 at {:.1}pt, before at {:.1}pt",
        first.y_min,
        before.y_min
    );
}

#[test]
fn an_abspos_box_without_insets_follows_a_sibling_that_moved_to_the_next_page() {
    // A9b: 900 px, then a 300 px block that does not fit the 109 px left and moves whole to page 2 (F83h: Chrome
    // splits an empty block, so its box lands 191 px down page 2; ours has the sibling end at 300 px and the box
    // follows it, instead of overlapping it).
    let pdf = render(&format!(
        "{}{}<div style=\"position:absolute\"><p>{}</p></div>",
        spacer(900),
        spacer(300),
        words(0, 1000)
    ));
    assert_every_word_once("A9b", &pdf, 1000);
    let first = find(&pdf, "word00000");
    assert_eq!(first.page, 2, "Chrome: page 2");
    // A word at the top of a later page is the reference for the content top.
    let top_of_page = printed_words(&pdf)
        .into_iter()
        .find(|w| w.page == 3 && w.text.starts_with("word"))
        .expect("a word on page 3")
        .y_min;
    assert!(
        (first.y_min - (top_of_page + 300.0 * 0.75)).abs() < 1.0,
        "word00000 at {:.1}pt, expected {:.1}pt (300 px below the content top)",
        first.y_min,
        top_of_page + 225.0
    );
}

// ── A10: position: fixed is not fragmented ────────────────────────────────────────────────────────────────────────

#[test]
fn a_fixed_box_is_still_repeated_whole_on_every_page() {
    // Guard (CSS 2.1 §9.6.1): `Strana` once per page over 9 pages of flow.
    let pdf = render(&format!(
        "<div style=\"position:fixed;bottom:0\">Strana</div><p>{}</p>",
        words(0, 3000)
    ));
    assert_every_word_once("A10a", &pdf, 3000);
    assert_eq!(page_count(&pdf), 9, "Chrome: 9");
    assert_eq!(count_of(&pdf, "Strana"), 9);
    let pages: Vec<usize> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text == "Strana")
        .map(|w| w.page)
        .collect();
    assert_eq!(pages, (1..=9).collect::<Vec<_>>());
}

#[test]
fn a_fixed_box_repeats_on_the_pages_an_abspos_flow_adds() {
    // The pages an abspos paragraph adds are known to the second fixed pass: `Strana` on each of the 6 pages.
    let pdf = render(&format!(
        "<div style=\"position:fixed;bottom:0\">Strana</div><p style=\"position:absolute;top:0;left:0\">{}</p>",
        words(0, 2000)
    ));
    assert_every_word_once("fixed + abspos", &pdf, 2000);
    assert_eq!(page_count(&pdf), 6);
    let pages: Vec<usize> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text == "Strana")
        .map(|w| w.page)
        .collect();
    assert_eq!(pages, (1..=6).collect::<Vec<_>>());
}

#[test]
fn a_fixed_box_taller_than_a_page_keeps_its_first_page_only() {
    // Guard, a recorded non-goal: Chrome prints the part that fits page 1 (358 words) and nothing more.
    let pdf = render(&format!(
        "<div style=\"position:fixed\">{}</div>",
        words(0, 2000)
    ));
    assert_eq!(page_count(&pdf), 1);
    let n = words_on_page(&pdf, 1);
    assert!((340..=400).contains(&n), "{n} words, Chrome 358");
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-af-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
