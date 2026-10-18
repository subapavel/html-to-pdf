//! `break-inside: avoid` on a block wrapper keeps the wrapper whole (F83b). The usual invoice idiom is
//! `<div class="break-inside-avoid">` around the totals, the payment details or the signature. Since F87 the class reaches
//! the break table, but the fragmenter still entered such a wrapper and split it between its children whenever its
//! children overflowed the rest of the page.
//!
//! Spec: css-break-3 §3.2 (`break-inside: avoid`), §4.1 (a break between siblings is a class A break point, one between
//! a container's edge and its first child a class C point), §4.3 (forced breaks win over `avoid`), §4.4 (an avoid box
//! taller than a page breaks like `auto`), §4.5 (a break before the box is preferred to one inside it, even when the box
//! is taller than a page) and §5.2 (the margin next to an unforced break is truncated). Every expectation below was
//! measured with headless Chromium (m141) first; the probes are in the F83b plan §7.
//!
//! The page is A4 with 15 mm margins (content box 1009.1 px) and 20 px lines. Words are found with `pdftotext -bbox`.
//! The `(I)` cases are written once with the Tailwind class and once with the inline `style` twin.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the 15 mm page margin.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_BELOW_PT: f32 = 5.0;
const SLACK_ABOVE_PT: f32 = 2.0;

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}";
const AVOID_STYLE: &str = "break-inside:avoid";

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
    let tailwind = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tailwind/tailwind.css");
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .css_file(tailwind)
        .unwrap()
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

/// Four paragraphs, each a few pages long: 2500 words together.
fn four_long_paragraphs() -> String {
    format!(
        "<p>{}</p><p>{}</p><p>{}</p><p>{}</p>",
        words(0, 700),
        words(700, 1400),
        words(1400, 2100),
        words(2100, 2500)
    )
}

/// `height: <px>px` and nothing in it.
fn spacer(px: u32) -> String {
    format!("<div style=\"height:{px}px\"></div>")
}

/// Three short paragraphs: `word00000 aaa`, `word00001 bbb`, `word00002 ccc`.
const THREE: &str = "<p>word00000 aaa</p><p>word00001 bbb</p><p>word00002 ccc</p>";

/// One printed word: its 1-based page, the top and bottom of its box in pt (from the page top) and its text.
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

/// The page (1-based) `word` is printed on.
fn page_of(pdf: &[u8], word: &str) -> usize {
    find(pdf, word).page
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

/// Pages of the three markers of `THREE`.
fn aaa_bbb_ccc(pdf: &[u8]) -> (usize, usize, usize) {
    (
        page_of(pdf, "aaa"),
        page_of(pdf, "bbb"),
        page_of(pdf, "ccc"),
    )
}

/// Both spellings of the wrapper: the Tailwind class and the inline style.
fn wrappers(extra_style: &str, inner: &str) -> [(&'static str, String); 2] {
    [
        (
            "class",
            format!("<div class=\"break-inside-avoid\" style=\"{extra_style}\">{inner}</div>"),
        ),
        (
            "style",
            format!("<div style=\"{AVOID_STYLE};{extra_style}\">{inner}</div>"),
        ),
    ]
}

#[test]
fn an_avoid_wrapper_that_fits_a_fresh_page_moves_whole() {
    // Probe 1: 930 px of spacer leave 79 px, the three paragraphs need about 130. Chrome moves the div whole to page
    // 2; the gate used to enter it and split it between the paragraphs (`aaa`, `bbb` on page 1, `ccc` on page 2).
    for (how, wrapper) in wrappers("margin-top:24px", THREE) {
        let pdf = render(&format!("{}{wrapper}", spacer(930)));
        assert_eq!(aaa_bbb_ccc(&pdf), (2, 2, 2), "{how}");
        assert_eq!(page_count(&pdf), 2, "{how}");
    }
    // The margin above the box truncates at the unforced break (css-break-3 §5.2): `aaa` is where the first line of
    // a page is, the same as in the same wrapper without `avoid`, which breaks inside and starts `ccc` on page 2
    // (Chrome: `aaa`, `bbb` on page 1).
    let control = render(&format!(
        "{}<div style=\"margin-top:24px\">{THREE}</div>",
        spacer(930)
    ));
    assert_eq!(
        aaa_bbb_ccc(&control),
        (1, 1, 2),
        "the control splits in place (guard)"
    );
    let top = find(&control, "ccc");
    assert_eq!(top.page, 2);
    let (_, wrapper) = &wrappers("margin-top:24px", THREE)[1];
    let pdf = render(&format!("{}{wrapper}", spacer(930)));
    let aaa = find(&pdf, "aaa");
    assert!(
        (aaa.y_min - top.y_min).abs() < 1.0,
        "aaa starts at {:.1}pt, a first line of a page starts at {:.1}pt",
        aaa.y_min,
        top.y_min
    );
}

#[test]
fn an_avoid_wrapper_first_in_the_body_stays_on_page_one() {
    // Nothing before it, so there is no better break point: it starts where it is (guard).
    for (how, wrapper) in wrappers("margin-top:24px", THREE) {
        let pdf = render(&wrapper);
        assert_eq!(aaa_bbb_ccc(&pdf), (1, 1, 1), "{how}");
        assert_eq!(page_count(&pdf), 1, "{how}");
    }
}

#[test]
fn an_avoid_wrapper_of_four_long_paragraphs_after_content_starts_on_the_next_page() {
    // The wrapper is taller than a page, so `avoid` is relaxed, but the break before it is still preferred to one
    // inside it (css-break-3 §4.5): it starts page 2 and breaks between lines from there. Chrome: 9 pages, `word00000`
    // on page 2.
    for (how, wrapper) in wrappers("", &four_long_paragraphs()) {
        let pdf = render(&format!("<p>before</p>{wrapper}"));
        assert_eq!(page_of(&pdf, "before"), 1, "{how}");
        assert_eq!(page_of(&pdf, "word00000"), 2, "{how}");
        assert_every_word_once(&format!("before + wrapper ({how})"), &pdf, 2500);
    }
}

#[test]
fn an_avoid_wrapper_of_four_long_paragraphs_loses_no_word_first_in_the_body() {
    // The guard that already existed in `break_inside_avoid.rs`, on this page style: first in the body it starts on
    // page 1 (Chrome: page 1, 8 pages).
    let (_, wrapper) = &wrappers("", &four_long_paragraphs())[1];
    let pdf = render(wrapper);
    assert_eq!(page_of(&pdf, "word00000"), 1);
    assert_every_word_once("wrapper first", &pdf, 2500);
}

#[test]
fn a_nested_relaxed_avoid_paragraph_starts_on_the_next_page() {
    // `<div><p>before</p><p avoid>` with 2500 words (probe 2): Chrome starts the paragraph on page 2 (9 pages); the
    // nested path started it below `before`.
    let pdf = render(&format!(
        "<div><p>before</p><p style=\"{AVOID_STYLE}\">{}</p></div>",
        words(0, 2500)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    assert_every_word_once("div > p + p{avoid}", &pdf, 2500);
    assert_eq!(page_count(&pdf), 9, "pages (Chrome: 9)");
    let pdf = render(&format!(
        "<div><p>before</p><p class=\"break-inside-avoid\">{}</p></div>",
        words(0, 2500)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 2, "class");
}

#[test]
fn a_nested_avoid_wrapper_that_fits_a_page_moves_whole() {
    let inner = format!("<div>{}", spacer(960));
    for (how, wrapper) in wrappers("", THREE) {
        let pdf = render(&format!("{inner}{wrapper}</div>"));
        assert_eq!(aaa_bbb_ccc(&pdf), (2, 2, 2), "{how}");
    }
}

#[test]
fn an_avoid_wrapper_inside_a_bordered_box_takes_the_box_with_it() {
    // `<p>before</p><div border-top><div avoid>four long paragraphs`: Chrome breaks before the outer box (a class A
    // point), not between its border and its content, so `word00000` and the outer border are on page 2.
    let pdf = render(&format!(
        "<p>before</p><div style=\"border-top:2px solid red\"><div class=\"break-inside-avoid\">{}</div></div>",
        four_long_paragraphs()
    ));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    assert_eq!(
        red_pixels(&pdf, 1),
        0,
        "the outer border is not left alone on page 1"
    );
    assert!(red_pixels(&pdf, 2) > 100, "the outer border starts page 2");
    assert_every_word_once("bordered box", &pdf, 2500);
}

#[test]
fn a_forced_break_inside_an_avoid_box_wins_and_the_piece_before_it_moves_whole() {
    // css-break-3 §4.3: a forced break is honoured whatever `break-inside` the ancestors have. The piece before it
    // (a1, a2, a3) does not fit the 49 px left and fits a page: Chrome moves it to page 2 and puts `bb` on page 3.
    let inner = "<p>a1</p><p>a2</p><p>a3</p><p style=\"break-before:page\">bb</p>";
    for (how, wrapper) in wrappers("", inner) {
        let pdf = render(&format!("{}{wrapper}", spacer(960)));
        assert_eq!(page_of(&pdf, "a1"), 2, "{how}");
        assert_eq!(page_of(&pdf, "a3"), 2, "{how}");
        assert_eq!(page_of(&pdf, "bb"), 3, "{how}");
    }
    // Guard: when the piece fits, nothing moves.
    let (_, wrapper) = &wrappers("", "<p>a1</p><p style=\"break-before:page\">bb</p>")[1];
    let pdf = render(&format!("{}{wrapper}", spacer(930)));
    assert_eq!((page_of(&pdf, "a1"), page_of(&pdf, "bb")), (1, 2));
}

#[test]
fn no_blank_page_when_the_avoid_box_has_nowhere_better_to_go() {
    // A box of 980 px in a body that starts 40 px down the page (the margin of its first child collapses through the
    // body): it does not fit page 1 below the offset, but nothing precedes it. Chrome does not break at the start of
    // a fragmentainer either: it starts on page 1.
    let pdf = render(&format!(
        "<div style=\"margin-top:40px;height:980px;{AVOID_STYLE}\"><p>word00000</p></div>"
    ));
    assert_eq!(page_of(&pdf, "word00000"), 1);
    // A relaxed avoid box with a top margin, first in the body (Chrome: page 1).
    for (how, wrapper) in wrappers("", &format!("<p>{}</p>", words(0, 2500))) {
        let pdf = render(&format!("<div class=\"mt-8\">{wrapper}</div>"));
        assert_eq!(page_of(&pdf, "word00000"), 1, "{how}");
    }
    let pdf = render(&format!(
        "<div class=\"mt-8 break-inside-avoid\">{}</div>",
        words(0, 2500)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 1, "an inline root");
    assert_every_word_once("mt-8 + avoid", &pdf, 2500);
}

#[test]
fn an_avoid_box_taller_than_a_page_only_by_height_still_spans_pages_two_and_three() {
    // F83c guard: three lines in a 1500 px wrapper after content. Pages 2-3 like Chrome, `a` at the top of page 2.
    let pdf = render(&format!(
        "<p>before</p><div style=\"height:1500px;{AVOID_STYLE}\"><p>a</p><p>b</p></div>"
    ));
    assert_eq!(page_count(&pdf), 3);
    assert_eq!(page_of(&pdf, "before"), 1);
    assert_eq!(page_of(&pdf, "a"), 2);
    assert_eq!(page_of(&pdf, "b"), 2);
}

#[test]
fn table_rows_with_avoid_stay_whole_and_the_table_pages_as_without() {
    // Fixture 02's shape: 60 rows of five cells, `tr { break-inside: avoid }`. `tr` has no box of its own in the
    // fragmenter, so the rule changes nothing, and no row is split (guard). Chrome also keeps the rows whole.
    let rows = |avoid: &str| -> String {
        (0..60)
            .map(|i| {
                let cells: String = (0..5).map(|j| format!("<td>r{i:02}c{j}</td>")).collect();
                format!("<tr{avoid}>{cells}</tr>")
            })
            .collect()
    };
    let css = format!("{HEAD}td{{padding:6px;height:20px}}");
    let with = render_with(
        &css,
        &format!("<table>{}</table>", rows(" class=\"break-inside-avoid\"")),
    );
    let without = render_with(&css, &format!("<table>{}</table>", rows("")));
    assert_eq!(page_count(&with), page_count(&without));
    for pdf in [&with, &without] {
        let found = printed_words(pdf);
        for i in 0..60 {
            let pages: Vec<usize> = found
                .iter()
                .filter(|w| w.text.starts_with(&format!("r{i:02}c")))
                .map(|w| w.page)
                .collect();
            assert_eq!(pages.len(), 5, "row {i}");
            assert!(
                pages.iter().all(|p| *p == pages[0]),
                "row {i} is split: {pages:?}"
            );
        }
    }
}

/// `<div>wordNNNNN</div>` lines of 20 px, `word<from> … word<to - 1>`.
fn word_lines(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("<div>word{i:05}</div>"))
        .collect()
}

// The next five are the vrt-validator findings of F83b (H1, H2, H3, M1); every expectation was measured with
// Chromium m141 on the validator's repros first.

#[test]
fn padding_border_and_margin_above_an_avoid_box_do_not_make_it_move() {
    // H1: `<div padding><div avoid mt-24>` of 49 lines (980 px) at the top of the document. Nothing precedes the
    // box but the parent's inset and the box's own margin, so there is no better break point: it starts on page 1
    // and breaks there (Chrome: 48 words on page 1, 2 on page 2). It used to move to page 2 and leave page 1 blank.
    for (how, parent_style, child_style) in [
        ("padding", "padding:8px", "margin-top:24px"),
        (
            "border",
            "padding:20px;border:2px solid #900",
            "margin-top:0",
        ),
    ] {
        let pdf = render(&format!(
            "<div style=\"{parent_style}\"><div style=\"{AVOID_STYLE};{child_style}\">{}</div></div><p>word90000</p>",
            word_lines(0, 49)
        ));
        assert_eq!(page_of(&pdf, "word00000"), 1, "{how}");
        assert!(
            page_of(&pdf, "word00040") == 1,
            "{how}: the box starts on page 1"
        );
        assert!(page_count(&pdf) <= 2, "{how}: no stub page");
    }
}

#[test]
fn a_forced_break_at_the_start_of_an_avoid_box_moves_the_box_whole() {
    // H2: the first child has `break-before: page`. A break before the box in effect: every word is on page 2
    // (Chrome); the first line used to stay at the bottom of page 1, in the margin.
    let pdf = render(&format!(
        "<div style=\"height:1000px\"></div><div style=\"{AVOID_STYLE}\">\
         <div style=\"break-before:page\">{}</div>{}</div>",
        word_lines(0, 5),
        word_lines(5, 10)
    ));
    assert_every_word_once("forced break at the start", &pdf, 10);
    assert!(
        printed_words(&pdf)
            .iter()
            .filter(|w| w.text.starts_with("word"))
            .all(|w| w.page == 2),
        "all ten words on page 2"
    );
}

#[test]
fn a_relaxed_padded_avoid_box_after_content_leaves_no_blank_page() {
    // H3: a padded avoid box taller than a page (1005 px of red inside 8 px of padding) after a one-line box starts
    // the next page, and its content starts there too: page 2 is inked (Chrome: the page is inked and a stub follows).
    let pdf = render(&format!(
        "<div style=\"{AVOID_STYLE}\"><div>word00000</div></div>\
         <div style=\"{AVOID_STYLE};padding:8px\"><div style=\"height:1005px;background:#e00\"></div></div>"
    ));
    assert_eq!(page_of(&pdf, "word00000"), 1);
    assert!(red_pixels(&pdf, 2) > 1000, "page 2 holds the red block");
    assert_eq!(red_pixels(&pdf, 1), 0, "page 1 has none of it");
}

#[test]
fn a_nested_relaxed_box_at_a_page_top_closes_inside_the_page() {
    // H3: `<div><spacer 20px><div avoid padding:15px border:3px>` of 49 lines: the box starts page 2 and the last
    // line moves on when the box's bottom padding and border would not fit below it (Chrome: 48 words on page 2,
    // the rest on page 3). The frame used to cross the content bottom by 7 px.
    let pdf = render(&format!(
        "<div><div style=\"height:20px\"></div><div style=\"{AVOID_STYLE};padding:15px;border:3px solid #333\">{}</div></div>\
         <p>word90000</p>",
        word_lines(0, 49)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    assert_eq!(page_of(&pdf, "word00047"), 2);
    assert_eq!(
        page_of(&pdf, "word00048"),
        3,
        "the last line closes the frame on page 3"
    );
}

#[test]
fn a_forced_break_inside_a_too_tall_avoid_box_keeps_the_first_piece_in_place() {
    // M1: 30 lines, a forced break, 30 more: the box is taller than a page only because of the break. The first
    // piece fits the room below `word90005` and stays there (Chrome: 31 words on page 1, 30 on page 2).
    let pdf = render(&format!(
        "<div>word90005</div><div style=\"{AVOID_STYLE}\">{}<div style=\"break-before:page\">{}</div></div>",
        word_lines(0, 30),
        word_lines(30, 60)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 1);
    assert_eq!(page_of(&pdf, "word00029"), 1);
    assert_eq!(page_of(&pdf, "word00030"), 2);
    assert_eq!(page_count(&pdf), 2);
}

#[test]
fn padded_parents_around_an_avoid_box_that_fills_a_page_leave_no_stub_behind() {
    // Harness `A_k50_*_d2_p1`: 1000 px of spacer, two padded and bordered parents (red) around an avoid box of 50 lines
    // (1000 px, a whole page). The chain cannot be kept together, so it starts page 2 and page 1 holds the spacer only
    // (Chrome: words 0-48 on page 2, 49 and the rest on page 3). A top inset used to stay on page 1 as a red stub.
    let o = "<div style=\"padding:12px;border:2px solid #e00\">";
    let pdf = render(&format!(
        "<div style=\"height:1000px;background:#cde\"></div>{o}{o}<div style=\"{AVOID_STYLE}\">{}</div></div></div>\
         <div>word90000</div>",
        word_lines(0, 50)
    ));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    assert_eq!(red_pixels(&pdf, 1), 0, "no frame stub on page 1");
}

#[test]
fn an_avoid_box_that_fits_without_its_parents_bottom_inset_closes_inside_the_page() {
    // Harness `A_k49_*_d2_p1`: two padded and bordered parents around an avoid box of 49 lines at the top of the
    // document: 980 px fit, the 48 px of insets around them do not. Chrome puts 48 words on page 1 and the last one on
    // page 2; the frames used to cross the content bottom.
    let o = "<div style=\"padding:12px;border:2px solid #e00\">";
    let pdf = render(&format!(
        "{o}{o}<div style=\"{AVOID_STYLE}\">{}</div></div></div><div>word90000</div>",
        word_lines(0, 49)
    ));
    assert_eq!(page_of(&pdf, "word00047"), 1);
    assert_eq!(page_of(&pdf, "word00048"), 2);
}

/// Number of strongly red pixels on 1-based `page` (rendered at 48 dpi), read from the binary PPM poppler writes.
fn red_pixels(pdf: &[u8], page: usize) -> usize {
    let path = temp(pdf);
    let root = path.with_extension("page");
    let out = std::process::Command::new("pdftoppm")
        .args([
            "-r",
            "48",
            "-singlefile",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
        ])
        .arg(&path)
        .arg(&root)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftoppm failed");
    let ppm = root.with_extension("page.ppm");
    let data = std::fs::read(&ppm).expect("pdftoppm wrote no PPM");
    let _ = std::fs::remove_file(&ppm);
    // "P6\n<w> <h>\n255\n" and then w * h RGB triples.
    let mut fields = Vec::new();
    let mut pos = 0;
    while fields.len() < 4 {
        while data[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        while !data[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..pos]).into_owned());
    }
    pos += 1;
    assert_eq!(fields[0], "P6");
    data[pos..]
        .chunks_exact(3)
        .filter(|px| px[0] > 200 && px[1] < 60 && px[2] < 60)
        .count()
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-aw-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
