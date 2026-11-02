//! Author `orphans` / `widows` are read and honoured by the fragmenter (F30c-o).
//!
//! css-break-3 §3.3: `orphans` is the minimum number of lines of a block container left before a break, `widows` the
//! minimum left after it (`<integer [1,∞]>`, initially 2, inherited). §4.4: a break between line boxes is allowed only
//! with at least `orphans` lines before it and `widows` after it; the rules are dropped only when no break keeps the
//! content inside the fragmentainer. Before this change the fork used the initial values for every paragraph: a print
//! sheet with `p { orphans: 3; widows: 3 }` left a stub of two lines at the page bottom where Chrome moves the
//! paragraph (since the F30c pre-advance, which starts a paragraph wherever its first two lines fit).
//!
//! Every number is Chrome's (m141, A4, 15 mm unless noted, Inter, `14px/21px`, `p { margin: 0 }`, lines `w0000<br>w0001…`;
//! a page holds 48 such lines, 1008 px against Chrome's 1010 px and ours 1009.13 px). A spacer of `1008 - 21 k` px leaves
//! room for `k` lines (42 to 43 px or more away from a line boundary, so the 1 px capacity difference, F30f-c, does not
//! decide). Probes: `out/scratch/f30co/` (plan §8.1).
//!
//! Where Chrome is not matched the test says so and pins ours: `calc()` in the value (Chrome reads it, the fork's reader
//! drops it), multicol (the fork ignores even the initial value inside columns: F30d-b), widows larger than a page,
//! `break-after: avoid` on the heading before a short paragraph (F215) and, on purpose, a short paragraph that fits a
//! page and has no allowed break (ADR-0010: the spec moves it whole, Chrome splits it).
//!
//! F30c-o3 (tests 10-13 below): a paragraph taller than a page, a paragraph in a `<section>`, `<li>`, `<td>` or with
//! padding keep their orphans like the body-level one, and an `orphans` larger than a page moves the paragraph to the
//! next page where it relaxes to the initial values (Chrome 0 / 48 / 48 / 4) instead of printing a line below the content box.

use htmltopdf::{RenderOptions, Renderer};

/// The content box of A4 with 15 mm margins, in pt from the page top: its bottom.
const CONTENT_BOTTOM: f32 = 799.37;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn red60() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/red60.png");
    std::fs::read(&path).unwrap_or_else(|_| panic!("{} missing", path.display()))
}

/// The whole style sheet is `css`; `body` goes into `<body>`.
fn render_doc(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .image("red60.png", red60())
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{css}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Body-level shapes: A4, 15 mm, `14px/21px`, no margins; `extra` is appended to the sheet.
fn render(extra: &str, body: &str) -> Vec<u8> {
    render_doc(
        &format!(
            "@page{{size:A4;margin:15mm}}body{{font:14px/21px Inter;margin:0}}p{{margin:0}}{extra}"
        ),
        body,
    )
}

/// `w0000<br>w0001<br>…` from word `from` (no trailing `<br>`).
fn lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

/// A spacer that leaves room for `k` lines of 21 px on page 1.
fn room(k: usize) -> String {
    format!("<div style=\"height:{}px\"></div>", 1008 - 21 * k)
}

fn spacer(h: usize) -> String {
    format!("<div style=\"height:{h}px\"></div>")
}

struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-slo-{}-{n}.pdf", std::process::id()));
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
                y_max: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

/// How many `wNNNN` words (one per line here) are printed on each page.
fn per_page(words: &[Word]) -> Vec<usize> {
    let pages = words.iter().map(|w| w.page).max().unwrap_or(0);
    let mut per = vec![0; pages];
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        per[w.page - 1] += 1;
    }
    per
}

fn after(words: &[Word]) -> (usize, f32) {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    (a[0].page, a[0].y_min)
}

/// No glyph of a `wNNNN` word is below `limit` pt.
fn assert_inside(words: &[Word], limit: f32) {
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        assert!(
            w.y_max <= limit,
            "{} on page {} ends at {:.1} pt, below {limit:.1} pt",
            w.text,
            w.page,
            w.y_max
        );
    }
}

/// `AFTER` is on `page` at `y` pt (±1 pt).
fn assert_after(words: &[Word], page: usize, y: f32) {
    let (p, got) = after(words);
    assert_eq!(p, page, "AFTER on page {p}");
    assert!(
        (got - y).abs() <= 1.0,
        "AFTER at {got:.1} pt, Chrome {y:.1} pt"
    );
}

/// Every word of `0..n` is printed once, in order, and no line stands below the content box. Lines of a `<br>` paragraph
/// keep their 21 px step on each page, so no fragment is placed over another.
fn assert_lines_in_order(words: &[Word], n: usize) {
    let names: Vec<&str> = words
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.text.as_str())
        .collect();
    let want: Vec<String> = (0..n).map(|i| format!("w{i:04}")).collect();
    assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>());
    assert_inside(words, CONTENT_BOTTOM);
}

const LINE: f32 = 15.75;

fn assert_line_step(words: &[Word]) {
    let ws: Vec<&Word> = words.iter().filter(|w| w.text.starts_with('w')).collect();
    for pair in ws.windows(2) {
        if pair[0].page == pair[1].page {
            let step = pair[1].y_min - pair[0].y_min;
            assert!(
                (step - LINE).abs() < 0.3,
                "{} to {} on page {}: {step:.2} pt, expected {LINE}",
                pair[0].text,
                pair[1].text,
                pair[0].page
            );
        }
    }
}

// ── helpers of this file ──────────────────────────────────────────────────────────────────────────────────────────

/// A 9-line paragraph (`attrs` on the `<p>`) below a spacer with room for `k` lines, then `AFTER`.
fn nine(extra: &str, attrs: &str, k: usize) -> Vec<Word> {
    printed_words(&render(
        extra,
        &format!("{}<p{attrs}>{}</p><p>AFTER</p>", room(k), lines(9)),
    ))
}

/// `per` lines per page and `AFTER` on `page` at `y` pt (±1 pt), every line printed once, in order, inside the page.
fn assert_case(words: &[Word], n: usize, per: &[usize], page: usize, y: f32, what: &str) {
    assert_eq!(per_page(words), per, "{what}");
    assert_lines_in_order(words, n);
    assert_after(words, page, y);
}

// ── 1. `orphans: 3` moves a paragraph that has room for two lines ─────────────────────────────────────────────────

/// A 9-line paragraph, room for 2 lines, `p { orphans: 3; widows: 3 }`. Chrome: 0 / 9, `AFTER` on page 2 at 186.3 pt.
/// Ours at `0feb944f`: 2 / 7 (`AFTER` at 154.8 pt).
#[test]
fn orphans_three_moves_a_paragraph_with_room_for_two_lines() {
    let words = nine("p{orphans:3;widows:3}", "", 2);
    assert_case(&words, 9, &[0, 9], 2, 186.3, "orphans 3, room 2");
}

/// `orphans: 4`: room for two or three lines is not enough either. Chrome 0 / 9 in both.
#[test]
fn orphans_four_moves_it_with_room_for_two_or_three() {
    for k in [2, 3] {
        for css in ["p{orphans:4;widows:4}", "p{orphans:4}"] {
            let words = nine(css, "", k);
            assert_case(&words, 9, &[0, 9], 2, 186.3, &format!("{css}, room {k}"));
        }
    }
}

/// Where the paragraph already goes where Chrome puts it: the declaration must not move these. Chrome: `orphans: 3` with
/// room for 0 or 1 line 0 / 9, with 3, 4, 5 lines 3 / 6, 4 / 5, 5 / 4; `orphans: 4` 0 / 9, 0 / 9, 4 / 5, 5 / 4 for 0, 1,
/// 4, 5.
#[test]
fn the_declaration_leaves_the_splits_that_already_match_chrome() {
    let cases: [(&str, usize, &[usize], f32); 10] = [
        ("p{orphans:3;widows:3}", 0, &[0, 9], 186.3),
        ("p{orphans:3;widows:3}", 1, &[0, 9], 186.3),
        ("p{orphans:3;widows:3}", 3, &[3, 6], 139.1),
        ("p{orphans:3;widows:3}", 4, &[4, 5], 123.3),
        ("p{orphans:3;widows:3}", 5, &[5, 4], 107.6),
        ("p{orphans:4;widows:4}", 0, &[0, 9], 186.3),
        ("p{orphans:4;widows:4}", 1, &[0, 9], 186.3),
        ("p{orphans:4;widows:4}", 4, &[4, 5], 123.3),
        ("p{orphans:4;widows:4}", 5, &[5, 4], 107.6),
        ("p{orphans:2;widows:2}", 2, &[2, 7], 154.8),
    ];
    for (css, k, per, y) in cases {
        let words = nine(css, "", k);
        assert_case(&words, 9, per, 2, y, &format!("{css}, room {k}"));
    }
}

// ── 2. `widows` pulls lines back ──────────────────────────────────────────────────────────────────────────────────

/// A paragraph at the top of the page. Chrome: `widows: 3` leaves three lines (50 lines: 47 / 3, 49: 46 / 3, 51: 48 / 3),
/// `widows: 4` four (50: 46 / 4, 49: 45 / 4), `widows: 1` allows one (49: 48 / 1). Ours at `0feb944f`: 48 / 2, 47 / 2, 48 / 3,
/// 48 / 2, 47 / 2, 47 / 2.
#[test]
fn widows_pull_lines_back_to_the_next_page() {
    let cases: [(usize, usize, &[usize], f32); 7] = [
        (3, 50, &[47, 3], 91.8),
        (3, 49, &[46, 3], 91.8),
        (3, 51, &[48, 3], 91.8),
        (4, 50, &[46, 4], 107.6),
        (4, 49, &[45, 4], 107.6),
        (1, 49, &[48, 1], 60.3),
        (3, 52, &[48, 4], 107.6),
    ];
    for (w, n, per, y) in cases {
        let words = printed_words(&render(
            &format!("p{{widows:{w}}}"),
            &format!("<p>{}</p><p>AFTER</p>", lines(n)),
        ));
        assert_case(&words, n, per, 2, y, &format!("widows {w}, {n} lines"));
    }
}

// ── 3. Inheritance and the cascade ────────────────────────────────────────────────────────────────────────────────

/// The `orphans: 3` result of test 1 whichever way the value reaches the paragraph: inherited from `body` or `html`, a
/// `style` attribute, a print `@media` block, an `@layer`, `!important` (the author's sheet beats a plain `style`; the
/// `style` attribute's important one beats the sheet's), the keywords `inherit`, `unset` and `revert` (all inherit the
/// parent's value for an inherited property). Chrome 0 / 9 in each. The last ones give back the initial value: a later
/// rule with a higher specificity, `initial`; Chrome 2 / 7.
#[test]
fn inherited_and_cascaded() {
    let moved: [(&str, &str); 11] = [
        ("body{orphans:3}", ""),
        ("html{orphans:3}", ""),
        ("", " style=\"orphans:3\""),
        ("@media print{p{orphans:3}}", ""),
        ("@layer a{p{orphans:3}}", ""),
        ("body{orphans:3}", " style=\"orphans:inherit\""),
        ("body{orphans:3}", " style=\"orphans:unset\""),
        ("body{orphans:3}", " style=\"orphans:revert\""),
        ("p{orphans:3 !important}", " style=\"orphans:2\""),
        ("p{orphans:2 !important}", " style=\"orphans:3 !important\""),
        ("p{orphans:3}", " class=\"y\""),
    ];
    for (css, attrs) in moved {
        let words = nine(css, attrs, 2);
        assert_case(&words, 9, &[0, 9], 2, 186.3, &format!("{css} {attrs}"));
    }
    let initial: [(&str, &str); 3] = [
        ("p{orphans:3} .x{orphans:2}", " class=\"x\""),
        ("body{orphans:3}", " style=\"orphans:initial\""),
        ("p{orphans:3} .x{orphans:2}", " class=\"x y\""),
    ];
    for (css, attrs) in initial {
        let words = nine(css, attrs, 2);
        assert_case(&words, 9, &[2, 7], 2, 154.8, &format!("{css} {attrs}"));
    }
}

/// A `div` whose own text is the paragraph (the block container is the `div`): Chrome 0 / 9 with `orphans: 3` on it.
#[test]
fn a_div_with_text_takes_its_own_value() {
    let words = printed_words(&render(
        "",
        &format!(
            "{}<div style=\"orphans:3\">{}</div><p>AFTER</p>",
            room(2),
            lines(9)
        ),
    ));
    assert_case(&words, 9, &[0, 9], 2, 186.3, "div with text");
}

// ── 4. Invalid values keep the initial rule ───────────────────────────────────────────────────────────────────────

/// `orphans: 0`, `-1`, `2.5`, `2.0`, `foo`, `3px` are invalid (css-break-3 §3.3: `<integer [1,∞]>`); the declaration is
/// dropped and the initial value applies: Chrome 2 / 7, `AFTER` at 154.8 pt.
#[test]
fn invalid_values_keep_the_initial_rule() {
    for v in ["0", "-1", "2.5", "2.0", "foo", "3px"] {
        let words = nine(&format!("p{{orphans:{v}}}"), "", 2);
        assert_case(&words, 9, &[2, 7], 2, 154.8, &format!("orphans: {v}"));
    }
}

/// `calc(3)` is a valid `<integer>` in Chrome (0 / 9, `AFTER` at 186.3 pt). The fork's reader drops `calc()` (plan 4),
/// so ours keeps 2 / 7. Pinned; the reader goes when Stylo computes the property.
#[test]
fn calc_in_the_value_is_not_read_yet() {
    let words = nine("p{orphans:calc(3)}", "", 2);
    assert_eq!(per_page(&words), [2, 7], "ours; Chrome 0 / 9");
}

// ── 5. `orphans: 1` keeps a single line ───────────────────────────────────────────────────────────────────────────

/// A 30-line paragraph, `orphans: 1`: Chrome puts one line on page 1 with room for one line (1 / 29, `AFTER` at 501.3 pt),
/// two lines with room for two (2 / 28, 485.6 pt) and moves it whole with none (0 / 30, 517.1 pt).
#[test]
fn orphans_one_keeps_a_single_line() {
    for css in ["p{orphans:1;widows:1}", "p{orphans:1}"] {
        for (k, per, y) in [
            (0, [0, 30], 517.1),
            (1, [1, 29], 501.3),
            (2, [2, 28], 485.6),
        ] {
            let words = printed_words(&render(
                css,
                &format!("{}<p>{}</p><p>AFTER</p>", room(k), lines(30)),
            ));
            assert_case(&words, 30, &per, 2, y, &format!("{css}, room {k}"));
        }
    }
}

// ── 6. Values larger than a page lose nothing ─────────────────────────────────────────────────────────────────────

/// 100 lines at the top of a page (48 lines fit) and after a 300 px spacer (33 lines of room), with values the page cannot
/// keep. Chrome gives up what it cannot satisfy and keeps the rest:
///
/// - `orphans: 60`: no break keeps 60 lines, so the paragraph starts the next page when something is above it (0 / 48 / 48 / 4)
///   and breaks naturally at the top of a page (48 / 48 / 4), `widows: 2` kept;
/// - `orphans: 30; widows: 30`: page 1 breaks at its natural place (48 or 33 lines); the next page pulls back to leave 30 lines
///   (37 / 30), and where that would leave it fewer than 30 itself, it keeps its 30 and gives up the widows (48 / 30 / 22);
/// - `widows: 30`: 48 / 22 / 30 and 33 / 37 / 30.
///
/// Every line is printed once, in order, inside the page.
#[test]
fn a_value_larger_than_a_page_loses_nothing() {
    type Case = (usize, usize, usize, &'static [usize], usize, f32);
    let cases: [Case; 6] = [
        (60, 2, 0, &[48, 48, 4], 3, 107.6),
        (60, 2, 300, &[0, 48, 48, 4], 4, 107.6),
        (30, 30, 0, &[48, 30, 22], 3, 391.1),
        (30, 30, 300, &[33, 37, 30], 3, 517.1),
        (2, 30, 0, &[48, 22, 30], 3, 517.1),
        (2, 30, 300, &[33, 37, 30], 3, 517.1),
    ];
    for (o, w, s, per, page, y) in cases {
        let sp = if s == 0 { String::new() } else { spacer(s) };
        let words = printed_words(&render(
            &format!("p{{orphans:{o};widows:{w}}}"),
            &format!("{sp}<p>{}</p><p>AFTER</p>", lines(100)),
        ));
        assert_case(
            &words,
            100,
            per,
            page,
            y,
            &format!("orphans {o}, widows {w}, spacer {s}"),
        );
        assert_line_step(&words);
    }
}

// ── 7. Table cells ────────────────────────────────────────────────────────────────────────────────────────────────

/// A collapsed table whose single cell holds a 60 px image and 150 words (9 to 10 lines of 16 px Inter), after a grey
/// spacer of `h` px on an A4 page without margins. `border` is 1 px (`u_col1`) or 2 px (`u_col2`); `td { orphans; widows }`.
fn cell(h: usize, orphans: usize, border: usize) -> Vec<usize> {
    let words = (1..=150)
        .map(|i| format!("w{i:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    let css = format!(
        "@page{{size:A4;margin:0}}body{{margin:0;font:16px Inter}}table{{border-collapse:collapse}}\
         td{{padding:5px;border:{border}px solid #000;vertical-align:top;orphans:{orphans};widows:{orphans}}}"
    );
    let body = format!(
        "<div style=\"height:{h}px;background:#ccc\"></div><table><tr><td><img src=\"red60.png\" width=\"60\" height=\"60\"> \
         {words}</td></tr></table>"
    );
    per_page(&printed_words(&render_doc(&css, &body)))
}

/// F183 §7.6: a cell with `orphans: 4` and room for 2 to 3 lines moves whole (Chrome 0 | 150 words for every spacer
/// from 996 to 1032 px, both border widths). Ours at `0feb944f`: 48 | 102 or 32 | 118.
#[test]
fn a_cell_with_orphans_four_moves_whole() {
    for border in [1, 2] {
        for h in (996..=1032).step_by(4) {
            assert_eq!(
                cell(h, 4, border),
                [0, 150],
                "orphans 4, border {border}, spacer {h}"
            );
        }
    }
}

/// The same cell with the initial `orphans: 2` splits: Chrome 48 | 102 for spacers up to 1012 px and 32 | 118 from 1016 px
/// (1 px border; with a 2 px border ours is one word off Chrome's, as before).
#[test]
fn a_cell_with_orphans_two_still_splits() {
    for h in (996..=1032).step_by(4) {
        let want: [usize; 2] = if h <= 1012 { [48, 102] } else { [32, 118] };
        assert_eq!(cell(h, 2, 1), want, "orphans 2, border 1, spacer {h}");
        let got = cell(h, 2, 2);
        assert!(
            got[0].abs_diff(want[0]) <= 1 && got.iter().sum::<usize>() == 150,
            "orphans 2, border 2, spacer {h}: {got:?}, Chrome {want:?}"
        );
    }
}

// ── 8. Nested and padded paragraphs ───────────────────────────────────────────────────────────────────────────────

/// A 60-line paragraph with `orphans: 3` in a `<section>` with room for 2 lines starts on the next page (Chrome 0 / 48 /
/// 12, `AFTER` on page 3 at 233.6 pt); with room for 3 it starts there (3 / 48 / 9, 186.3 pt); `orphans: 2` with room for 2
/// does (2 / 48 / 10, 202.1 pt).
#[test]
fn a_nested_paragraph_keeps_three_lines_too() {
    let cases: [(usize, usize, &[usize], f32); 4] = [
        (3, 1, &[0, 48, 12], 233.6),
        (3, 2, &[0, 48, 12], 233.6),
        (3, 3, &[3, 48, 9], 186.3),
        (2, 2, &[2, 48, 10], 202.1),
    ];
    for (o, k, per, y) in cases {
        let words = printed_words(&render(
            "",
            &format!(
                "{}<section><p style=\"orphans:{o}\">{}</p></section><p>AFTER</p>",
                room(k),
                lines(60)
            ),
        ));
        assert_case(
            &words,
            60,
            per,
            3,
            y,
            &format!("nested, orphans {o}, room {k}"),
        );
    }
}

/// A padded, bordered body-level paragraph (the F30f-d branch; 9 px above its first line, so a room of 3 lines holds two):
/// `orphans: 3` moves it whole (Chrome 0 / 30, `AFTER` at 530.6 pt), `orphans: 2` splits it 2 / 28 (492.3 pt).
#[test]
fn a_padded_paragraph_keeps_three_lines_too() {
    let cases: [(usize, usize, &[usize], f32); 3] = [
        (3, 2, &[0, 30], 530.6),
        (3, 3, &[0, 30], 530.6),
        (2, 3, &[2, 28], 492.3),
    ];
    for (o, k, per, y) in cases {
        let words = printed_words(&render(
            "",
            &format!(
                "{}<p style=\"orphans:{o};padding:8px;border:1px solid #888\">{}</p><p>AFTER</p>",
                room(k),
                lines(30)
            ),
        ));
        assert_case(
            &words,
            30,
            per,
            2,
            y,
            &format!("padded, orphans {o}, room {k}"),
        );
    }
}

// ── 9. Columns keep their output ──────────────────────────────────────────────────────────────────────────────────

/// The fork ignores `orphans` / `widows` inside columns, even the initial value (F30d-b). A two-column box with
/// `p { orphans: 4 }` renders byte for byte like the same box without it. Chrome: 40 lines per column box page, `AFTER` at
/// 675 pt; ours is the same here.
#[test]
fn columns_keep_their_output() {
    let body: String = format!(
        "<div style=\"columns:2;column-gap:20px\">{}</div><p>AFTER</p>",
        (0..8)
            .map(|i| format!(
                "<p>{}</p>",
                (i * 10..i * 10 + 10)
                    .map(|n| format!("w{n:04}"))
                    .collect::<Vec<_>>()
                    .join("<br>")
            ))
            .collect::<String>()
    );
    let plain = render("", &body);
    let declared = render("p{orphans:4;widows:4}", &body);
    assert_eq!(plain, declared, "the declaration changed the columns");
    assert_eq!(per_page(&printed_words(&plain)), [80]);
}

// ── 10. F30c-o3: a paragraph taller than a page keeps its orphans where no break keeps its widows ──────────────────

/// Shapes of the F30c-o3 probes (`f30cov/g2/`, `ov/`): the same 60 or 100 lines wrapped as a body-level `<p>`, in a
/// `<section>`, a `<li>`, a `<td>` of a collapsed table, or in a padded and bordered `<p>`. `css` is the whole rule list
/// after the page set-up (the properties go on `p`, `li` and `td` alike).
fn shaped(kind: &str, css: &str, spacer_px: usize, n: usize) -> Vec<Word> {
    let sp = if spacer_px == 0 {
        String::new()
    } else {
        spacer(spacer_px)
    };
    let l = lines(n);
    let body = match kind {
        "body" => format!("{sp}<p>{l}</p><p>AFTER</p>"),
        "nest" => format!("{sp}<section><p>{l}</p></section><p>AFTER</p>"),
        "pad" => format!("{sp}<p style=\"padding:8px;border:1px solid #888\">{l}</p><p>AFTER</p>"),
        "li" => {
            format!("{sp}<ul style=\"margin:0;padding-left:20px\"><li>{l}</li></ul><p>AFTER</p>")
        }
        "td" => format!(
            "{sp}<table style=\"border-collapse:collapse;width:100%\"><tr><td style=\"vertical-align:top\">{l}</td></tr></table><p>AFTER</p>"
        ),
        _ => panic!("unknown shape {kind}"),
    };
    printed_words(&render(css, &body))
}

/// 60 lines, `orphans: 30; widows: 47` (or 50), room for 40 lines. No break leaves 30 lines before it and 47 after it
/// (30 + 47 > 60), and the paragraph is taller than a page, so the class A break before it does not avoid the overflow
/// either: css-break-3 §4.4 lets the widows go. Chrome keeps the orphans: 30 / 30, `AFTER` on page 2 at 517.1 pt. Ours
/// at `cd9fb62e` moved the paragraph whole (0 / 30 / 30, three pages).
#[test]
fn a_paragraph_taller_than_a_page_starts_where_its_orphans_fit_when_no_break_keeps_the_widows() {
    for w in [47, 50] {
        let words = shaped("body", &format!("p{{orphans:30;widows:{w}}}"), 168, 60);
        assert_case(&words, 60, &[30, 30], 2, 517.1, &format!("widows {w}"));
        assert_line_step(&words);
    }
}

/// The nested splitter keeps the orphans too. 100 lines on an empty page, `orphans: 30; widows: 30`: page 2 takes 48,
/// the 52 left cannot give 30 after a break that leaves 30 before it, so the next page keeps its 30 and gives up the
/// widows (Chrome 0 / 48 / 30 / 22, `AFTER` on page 4 at 391.1 pt for `<section>` and `<li>`, 391.8 for a cell; the
/// padded paragraph has 9 px above its first line, 0 / 47 / 30 / 23, 413.6 pt). Ours at `cd9fb62e`: 48 / 48 / 4 (and
/// 47 / 48 / 5). With 60 lines and room for 40, `orphans: 30; widows: 47`: 30 / 30 (517.1 pt; ours 40 / 20).
#[test]
fn a_long_nested_paragraph_keeps_its_orphans() {
    for kind in ["nest", "li", "td", "pad"] {
        let css = "p{orphans:30;widows:30}li{orphans:30;widows:30}td{orphans:30;widows:30}";
        let words = shaped(kind, css, 1008, 100);
        let (per, y): (&[usize], f32) = match kind {
            "pad" => (&[0, 47, 30, 23], 413.6),
            "td" => (&[0, 48, 30, 22], 391.8),
            _ => (&[0, 48, 30, 22], 391.1),
        };
        assert_case(&words, 100, per, 4, y, &format!("{kind}, 100 lines"));
    }
    for kind in ["nest", "li", "td"] {
        let css = "p{orphans:30;widows:47}li{orphans:30;widows:47}td{orphans:30;widows:47}";
        let words = shaped(kind, css, 168, 60);
        let y = if kind == "td" { 517.8 } else { 517.1 };
        assert_case(&words, 60, &[30, 30], 2, y, &format!("{kind}, 60 lines"));
    }
}

/// `widows` larger than a page's lines on a continuation fragment (F30c-o2, probes `out/scratch/f30co3/wid`): 100 lines at the top
/// of a page, `orphans: 10`. Chrome keeps the orphans and gives up the widows while a page can still hold them (`widows: 47`
/// and `48`: 0 / 48 / 10 / 42, `AFTER` at 706.1 pt) and ignores them when no page can (`widows: 49`, `50`: 0 / 48 / 48 / 4,
/// 107.6 pt). A `<section>`, `<li>`, cell and the body-level paragraph agree; the nested splitter pulled back to 10 / 42 for
/// both before the chooser knew the page (and was at Chrome by accident for 49 and 50, F30c-o3 first version: 48 / 10 / 42).
#[test]
fn widows_larger_than_a_page_are_ignored_on_a_continuation_fragment() {
    for kind in ["body", "nest", "li", "td"] {
        for (w, per, y) in [
            (47, [0, 48, 10, 42], 706.1),
            (48, [0, 48, 10, 42], 706.1),
            (49, [0, 48, 48, 4], 107.6),
            (50, [0, 48, 48, 4], 107.6),
        ] {
            let css = format!(
                "p{{orphans:10;widows:{w}}}li{{orphans:10;widows:{w}}}td{{orphans:10;widows:{w}}}"
            );
            let words = shaped(kind, &css, 1008, 100);
            assert_eq!(per_page(&words), per, "{kind}, widows {w}");
            assert_lines_in_order(&words, 100);
            if kind != "td" {
                assert_after(&words, 4, y);
            }
        }
    }
}

/// `widows` as large as the whole paragraph (a paragraph taller than a page: 52 or 60 lines, `widows: 60`) cannot be had at all.
/// Chrome ignores it in the first fragment and keeps the `orphans`: after 300 px (33 lines of room) `orphans: 1` gives 1 / 48 / 3
/// (`AFTER` on page 3 at 91.8 pt), `orphans: 2` 2 / 48 / 2 (76.1 pt), 60 lines with `orphans: 10` 10 / 48 / 2 (76.1 pt). Ours
/// at `cd9fb62e` moved the paragraph whole (0 / 48 / 4): the same page count, the first lines on the wrong page.
#[test]
fn widows_as_large_as_a_paragraph_taller_than_a_page_keep_the_orphans() {
    for (n, o, per, y) in [
        (52, 1, [1, 48, 3], 91.8),
        (52, 2, [2, 48, 2], 76.1),
        (60, 10, [10, 48, 2], 76.1),
    ] {
        let words = shaped("body", &format!("p{{orphans:{o};widows:60}}"), 300, n);
        assert_case(&words, n, &per, 3, y, &format!("{n} lines, orphans {o}"));
    }
}

// ── 11. `orphans` larger than a page ──────────────────────────────────────────────────────────────────────────────

/// A 100-line paragraph with `orphans: 49`, `50` or `52` (more than the 48 lines a page holds; 47 for the padded one,
/// which has 9 px above its first line, so `48` is too many there as well) after a spacer that leaves room for 0, 3 or
/// 10 lines. No break keeps that many lines on any page, so Chrome moves the paragraph to the next page, where it
/// relaxes to the initial 2 / 2: 0 / 48 / 48 / 4 (`AFTER` on page 4 at 107.6 pt), padded 0 / 47 / 48 / 5 (130.1 pt).
/// Ours at `cd9fb62e` started it in place with its `orphans` first lines "fitting" a 1 px strip: 1, 3 or 10 lines on
/// page 1 and one of them (`w0000` at 800 pt) below the content box. `td` is a cell with no padding.
#[test]
fn orphans_larger_than_a_page_move_the_paragraph_and_keep_every_line_inside() {
    for kind in ["nest", "td", "li", "pad"] {
        let first_too_big = if kind == "pad" { 48 } else { 49 };
        for o in [first_too_big, 50, 52] {
            for k in [0, 3, 10] {
                let css = format!("p{{orphans:{o}}}td{{orphans:{o}}}li{{orphans:{o}}}");
                let words = if kind == "td" {
                    printed_words(&render(
                        &css,
                        &format!(
                            "{}<table style=\"border-collapse:collapse;width:100%\"><tr><td style=\"vertical-align:top;padding:0\">{}</td></tr></table><p>AFTER</p>",
                            spacer(1008 - 21 * k),
                            lines(100)
                        ),
                    ))
                } else {
                    shaped(kind, &css, 1008 - 21 * k, 100)
                };
                let (per, y): (&[usize], f32) = if kind == "pad" {
                    (&[0, 47, 48, 5], 130.1)
                } else {
                    (&[0, 48, 48, 4], 107.6)
                };
                assert_case(
                    &words,
                    100,
                    per,
                    4,
                    y,
                    &format!("{kind}, orphans {o}, room {k}"),
                );
                assert_inside(&words, CONTENT_BOTTOM);
            }
        }
    }
}

/// `td { orphans: 60; widows: 2 }` in a fixed-layout table after a full page (`tbl/T6_wideorph_k0`): Chrome 0 / 48 / 48 /
/// 4. Ours at `cd9fb62e` printed `w0000` below the content box (1 px strip).
#[test]
fn a_cell_with_orphans_sixty_after_a_full_page_is_not_cut_into_a_strip() {
    let words = printed_words(&render(
        "td{orphans:60;widows:2}",
        &format!(
            "{}<table style=\"border-collapse:collapse;width:100%;table-layout:fixed\"><tr><td style=\"vertical-align:top;padding:0\">{}</td></tr></table><p>AFTER</p>",
            spacer(1008),
            lines(100)
        ),
    ));
    assert_case(&words, 100, &[0, 48, 48, 4], 4, 107.6, "td orphans 60");
}

/// The values just below a page's lines are not touched: `orphans` 44 to 48 (47 for the padded paragraph) already
/// started the next page, as Chrome does. Green before and after F30c-o3.
#[test]
fn orphans_that_a_page_can_keep_still_move_the_paragraph() {
    for kind in ["nest", "td", "li", "pad"] {
        let top = if kind == "pad" { 47 } else { 48 };
        for o in [44, 46, 47, top] {
            for k in [0, 3, 10] {
                let css = format!("p{{orphans:{o}}}td{{orphans:{o}}}li{{orphans:{o}}}");
                let words = shaped(kind, &css, 1008 - 21 * k, 100);
                let (per, y): (&[usize], f32) = if kind == "pad" {
                    (&[0, 47, 48, 5], 130.1)
                } else {
                    (&[0, 48, 48, 4], 107.6)
                };
                // A cell has 1 px of border spacing, so only the line counts are compared there.
                assert_eq!(per_page(&words), per, "{kind}, orphans {o}, room {k}");
                assert_inside(&words, CONTENT_BOTTOM);
                if kind != "td" {
                    assert_after(&words, 4, y);
                }
            }
        }
    }
}

// ── 12. A short paragraph that fits a page and has no allowed break moves whole (ADR-0010) ─────────────────────────

/// Deliberately not Chrome. A paragraph of `n` lines that fits a page has no allowed break between its lines when
/// `n < orphans + widows` (css-break-3 §3.3: "all lines in the block must be kept together"), and the class A break before it
/// avoids the overflow, so §4.4 does not relax the rule: it moves whole. Chrome m141 splits it (it fails Blink's and
/// WebKit's own `printing/page-break-orphans-and-widows` test). ADR-0010; do not "fix" this towards Chrome.
///
/// - 5 lines, `orphans: 3; widows: 3`, room for 3: Chrome 3 / 2 (`AFTER` at 76.1 pt), ours 0 / 5 (123.1 pt);
/// - 4 lines, `orphans: 1; widows: 5`, room for 2: Chrome 1 / 3 (91.8 pt), ours 0 / 4 (107.3 pt);
/// - 7 lines, `orphans: 3; widows: 5`, room for 6: Chrome 3 / 4 (107.6 pt), ours 0 / 7 (154.6 pt);
/// - two `li` of 4 lines, `orphans: 3; widows: 3`, room for 3: Chrome 3 / 1 in the first (`w0000`-`w0002` on page 1), ours
///   keeps the first `li` on page 2.
#[test]
fn a_short_paragraph_that_fits_a_page_keeps_its_lines_together() {
    // (lines, orphans, widows, room in lines, lines per page, `AFTER` y in pt)
    type Case = (usize, usize, usize, usize, &'static [usize], f32);
    let cases: [Case; 3] = [
        (5, 3, 3, 3, &[0, 5], 123.1),
        (4, 1, 5, 2, &[0, 4], 107.3),
        (7, 3, 5, 6, &[0, 7], 154.6),
    ];
    for (n, o, w, k, per, y) in cases {
        let words = printed_words(&render(
            &format!("p{{orphans:{o};widows:{w}}}"),
            &format!("{}<p>{}</p><p>AFTER</p>", room(k), lines(n)),
        ));
        assert_case(&words, n, per, 2, y, &format!("n {n}, {o} / {w}, room {k}"));
    }
    let list = format!(
        "{}<ul style=\"margin:0;padding-left:20px\"><li>{}</li><li>{}</li></ul><p>AFTER</p>",
        room(3),
        lines(4),
        (4..8)
            .map(|i| format!("w{i:04}"))
            .collect::<Vec<_>>()
            .join("<br>")
    );
    let words = printed_words(&render("li{orphans:3;widows:3}", &list));
    assert_eq!(per_page(&words), [0, 8], "two li, Chrome 3 / 5");
}

// ── 13. The break before the paragraph is forbidden ──────────────────────────────────────────────────────────────

/// A 5-line paragraph with `orphans: 3; widows: 3` has no allowed break between its lines, so it moves whole when the
/// class A break before it is allowed. When that break is forbidden, css-break-3 §4.4 drops Rule 3 first (before the
/// `avoid` rules): `h2 { break-after: avoid }` or `p { break-before: avoid }`, with room for the heading and 3 or 4 lines,
/// Chrome keeps the orphans and prints 3 / 2 (`AFTER` on page 2 at 76.1 pt); with room for the heading and fewer than 3
/// lines the heading moves with the paragraph (0 / 5, the heading on page 2, `AFTER` at 139.1 pt).
///
/// Ours does not honour `break-before: avoid` / `break-after: avoid` at all (F87c parses them as `auto`; there is no
/// keep-with-next: F215), so this pins what ours does: the heading stays on page 1 and the paragraph moves whole,
/// 0 / 5 with `AFTER` at 123.1 pt. The red-then-green fix belongs to F215. An ancestor's `break-inside: avoid` is a
/// different case: the ancestor itself moves whole, and ours matches Chrome (0 / 5, 123.3 pt).
#[test]
fn a_short_paragraph_after_an_avoid_break_keeps_its_orphans() {
    let head_css = "h2{margin:0;font:600 14px/21px Inter;break-after:avoid}p{orphans:3;widows:3}";
    for k in [4, 5] {
        let words = printed_words(&render(
            head_css,
            &format!(
                "{}<h2>HEAD</h2><p>{}</p><p>AFTER</p>",
                spacer(1008 - 21 * k),
                lines(5)
            ),
        ));
        // Ours (F215 open); Chrome 3 / 2 and `AFTER` at 76.1 pt.
        assert_eq!(
            per_page(&words),
            [0, 5],
            "heading + room for {} lines",
            k - 1
        );
        assert_eq!(
            words.iter().find(|w| w.text == "HEAD").map(|w| w.page),
            Some(1)
        );
    }
    for k in [3, 4] {
        let words = printed_words(&render(
            "div.w{break-inside:avoid}p{orphans:3;widows:3}",
            &format!(
                "{}<div class=\"w\"><p>{}</p></div><p>AFTER</p>",
                spacer(1008 - 21 * k),
                lines(5)
            ),
        ));
        assert_case(
            &words,
            5,
            &[0, 5],
            2,
            123.3,
            &format!("avoid ancestor, room {k}"),
        );
    }
}
