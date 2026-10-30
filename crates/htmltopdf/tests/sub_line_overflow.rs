//! A block that overflows its page by less than one line leaves two lines for the next page (F30f-l, and the first two
//! bullets of F30c).
//!
//! css-break-3 §4.4: a break between lines is allowed with at least `orphans` lines before it and `widows` lines after it
//! (both 2 initially), and these rules are relaxed only when no other break keeps the content inside the fragmentainer.
//! A body-level block whose last line does not fit used to skip the break instead of pulling it back one line: all lines
//! stayed on the page and the last one stood in the bottom margin (at `@page { margin: 0 }`, off the page and cut).
//! Chrome moves the last two lines to the next page.
//!
//! Every number is Chrome's (m141, A4, 15 mm unless noted, Inter, `14px/21px`, lines `w0000<br>w0001…` without a
//! trailing `<br>`), measured with `out/scratch/f30fl/{gen,sum}.py` (plan §6 step 1). A page holds 48 such lines (48 x 21 =
//! 1008 px against a content box of 1009.13 px, Chrome's 1010 px).

use htmltopdf::{RenderOptions, Renderer};

/// The content box of A4 with 15 mm margins, in pt from the page top: its bottom.
const CONTENT_BOTTOM: f32 = 799.37;
/// The A4 page height in pt.
const PAGE_HEIGHT: f32 = 841.89;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(margin: &str, body: &str) -> Vec<u8> {
    let head = format!(
        "@page{{size:A4;margin:{margin}}}body{{font:14px/21px Inter;margin:0}}p{{margin:0}}"
    );
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

/// `w0000<br>w0001<br>…` (no trailing `<br>`).
fn lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
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

fn page_of_each_word(words: &[Word]) -> Vec<usize> {
    words
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.page)
        .collect()
}

/// F30f-l: 49 lines of 21 px are 1029 px; the content box is 1009.13 px. Chrome: 47 / 2, `AFTER` on page 2 at 76.1 pt.
/// Before the fix: 49 / 0, `w0048` at 800-813 pt below the content bottom, `AFTER` at 44.3 pt on page 2.
#[test]
fn a_block_one_line_taller_than_its_page_leaves_two_lines_for_the_next() {
    let words = printed_words(&render(
        "15mm",
        &format!("<div>{}</div><p>AFTER</p>", lines(49)),
    ));
    assert_eq!(per_page(&words), vec![47, 2], "47 / 2 like Chrome");
    assert_inside(&words, CONTENT_BOTTOM);
    assert_eq!(at(&words, "w0046").0, 1);
    assert_eq!(at(&words, "w0047").0, 2, "the last two lines go to page 2");
    let (page, y) = after(&words);
    assert_eq!(page, 2);
    assert!(
        (y - 76.1).abs() <= 1.0,
        "AFTER at {y:.1} pt, Chrome 76.1 pt"
    );
}

/// The same with a `<p>`.
#[test]
fn a_paragraph_one_line_taller_than_its_page_leaves_two_lines_for_the_next() {
    let words = printed_words(&render(
        "15mm",
        &format!("<p>{}</p><p>AFTER</p>", lines(49)),
    ));
    assert_eq!(per_page(&words), vec![47, 2], "47 / 2 like Chrome");
    assert_inside(&words, CONTENT_BOTTOM);
    let (page, y) = after(&words);
    assert_eq!(page, 2);
    assert!(
        (y - 76.1).abs() <= 1.0,
        "AFTER at {y:.1} pt, Chrome 76.1 pt"
    );
}

/// 97 lines: 48 + 48 + 1. Chrome: 48 / 47 / 2 (page 2 gives the last line a companion), `AFTER` on page 3 at 76.1 pt.
/// Before the fix: 48 / 49 / 0, page 2's last line below its content box.
#[test]
fn a_long_paragraph_with_a_one_line_remainder_on_its_last_page() {
    let words = printed_words(&render(
        "15mm",
        &format!("<p>{}</p><p>AFTER</p>", lines(97)),
    ));
    assert_eq!(per_page(&words), vec![48, 47, 2], "48 / 47 / 2 like Chrome");
    assert_inside(&words, CONTENT_BOTTOM);
    let (page, y) = after(&words);
    assert_eq!(page, 3);
    assert!(
        (y - 76.1).abs() <= 1.0,
        "AFTER at {y:.1} pt, Chrome 76.1 pt"
    );
}

/// F30c bullet 1: at `@page { margin: 0 }` a page holds 53 lines (1113 of 1122.5 px). 54 lines: Chrome 52 / 2. Before the
/// fix the 54th line was at the page edge, off the page, and cut: 53 / 0.
#[test]
fn at_margin_zero_the_last_line_is_not_cut_off() {
    let words = printed_words(&render("0", &format!("<div>{}</div>", lines(54))));
    assert_eq!(per_page(&words), vec![52, 2], "52 / 2 like Chrome");
    assert_inside(&words, PAGE_HEIGHT);
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        assert_eq!(w.page, if w.text.as_str() < "w0052" { 1 } else { 2 });
    }
    let (page, y) = at(&words, "w0052");
    assert_eq!(page, 2);
    assert!(y < 20.0, "w0052 at the top of page 2, {y:.1} pt");
}

/// Guards (green before and after, bytes unchanged): lines that fit, a remainder of two or more lines, and the same
/// block nested in a `<section>` (which was already split by the nested path).
#[test]
fn blocks_that_fit_or_leave_two_or_more_lines_keep_their_split() {
    // 48 lines fit exactly: Chrome 48 / 0, AFTER on page 2.
    let words = printed_words(&render(
        "15mm",
        &format!("<div>{}</div><p>AFTER</p>", lines(48)),
    ));
    assert_eq!(per_page(&words), vec![48, 0]);
    assert_eq!(after(&words).0, 2);
    // 50 and 51 lines: Chrome 48 / 2 and 48 / 3.
    for (n, second) in [(50, 2), (51, 3)] {
        let words = printed_words(&render(
            "15mm",
            &format!("<div>{}</div><p>AFTER</p>", lines(n)),
        ));
        assert_eq!(per_page(&words), vec![48, second], "{n} lines");
        assert_inside(&words, CONTENT_BOTTOM);
    }
    // 98 and 99 lines: Chrome 48 / 48 / 2 and 48 / 48 / 3.
    for (n, third) in [(98, 2), (99, 3)] {
        let words = printed_words(&render("15mm", &format!("<p>{}</p>", lines(n))));
        assert_eq!(per_page(&words), vec![48, 48, third], "{n} lines");
    }
    // Nested in a <section> the nested path splits 47 / 2 (it pulls the break back already), like Chrome.
    let words = printed_words(&render(
        "15mm",
        &format!("<section><div>{}</div></section><p>AFTER</p>", lines(49)),
    ));
    assert_eq!(per_page(&words), vec![47, 2]);
    assert_inside(&words, CONTENT_BOTTOM);
    let words = printed_words(&render(
        "15mm",
        &format!("<section><div>{}</div></section><p>AFTER</p>", lines(97)),
    ));
    assert_eq!(per_page(&words), vec![48, 47, 2]);
}

/// Shape 2 of the finding (a box whose lines all fit but whose bottom padding and border end in the last pixel of Chrome's
/// 1010 px page) is not this fix; it is F30f-c (Chrome's page capacity). These four files render as before. Chrome keeps
/// the whole box on page 1 (46 / 0, 47 / 0, 47 / 0 and 45 / 0 with `AFTER` on page 1); ours moves two lines (44 / 2, 45 / 2) or
/// `AFTER` (45 / 0, `AFTER` on page 2). When F30f-c lands, these expectations become Chrome's.
#[test]
fn boxes_whose_bottom_inset_ends_in_chromes_last_pixel_keep_main_output_until_f30f_c() {
    let case = |style: &str, n: usize| -> Vec<Word> {
        printed_words(&render(
            "15mm",
            &format!("<div style='{style}'>{}</div><p>AFTER</p>", lines(n)),
        ))
    };
    let w = case(
        "padding:20px 20px 22px;border:1px solid #888;border-radius:8px",
        46,
    );
    assert_eq!(per_page(&w), vec![44, 2], "exact_n46_pb22");
    let w = case("padding:0 10px 21px;border:1px solid #888", 47);
    assert_eq!(per_page(&w), vec![45, 2], "pd_n47");
    let w = case("padding:21px 10px 0px;border:1px solid #888", 47);
    assert_eq!(per_page(&w), vec![45, 2], "pdt_n47");
    let w = case(
        "padding:20px 20px 22px;border:1px solid #888;border-radius:8px",
        45,
    );
    assert_eq!(per_page(&w), vec![45, 0], "exact_n45_pb22");
    assert_eq!(after(&w).0, 2, "AFTER moves to page 2 (Chrome: page 1)");
}

/// Every word is on the page it should be on, in order (no word lost or duplicated by the pulled-back break).
#[test]
fn every_line_is_printed_once_in_order() {
    let words = printed_words(&render("15mm", &format!("<p>{}</p>", lines(97))));
    let pages = page_of_each_word(&words);
    assert_eq!(pages.len(), 97);
    assert!(pages.windows(2).all(|p| p[0] <= p[1]));
}
