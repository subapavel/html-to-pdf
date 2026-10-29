//! A forced break at the end of a block ends its line and makes no line of its own (F66f, F37d).
//!
//! CSS 2.1 §9.4.2: a line box with no text, no preserved white space and no other in-flow content is treated as
//! zero-height, and the line that holds a `<br>` or a preserved newline is the line before it. Parley commits an empty
//! last line after the final newline; the Blitz fork's `break_lines` takes it back (plan `2026-10-29-f66f`).
//!
//! Every number is Chrome's (m141, A4, 15 mm, Inter, 14px/20px unless a test says otherwise, an `AFTER` paragraph with no
//! margin below), measured with `out/scratch/f66f/{gen.py,run.sh}` and `out/scratch/f30fdv/a`. The text layer's
//! coordinates are pt, so the test converts no units.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";
const CARD: &str = "border:1px solid #000;padding:9px;background:#eee";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_with(head: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

struct Word {
    page: usize,
    y_min: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-tb-{}-{n}.pdf", std::process::id()));
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
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn after(words: &[Word]) -> &Word {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    a[0]
}

/// AFTER's top is Chrome's within a point.
fn assert_after_at(what: &str, body: &str, chrome_y: f32) {
    let words = printed_words(&render_with(HEAD, body));
    let a = after(&words);
    assert_eq!(a.page, 1, "{what}: AFTER is on page 1");
    assert!(
        (a.y_min - chrome_y).abs() <= 1.0,
        "{what}: AFTER at {:.1} pt, Chrome puts it at {chrome_y:.1} pt",
        a.y_min
    );
}

/// `<div style=..>{inner}</div><p>AFTER</p>` plain, and the same inside the F30f-h card (15 pt lower).
fn both(what: &str, style: &str, inner: &str, chrome_plain: f32) {
    assert_after_at(
        what,
        &format!("<div style=\"{style}\">{inner}</div><p>AFTER</p>"),
        chrome_plain,
    );
    assert_after_at(
        &format!("{what}, in a card"),
        &format!("<div style=\"{CARD}\"><div style=\"{style}\">{inner}</div></div><p>AFTER</p>"),
        chrome_plain + 15.0,
    );
}

#[test]
fn a_br_at_the_end_of_a_block_adds_no_line() {
    both("a<br>b<br>", "", "a<br>b<br>", 73.8);
    both("a<br>", "", "a<br>", 58.8);
    both(
        "a<br>b<br> (source white space after)",
        "",
        "a<br>b<br>\n  ",
        73.8,
    );
    both("<b>x<br></b>", "", "<b>x<br></b>", 58.8);
    both(
        "<br> before an empty span",
        "",
        "<span>a<br></span><span></span>",
        58.8,
    );
}

#[test]
fn a_card_that_ends_in_br_is_as_tall_as_its_lines() {
    // f30fhv/g1/z06_br_trailing: main (after F30f-h) 133.6, Chrome 118.8.
    assert_after_at(
        "card, four <br>-ended lines",
        &format!("<div style=\"{CARD}\">L1<br>L2<br>L3<br>L4<br></div><p>AFTER</p>"),
        118.8,
    );
    // z02: only a <br>.
    assert_after_at(
        "card with only <br>",
        &format!("<div style=\"{CARD}\"><br></div><p>AFTER</p>"),
        73.8,
    );
}

#[test]
fn two_brs_keep_two_lines_and_a_lone_br_keeps_one() {
    both("a<br><br>", "", "a<br><br>", 73.8);
    both("a<br>b<br><br><br>", "", "a<br>b<br><br><br>", 103.8);
    both("<br>", "", "<br>", 58.8);
    both("<br><br><br>", "", "<br><br><br>", 88.8);
}

#[test]
fn a_preserved_newline_at_the_end_adds_no_line() {
    both("pre a\\nb\\n", "white-space:pre", "a\nb\n", 73.8);
    both("pre a\\n\\n", "white-space:pre", "a\n\n", 73.8);
    both("pre \\n", "white-space:pre", "\n", 58.8);
    both("pre-wrap a___\\n", "white-space:pre-wrap", "a   \n", 58.8);
    both("pre \\r\\n", "white-space:pre", "a\r\nb\r\n", 73.8);
    // The address block of the finding.
    both(
        "pre-line address",
        "white-space:pre-line",
        "\n    Firma\n    Ulice 1\n",
        88.8,
    );
    both("pre-line a\\n___", "white-space:pre-line", "a\n   ", 58.8);
}

#[test]
fn what_is_on_the_last_line_keeps_it() {
    both(
        "pre empty line in the middle",
        "white-space:pre",
        "a\n\nb",
        88.8,
    );
    both("pre a\\n__", "white-space:pre", "a\n  ", 73.8);
    both("pre-wrap a\\n___", "white-space:pre-wrap", "a\n   ", 73.8);
    both("pre-line \\nx", "white-space:pre-line", "\nx", 73.8);
    both("a<br>&nbsp;", "", "a<br>&nbsp;", 73.8);
}

#[test]
fn list_items_table_cells_and_flex_items_follow_the_same_rule() {
    assert_after_at(
        "li",
        "<ul style=\"margin:0;padding:0 0 0 20px\"><li>a<br></li></ul><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "td",
        "<table style=\"border-collapse:collapse\"><tr><td style=\"padding:0\">a<br></td><td style=\"padding:0\">b</td></tr></table><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "flex item",
        "<div style=\"display:flex\"><div>a<br>b<br></div></div><p>AFTER</p>",
        73.8,
    );
    assert_after_at("rtl", "<div dir=\"rtl\">a<br>b<br></div><p>AFTER</p>", 73.8);
    assert_after_at(
        "inline-block holding a<br>",
        "<div><span style=\"display:inline-block\">a<br></span></div><p>AFTER</p>",
        58.8,
    );
}

#[test]
fn text_directly_in_the_body_is_the_same_case() {
    // F37d: the body is the inline root.
    assert_after_at(
        "Line1<br>Line2<br> then a block",
        "Line1<br>Line2<br><div>AFTER</div>",
        73.8,
    );
}

#[test]
fn an_inline_box_after_the_br_is_content() {
    // Not a trailing empty line: 4 pt lower than the 58.8 + 15 of a bare second line is the inline-block strut
    // (F183b), which this finding leaves alone; the line itself must stay.
    let words = printed_words(&render_with(
        HEAD,
        "<div>a<br><span style=\"display:inline-block;width:0;height:30px\"></span></div><p>AFTER</p>",
    ));
    assert!(
        after(&words).y_min > 80.0,
        "the line holding the inline box stays (AFTER at {:.1})",
        after(&words).y_min
    );
}

/// `w0000<br>w0001<br>…` of the F30f-d exact-fit probes, 14px/21px, a card with padding and a border.
fn exact(n: usize, pad_bottom: u32) -> String {
    let words: String = (0..n).map(|i| format!("w{i:04}<br>")).collect();
    format!(
        "<div style=\"padding:20px 20px {pad_bottom}px;border:1px solid #888;border-radius:8px\">{words}</div><p>AFTER</p>"
    )
}

#[test]
fn an_exact_fit_card_ending_in_br_stays_on_its_page() {
    let head = "@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}";
    // f30fdv/a: one page in Chrome, AFTER at these tops; the empty line used to push AFTER to page 2.
    for (n, pad, y) in [(44, 22, 770.6), (45, 20, 784.8), (45, 21, 785.6)] {
        let words = printed_words(&render_with(head, &exact(n, pad)));
        let a = after(&words);
        assert_eq!(a.page, 1, "n={n} pad={pad}: AFTER stays on page 1");
        assert!(
            (a.y_min - y).abs() <= 1.0,
            "n={n} pad={pad}: AFTER at {:.1} pt, Chrome {y}",
            a.y_min
        );
    }
}

#[test]
fn the_widows_rule_does_not_count_the_empty_line() {
    let head = "@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}";
    // f30fdv/a/exact_n47_pb21: Chrome 45 lines on page 1, the last two and AFTER on page 2.
    let words = printed_words(&render_with(head, &exact(47, 21)));
    let on_first = words.iter().filter(|w| w.page == 1).count();
    assert_eq!(on_first, 45, "words on page 1");
    let a = after(&words);
    assert_eq!(a.page, 2);
    assert!((a.y_min - 92.6).abs() <= 1.0, "AFTER at {:.1}", a.y_min);
}

// ---------------------------------------------------------------------------------------------------------------
// Validator round 1 (plan §7.1): break-* on inline-level boxes, and what keeps the line after a `<br>`.
// ---------------------------------------------------------------------------------------------------------------

/// Every printed word, in reading order, is `(page, text, Chrome's top)` within a point. Fails when a word is lost.
fn assert_words(what: &str, body: &str, expected: &[(usize, &str, f32)]) {
    let words = printed_words(&render_with(HEAD, body));
    let got: Vec<String> = words
        .iter()
        .map(|w| format!("p{}:{}@{:.1}", w.page, w.text, w.y_min))
        .collect();
    assert_eq!(
        words.len(),
        expected.len(),
        "{what}: expected {expected:?}, printed {got:?}"
    );
    for (w, (page, text, y)) in words.iter().zip(expected) {
        assert!(
            w.page == *page && w.text == *text && (w.y_min - y).abs() <= 1.0,
            "{what}: expected p{page}:{text}@{y}, printed {got:?}"
        );
    }
}

#[test]
fn a_forced_break_on_an_inline_box_is_ignored_and_loses_no_text() {
    // css-break-3 §3.1: break-before / break-after apply to block-level boxes, flex and grid items, table rows and
    // row groups. Chrome m141 prints one page for each of these (out/scratch/f66fv/{d,c,e,h}).
    assert_words(
        "d1: br with page-break-after in a line",
        "<div>a<br style=\"page-break-after:always\"></div>b",
        &[(1, "a", 43.8), (1, "b", 58.8)],
    );
    assert_words(
        "c6: empty span with break-after after text",
        "<div>a<span style=\"break-after:page\"></span></div>b",
        &[(1, "a", 43.8), (1, "b", 58.8)],
    );
    assert_words(
        "c7: br then an empty span with break-after, nothing after",
        "<div>a<br><span style=\"break-after:page\"></span></div>",
        &[(1, "a", 43.8)],
    );
    assert_words(
        "d6: br, then an empty b with break-after",
        "<div>a<br style=\"page-break-after:always\"><b style=\"break-after:page\"></b></div>b",
        &[(1, "a", 43.8), (1, "b", 58.8)],
    );
    // An inline-block sits 2 pt lower than Chrome's (the F183b strut), so only the pages and the order are checked.
    let words = printed_words(&render_with(
        HEAD,
        "<div>a <span style=\"display:inline-block;break-after:page\">x</span></div>b",
    ));
    let got: Vec<(usize, &str)> = words.iter().map(|w| (w.page, w.text.as_str())).collect();
    assert_eq!(
        got,
        [(1, "a"), (1, "x"), (1, "b")],
        "inline-block with break-after: one page, nothing lost"
    );
    assert!(
        words[2].y_min > words[0].y_min + 10.0,
        "b is below the line"
    );
}

#[test]
fn a_forced_break_on_a_br_between_paragraphs_does_not_overlap_or_reorder() {
    // e2: a and b were drawn at the same y; d3 / d4 printed b before a.
    assert_words(
        "e2: br clear=all with page-break-before in a p",
        "<p>a<br clear=all style='page-break-before:always'></p><p>b</p>",
        &[(1, "a", 43.8), (1, "b", 58.8)],
    );
    // h7: a paragraph that holds only a br with break-after is one line, on one page.
    assert_words(
        "h7: p with only a br with break-after",
        "<p>a</p><p><br style='break-after:page'></p><p>b</p>",
        &[(1, "a", 43.8), (1, "b", 73.8)],
    );
}

#[test]
fn the_word_idiom_of_a_span_around_a_br_keeps_its_line() {
    // g1 / h3 / h4: `<p><span><br clear=all style='page-break-before:always'></span></p>` is one line in Chrome.
    assert_words(
        "g1: Word page break idiom",
        "<p>a</p><p class=MsoNormal><span><br clear=all style='page-break-before:always'></span></p><p>b</p>",
        &[(1, "a", 43.8), (1, "b", 73.8)],
    );
    assert_words(
        "h4: br with page-break-before in a p",
        "<p>a</p><p><br style='page-break-before:always'></p><p>b</p>",
        &[(1, "a", 43.8), (1, "b", 73.8)],
    );
}

#[test]
fn a_forced_break_on_a_block_level_box_still_breaks() {
    // The reading stays for what the spec names: blocks, and an inline element made block-level.
    assert_words(
        "div with break-after",
        "<div style=\"break-after:page\">a</div>b",
        &[(1, "a", 43.8), (2, "b", 43.8)],
    );
    assert_words(
        "span with display:block and break-before",
        "<div>a<span style=\"display:block;break-before:page\">b</span></div>",
        &[(1, "a", 43.8), (2, "b", 43.8)],
    );
    assert_words(
        "flex item that is a span with break-after",
        "<div style=\"display:flex;flex-direction:column\"><span style=\"break-after:page\">a</span><span>b</span></div>",
        &[(1, "a", 43.8), (2, "b", 43.8)],
    );
}

#[test]
fn an_inline_box_with_padding_border_or_a_wbr_after_the_br_keeps_the_line() {
    // CSS 2.1 §9.4.2: a line box with an inline element with non-zero margin, padding or border is not zero-height.
    // a/br_empty_span_pad, br_empty_span_border, wbr: Chrome 73.8, the F66f takeback gave 58.6.
    assert_after_at(
        "empty span with horizontal padding",
        "<div>a<br><span style=\"padding:0 4px\"></span></div><p>AFTER</p>",
        73.8,
    );
    assert_after_at(
        "empty span with a border",
        "<div>a<br><span style=\"border-left:1px solid #000\"></span></div><p>AFTER</p>",
        73.8,
    );
    assert_after_at(
        "wbr after the br",
        "<div>a<br><wbr></div><p>AFTER</p>",
        73.8,
    );
    assert_after_at(
        "empty span with horizontal margin",
        "<div>a<br><span style=\"margin:0 4px\"></span></div><p>AFTER</p>",
        73.8,
    );
    assert_after_at("two wbr", "<div>a<br><wbr><wbr></div><p>AFTER</p>", 73.8);
    // Guards: nothing in the inline axis, or only the closing edge of a box, makes no line (Chrome k5-k15).
    assert_after_at(
        "empty span without box",
        "<div>a<br><span></span></div><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "empty span with vertical padding",
        "<div>a<br><span style=\"padding:4px 0\"></span></div><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "empty span with vertical margin",
        "<div>a<br><span style=\"margin:4px 0\"></span></div><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "empty a with an id",
        "<div>a<br><a id=x></a></div><p>AFTER</p>",
        58.8,
    );
    assert_after_at(
        "padded span that ends in the br: its closing edge is no line",
        "<div>a<br><span style=\"padding:0 4px\">x<br></span></div><p>AFTER</p>",
        73.8,
    );
}
