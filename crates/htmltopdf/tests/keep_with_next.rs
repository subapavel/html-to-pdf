//! `break-before: avoid` / `break-after: avoid` keep a box with its neighbour (F215).
//!
//! css-break-3 §3.1: `avoid` (and `avoid-page`) forbids a page break at that point, `avoid-column` only a column break; §3.4:
//! the legacy `page-break-before|after: avoid` alias `avoid`; §4.4 Rule 1: a Class A break (between siblings, §4.1) is allowed
//! only if the values of both boxes allow it, and a forced value wins over `avoid` (§4.3). The rules are dropped only when no
//! allowed break keeps the content inside the fragmentainer (Rule 3 first). So a heading with `break-after: avoid` that has
//! content above it on the page moves to the next page together with the box after it whenever the pair fits a page. Plan:
//! `docs/plans/2026-11-02-f215-keep-with-next.md`.
//!
//! Before F215 the fork read the values as `auto` (F87c), so the heading stayed alone at the bottom of a page when the box
//! after it moved on. Every number is Chrome's (m153, A4, 15 mm, Inter, `14px/21px`, `p { margin: 0 }`, `h2` 21 px with no
//! margin, lines `w0000<br>w0001…`; a page holds 48 such lines, 1008 px against Chrome's 1010 px and ours 1009.13 px). A
//! spacer of `1008 - 21 (j + 1)` px leaves room for the heading and `j` lines (42 to 43 px away from a line boundary, so the
//! 1 px capacity difference, F30f-c, does not decide). Probes: `scratchpad/f215/` of the session (plan section 9).
//!
//! Where Chrome is not matched the test says so and pins ours.

use htmltopdf::{RenderOptions, Renderer};

/// The content box of A4 with 15 mm margins, in pt from the page top: its bottom.
const CONTENT_BOTTOM: f32 = 799.37;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// The whole style sheet is `css`; `body` goes into `<body>`.
fn render_doc(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
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
            "@page{{size:A4;margin:15mm}}body{{font:14px/21px Inter;margin:0}}p{{margin:0}}h1,h2,h3{{margin:0;font:600 14px/21px Inter}}{extra}"
        ),
        body,
    )
}

/// `w0000<br>w0001<br>…` from word `from`.
fn lines_from(from: usize, n: usize) -> String {
    (from..from + n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn lines(n: usize) -> String {
    lines_from(0, n)
}

fn spacer(h: usize) -> String {
    format!("<div style=\"height:{h}px\"></div>")
}

/// A spacer that leaves room for the heading (one line) and `j` more lines on page 1.
fn room_for_heading_and(j: usize) -> String {
    spacer(1008 - 21 * (j + 1))
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
    let path = std::env::temp_dir().join(format!("htmltopdf-kwn-{}-{n}.pdf", std::process::id()));
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

/// How many `wNNNN` words (one per line here) are printed on each page, for `pages` pages.
fn per_page(words: &[Word]) -> Vec<usize> {
    let pages = words.iter().map(|w| w.page).max().unwrap_or(0);
    let mut per = vec![0; pages];
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        per[w.page - 1] += 1;
    }
    per
}

/// The first printed occurrence of `text`: (page, yMin).
fn at(words: &[Word], text: &str) -> (usize, f32) {
    let w = words
        .iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("`{text}` is not printed"));
    (w.page, w.y_min)
}

fn page_of(words: &[Word], text: &str) -> usize {
    at(words, text).0
}

/// `text` is on `page` at `y` pt (±1 pt).
fn assert_at(words: &[Word], text: &str, page: usize, y: f32) {
    let (p, got) = at(words, text);
    assert_eq!(p, page, "`{text}` on page {p}, expected {page}");
    assert!(
        (got - y).abs() <= 1.0,
        "`{text}` at {got:.1} pt, expected {y:.1} pt"
    );
}

/// Every `wNNNN` word of `0..n` is printed once, in order, and no line stands below the content box.
fn assert_lines_in_order(words: &[Word], n: usize) {
    let names: Vec<&str> = words
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.text.as_str())
        .collect();
    let want: Vec<String> = (0..n).map(|i| format!("w{i:04}")).collect();
    assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>());
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        assert!(
            w.y_max <= CONTENT_BOTTOM,
            "{} on page {} ends at {:.1} pt, below {CONTENT_BOTTOM} pt",
            w.text,
            w.page,
            w.y_max
        );
    }
}

/// The heading `HEAD` moved to page 2 with a 30-line paragraph (`0 / 30`; Chrome: `HEAD` at 44.6 pt, `AFTER` at 532.8 pt).
fn assert_moved_with_30(words: &[Word], what: &str) {
    assert_eq!(page_of(words, "HEAD"), 2, "{what}: HEAD");
    assert_eq!(per_page(words), [0, 30], "{what}");
    assert_at(words, "AFTER", 2, 532.8);
    assert_lines_in_order(words, 30);
}

/// The 30-line shape: heading, paragraph, `AFTER`, below a spacer with room for the heading and `j` lines.
fn thirty(extra: &str, j: usize, para_attrs: &str) -> Vec<Word> {
    printed_words(&render(
        extra,
        &format!(
            "{}<h2>HEAD</h2><p{para_attrs}>{}</p><p>AFTER</p>",
            room_for_heading_and(j),
            lines(30)
        ),
    ))
}

// ── 1. `break-after: avoid` on a heading before a long paragraph ─────────────────────────────────────────────────

/// `h2 { break-after: avoid }`, a 30-line paragraph (`orphans: 2; widows: 2`). With room for the heading and 0 or 1 line the
/// paragraph cannot start on page 1 (two lines are needed) and the heading goes with it: Chrome 0 / 30, `HEAD` at 44.6 pt on
/// page 2, `AFTER` at 532.8 pt. Ours before F215: `HEAD` alone at the bottom of page 1 (784.6 / 768.8 pt), `AFTER` at 516.8 pt.
#[test]
fn a_heading_with_break_after_avoid_moves_with_a_long_paragraph() {
    for j in [0, 1] {
        let words = thirty("h2{break-after:avoid}", j, "");
        assert_moved_with_30(&words, &format!("room for the heading and {j} lines"));
    }
}

/// Where the paragraph can start, the heading stays with it on page 1 and nothing changes: Chrome 2 / 28 (`AFTER` 485.6 pt) and
/// 3 / 27 (469.8 pt) with room for the heading and 2 or 3 lines. A pair that fits is not moved: green before and after F215.
#[test]
fn the_avoid_leaves_the_split_that_already_matches_chrome() {
    for (j, per, y) in [(2, [2, 28], 485.6), (3, [3, 27], 469.8)] {
        let words = thirty("h2{break-after:avoid}", j, "");
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_eq!(per_page(&words), per, "room {j}");
        assert_at(&words, "AFTER", 2, y);
    }
}

/// Without the declaration the heading stays at the bottom of page 1 with room for 0 or 1 line (Chrome, measured; this is why the
/// UA sheet has no `h2 { break-after: avoid }`): `HEAD` at 784.8 / 769.1 pt, paragraph and `AFTER` (517.1 pt) on page 2.
#[test]
fn without_the_avoid_the_heading_stays_at_the_page_bottom() {
    for j in [0, 1] {
        let words = thirty("", j, "");
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_eq!(per_page(&words), [0, 30], "room {j}");
        assert_at(&words, "AFTER", 2, 516.8); // Chrome 517.1
    }
}

/// A pair that fits the room stays on page 1: heading and 5-line paragraph with room for both (Chrome 5 / 0, `HEAD` at
/// 706.1 pt, `AFTER` on page 2 at 44.6 pt).
#[test]
fn a_pair_that_fits_the_room_stays_on_page_one() {
    let words = printed_words(&render(
        "h2{break-after:avoid}p{orphans:3;widows:3}",
        &format!(
            "{}<h2>HEAD</h2><p>{}</p><p>AFTER</p>",
            room_for_heading_and(5),
            lines(5)
        ),
    ));
    assert_eq!(page_of(&words, "HEAD"), 1);
    assert_eq!(per_page(&words), [5, 0]);
    assert_at(&words, "AFTER", 2, 44.3); // Chrome 44.6
}

// ── 2. `break-before: avoid` on the paragraph ───────────────────────────────────────────────────────────────────

/// The same numbers when the paragraph carries `break-before: avoid` (the break between the two boxes is the same one).
/// Chrome: 0 / 30 with `HEAD` on page 2 for room 0 / 1, 2 / 28 and 3 / 27 for 2 / 3.
#[test]
fn break_before_avoid_on_the_paragraph_keeps_the_heading() {
    for j in [0, 1] {
        let words = thirty("p.x{break-before:avoid}", j, " class=\"x\"");
        assert_moved_with_30(&words, &format!("room {j}"));
    }
    for (j, per) in [(2, [2, 28]), (3, [3, 27])] {
        let words = thirty("p.x{break-before:avoid}", j, " class=\"x\"");
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_eq!(per_page(&words), per, "room {j}");
    }
}

// ── 3. The values ───────────────────────────────────────────────────────────────────────────────────────────────

/// `avoid-page` and the legacy `page-break-after|before: avoid` behave like `avoid` (css-break-3 §3.1, §3.4): Chrome 0 / 30 with
/// `HEAD` on page 2 for room 0. `avoid-column` constrains only column breaks (§3.1: "only have an effect in multi-column
/// contexts"): Chrome leaves `HEAD` at the bottom of page 1 (784.8 pt) and `AFTER` at 517.1 pt, and so does ours.
#[test]
fn avoid_page_and_the_legacy_alias_keep_avoid_column_does_not() {
    for (css, attrs) in [
        ("h2{break-after:avoid-page}", ""),
        ("h2{page-break-after:avoid}", ""),
        ("p.x{break-before:avoid-page}", " class=\"x\""),
        ("p.x{page-break-before:avoid}", " class=\"x\""),
    ] {
        let words = thirty(css, 0, attrs);
        assert_moved_with_30(&words, css);
    }
    for (css, attrs) in [
        ("h2{break-after:avoid-column}", ""),
        ("p.x{break-before:avoid-column}", " class=\"x\""),
        ("h2{break-after:column}", ""),
    ] {
        let words = thirty(css, 0, attrs);
        assert_eq!(page_of(&words, "HEAD"), 1, "{css}");
        assert_eq!(per_page(&words), [0, 30], "{css}");
        assert_at(&words, "AFTER", 2, 516.8); // Chrome 517.1
    }
}

/// The cascade picks the value: `.a { break-before: page } .b { break-before: avoid-page }` on one box, the later rule wins, so
/// there is no break (Chrome: all three words on page 1); in the other order the box starts page 2. Green before and after.
#[test]
fn the_later_of_two_break_values_wins() {
    let words = printed_words(&render(
        ".a{break-before:page}.b{break-before:avoid-page}",
        "<p>one</p><p class=\"a b\">two</p><p>AFTER</p>",
    ));
    assert_eq!(words.iter().map(|w| w.page).collect::<Vec<_>>(), [1, 1, 1]);
    let words = printed_words(&render(
        ".a{break-before:avoid-page}.b{break-before:page}",
        "<p>one</p><p class=\"a b\">two</p><p>AFTER</p>",
    ));
    assert_eq!(page_of(&words, "one"), 1);
    assert_eq!(page_of(&words, "two"), 2);
}

// ── 4. A chain of avoided breaks ────────────────────────────────────────────────────────────────────────────────

/// `h1` and `h2` both with `break-after: avoid`, then a 30-line paragraph. Room for `h1` only, `h1 + h2`, or `h1 + h2` and one
/// line: the whole chain moves, Chrome 0 / 30, `H1` at 44.6 pt, `H2` at 60.3 pt, `AFTER` at 548.6 pt. Room for `h1 + h2` and two
/// lines: 2 / 28 on page 1 (`AFTER` 485.6 pt). Ours before F215: heading(s) at the bottom of page 1.
#[test]
fn a_chain_of_two_headings_moves_with_the_paragraph() {
    for r in [1, 2, 3] {
        let words = printed_words(&render(
            "h1,h2{break-after:avoid}",
            &format!(
                "{}<h1>H1</h1><h2>H2</h2><p>{}</p><p>AFTER</p>",
                spacer(1008 - 21 * r),
                lines(30)
            ),
        ));
        assert_eq!(page_of(&words, "H1"), 2, "room for {r} lines: H1");
        assert_eq!(page_of(&words, "H2"), 2, "room for {r} lines: H2");
        assert_eq!(per_page(&words), [0, 30], "room for {r} lines");
        assert_at(&words, "AFTER", 2, 548.6);
    }
}

/// The chain with room for `h1 + h2` and two lines does not move (Chrome 2 / 28, `AFTER` at 485.6 pt); with `avoid` on `h1` only,
/// `h2` plain, the two headings stay at the bottom of page 1 and the paragraph moves (Chrome: `H1` 769.1 pt, `H2` 784.8 pt,
/// `AFTER` 517.1 pt). Green guards.
#[test]
fn a_chain_that_fits_or_ends_at_a_plain_heading_is_left_alone() {
    let words = printed_words(&render(
        "h1,h2{break-after:avoid}",
        &format!(
            "{}<h1>H1</h1><h2>H2</h2><p>{}</p><p>AFTER</p>",
            spacer(1008 - 21 * 4),
            lines(30)
        ),
    ));
    assert_eq!(page_of(&words, "H2"), 1);
    assert_eq!(per_page(&words), [2, 28]);
    assert_at(&words, "AFTER", 2, 485.3); // Chrome 485.6
    let words = printed_words(&render(
        "h1{break-after:avoid}",
        &format!(
            "{}<h1>H1</h1><h2>H2</h2><p>{}</p><p>AFTER</p>",
            spacer(1008 - 21 * 2),
            lines(30)
        ),
    ));
    assert_eq!(page_of(&words, "H1"), 1);
    assert_eq!(page_of(&words, "H2"), 1);
    assert_eq!(per_page(&words), [0, 30]);
    assert_at(&words, "AFTER", 2, 516.8); // Chrome 517.1
}

// ── 5. A short paragraph after an avoid break: the pinned shape of F30c-o3 ─────────────────────────────────────────

// The flipped pin lives in `orphans_widows.rs`
// (`a_short_paragraph_after_an_avoid_break_moves_with_its_heading`).

// ── 6. A heading before a kept-together card ───────────────────────────────────────────────────────────────────────

/// `h2 { break-after: avoid }` and a `break-inside: avoid` card of 6 lines. The card cannot start on page 1 unless all six lines
/// fit, so the heading goes with it: Chrome 0 / 6 with `HEAD` at 44.6 pt on page 2 and `AFTER` at 154.8 pt, for room for 1 to 6
/// lines in all (the heading included). With room for 7 lines all of it fits (Chrome 6 / 0, `HEAD` 690.3 pt, `AFTER` on page 2 at
/// 44.6 pt, ours the same). Ours before F215: `HEAD` alone at the bottom of page 1, `AFTER` at 138.8 pt.
#[test]
fn a_heading_before_a_kept_together_card_moves_with_it() {
    for k in 1..=6 {
        let words = printed_words(&render(
            "h2{break-after:avoid}.c{break-inside:avoid}",
            &format!(
                "{}<h2>HEAD</h2><div class=\"c\"><p>{}</p></div><p>AFTER</p>",
                spacer(1008 - 21 * k),
                lines(6)
            ),
        ));
        assert_eq!(page_of(&words, "HEAD"), 2, "room for {k} lines: HEAD");
        assert_eq!(per_page(&words), [0, 6], "room for {k} lines");
        assert_at(&words, "AFTER", 2, 154.5); // Chrome 154.8
    }
    let words = printed_words(&render(
        "h2{break-after:avoid}.c{break-inside:avoid}",
        &format!(
            "{}<h2>HEAD</h2><div class=\"c\"><p>{}</p></div><p>AFTER</p>",
            spacer(1008 - 21 * 7),
            lines(6)
        ),
    ));
    assert_eq!(page_of(&words, "HEAD"), 1);
    assert_eq!(per_page(&words), [6, 0]);
}

// ── 7. A heading before a table ──────────────────────────────────────────────────────────────────────────────────

fn table(rows: usize) -> String {
    let body: String = (0..rows)
        .map(|i| format!("<tr><td>r{i:02}</td></tr>"))
        .collect();
    format!(
        "<table style=\"border-collapse:collapse;width:100%\"><thead><tr><th style=\"text-align:left;font:400 14px/21px Inter\">THEAD</th></tr></thead><tbody>{body}</tbody></table>"
    )
}

/// `h2 { break-after: avoid }` (or `table { break-before: avoid }`) and a table of a header and ten rows of 23 px. With room for the
/// heading only, or the heading and 21 px (the header row needs 23 px), the table moves and the heading goes with it: Chrome
/// `HEAD` at 44.6 pt on page 2, `THEAD` at 61.1 pt, `AFTER` at 250.1 pt. Without the avoid Chrome leaves `HEAD` alone on page 1.
/// Ours before F215: `HEAD` alone at the bottom of page 1.
#[test]
fn a_heading_before_a_table_moves_with_its_header() {
    for css in ["h2{break-after:avoid}", "table{break-before:avoid}"] {
        for k in [1, 2] {
            let words = printed_words(&render(
                css,
                &format!(
                    "{}<h2>HEAD</h2>{}<p>AFTER</p>",
                    spacer(1008 - 21 * k),
                    table(10)
                ),
            ));
            assert_eq!(page_of(&words, "HEAD"), 2, "{css}, room {k}: HEAD");
            assert_eq!(page_of(&words, "THEAD"), 2, "{css}, room {k}: THEAD");
            assert_eq!(page_of(&words, "r09"), 2, "{css}, room {k}");
            assert_at(&words, "AFTER", 2, 250.1); // Chrome 250.1
        }
    }
}

/// With room for the heading, the header row and no body row (63 px: 21 + 23 + 19 left) Chrome moves the heading and the whole
/// table to page 2 (`HEAD` 44.6 pt, `THEAD` 61.1 pt). Ours leaves the header row alone at the bottom of page 1 and prints the
/// rows on page 2 without a header (the F12 step 5 header orphan, also without any `avoid`: `HEAD` 753.1 pt, `THEAD` 769.6 pt,
/// `AFTER` 216.8 pt). The keep follows the table's own move rule, so F215 does not change this: pinned, to be fixed with that
/// rule, not here. With room for the header and one row (84 px) both agree with Chrome: `HEAD`, `THEAD`, `r00` on page 1, the
/// other nine rows under a repeated header on page 2.
#[test]
fn a_heading_before_a_table_keeps_the_table_s_own_header_rule() {
    let words = printed_words(&render(
        "h2{break-after:avoid}",
        &format!(
            "{}<h2>HEAD</h2>{}<p>AFTER</p>",
            spacer(1008 - 21 * 3),
            table(10)
        ),
    ));
    assert_eq!(page_of(&words, "HEAD"), 1);
    assert_eq!(page_of(&words, "THEAD"), 1, "the header orphan (F12)");
    assert_eq!(page_of(&words, "r00"), 2);
    assert_at(&words, "AFTER", 2, 216.8);
    let words = printed_words(&render(
        "h2{break-after:avoid}",
        &format!(
            "{}<h2>HEAD</h2>{}<p>AFTER</p>",
            spacer(1008 - 21 * 4),
            table(10)
        ),
    ));
    assert_eq!(page_of(&words, "HEAD"), 1);
    assert_eq!(page_of(&words, "r00"), 1);
    assert_eq!(page_of(&words, "r01"), 2);
    assert_at(&words, "AFTER", 2, 216.8); // Chrome 217.1
}

// ── 8. A forced break wins over `avoid` ──────────────────────────────────────────────────────────────────────────

/// css-break-3 §4.3: "a forced break value effectively overrides any avoid break value that also applies at that break point".
/// Chrome and ours: `h2 { break-after: avoid }` before `break-before: page` / `page-break-before: always`: `HEAD` stays on
/// page 1 and the paragraph starts page 2 (`AFTER` at 123.3 pt); `h2 { break-after: page }` before `break-before: avoid` the
/// same; `h2 { break-before: page; break-after: avoid }` starts page 2 with the heading and the paragraph (0 / 30, `AFTER`
/// 532.8 pt). Green before and after.
#[test]
fn a_forced_break_wins_over_avoid() {
    for k in [1, 3] {
        for (css, attrs) in [
            ("h2{break-after:avoid}", " style=\"break-before:page\""),
            (
                "h2{break-after:avoid}",
                " style=\"page-break-before:always\"",
            ),
            (
                "h2{break-after:page}p.n{break-before:avoid}",
                " class=\"n\"",
            ),
        ] {
            let words = printed_words(&render(
                css,
                &format!(
                    "{}<h2>HEAD</h2><p{attrs}>{}</p><p>AFTER</p>",
                    spacer(1008 - 21 * k),
                    lines(5)
                ),
            ));
            assert_eq!(page_of(&words, "HEAD"), 1, "{css} {attrs}, room {k}");
            assert_eq!(per_page(&words), [0, 5], "{css} {attrs}, room {k}");
            assert_at(&words, "AFTER", 2, 123.1); // Chrome 123.3
        }
        let words = printed_words(&render(
            "h2{break-before:page;break-after:avoid}",
            &format!(
                "{}<h2>HEAD</h2><p>{}</p><p>AFTER</p>",
                spacer(1008 - 21 * k),
                lines(30)
            ),
        ));
        assert_moved_with_30(&words, &format!("break-before: page, room {k}"));
    }
}

// ── 9. A pair taller than a page is not kept ────────────────────────────────────────────────────────────────────

/// `h2 { break-after: avoid }` and a 1000 px box: together they are 1021 px, taller than the 1009 px page, so no page break
/// keeps them together and §4.4 relaxes the rule; the keep is not applied. Ours puts the heading at the bottom of page 1, the
/// box whole on page 2 (`DIVX` at 44.3 pt) and `AFTER` on page 3 (44.3 pt), for room 0 and 1.
///
/// Not Chrome, for a reason that has nothing to do with the keep: Chrome splits the fixed-height box (the box is block
/// content, one line and 979 px of nothing), so with room for 0 lines it moves the heading and starts the box under it on page 2
/// (`HEAD` 44.6 pt, `DIVX` 60.3 pt, 12 px of the box on page 3, `AFTER` 52.8 pt), and with room for 1 line it starts the box on
/// page 1 (`DIVX` 784.8 pt, `AFTER` on page 2 at 777.3 pt). The same without the `avoid` (Chrome: `HEAD` page 1, `DIVX` page 2
/// at 44.6 pt, `AFTER` page 3) is what ours does. For an image of 1000 px Chrome moves the heading to an otherwise empty page 2
/// (4 pages), ours does not (F215d is the relaxed-box case). Pinned as today.
#[test]
fn a_pair_taller_than_a_page_is_not_kept() {
    for j in [0, 1] {
        let words = printed_words(&render(
            "h2{break-after:avoid}",
            &format!(
                "{}<h2>HEAD</h2><div style=\"height:1000px\">DIVX</div><p>AFTER</p>",
                room_for_heading_and(j)
            ),
        ));
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_at(&words, "DIVX", 2, 44.3);
        assert_at(&words, "AFTER", 3, 44.3);
    }
}

// ── 10. The keep holds inside a wrapper ────────────────────────────────────────────────────────────────────────────

/// The nested twin of test 1: `<section><h2>HEAD</h2><p>…</p></section>`. Chrome: 0 / 30, `HEAD` at 44.6 pt on page 2 and `AFTER`
/// at 532.8 pt for room 0 / 1; 2 / 28 and 3 / 27 for room 2 / 3. Ours before F215: `HEAD` alone at the bottom of page 1.
#[test]
fn the_keep_holds_inside_a_section() {
    for j in [0, 1] {
        let words = printed_words(&render(
            "h2{break-after:avoid}",
            &format!(
                "{}<section><h2>HEAD</h2><p>{}</p></section><p>AFTER</p>",
                room_for_heading_and(j),
                lines(30)
            ),
        ));
        assert_moved_with_30(&words, &format!("section, room {j}"));
    }
    for (j, per) in [(2, [2, 28]), (3, [3, 27])] {
        let words = printed_words(&render(
            "h2{break-after:avoid}",
            &format!(
                "{}<section><h2>HEAD</h2><p>{}</p></section><p>AFTER</p>",
                room_for_heading_and(j),
                lines(30)
            ),
        ));
        assert_eq!(page_of(&words, "HEAD"), 1, "section, room {j}");
        assert_eq!(per_page(&words), per, "section, room {j}");
    }
}

// ── 11. Grid items and columns ───────────────────────────────────────────────────────────────────────────────────────

/// Not Chrome, on purpose (F215e, F30d-b): ours does not read the keep inside a multicol container (an `avoid` before a column
/// end: Chrome moves `H3` to column 2, `AFTER` at 233.6 pt against 217.8 pt without the declaration; with `avoid-column` the
/// same) or on grid items (Chrome moves both items' headings with the paragraphs: `HEAD` at 44.6 pt on page 2 for room 0 and 1;
/// ours leaves them at the bottom of page 1). The PDF bytes equal those of the twin without the `avoid`.
#[test]
fn grid_items_and_columns_ignore_the_keep() {
    let columns = |avoid: &str| {
        render(
            &format!("h3{{{avoid}}}.mc{{columns:2;column-gap:20px;width:600px}}"),
            &format!(
                "<div class=\"mc\"><p>{}</p><h3>H3</h3><p>{}</p></div><p>AFTER</p>",
                lines(9),
                lines_from(9, 11)
            ),
        )
    };
    let plain = columns("");
    assert_eq!(columns("break-after:avoid"), plain);
    assert_eq!(columns("break-after:avoid-column"), plain);
    let grid = |avoid: &str| {
        render(
            &format!(
                "h2{{{avoid}}}.g{{display:grid;grid-template-columns:1fr 1fr;gap:20px;width:600px}}"
            ),
            &format!(
                "{}<div class=\"g\"><div><h2>HEAD</h2><p>{}</p></div><div><h2>HEADB</h2><p>{}</p></div></div><p>AFTER</p>",
                room_for_heading_and(0),
                lines(10),
                lines_from(10, 10)
            ),
        )
    };
    assert_eq!(grid("break-after:avoid"), grid(""));
}

// ── 12. A chain taller than a page: Rule 3 gives way first ────────────────────────────────────────────────────────────

/// 60 three-line paragraphs, each with `break-after: avoid`, `orphans: 2; widows: 2`: a paragraph of three lines has no allowed
/// break between its lines (2 + 2 > 3) and every break between two paragraphs is avoided, so there is no allowed break at all.
/// css-break-3 §4.4 drops Rule 3 first, so Chrome breaks inside a paragraph (2 / 1: it violates `widows`) rather than between
/// two: pages of 47, 48, 48 and 37 lines, `AFTER` at 627.3 pt. (Blink: violating `break-avoid` ranks below violating orphans
/// and widows.) Ours before F215 breaks between paragraphs: 48, 48, 48, 36 lines, `AFTER` at 611.3 pt.
#[test]
fn a_chain_taller_than_a_page_breaks_inside_a_short_paragraph_first() {
    let body: String = (0..60)
        .map(|i| format!("<p>{}</p>", lines_from(i * 3, 3)))
        .collect();
    let words = printed_words(&render(
        "p{break-after:avoid}",
        &format!("{body}<p>AFTER</p>"),
    ));
    assert_eq!(per_page(&words), [47, 48, 48, 37]);
    assert_lines_in_order(&words, 180);
    assert_at(&words, "AFTER", 4, 627.0); // Chrome 627.3
}

// ── 13. Extra shapes measured in step 1 ──────────────────────────────────────────────────────────────────────────

/// `h2 { break-after: avoid }` before a two-column container of 40 lines (balanced, 20 lines per column). Room for the heading
/// only: the container moves and the heading goes with it (Chrome `HEAD` at 44.6 pt on page 2, `AFTER` at 375.3 pt). Room for
/// the heading and 2 / 3 lines: the container starts on page 1 in both (Chrome 4 / 36 and 6 / 34, `AFTER` 328.1 / 312.3 pt;
/// ours the same: green). With room for the heading and one line Chrome also moves the container, ours starts it with 2 lines
/// (a multicol difference that has nothing to do with the keep; not tested).
#[test]
fn a_heading_before_a_multicol_container_moves_with_it() {
    let body = |j: usize| {
        format!(
            "{}<h2>HEAD</h2><div class=\"mc\"><p>{}</p></div><p>AFTER</p>",
            room_for_heading_and(j),
            lines(40)
        )
    };
    let css = "h2{break-after:avoid}.mc{columns:2;column-gap:20px;width:600px}";
    let words = printed_words(&render(css, &body(0)));
    assert_eq!(page_of(&words, "HEAD"), 2);
    assert_eq!(per_page(&words), [0, 40]);
    assert_at(&words, "AFTER", 2, 375.0); // Chrome 375.3
    for (j, per, y) in [(2, [4, 36], 327.8), (3, [6, 34], 312.1)] {
        let words = printed_words(&render(css, &body(j)));
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_eq!(per_page(&words), per, "room {j}");
        assert_at(&words, "AFTER", 2, y);
    }
}

/// css-break-3 §4.4 relaxes Rule 1 when the heading is already first on the page: there is no earlier allowed break that keeps
/// the content inside the page, so the paragraph after it starts in place with the orphans it can have. A 5-line paragraph with
/// `orphans: 3; widows: 3` below a heading that fills the top of the page (height `1008 - 21 j` px, first in the body; room for `j`
/// lines under it): Chrome 1 / 4 (`AFTER` at 107.6 pt on page 2), 2 / 3 (91.8 pt), 3 / 2 (76.1 pt) and 3 / 2 for `j` = 1 to 4; 0 / 5
/// (123.3 pt, the heading alone on page 1) for `j` = 0; 5 / 0 for `j` = 5. Without the avoid Chrome moves the paragraph whole for
/// `j` = 1 (0 / 5) and splits it 3 / 2 for 3 and 4 (the short-paragraph case of ADR-0010, not matched). Ours today: 0 / 5
/// for every `j` from 0 to 4 (the paragraph moves whole).
///
/// The plan's design (sections 4 and 6) has no step that makes this green: the keep applies only with content above the heading
/// and step 7 only to an over-long chain. Red until that is planned (see plan section 9, "Gaps found by the measurements").
#[test]
fn a_short_paragraph_after_a_heading_at_the_page_top_relaxes_rule_3() {
    for (j, per, y) in [
        (1, [1, 4], 107.6),
        (2, [2, 3], 91.8),
        (3, [3, 2], 76.1),
        (4, [3, 2], 76.1),
    ] {
        let words = printed_words(&render(
            "h2{break-after:avoid}p{orphans:3;widows:3}",
            &format!(
                "<h2 style=\"height:{}px\">HEAD</h2><p>{}</p><p>AFTER</p>",
                1008 - 21 * j,
                lines(5)
            ),
        ));
        assert_eq!(page_of(&words, "HEAD"), 1, "room {j}");
        assert_eq!(per_page(&words), per, "room {j}");
        assert_at(&words, "AFTER", 2, y - 0.3);
    }
}
