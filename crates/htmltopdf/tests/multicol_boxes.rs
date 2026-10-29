//! A box taller than a page inside a multi-column container is cut into the columns and pages, not placed whole (F198).
//!
//! `<div style="column-count:2"><div style="padding:16px;border:1px solid;…">2500 words</div></div><p>AFTER</p>`
//! is 7140 px tall at the column width. The multicol fragmenter placed such a box alone in its column ("a box taller than a
//! page is placed alone and overflows", F30d-b): 361 of 2500 words were printed, the first column ran 37 pt past the
//! page edge, the second was empty, and `AFTER` sat on page 2. Chrome prints 4 pages: the box is a fragmentainer
//! child like any block and breaks between its lines (css-break-3 §4.1, §4.4), column after column and page after
//! page, with its frame sliced at every break (`box-decoration-break: slice`, §5.4: no border, padding or radius at a
//! break edge) and the columns of the last page balanced (css-multicol-1 §7.1, §8.2).
//!
//! The numbers in the tables are Chrome 148's for the same HTML (A4, 15 mm margins, Inter 14px / 1.2, words `w0000 …`,
//! `scripts/reference` probes): pages, the first word of every column of every page, where `AFTER` lands. Asserted on
//! `pdftotext -bbox`: every word exactly once in column-major order (page, then column, then top to bottom), the first
//! word of every column within one line (nine words here) of Chrome's, nothing below the content box, the text inside the
//! frame by the padding, the frame painted in both columns of the pages.

use htmltopdf::{RenderOptions, Renderer};

const PAGE_W_PT: f32 = 595.28;
const PAGE_H_PT: f32 = 841.89;
/// 15 mm in pt.
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// `column-gap: 20px`, in pt.
const GAP_PT: f32 = 15.0;
/// Chrome's content bottom is at `PAGE_H_PT - MARGIN_PT` = 799.37; its last line ends at 798.3.
const BOTTOM_PT: f32 = PAGE_H_PT - MARGIN_PT;
/// One line holds at most this many of the 5-character words in a 296 px column.
const LINE_WORDS: usize = 9;

/// How far from Chrome a shape may be: the words a column starts from Chrome's first word of it, and the pt between
/// `AFTER` and Chrome's.
#[derive(Clone, Copy)]
struct Tol {
    col_words: usize,
    after_pt: f32,
}

/// One line, or so (a line is 20 pt for `AFTER`).
const ONE_LINE: Tol = Tol {
    col_words: LINE_WORDS,
    after_pt: 20.0,
};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// Words `w<from> … w<to-1>`, four digits each.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

const CARD: &str = "padding:16px;border:1px solid #888;border-radius:8px;background:#f5f5f5;";

/// A 60 x 60 red PNG (`data/red60.png`), registered as `red60.png`.
const RED60: &[u8] = include_bytes!("data/red60.png");

/// The document of `body`: A4 with 15 mm margins (the engine's default; `@page` says it for Chrome), Inter 14px / 1.2.
fn document(body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=utf-8><style>\
         @page{{size:A4;margin:15mm}}body{{margin:0;font:14px/1.2 Inter}}p{{margin:0}}\
         </style></head><body>{body}</body></html>"
    )
}

fn render(body: &str) -> Vec<u8> {
    let html = document(body);
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .image("red60.png", RED60.to_vec())
        .strict_fonts(true)
        .build()
        .render(&html, &RenderOptions::default())
        .expect("render")
}

#[derive(Clone, Debug)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    y_max: f32,
    text: String,
}

impl Word {
    /// The word's index when it is a `wNNNN` word.
    fn index(&self) -> Option<usize> {
        let t = self.text.strip_prefix('w')?;
        (t.len() == 4).then(|| t.parse().ok()).flatten()
    }
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-mcb-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
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
                x_min: attr(attrs, "xMin"),
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

/// Gray values (0..255) of page `page` (1-based) rasterised at 72 dpi (one pixel per pt): `(width, height, bytes)`.
fn gray_page(pdf: &[u8], page: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let root = path.with_extension("pg");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "72", "-gray", "-singlefile", "-f"])
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg(&path)
        .arg(&root)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftoppm failed");
    let pgm = root.with_extension("pg.pgm");
    let data = std::fs::read(&pgm).expect("pdftoppm wrote no page");
    let _ = std::fs::remove_file(&pgm);
    // Binary PGM: `P5\n<w> <h>\n255\n` then w*h bytes.
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
    let (w, h) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    (w, h, data[pos..].to_vec())
}

/// What Chrome prints for a shape: the first word of every column of every page (`None`: the column is empty) and
/// where `AFTER` is.
struct Chrome {
    first: &'static [&'static [Option<usize>]],
    after_page: usize,
    after_y: f32,
}

/// Column `0..n` of a word, by its left edge against the middle of the gutters.
fn column(w: &Word, n: usize) -> usize {
    let col_w = (PAGE_W_PT - 2.0 * MARGIN_PT - (n - 1) as f32 * GAP_PT) / n as f32;
    let c = ((w.x_min - MARGIN_PT + GAP_PT / 2.0) / (col_w + GAP_PT)).floor();
    (c.max(0.0) as usize).min(n - 1)
}

/// The `wNNNN` words in reading order: page, then column, then line (top to bottom), then left to right. Words of one
/// line can differ in height by a pt or two (the text of an inline-block sits lower in its line than the text beside it), so
/// a line is a run of words less than half a line pitch (6 pt) apart, not words with the same top.
fn column_major(found: &[Word], n: usize) -> Vec<Word> {
    let mut v: Vec<Word> = found
        .iter()
        .filter(|w| w.index().is_some())
        .cloned()
        .collect();
    v.sort_by(|a, b| {
        (a.page, column(a, n))
            .cmp(&(b.page, column(b, n)))
            .then(a.y_min.total_cmp(&b.y_min))
    });
    // Number the lines, then order each by x.
    let mut line = 0_usize;
    let mut keyed: Vec<(usize, Word)> = Vec::with_capacity(v.len());
    for (i, w) in v.iter().enumerate() {
        if i > 0 {
            let prev = &v[i - 1];
            if (prev.page, column(prev, n)) != (w.page, column(w, n)) || w.y_min - prev.y_min > 6.0
            {
                line += 1;
            }
        }
        keyed.push((line, w.clone()));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.x_min.total_cmp(&b.1.x_min)));
    keyed.into_iter().map(|(_, w)| w).collect()
}

/// `w0000 … w<last-1>` exactly once each, in column-major order, nothing below the content box.
fn assert_every_word_once(what: &str, found: &[Word], n_cols: usize, last: usize) -> Vec<Word> {
    let ordered = column_major(found, n_cols);
    let seen: Vec<usize> = ordered.iter().filter_map(Word::index).collect();
    let first_diff = seen
        .iter()
        .enumerate()
        .position(|(i, w)| *w != i)
        .or((seen.len() != last).then_some(seen.len().min(last)));
    assert!(
        first_diff.is_none(),
        "{what}: every word exactly once, column-major: printed {} of {last}, first difference at index {first_diff:?} \
         (printed {:?})",
        seen.len(),
        first_diff.and_then(|i| seen.get(i)),
    );
    for w in found {
        assert!(
            w.y_max <= BOTTOM_PT + 0.5,
            "{what}: {} on page {} ends at {:.1}pt, below the content box ({BOTTOM_PT:.1}pt)",
            w.text,
            w.page,
            w.y_max
        );
    }
    ordered
}

/// The whole check of a long card: words, pages, the first word of every column, `AFTER`, the text inside the frame.
/// `inset_pt` is the distance of the text from the column edge (the card's border and padding: 17 px, or 0).
fn check_shape(what: &str, pdf: &[u8], n_cols: usize, chrome: &Chrome, inset_pt: f32) {
    check_shape_with(
        what,
        pdf,
        n_cols,
        chrome,
        inset_pt,
        2500,
        Some(inset_pt),
        ONE_LINE,
    );
}

/// [`check_shape`] for a shape of `total` words. `top_inset_pt` is how far below the content box the first line of page 1
/// column 1 starts (the card's border and padding, and a header above its text); `None` skips the check of where each
/// column's first line is (a line that holds an inline box is taller than its neighbours: its text sits lower in it).
/// `tol` is how far from Chrome the columns and `AFTER` may be: one line, or more where Parley's line box differs from
/// Chrome's (F198h).
#[allow(clippy::too_many_arguments)]
fn check_shape_with(
    what: &str,
    pdf: &[u8],
    n_cols: usize,
    chrome: &Chrome,
    inset_pt: f32,
    total: usize,
    top_inset_pt: Option<f32>,
    tol: Tol,
) {
    let found = printed_words(pdf);
    let ordered = assert_every_word_once(what, &found, n_cols, total);
    let pages = page_count(pdf);
    assert_eq!(
        pages,
        chrome.first.len().max(chrome.after_page),
        "{what}: pages like Chrome"
    );
    // The first word of every column of every page, within one line of Chrome's.
    for (p, cols) in chrome.first.iter().enumerate() {
        for (c, want) in cols.iter().enumerate() {
            let got = ordered
                .iter()
                .find(|w| w.page == p + 1 && column(w, n_cols) == c)
                .and_then(Word::index);
            match (want, got) {
                (Some(want), Some(got)) => assert!(
                    want.abs_diff(got) <= tol.col_words,
                    "{what}: page {} column {} starts with w{got:04}, Chrome w{want:04} (at most {} words apart)",
                    p + 1,
                    c + 1,
                    tol.col_words
                ),
                (None, None) => {}
                _ => panic!(
                    "{what}: page {} column {}: ours {got:?}, Chrome {want:?}",
                    p + 1,
                    c + 1
                ),
            }
        }
    }
    // `AFTER` once, on Chrome's page, within a line or two of where Chrome puts it, below every word.
    let after: Vec<&Word> = found.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(after.len(), 1, "{what}: AFTER printed once");
    assert_eq!(
        after[0].page, chrome.after_page,
        "{what}: AFTER on Chrome's page"
    );
    assert!(
        (after[0].y_min - chrome.after_y).abs() <= tol.after_pt,
        "{what}: AFTER at {:.1}pt, Chrome {:.1}pt",
        after[0].y_min,
        chrome.after_y
    );
    // The text is inside the frame: column 1 starts `inset_pt` right of the margin (Chrome 55.5 for 17 px) and every
    // column one stride (column width + gap) right of it; a column that starts a page starts at its top (Chrome 42.3,
    // 55.1 in the first column of the first page, below the card's top padding and border).
    let col_w = (PAGE_W_PT - 2.0 * MARGIN_PT - (n_cols - 1) as f32 * GAP_PT) / n_cols as f32;
    for w in ordered.iter() {
        let c = column(w, n_cols);
        let x0 = MARGIN_PT + inset_pt + c as f32 * (col_w + GAP_PT);
        assert!(
            w.x_min >= x0 - 1.0 && w.x_min <= x0 + col_w,
            "{what}: {} on page {} column {} at x {:.1}pt, the column's text starts at {x0:.1}pt",
            w.text,
            w.page,
            c + 1,
            w.x_min
        );
    }
    for p in 1..=chrome.first.len() {
        let Some(top_inset_pt) = top_inset_pt else {
            break;
        };
        for c in 0..n_cols {
            let top = ordered
                .iter()
                .filter(|w| w.page == p && column(w, n_cols) == c)
                .map(|w| w.y_min)
                .reduce(f32::min);
            let Some(top) = top else { continue };
            let want = if p == 1 && c == 0 {
                MARGIN_PT + top_inset_pt - 0.3 // below the card's top border and padding
            } else {
                MARGIN_PT - 0.3
            };
            assert!(
                (top - want).abs() <= 1.5,
                "{what}: page {p} column {} starts at y {top:.1}pt, expected {want:.1}pt",
                c + 1
            );
        }
    }
}

fn card(n: usize) -> String {
    format!("<div style='{CARD}'>{}</div>", words(0, n))
}

fn two_columns(inner: &str) -> String {
    format!("<div style='column-count:2;column-gap:20px'>{inner}</div><p>AFTER</p>")
}

const CHROME_A: Chrome = Chrome {
    first: &[
        &[Some(0), Some(346)],
        &[Some(690), Some(1043)],
        &[Some(1406), Some(1766)],
        &[Some(2125), Some(2317)],
    ],
    after_page: 4,
    after_y: 445.8,
};

/// 17 px (1 px border, 16 px padding) in pt, as Chrome draws the text inside the card's frame: 55.5 - 42.52.
const CARD_INSET_PT: f32 = 12.75;

#[test]
fn a_headline_card_of_direct_text_flows_over_columns_and_pages() {
    // A. Main: 361 of 2500 words, 2 pages (the card is one 7140 px tall box placed whole in column 1).
    let pdf = render(&two_columns(&card(2500)));
    check_shape("card of text", &pdf, 2, &CHROME_A, CARD_INSET_PT);
}

#[test]
fn a_card_has_its_background_in_both_columns_of_every_page() {
    // The frame is sliced at the breaks, not drawn once (css-break-3 §5.4): both columns of page 1 and of the pages
    // after it show the card's background (#f5f5f5, gray 245) beside the text, the gutter between them is white.
    let pdf = render(&two_columns(&card(2500)));
    let col_w = (PAGE_W_PT - 2.0 * MARGIN_PT - GAP_PT) / 2.0;
    let left1 = MARGIN_PT + 5.0; // inside the left padding of column 1
    let left2 = MARGIN_PT + col_w + GAP_PT + 5.0;
    let gutter = MARGIN_PT + col_w + GAP_PT / 2.0;
    for page in 1..=3 {
        let (w, _, px) = gray_page(&pdf, page);
        let at = |x: f32, y: f32| px[y as usize * w + x as usize];
        for (what, x) in [("column 1", left1), ("column 2", left2)] {
            let v = at(x, 420.0);
            assert!(
                (240..=250).contains(&v),
                "page {page} {what}: gray {v} at ({x:.0}, 420), the card's background is 245"
            );
        }
        let g = at(gutter, 420.0);
        assert!(
            g >= 253,
            "page {page}: gray {g} in the gutter, white expected"
        );
    }
}

#[test]
fn the_text_of_every_column_is_painted_above_the_frames() {
    // A box that is its own paragraph (a `<div>` or `<p>` of text) has a frame in each column, and the text of both
    // columns is one slice list painted once: it has to come after the last frame, or the background of column 2 covers
    // the text of column 1 and 2 (the first version painted it with the first piece and column 2 was blank).
    let col_w = (PAGE_W_PT - 2.0 * MARGIN_PT - GAP_PT) / 2.0;
    for (what, inner) in [
        ("div of text", card(2500)),
        (
            "p of text",
            format!("<p style='{CARD}'>{}</p>", words(0, 2500)),
        ),
    ] {
        let pdf = render(&two_columns(&inner));
        for page in 1..=3 {
            let (w, _, px) = gray_page(&pdf, page);
            for (col, x0) in [MARGIN_PT, MARGIN_PT + col_w + GAP_PT]
                .into_iter()
                .enumerate()
            {
                let mut ink = 0;
                for y in 100..700 {
                    for x in (x0 as usize + 5)..(x0 + col_w - 5.0) as usize {
                        if px[y * w + x] < 150 {
                            ink += 1;
                        }
                    }
                }
                assert!(
                    ink > 800,
                    "{what}: page {page} column {} has {ink} dark pixels, its text is hidden",
                    col + 1
                );
            }
        }
    }
}

#[test]
fn a_plain_div_of_text_flows_over_columns_and_pages() {
    // B. The same `<div>` without padding, border or background. Chrome: 742 / 801 / 803 / 154 words.
    let pdf = render(&two_columns(&format!("<div>{}</div>", words(0, 2500))));
    let chrome = Chrome {
        first: &[
            &[Some(0), Some(377)],
            &[Some(742), Some(1123)],
            &[Some(1543), Some(1963)],
            &[Some(2346), Some(2425)],
        ],
        after_page: 4,
        after_y: 205.8,
    };
    check_shape("plain div", &pdf, 2, &chrome, 0.0);
}

#[test]
fn a_card_of_paragraphs_flows_over_columns_and_pages() {
    // C. The card holds 10 `<p>` of 250 words. Chrome: 689 / 705 / 713 / 393 words.
    let paragraphs: String = (0..10)
        .map(|i| format!("<p>{}</p>", words(i * 250, i * 250 + 250)))
        .collect();
    let pdf = render(&two_columns(&format!(
        "<div style='{CARD}'>{paragraphs}</div>"
    )));
    let chrome = Chrome {
        first: &[
            &[Some(0), Some(345)],
            &[Some(689), Some(1036)],
            &[Some(1394), Some(1750)],
            &[Some(2107), Some(2310)],
        ],
        after_page: 4,
        after_y: 470.6,
    };
    check_shape("card of paragraphs", &pdf, 2, &chrome, CARD_INSET_PT);
}

#[test]
fn a_card_in_three_columns_flows_over_columns_and_pages() {
    // D. `column-count: 3`. Chrome: 5 pages, 541 / 545 / 591 / 572 / 251 words.
    let pdf = render(&format!(
        "<div style='column-count:3;column-gap:20px'>{}</div><p>AFTER</p>",
        card(2500)
    ));
    let chrome = Chrome {
        first: &[
            &[Some(0), Some(181), Some(361)],
            &[Some(541), Some(721), Some(901)],
            &[Some(1086), Some(1298), Some(1486)],
            &[Some(1677), Some(1878), Some(2064)],
            &[Some(2249), Some(2336), Some(2423)],
        ],
        after_page: 5,
        after_y: 407.6,
    };
    check_shape("three columns", &pdf, 3, &chrome, CARD_INSET_PT);
}

#[test]
fn a_card_with_column_fill_auto_is_not_balanced_on_its_last_page() {
    // E. `column-fill: auto`: columns are filled in order, the last page too. Chrome: 5 pages, the 4th page's column 1 is
    // full (359 words) and column 2 holds the last 16; `AFTER` is on page 5, the container being a page tall.
    let pdf = render(&format!(
        "<div style='column-count:2;column-gap:20px;column-fill:auto'>{}</div><p>AFTER</p>",
        card(2500)
    ));
    let chrome = Chrome {
        first: &[
            &[Some(0), Some(346)],
            &[Some(690), Some(1043)],
            &[Some(1406), Some(1766)],
            &[Some(2125), Some(2484)],
        ],
        after_page: 5,
        after_y: 42.3,
    };
    check_shape("column-fill: auto", &pdf, 2, &chrome, CARD_INSET_PT);
    let found = printed_words(&pdf);
    let on_last = |c: usize| {
        found
            .iter()
            .filter(|w| w.index().is_some() && w.page == 4 && column(w, 2) == c)
            .count()
    };
    assert!(
        on_last(0) > 300 && on_last(1) < 40,
        "page 4 is not balanced: {} words in column 1, {} in column 2 (Chrome 359 / 16)",
        on_last(0),
        on_last(1)
    );
}

#[test]
fn a_padded_paragraph_in_columns_keeps_its_padding() {
    // F. A `<p>` with padding and a border as the direct child: it was already cut over the columns and pages
    // (all words printed), but the padding was ignored: the text started at the column edge, 17 px too far left and
    // 13 pt too high, outside the frame.
    let pdf = render(&two_columns(&format!(
        "<p style='{CARD}'>{}</p>",
        words(0, 2500)
    )));
    check_shape("padded paragraph", &pdf, 2, &CHROME_A, CARD_INSET_PT);
}

// ── G. guards: shapes that already worked keep all their words ─────────────────────────────────────────────────

fn all_words_once(what: &str, body: &str, n: usize, pages: Option<usize>) -> Vec<Word> {
    let pdf = render(body);
    let found = printed_words(&pdf);
    assert_every_word_once(what, &found, 2, n);
    if let Some(pages) = pages {
        assert_eq!(page_count(&pdf), pages, "{what}: pages");
    }
    found
}

#[test]
fn guard_direct_paragraphs_in_columns() {
    // Chrome: 4 pages, 742 / 798 / 801 / 59 words.
    let paragraphs: String = (0..3)
        .map(|i| format!("<p>{}</p>", words(i * 800, i * 800 + 800)))
        .collect();
    let found = all_words_once("direct <p>s", &two_columns(&paragraphs), 2400, Some(4));
    assert_eq!(found.iter().filter(|w| w.text == "AFTER").count(), 1);
}

#[test]
fn guard_bare_text_in_columns() {
    // Chrome: 4 pages, 742 / 801 / 803 / 154 words.
    all_words_once("bare text", &two_columns(&words(0, 2500)), 2500, Some(4));
}

#[test]
fn guard_a_card_that_fits_a_page_is_not_entered() {
    // 300 words in a card: it fits a page and stays one box in column 1 (Chrome balances it 150 / 150: F30d-c).
    all_words_once("short card", &two_columns(&card(300)), 300, Some(1));
}

#[test]
fn guard_a_short_card_that_starts_in_the_middle_of_a_page_loses_nothing() {
    // After a 600 px filler: main moves the card whole to page 2 (Chrome splits it 275 / 25). No word is lost.
    all_words_once(
        "card mid-page",
        &format!(
            "<div style='height:600px'></div>{}",
            two_columns(&card(300))
        ),
        300,
        Some(2),
    );
}

#[test]
fn guard_a_multicol_container_in_a_card_still_prints_everything() {
    // `multicol_in_card`: 4 pages, 710 / 739 / 733 / 318 words, the same as Chrome.
    all_words_once(
        "multicol in a card",
        &format!(
            "<div style='{CARD}'><div style='column-count:2;column-gap:20px'>{}</div></div><p>AFTER</p>",
            words(0, 2500)
        ),
        2500,
        Some(4),
    );
}

#[test]
fn guard_a_table_in_a_card_prints_at_least_what_main_printed() {
    // A 130-row table in a card in two columns (F174c): main prints 112 of 260 words, Chrome all of them. The box path
    // must not make it worse.
    let rows: String = (0..130)
        .map(|i| format!("<tr><td>w{:04}</td><td>w{:04}</td></tr>", 2 * i, 2 * i + 1))
        .collect();
    let pdf = render(&two_columns(&format!(
        "<div style='{CARD}'><table style='width:100%'>{rows}</table></div>"
    )));
    let found = printed_words(&pdf);
    let n = found.iter().filter(|w| w.index().is_some()).count();
    assert!(n >= 112, "{n} of 260 words printed, main printed 112");
}

// ── F198b: clip cards, and lines with inline boxes, in a box taller than a page ─────────────────────────────────

/// Pixels (`3 * w * h` bytes, RGB) of page `page` (1-based) rasterised at `dpi`.
fn rgb_page(pdf: &[u8], page: usize, dpi: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let root = path.with_extension("rgb");
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-singlefile", "-f"])
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg(&path)
        .arg(&root)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftoppm failed");
    let ppm = root.with_extension("rgb.ppm");
    let data = std::fs::read(&ppm).expect("pdftoppm wrote no page");
    let _ = std::fs::remove_file(&ppm);
    // Binary PPM: `P6\n<w> <h>\n255\n` then 3 * w * h bytes.
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
    (
        fields[1].parse().unwrap(),
        fields[2].parse().unwrap(),
        data[pos..].to_vec(),
    )
}

/// A pixel rectangle `(x_min, y_min, x_max, y_max)`, inclusive.
type PxBox = (usize, usize, usize, usize);

/// The connected groups of pixels that satisfy `is`, as bounding boxes (of the pixels that satisfy it) in reading order.
/// Pixels with up to `gap` empty pixels between them are one group: a 1 px border that glyphs overlap is one ring, not a
/// handful of pieces.
fn blobs(w: usize, h: usize, px: &[u8], is: impl Fn(u8, u8, u8) -> bool, gap: usize) -> Vec<PxBox> {
    let hit: Vec<bool> = (0..w * h)
        .map(|i| is(px[3 * i], px[3 * i + 1], px[3 * i + 2]))
        .collect();
    // Neighbours are the pixels within `reach`: the 8 around one for `gap` 0.
    let reach = gap + 1;
    let mut seen = vec![false; w * h];
    let mut found = Vec::new();
    for start in 0..w * h {
        if seen[start] || !hit[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            if hit[i] {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            // Every pixel within `gap` is a neighbour.
            for ny in y.saturating_sub(reach)..=(y + reach).min(h - 1) {
                for nx in x.saturating_sub(reach)..=(x + reach).min(w - 1) {
                    let n = ny * w + nx;
                    if !seen[n] && hit[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        found.push((x0, y0, x1, y1));
    }
    found
}

/// Red, however much of the pixel a 1 px line covers (the anti-aliased edges are pink).
fn is_red(r: u8, g: u8, b: u8) -> bool {
    r > 150 && r as i32 - g as i32 > 60 && r as i32 - b as i32 > 60
}

/// Raster scale of [`rgb_page`] at 144 dpi: px per pt.
const SCALE: f32 = 2.0;

/// The blobs of every page, as `(page, box in pt)`.
fn blobs_in_pt(
    pdf: &[u8],
    is: impl Fn(u8, u8, u8) -> bool + Copy,
    gap: usize,
) -> Vec<(usize, [f32; 4])> {
    let mut all = Vec::new();
    for page in 1..=page_count(pdf) {
        let (w, h, px) = rgb_page(pdf, page, 144);
        for (x0, y0, x1, y1) in blobs(w, h, &px, is, gap) {
            all.push((
                page,
                [
                    x0 as f32 / SCALE,
                    y0 as f32 / SCALE,
                    (x1 + 1) as f32 / SCALE,
                    (y1 + 1) as f32 / SCALE,
                ],
            ));
        }
    }
    all
}

/// Image placements in the PDF (`pdfimages -list`, one row per image drawn).
fn images_drawn(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfimages")
        .arg("-list")
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdfimages failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(2)
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// No word is hidden: every `wNNNN` has a dark pixel inside its box. A frame painted over a column's text (or a clip that
/// hides it) leaves the words written and the page blank there, which `pdftotext` cannot see.
fn assert_every_word_is_inked(what: &str, pdf: &[u8]) {
    let found = printed_words(pdf);
    for page in 1..=page_count(pdf) {
        let (w, h, px) = rgb_page(pdf, page, 144);
        let (mut bad, mut n) = (0, 0);
        for word in found
            .iter()
            .filter(|x| x.page == page && x.index().is_some())
        {
            // The box of the word at 144 dpi; the x range is the left edge to a 5-character word's width.
            let (x0, x1) = (
                (word.x_min * SCALE) as usize,
                (((word.x_min + 28.0) * SCALE) as usize).min(w - 1),
            );
            let (y0, y1) = (
                (word.y_min * SCALE) as usize,
                ((word.y_max * SCALE) as usize).min(h - 1),
            );
            let dark = (y0..=y1).any(|y| {
                (x0..=x1).any(|x| {
                    let i = 3 * (y * w + x);
                    (px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32) / 3 < 160
                })
            });
            n += 1;
            bad += usize::from(!dark);
        }
        assert_eq!(
            bad, 0,
            "{what}: page {page}: {bad} of {n} words have no ink where they are written (something is painted over them)"
        );
    }
}

/// The printed word `w<index>`.
fn word_at(found: &[Word], index: usize) -> &Word {
    found
        .iter()
        .find(|w| w.index() == Some(index))
        .unwrap_or_else(|| panic!("w{index:04} is not printed"))
}

/// Every box of `boxes` (an inline box drawn by the paragraph) stands beside the word that precedes it in the text, or the
/// one that follows it when it starts a line: on that page, on that word's line, and its bottom `dy_chrome` pt (± 1)
/// below the word's bottom, the offset Chrome has between the same two. `before[k]` is the index of the word before the
/// `k`-th box.
fn assert_boxes_beside_their_words(
    what: &str,
    found: &[Word],
    boxes: &[(usize, [f32; 4])],
    before: &[usize],
    dy_chrome: f32,
) {
    assert_eq!(
        boxes.len(),
        before.len(),
        "{what}: {} boxes drawn, {} expected, each once",
        boxes.len(),
        before.len()
    );
    let on_line = |word: &Word, b: &[f32; 4]| (b[3] - word.y_max).abs() < 8.0;
    for (k, &index) in before.iter().enumerate() {
        let prev = word_at(found, index);
        let next = found.iter().find(|w| w.index() == Some(index + 1));
        let beside_prev = boxes
            .iter()
            .filter(|(page, b)| *page == prev.page && b[0] >= prev.x_min && on_line(prev, b))
            .min_by(|a, b| a.1[0].total_cmp(&b.1[0]))
            .map(|b| (b, prev));
        let beside_next = || {
            let next = next?;
            boxes
                .iter()
                .filter(|(page, b)| {
                    *page == next.page && b[2] <= next.x_min + 30.0 && on_line(next, b)
                })
                .max_by(|a, b| a.1[2].total_cmp(&b.1[2]))
                .map(|b| (b, next))
        };
        let Some(((_, b), word)) = beside_prev.or_else(beside_next) else {
            // The last box of a paragraph that fills its line wraps to a line of its own, with no word beside it.
            assert!(
                next.is_none()
                    && boxes.iter().any(|(page, b)| *page == prev.page
                        && b[3] > prev.y_max
                        && b[3] - prev.y_max < 24.0),
                "{what}: no box beside w{index:04} (box {k}, page {}) or the word after it",
                prev.page
            );
            continue;
        };
        let dy = b[3] - word.y_max;
        assert!(
            (dy - dy_chrome).abs() <= 1.0,
            "{what}: box {k} (after w{index:04}) ends {dy:.1}pt below the bottom of its neighbour {}, Chrome {dy_chrome:.1}pt",
            word.text
        );
    }
}

/// The HTML of every shape of this section (`cargo test --test multicol_boxes -- --ignored dump_chrome_inputs` with
/// `MCB_DUMP=<dir>` writes them as `<name>.html`, so Chrome can print the same documents the numbers below come from).
/// A shape's name and the body of its document.
type Shape = (&'static str, fn() -> String);

const SHAPES: &[Shape] = &[
    ("overflow", shape_overflow),
    ("clipcard", shape_clip_card),
    ("imginline", shape_inline_images),
    ("imgp", shape_image_paragraph),
    ("imgcard", shape_image_paragraph_in_card),
    ("inlineblock", shape_inline_blocks),
    ("badge", shape_inline_flex_badges),
    ("svgdiv", shape_inline_svgs),
    ("svgp", shape_inline_svg_paragraph),
    ("clipheight", shape_clip_height),
    ("abspos", shape_abspos),
    ("float", shape_float),
];

#[test]
#[ignore = "writes the shapes' HTML for the Chrome reference run"]
fn dump_chrome_inputs() {
    let dir = std::env::var("MCB_DUMP").expect("MCB_DUMP=<dir>");
    // Chrome needs the font by URL; the engine gets the same file as bytes.
    let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf")
        .canonicalize()
        .unwrap();
    let face = format!(
        "<style>@font-face{{font-family:Inter;src:url(file://{})}}",
        font.display()
    );
    // The images are next to the HTML.
    for (name, shape) in SHAPES {
        let html = document(&shape()).replacen("<style>", &face, 1);
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.html")),
            html,
        )
        .unwrap();
    }
    std::fs::write(std::path::Path::new(&dir).join("red60.png"), RED60).unwrap();
}

fn three_columns(inner: &str) -> String {
    format!("<div style='column-count:3;column-gap:20px'>{inner}</div><p>AFTER</p>")
}

/// H: the card of `a_headline_card_of_direct_text…` with `overflow:hidden` (Tailwind's `overflow-hidden`).
fn shape_overflow() -> String {
    two_columns(&format!(
        "<div style='{CARD}overflow:hidden'>{}</div>",
        words(0, 2500)
    ))
}

/// H2: Tailwind's card idiom, `overflow-hidden rounded-lg border` with a header strip that the corners clip, and
/// ten paragraphs of 250 words in a padded body.
fn shape_clip_card() -> String {
    let paragraphs: String = (0..10)
        .map(|i| format!("<p>{}</p>", words(i * 250, i * 250 + 250)))
        .collect();
    two_columns(&format!(
        "<div style='border:1px solid #888;border-radius:24px;overflow:hidden'>\
         <div style='background:#ff0000;padding:8px 12px;color:#fff'>HEADER STRIP</div>\
         <div style='padding:12px'>{paragraphs}</div></div>"
    ))
}

const IMG: &str = "<img src='red60.png' style='width:30px;height:18px'>";

/// I: 14 inline images, one after every 150 words, in the text of the card (the card is the inline root).
fn shape_inline_images() -> String {
    let text: String = (0..14)
        .map(|k| format!("{} {IMG} ", words(k * 150, k * 150 + 150)))
        .collect();
    two_columns(&format!("<div style='{CARD}'>{text}</div>"))
}

/// I2: a `<p>` of 1200 words with one inline image in the middle, a direct child of three columns.
fn image_paragraph() -> String {
    format!(
        "<p style='orphans:1;widows:1'>{} <img src='red60.png' style='width:14px;height:14px'> {}</p>",
        words(0, 600),
        words(600, 1200)
    )
}

fn shape_image_paragraph() -> String {
    three_columns(&image_paragraph())
}

/// I2: the same `<p>` inside a card.
fn shape_image_paragraph_in_card() -> String {
    three_columns(&format!("<div style='{CARD}'>{}</div>", image_paragraph()))
}

const BOX: &str = "display:inline-block;border:1px solid #ff0000;padding:2px";

/// J: 60 `inline-block`s of two words in the text of the card, one after every 30 words (1920 words).
fn shape_inline_blocks() -> String {
    let text: String = (0..60)
        .map(|k| {
            format!(
                "{} <span style='{BOX}'>w{:04} w{:04}</span> ",
                words(k * 32, k * 32 + 30),
                k * 32 + 30,
                k * 32 + 31
            )
        })
        .collect();
    two_columns(&format!("<div style='{CARD}'>{text}</div>"))
}

const BADGE: &str = "display:inline-flex;align-items:center;border:1px solid #00aa00;padding:1px 4px;border-radius:4px";

/// J2: 50 `inline-flex` badges `b000` … in the text of the card, one after every 50 words (2500 words).
fn shape_inline_flex_badges() -> String {
    let text: String = (0..50)
        .map(|k| {
            format!(
                "{} <span style='{BADGE}'>b{k:03}</span> ",
                words(k * 50, k * 50 + 50)
            )
        })
        .collect();
    two_columns(&format!("<div style='{CARD}'>{text}</div>"))
}

const SVG: &str = "<svg width='12' height='12' viewBox='0 0 12 12' style='display:inline'>\
                   <rect width='12' height='12' fill='#ff0000'/></svg>";

/// K: 50 inline `<svg>` icons, one after every 50 words, in the text of the card (2500 words).
fn shape_inline_svgs() -> String {
    let text: String = (0..50)
        .map(|k| format!("{} {SVG} ", words(k * 50, k * 50 + 50)))
        .collect();
    two_columns(&format!("<div style='{CARD}'>{text}</div>"))
}

/// K: the same icons in one `<p>` of the card.
fn shape_inline_svg_paragraph() -> String {
    let text: String = (0..50)
        .map(|k| format!("{} {SVG} ", words(k * 50, k * 50 + 50)))
        .collect();
    two_columns(&format!("<div style='{CARD}'><p>{text}</p></div>"))
}

/// G: a clip box with a block size of its own (`height:2000px`): it hides what does not fit (ADR-0009), a different
/// case from the auto-height card (F198g).
fn shape_clip_height() -> String {
    two_columns(&format!(
        "<div style='{CARD}height:2000px;overflow:hidden'>{}</div>",
        words(0, 2500)
    ))
}

/// G: an absolutely positioned child in the card (F198e).
fn shape_abspos() -> String {
    two_columns(&format!(
        "<div style='{CARD}position:relative'>\
         <span style='position:absolute;right:0;top:0;color:red'>STAMP</span>{}</div>",
        words(0, 2500)
    ))
}

/// G: a float in the card (F198f).
fn shape_float() -> String {
    two_columns(&format!(
        "<div style='{CARD}'><div style='float:left;width:60px;height:60px;background:#c00'></div>{}</div>",
        words(0, 2500)
    ))
}

// Chrome 141's numbers for the shapes above (`scripts/reference` probes on the HTML of `dump_chrome_inputs`): the first
// word of every column of every page, and where `AFTER` lands.

const CHROME_CLIP_CARD: Chrome = Chrome {
    first: &[
        &[Some(0), Some(340)],
        &[Some(698), Some(1054)],
        &[Some(1425), Some(1780)],
        &[Some(2138), Some(2322)],
    ],
    after_page: 4,
    after_y: 433.1,
};

const CHROME_IMAGES: Chrome = Chrome {
    first: &[
        &[Some(0), Some(337)],
        &[Some(673), Some(1019)],
        &[Some(1373), Some(1725)],
        &[Some(2077), Some(2089)],
    ],
    after_page: 4,
    after_y: 84.3,
};

const CHROME_IMAGE_PARAGRAPH: Chrome = Chrome {
    first: &[
        &[Some(0), Some(240), Some(480)],
        &[Some(720), Some(880), Some(1040)],
    ],
    after_page: 2,
    after_y: 546.3,
};

const CHROME_IMAGE_PARAGRAPH_IN_CARD: Chrome = Chrome {
    first: &[
        &[Some(0), Some(181), Some(361)],
        &[Some(541), Some(721), Some(901)],
        &[Some(1086), Some(1126), Some(1170)],
    ],
    after_page: 3,
    after_y: 181.1,
};

const CHROME_INLINE_BLOCKS: Chrome = Chrome {
    first: &[
        &[Some(0), Some(314)],
        &[Some(627), Some(947)],
        &[Some(1278), Some(1602)],
    ],
    after_page: 3,
    after_y: 772.1,
};

const CHROME_BADGES: Chrome = Chrome {
    first: &[
        &[Some(0), Some(330)],
        &[Some(659), Some(993)],
        &[Some(1336), Some(1677)],
        &[Some(2018), Some(2264)],
    ],
    after_page: 4,
    after_y: 586.8,
};

const CHROME_SVGS: Chrome = Chrome {
    first: &[
        &[Some(0), Some(343)],
        &[Some(683), Some(1027)],
        &[Some(1389), Some(1749)],
        &[Some(2106), Some(2308)],
    ],
    after_page: 4,
    after_y: 470.6,
};

/// Parley's line box holding an inline box is 17 px tall where the line beside it, and Chrome's, is 16.8 (F198h): a column
/// whose lines hold icons or badges has a line less than Chrome's when the slack at its bottom is under the sum of the
/// 0.2 px, which is nearly every column of 16.8 px lines (59 or 60 of them fill a page to within 1 or 2 px). A column starts
/// up to this many words after Chrome's (a line is 6 to 9 words here), and `AFTER` of the shape with the most icons
/// is 3 lines low.
const BADGE_TOL: Tol = Tol {
    col_words: 2 * LINE_WORDS,
    after_pt: 40.0,
};
const ICON_TOL: Tol = Tol {
    col_words: 5 * LINE_WORDS,
    after_pt: 60.0,
};

/// 13 px (1 px border, 12 px padding) in pt.
const BODY_INSET_PT: f32 = 9.75;

#[test]
fn an_overflow_hidden_card_flows_over_columns_and_pages_like_a_plain_one() {
    // H. Main: 361 of 2500 words (the card is clipped, so it was left as one box and placed whole). An auto-height clip
    // box is fragmented like any block (css-break-3 §2: only a clip box with a block size of its own is monolithic).
    let pdf = render(&shape_overflow());
    check_shape("overflow:hidden card", &pdf, 2, &CHROME_A, CARD_INSET_PT);
    assert_every_word_is_inked("overflow:hidden card", &pdf);
}

#[test]
fn a_clip_card_clips_its_header_strip_to_the_rounded_corners_of_its_first_piece() {
    // H2. `overflow-hidden rounded-lg border` with a red header strip, in the first column of page 1 only. The strip's
    // colour must not show outside the radius (24 px) at the top corners of that column, the card's side borders are in
    // both columns of every page, and the text of both columns is above the frames.
    let pdf = render(&shape_clip_card());
    // The strip is 8 + 8 px of padding and a 16.8 px line below the card's top border; the text of the body starts
    // under it, 12 px of padding below.
    check_shape_with(
        "clip card",
        &pdf,
        2,
        &CHROME_CLIP_CARD,
        BODY_INSET_PT,
        2500,
        Some(34.6),
        ONE_LINE,
    );
    assert_every_word_is_inked("clip card", &pdf);
    let col_w = (PAGE_W_PT - 2.0 * MARGIN_PT - GAP_PT) / 2.0;
    let col2 = MARGIN_PT + col_w + GAP_PT;
    let (w, _, px) = rgb_page(&pdf, 1, 144);
    let at = |x_pt: f32, y_pt: f32| {
        let i = 3 * ((y_pt * SCALE) as usize * w + (x_pt * SCALE) as usize);
        (px[i], px[i + 1], px[i + 2])
    };
    let red = |p: (u8, u8, u8)| is_red(p.0, p.1, p.2);
    // Inside the strip: red.
    assert!(
        red(at(MARGIN_PT + 60.0, MARGIN_PT + 6.0)),
        "the strip is red at the top of column 1: {:?}",
        at(MARGIN_PT + 60.0, MARGIN_PT + 6.0)
    );
    // Outside the radius at the top corners of column 1: not red (the clip rounds the strip).
    for (what, x) in [
        ("left", MARGIN_PT + 1.0),
        ("right", MARGIN_PT + col_w - 1.0),
    ] {
        let p = at(x, MARGIN_PT + 1.0);
        assert!(
            !red(p),
            "the strip shows outside the radius at the {what} top corner of column 1: {p:?}"
        );
    }
    // No strip at the top of column 2 (the card goes on there).
    assert!(
        !red(at(col2 + 60.0, MARGIN_PT + 6.0)),
        "column 2 has no header strip"
    );
    // The side borders (#888) are drawn in both columns of the first page, to its bottom.
    for (what, x) in [
        ("column 1 left", MARGIN_PT),
        ("column 1 right", MARGIN_PT + col_w - 0.75),
        ("column 2 left", col2),
        ("column 2 right", col2 + col_w - 0.75),
    ] {
        let dark = (0..4).any(|d| {
            let p = at(x + d as f32 * 0.5, 700.0);
            (p.0 as u32 + p.1 as u32 + p.2 as u32) / 3 < 200
        });
        assert!(
            dark,
            "the {what} border is not drawn beside y 700 on page 1"
        );
    }
}

#[test]
fn inline_images_in_a_tall_card_are_printed_once_each_beside_their_words() {
    // I. 14 images of 30 x 18 px in the text of a card of 2100 words. Main: 360 of 2100 words and one image on page 1.
    let pdf = render(&shape_inline_images());
    check_shape_with(
        "inline images",
        &pdf,
        2,
        &CHROME_IMAGES,
        CARD_INSET_PT,
        2100,
        None,
        ONE_LINE,
    );
    assert_every_word_is_inked("inline images", &pdf);
    assert_eq!(
        images_drawn(&pdf),
        14,
        "every image is drawn once, none twice"
    );
    let before: Vec<usize> = (0..14).map(|k| k * 150 + 149).collect();
    assert_boxes_beside_their_words(
        "inline images",
        &printed_words(&pdf),
        &blobs_in_pt(&pdf, is_red, 0),
        &before,
        -2.3,
    );
}

#[test]
fn a_paragraph_with_an_inline_image_is_cut_over_columns_like_any_paragraph() {
    // I2. A `<p>` of 1200 words with an image in the middle, direct child of three columns (main: 248 words), and the
    // same `<p>` in a card (main: 185).
    let pdf = render(&shape_image_paragraph());
    check_shape_with(
        "image paragraph",
        &pdf,
        3,
        &CHROME_IMAGE_PARAGRAPH,
        0.0,
        1200,
        Some(0.0),
        ONE_LINE,
    );
    assert_every_word_is_inked("image paragraph", &pdf);
    assert_eq!(images_drawn(&pdf), 1, "the image is drawn once");
    assert_boxes_beside_their_words(
        "image paragraph",
        &printed_words(&pdf),
        &blobs_in_pt(&pdf, is_red, 0),
        &[599],
        -2.5,
    );

    let pdf = render(&shape_image_paragraph_in_card());
    check_shape_with(
        "image paragraph in a card",
        &pdf,
        3,
        &CHROME_IMAGE_PARAGRAPH_IN_CARD,
        CARD_INSET_PT,
        1200,
        Some(CARD_INSET_PT),
        ONE_LINE,
    );
    assert_every_word_is_inked("image paragraph in a card", &pdf);
    assert_eq!(images_drawn(&pdf), 1, "the image is drawn once");
    assert_boxes_beside_their_words(
        "image paragraph in a card",
        &printed_words(&pdf),
        &blobs_in_pt(&pdf, is_red, 0),
        &[599],
        -2.5,
    );
}

#[test]
fn inline_blocks_in_a_tall_card_keep_their_border_and_their_text_together() {
    // J. 60 inline-blocks of two words with a red border, in a card of 1920 words. Main: 337 words. Each border is
    // painted once (a connected ring), and the two words of a box are inside it.
    let pdf = render(&shape_inline_blocks());
    check_shape_with(
        "inline blocks",
        &pdf,
        2,
        &CHROME_INLINE_BLOCKS,
        CARD_INSET_PT,
        1920,
        None,
        ONE_LINE,
    );
    assert_every_word_is_inked("inline blocks", &pdf);
    let found = printed_words(&pdf);
    // The glyphs of the next line cross the bottom edge of a box (Parley counts the box above the baseline, so the line is 3 px
    // short of it; an unbroken paragraph does the same, F198h): the ring is in pieces up to a glyph apart.
    let rings = blobs_in_pt(&pdf, is_red, 12);
    assert_eq!(rings.len(), 60, "each inline-block border is drawn once");
    for k in 0..60 {
        let (a, b) = (word_at(&found, k * 32 + 30), word_at(&found, k * 32 + 31));
        let ring = rings
            .iter()
            .find(|(page, r)| {
                *page == a.page
                    && r[0] <= a.x_min
                    && a.x_min < r[2]
                    && r[1] <= a.y_min + 1.0
                    && a.y_max - 1.0 <= r[3]
            })
            .unwrap_or_else(|| panic!("w{:04} is not inside a border", k * 32 + 30));
        assert!(
            b.page == a.page
                && ring.1[0] <= b.x_min
                && b.x_min < ring.1[2]
                && (a.y_min - b.y_min).abs() < 1.0,
            "w{:04} and w{:04} are not inside the same border: {a:?} {b:?} {ring:?}",
            k * 32 + 30,
            k * 32 + 31
        );
    }
}

#[test]
fn inline_flex_badges_in_a_tall_card_are_drawn_once_with_their_text() {
    // J2. 50 `inline-flex` badges `b000` … with a green border in a card of 2500 words.
    let pdf = render(&shape_inline_flex_badges());
    check_shape_with(
        "badges",
        &pdf,
        2,
        &CHROME_BADGES,
        CARD_INSET_PT,
        2500,
        None,
        BADGE_TOL,
    );
    let found = printed_words(&pdf);
    for k in 0..50 {
        let name = format!("b{k:03}");
        assert_eq!(
            found.iter().filter(|w| w.text == name).count(),
            1,
            "{name} is printed once"
        );
    }
    let green =
        |r: u8, g: u8, b: u8| g > 100 && g as i32 - r as i32 > 50 && g as i32 - b as i32 > 50;
    assert_eq!(
        blobs_in_pt(&pdf, green, 12).len(),
        50,
        "each badge border is drawn once"
    );
}

#[test]
fn inline_svgs_in_a_tall_card_are_drawn_once_each_beside_their_words() {
    // K. 50 inline `<svg>` icons in the text of a card, and in one `<p>` of it. Main: 358 of 2500 words.
    for (what, body) in [
        ("svg in the card's text", shape_inline_svgs()),
        ("svg in a paragraph", shape_inline_svg_paragraph()),
    ] {
        let pdf = render(&body);
        check_shape_with(
            what,
            &pdf,
            2,
            &CHROME_SVGS,
            CARD_INSET_PT,
            2500,
            Some(CARD_INSET_PT),
            ICON_TOL,
        );
        assert_every_word_is_inked(what, &pdf);
        let before: Vec<usize> = (0..50).map(|k| k * 50 + 49).collect();
        assert_boxes_beside_their_words(
            what,
            &printed_words(&pdf),
            &blobs_in_pt(&pdf, is_red, 0),
            &before,
            -2.5,
        );
    }
}

// ── G. guards: shapes that keep the old path (F198e, F198f, F198g) print what main printed ─────────────────────

fn words_printed(pdf: &[u8]) -> usize {
    printed_words(pdf)
        .iter()
        .filter(|w| w.index().is_some())
        .count()
}

#[test]
fn guard_a_clip_box_with_a_block_size_of_its_own_keeps_the_old_path() {
    // `height:2000px; overflow:hidden`: the box hides what does not fit (Chrome prints 696 words on 2 pages: ADR-0009);
    // a constrained clip box is F198g, not this change. Main: 2 pages, 361 words.
    let pdf = render(&shape_clip_height());
    assert_eq!(words_printed(&pdf), 361);
    assert_eq!(page_count(&pdf), 2);
}

#[test]
fn guard_absolutely_positioned_and_floating_children_keep_the_old_path() {
    // F198e / F198f: main prints 361 and 344 words of 2500 (warned). They move when those findings are fixed.
    let pdf = render(&shape_abspos());
    assert_eq!(words_printed(&pdf), 361, "abspos child");
    let pdf = render(&shape_float());
    assert_eq!(words_printed(&pdf), 344, "float child");
}
