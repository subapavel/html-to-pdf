//! A body-level paragraph that does not fit the room below the cursor starts there and is split between its lines when
//! its first two lines fit (F30c, the pre-advance part).
//!
//! css-break-3 §4.1: a break between two line boxes (Class B) is as good a break opportunity as one between two blocks
//! (Class A). §4.4: with `orphans` / `widows` at 2 a paragraph may start on a page when two of its lines fit and two can
//! be left for the next one; the UA fills the fragmentainer. The pre-advance in the fork's line path moved a paragraph
//! that did not fit to the next page whole, even when it could start where it was: a second long paragraph lost a page
//! after the first one's tail (`two_49_49`: 47 / 2 / 47 / 2 on four pages, Chrome 47 / 48 / 3), a 600-word paragraph
//! after a 650 px spacer left the bottom of page 1 empty, and `break-inside: avoid` made no difference to a body-level
//! `<p>` (F87).
//!
//! Every number is Chrome's (m141, A4, 15 mm unless noted, Inter, `14px/21px`, `p { margin: 0 }`, lines
//! `w0000<br>w0001…` or words `w0000 w0001 …`, `AFTER` in a `<p>` below), measured with `out/scratch/f30c/{gen,sum}.py`
//! (plan §8). A page holds 48 such lines (1008 px against Chrome's 1010 px; ours is 1009.13 px, so the tests keep a spacer
//! that leaves a room of 42 px to 43 px or more than 1 px from a line boundary, F30f-c).
//!
//! Where Chrome is not matched, the test says so and pins ours: a 3-line paragraph with two lines of room (Chrome 2 / 1,
//! it keeps `orphans` and gives up `widows`; ours moves it whole) and a floated paragraph (Chrome splits it, ours moves it
//! whole). Both are filed (F30c-w, F30c-f).

use htmltopdf::{RenderOptions, Renderer};

/// The content box of A4 with 15 mm margins, in pt from the page top: its bottom.
const CONTENT_BOTTOM: f32 = 799.37;
/// One line of 21 px in pt.
const LINE: f32 = 15.75;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_with(sheet: bool, body: &str) -> Vec<u8> {
    let head = "@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}";
    let mut b = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true);
    if sheet {
        b = b
            .css_file(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../fixtures/tailwind/tailwind.css"),
            )
            .unwrap();
    }
    b.build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn render(body: &str) -> Vec<u8> {
    render_with(false, body)
}

/// `w0000<br>w0001<br>…` from word `from` (no trailing `<br>`).
fn lines(n: usize, from: usize) -> String {
    (from..from + n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

/// `w0000 w0001 …` (wrapped by the browser).
fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
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

/// The page and y of one `wNNNN` word.
fn at(words: &[Word], text: &str) -> (usize, f32) {
    let w: Vec<&Word> = words.iter().filter(|w| w.text == text).collect();
    assert_eq!(w.len(), 1, "{text} is printed once");
    (w[0].page, w[0].y_min)
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

// ── 1. A second long paragraph starts below the first one's tail ──────────────────────────────────────────────────

/// Chrome: `(first, second, lines per page, AFTER page, AFTER y)`. Lines per page are counted over both paragraphs.
/// Ours at `5ec1ac67`: the second paragraph moved whole (47 / 2 / 47, 47 / 2 / 48 / 0, …).
#[test]
fn a_second_long_paragraph_starts_below_the_first() {
    let cases: [(usize, usize, &[usize], usize, f32); 10] = [
        (49, 47, &[47, 47, 2], 3, 76.1),
        (49, 48, &[47, 48, 2], 3, 76.1),
        (49, 49, &[47, 48, 3], 3, 91.8),
        (49, 50, &[47, 48, 4], 3, 107.6),
        (49, 60, &[47, 48, 14], 3, 265.1),
        (50, 47, &[48, 47, 2], 3, 76.1),
        (50, 48, &[48, 48, 2], 3, 76.1),
        (50, 49, &[48, 48, 3], 3, 91.8),
        (50, 50, &[48, 48, 4], 3, 107.6),
        (50, 60, &[48, 48, 14], 3, 265.1),
    ];
    for (first, second, per, page, y) in cases {
        let words = printed_words(&render(&format!(
            "<p>{}</p><p>{}</p><p>AFTER</p>",
            lines(first, 0),
            lines(second, first)
        )));
        assert_eq!(per_page(&words), per, "two_{first}_{second}");
        assert_lines_in_order(&words, first + second);
        assert_line_step(&words);
        assert_after(&words, page, y);
    }
}

// ── 2. A paragraph below a spacer starts where two lines fit ──────────────────────────────────────────────────────

/// A 30-line paragraph after a spacer. Chrome: H = 650 gives 17 / 13 (`AFTER` on page 2 at 249.3 pt), H = 966 gives 2 / 28
/// (485.6 pt); at H = 975 one line of room is left and the paragraph moves whole (517.1 pt).
/// Ours at `5ec1ac67`: 0 / 30 for all three.
#[test]
fn a_paragraph_below_a_spacer_starts_where_two_lines_fit() {
    for (h, per, y) in [
        (650, [17, 13], 249.3),
        (966, [2, 28], 485.6),
        (975, [0, 30], 517.1),
    ] {
        let words = printed_words(&render(&format!(
            "{}<p>{}</p><p>AFTER</p>",
            spacer(h),
            lines(30, 0)
        )));
        assert_eq!(per_page(&words), per, "H={h}");
        assert_lines_in_order(&words, 30);
        assert_line_step(&words);
        assert_after(&words, 2, y);
    }
}

/// A 4-line paragraph after H = 945 (three lines of room): Chrome 2 / 2, `AFTER` on page 2 at 76.1 pt. Ours: 0 / 4.
/// A 30-line one with the same room: 3 / 27.
#[test]
fn three_lines_of_room_start_a_four_line_paragraph_and_leave_it_two_lines() {
    let words = printed_words(&render(&format!(
        "{}<p>{}</p><p>AFTER</p>",
        spacer(945),
        lines(4, 0)
    )));
    assert_eq!(per_page(&words), [2, 2]);
    assert_lines_in_order(&words, 4);
    assert_after(&words, 2, 76.1);
    let words = printed_words(&render(&format!(
        "{}<p>{}</p><p>AFTER</p>",
        spacer(945),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [3, 27]);
    assert_after(&words, 2, 469.8);
}

/// A paragraph taller than a page started the next page as well (the pre-advance fires for it too). Chrome: 60 lines
/// after a 300 px spacer are 33 / 27, `AFTER` at 469.8 pt. Ours: 0 / 48 / 12.
#[test]
fn a_paragraph_taller_than_a_page_starts_below_the_spacer_too() {
    let words = printed_words(&render(&format!(
        "{}<p>{}</p><p>AFTER</p>",
        spacer(300),
        lines(60, 0)
    )));
    assert_eq!(per_page(&words), [33, 27]);
    assert_lines_in_order(&words, 60);
    assert_after(&words, 2, 469.8);
}

// ── 3. A heading, then a long paragraph ───────────────────────────────────────────────────────────────────────────

/// An `h1` (default margins) and a 46-line paragraph. Chrome: the heading and 44 lines on page 1, 2 lines on page 2,
/// `AFTER` at 76.1 pt. Ours: the heading alone on page 1, 46 lines on page 2. With 48 lines: 45 / 3, `AFTER` at 91.8 pt.
#[test]
fn a_heading_then_a_long_paragraph_starts_below_the_heading() {
    for (n, per, y) in [(46, [44, 2], 76.1), (48, [45, 3], 91.8)] {
        let words = printed_words(&render(&format!(
            "<h1>Title</h1><p>{}</p><p>AFTER</p>",
            lines(n, 0)
        )));
        assert_eq!(per_page(&words), per, "{n} lines");
        assert_eq!(at(&words, "Title").0, 1);
        assert_lines_in_order(&words, n);
        assert_after(&words, 2, y);
    }
}

/// Chrome does not keep a heading with the lines after it: with room for the heading and 0 or 1 line the paragraph moves
/// whole and the heading stays alone at the bottom of page 1 (so does ours: guard), with room for 2 or 3 lines the
/// paragraph starts there (Chrome 2 / 28 and 3 / 27; ours moves it whole). The heading is `18px/27px`, `margin:0`.
#[test]
fn chrome_leaves_a_heading_alone_when_fewer_than_two_lines_fit_below_it() {
    for (h, per) in [
        (973, [0, 30]),
        (953, [0, 30]),
        (933, [2, 28]),
        (913, [3, 27]),
    ] {
        let words = printed_words(&render(&format!(
            "{}<h2 style=\"font:700 18px/27px Inter;margin:0\">Heading</h2><p>{}</p><p>AFTER</p>",
            spacer(h),
            lines(30, 0)
        )));
        assert_eq!(per_page(&words), per, "room after H={h}");
        assert_eq!(
            at(&words, "Heading").0,
            1,
            "H={h}: the heading stays on page 1"
        );
        assert_lines_in_order(&words, 30);
    }
}

// ── 4. 600 words after a 650 px spacer ────────────────────────────────────────────────────────────────────────────

/// The finding (F30d review L3): the paragraph moved whole to page 2. Chrome: 232 words on page 1 and 368 on page 2 (a
/// line is 13 to 14 words), `AFTER` at 501.3 pt.
#[test]
fn six_hundred_words_after_a_650px_spacer_start_on_page_one() {
    let words = printed_words(&render(&format!(
        "{}<p>{}</p><p>AFTER</p>",
        spacer(650),
        words(600)
    )));
    let per = per_page(&words);
    assert_eq!(per.len(), 2, "{per:?}");
    assert!(
        per[0].abs_diff(232) <= 14,
        "page 1: {} words, Chrome 232",
        per[0]
    );
    assert_eq!(per[0] + per[1], 600);
    assert_lines_in_order(&words, 600);
    let (page, y) = after(&words);
    assert_eq!(page, 2);
    assert!(
        (y - 501.3).abs() <= 16.0,
        "AFTER at {y:.1} pt, Chrome 501.3"
    );
}

// ── 5. break-inside: avoid is visible on a body-level paragraph (F87) ─────────────────────────────────────────────

/// 100 words after a 960 px spacer (49 px of room: two lines). Chrome: `auto` puts 27 words on page 1 (two lines), `avoid`
/// moves the paragraph whole (0 / 100). The property comes from a `style` attribute or from the bundled sheet's
/// `break-inside-avoid` utility. Ours at `5ec1ac67`: 0 / 100 for all three.
#[test]
fn break_inside_avoid_is_visible_on_a_body_level_paragraph() {
    let case = |sheet: bool, attrs: &str| -> Vec<usize> {
        per_page(&printed_words(&render_with(
            sheet,
            &format!("{}<p {attrs}>{}</p><p>AFTER</p>", spacer(960), words(100)),
        )))
    };
    let auto = case(false, "");
    assert_eq!(auto.len(), 2);
    assert!(
        auto[0].abs_diff(27) <= 14 && auto[0] >= 2,
        "auto: {auto:?}, Chrome 27 / 73"
    );
    assert_eq!(case(false, "style=\"break-inside:avoid\""), [0, 100]);
    assert_eq!(case(true, "class=\"break-inside-avoid\""), [0, 100]);
}

// ── 6. Guards: shapes that must keep their output ─────────────────────────────────────────────────────────────────

/// A 3-line paragraph with two lines of room. Chrome keeps `orphans` and gives up `widows`: 2 / 1, `AFTER` at 60.3 pt. Ours
/// moves it whole (0 / 3, `AFTER` at 91.6 pt) because it has no break that leaves two lines on each side; filed as
/// F30c-w. Pinned so that the fix of this plan does not change it by accident.
#[test]
fn a_three_line_paragraph_moves_whole_where_chrome_gives_up_the_widows_rule() {
    let words = printed_words(&render(&format!(
        "{}<p>{}</p><p>AFTER</p>",
        spacer(966),
        lines(3, 0)
    )));
    assert_eq!(per_page(&words), [0, 3], "ours; Chrome 2 / 1");
    assert_after(&words, 2, 91.6);
}

/// A relaxed `avoid` paragraph (60 lines, taller than a page) starts the next page: Chrome 0 / 48 / 12, `AFTER` at 233.6
/// pt (fixture 13 and `b200-text` rely on it).
#[test]
fn a_relaxed_avoid_paragraph_still_starts_the_next_page() {
    let words = printed_words(&render(&format!(
        "{}<p style=\"break-inside:avoid\">{}</p><p>AFTER</p>",
        spacer(300),
        lines(60, 0)
    )));
    assert_eq!(per_page(&words), [0, 48, 12]);
    assert_after(&words, 3, 233.6);
}

/// A padded paragraph goes through the F30f-d path, which already starts it in place: Chrome 16 / 14, `AFTER` at 271.8 pt.
#[test]
fn a_padded_paragraph_keeps_its_split() {
    let words = printed_words(&render(&format!(
        "{}<p style=\"padding:8px;border:1px solid #888\">{}</p><p>AFTER</p>",
        spacer(650),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [16, 14]);
    assert_after(&words, 2, 271.8);
}

/// A floated paragraph is not placed by this path. Chrome splits it 17 / 13 (`AFTER` at 249.3 pt); ours moves it whole
/// (0 / 30, 516.8 pt), as before. Filed as F30c-f.
#[test]
fn a_floated_paragraph_keeps_its_page_until_f30c_f() {
    let words = printed_words(&render(&format!(
        "{}<p style=\"float:left;width:300px\">{}</p><p style=\"clear:both\">AFTER</p>",
        spacer(650),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [0, 30], "ours; Chrome 17 / 13");
    assert_after(&words, 2, 516.8);
}
