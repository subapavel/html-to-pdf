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

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><meta charset=utf-8><style>\
                 body{{margin:0;font:14px/1.2 Inter}}p{{margin:0}}\
                 </style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
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

/// The `wNNNN` words in reading order: page, then column, then top to bottom, left to right.
fn column_major(found: &[Word], n: usize) -> Vec<Word> {
    let mut v: Vec<Word> = found
        .iter()
        .filter(|w| w.index().is_some())
        .cloned()
        .collect();
    v.sort_by(|a, b| {
        (a.page, column(a, n), (a.y_min * 10.0).round() as i64)
            .cmp(&(b.page, column(b, n), (b.y_min * 10.0).round() as i64))
            .then(a.x_min.total_cmp(&b.x_min))
    });
    v
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
    let found = printed_words(pdf);
    let ordered = assert_every_word_once(what, &found, n_cols, 2500);
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
                    want.abs_diff(got) <= LINE_WORDS,
                    "{what}: page {} column {} starts with w{got:04}, Chrome w{want:04}",
                    p + 1,
                    c + 1
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
        (after[0].y_min - chrome.after_y).abs() <= 20.0,
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
        for c in 0..n_cols {
            let top = ordered
                .iter()
                .filter(|w| w.page == p && column(w, n_cols) == c)
                .map(|w| w.y_min)
                .reduce(f32::min);
            let Some(top) = top else { continue };
            let want = if p == 1 && c == 0 {
                MARGIN_PT + inset_pt - 0.3 // below the card's top border and padding
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
