//! A table cell made of image lines and text, cut by a page break, keeps every word and every image (F183, plan 7.8).
//!
//! An inline root (a cell, a block whose content is lines) holds its `<img>` as inline boxes: the line puts them where
//! it wants, so the block walk of the fragmenter must never cut it between two boxes. It did, for every row of a table
//! except the first on its page (`nested_line_split` only looks at the first thing in a table): the cell was cut
//! between its images in fragments as tall as the images, without line ranges, and the renderer, measuring the lines
//! from those heights, dropped the 64 px line (60 px image plus the strut) that did not fit its 60 px fragment, with the
//! text under it; or an image was drawn at the bottom of page 1 and the rest of the cell was gone. Now an inline root that
//! the gate would have entered is cut between its line boxes (css-break-3 section 4.4, orphans and widows of 2), the same
//! way a cell that is first in its table already was.
//!
//! The documents are sweep cases of the vrt-validator's fourth pass (`out/scratch/f183v6`: `min2_tf0_0`,
//! `b_a4_two_673`, `b_land_thead_357`, `b_land_thead_619`, `b_a6_thead_475`, `g_hf2_none_sep_0`); the numbers are
//! Chrome's (headless Chromium, same HTML, `pdftotext` and `pdfimages` on its PDF). The words are numbered in document
//! order (`w001`..), so a lost word shows as a gap and a repeated one as a count of 2.
//! Plan: docs/plans/2026-10-27-f183-inline-img-strut.md section 7.8.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const RED60: &[u8] = include_bytes!("data/red60.png");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn render(page: &str, css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("red60.png", RED60.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>@page {{ size: {page}; margin: 0 }} body {{ margin: 0; font: 16px Inter }} {css}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-rowsplit-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

/// The text of `page` (1-based).
fn text_on(pdf: &[u8], page: usize) -> String {
    let path = temp(pdf);
    let out = Command::new("pdftotext")
        .args(["-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The words `w001`.. on `page` as numbers.
fn words_on(pdf: &[u8], page: usize) -> Vec<u32> {
    let mut w: Vec<u32> = text_on(pdf, page)
        .split_whitespace()
        .filter_map(|w| w.strip_prefix('w')?.parse().ok())
        .collect();
    w.sort_unstable();
    w
}

fn images_on(pdf: &[u8], page: usize) -> usize {
    let path = temp(pdf);
    let out = Command::new("pdfimages")
        .args(["-list", "-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout).lines().skip(2).count()
}

fn pages(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:")?.trim().parse().ok())
        .expect("a page count")
}

/// How many times each word `1..=last` occurs in the whole document, as the words that do not occur exactly once.
fn not_once(pdf: &[u8], last: u32) -> BTreeMap<u32, usize> {
    let mut count: BTreeMap<u32, usize> = (1..=last).map(|w| (w, 0)).collect();
    for page in 1..=pages(pdf) {
        for w in words_on(pdf, page) {
            if let Some(c) = count.get_mut(&w) {
                *c += 1;
            }
        }
    }
    count.retain(|_, c| *c != 1);
    count
}

fn range(from: u32, to: u32) -> Vec<u32> {
    (from..=to).collect()
}

fn words(from: u32, n: u32) -> String {
    (from..from + n)
        .map(|i| format!("w{i:03}"))
        .collect::<Vec<_>>()
        .join(" ")
}

const IMG: &str = "<img src=\"red60.png\" width=\"60\" height=\"60\">";

fn filler(h: u32) -> String {
    format!("<div style=\"height: {h}px; background: #ccc\"></div>")
}

/// `img<br>img<br>28 words<br>img` starting at word `from`; the cell of the `b_*` sweeps.
fn cell_iiti(from: u32) -> String {
    format!("{IMG}<br>{IMG}<br>{}<br>{IMG}", words(from, 28))
}

#[test]
fn s1_ten_collapsed_rows_of_two_images_and_twenty_words_lose_nothing() {
    // `min2_tf0_0`: ten rows `img<br>img<br>20 words` and two short cells, from the top of page 1. Five rows fit; the
    // sixth starts 23 px above the bottom: Chrome moves it whole (10 images and w001..w130 on page 1). Before the fix
    // the sixth row's first image was drawn at the bottom of page 1 (11 | 8 images) and 20 words were gone.
    let mut rows = String::new();
    let mut n = 1;
    for _ in 0..10 {
        rows += &format!(
            "<tr><td>{IMG}<br>{IMG}<br>{}</td><td>{}</td><td>{}</td></tr>",
            words(n, 20),
            words(n + 20, 3),
            words(n + 23, 3)
        );
        n += 26;
    }
    let pdf = render(
        "A4",
        "td { padding: 6px; border: 1px solid #000; vertical-align: top } table { border-collapse: collapse; table-layout: fixed }",
        &format!("{}<table style=\"width: 100%\"><tbody>{rows}</tbody></table>", filler(0)),
    );
    assert_eq!(not_once(&pdf, 260), BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 2, "Chrome: 2 pages");
    assert_eq!(words_on(&pdf, 1), range(1, 130));
    assert_eq!(words_on(&pdf, 2), range(131, 260));
    assert_eq!((images_on(&pdf, 1), images_on(&pdf, 2)), (10, 10));
}

#[test]
fn s2_two_cells_per_row_are_cut_between_lines_as_in_chrome() {
    // `b_a4_two_673`: four rows of two cells `img<br>img<br>28 words<br>img`, the table 673 px down a collapsed
    // `border: 1px` table. The main branch lost nothing here but cut the third row differently; the branch before the fix
    // lost 24 words and 2 images. Chrome: page 1 w001..w064 and w085..w092 (the first two rows and the lines of the third
    // that fit) with 10 images, page 2 the rest with 14.
    let mut rows = String::new();
    for r in 0..4 {
        rows += &format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            cell_iiti(1 + r * 56),
            cell_iiti(29 + r * 56)
        );
    }
    let pdf = render(
        "A4",
        "td, th { padding: 5px; border: 1px solid #000; vertical-align: top }",
        &format!(
            "{}<table style=\"width: 100%; border-collapse: collapse; border: 1px solid #000\"><tbody>{rows}</tbody></table>",
            filler(673)
        ),
    );
    assert_eq!(not_once(&pdf, 224), BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 2);
    let mut p1 = range(1, 64);
    p1.extend(85..=92);
    let mut p2 = range(65, 84);
    p2.extend(93..=224);
    assert_eq!(words_on(&pdf, 1), p1);
    assert_eq!(words_on(&pdf, 2), p2);
    assert_eq!((images_on(&pdf, 1), images_on(&pdf, 2)), (10, 14));
}

/// The `b_*_thead_*` documents: a header row and three rows of `img<br>img<br>28 words<br>img` in a collapsed table
/// `filler` px down.
fn thead_table(page: &str, filler_px: u32) -> Vec<u8> {
    let rows: String = (0..3)
        .map(|r| format!("<tr><td>{}</td></tr>", cell_iiti(1 + r * 28)))
        .collect();
    render(
        page,
        "td, th { padding: 5px; border: 1px solid #000; vertical-align: top }",
        &format!(
            "{}<table style=\"width: 100%; border-collapse: collapse; border: 1px solid #000\"><thead><tr><th>HEAD</th></tr></thead><tbody>{rows}</tbody></table>",
            filler(filler_px)
        ),
    )
}

fn heads_on(pdf: &[u8], page: usize) -> usize {
    text_on(pdf, page).matches("HEAD").count()
}

#[test]
fn s3_landscape_header_table_keeps_every_word_and_matches_chrome() {
    // `b_land_thead_357`: the second row is cut between its lines, Chrome keeps w001..w051 and 5 images on page 1 and
    // the rest (w052..w084, 4 images) on page 2, the header on both. The branch before the fix lost 5 words.
    let pdf = thead_table("A4 landscape", 357);
    assert_eq!(not_once(&pdf, 84), BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 2);
    assert_eq!(words_on(&pdf, 1), range(1, 51));
    assert_eq!(words_on(&pdf, 2), range(52, 84));
    assert_eq!((images_on(&pdf, 1), images_on(&pdf, 2)), (5, 4));
    assert_eq!((heads_on(&pdf, 1), heads_on(&pdf, 2)), (1, 1));

    // `b_land_thead_619`: the first row is cut after its two image lines, its 28 words and the last image are on
    // page 2 with the other rows (Chrome: no word on page 1, images 2 | 7, header on both). The branch before the fix
    // lost 24 of the words of page 2.
    let pdf = thead_table("A4 landscape", 619);
    assert_eq!(not_once(&pdf, 84), BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 2);
    assert_eq!(words_on(&pdf, 1), Vec::<u32>::new());
    assert_eq!(words_on(&pdf, 2), range(1, 84));
    assert_eq!((images_on(&pdf, 1), images_on(&pdf, 2)), (2, 7));
    assert_eq!((heads_on(&pdf, 1), heads_on(&pdf, 2)), (1, 1));
}

#[test]
fn s4_a6_header_table_keeps_every_word_and_image() {
    // `b_a6_thead_475`: words as in Chrome (w001..w056 | w057..w084). Chrome moves the whole third row (6 | 3 images);
    // we cut it after its two image lines (8 | 1), as the main branch did (which lost 12 words here). Complete and
    // the words per page are Chrome's; the image split is not (noted in plan 7.8).
    let pdf = thead_table("A6", 475);
    assert_eq!(not_once(&pdf, 84), BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 2);
    assert_eq!(words_on(&pdf, 1), range(1, 56));
    assert_eq!(words_on(&pdf, 2), range(57, 84));
    assert_eq!(images_on(&pdf, 1) + images_on(&pdf, 2), 9);
    assert_eq!((heads_on(&pdf, 1), heads_on(&pdf, 2)), (1, 1));
}

#[test]
fn s5_separated_table_with_header_and_footer_keeps_every_word_and_image() {
    // `g_hf2_none_sep_0`: ten rows `img<br>img<br>20 words` and two short cells in a separated, fixed table with a header
    // and a two-row footer holding an image cell. Chrome has 3 pages and so does the main branch; the branch before the
    // fix had 2 pages and lost 12 words and 2 images. (The footer is not repeated as in Chrome, as on main: not this
    // change.)
    let mut rows = String::new();
    let mut n = 1;
    for _ in 0..10 {
        rows += &format!(
            "<tr><td>{IMG}<br>{IMG}<br>{}</td><td>{}</td><td>{}</td></tr>",
            words(n, 20),
            words(n + 20, 3),
            words(n + 23, 3)
        );
        n += 26;
    }
    let pdf = render(
        "A4",
        "td, th { padding: 6px; border: 1px solid #000; vertical-align: top } table { border-collapse: separate; border-spacing: 0; table-layout: fixed } thead { display: table-header-group } tfoot { display: table-footer-group }",
        &format!(
            "{}<table style=\"width: 100%\"><thead><tr><th>HEAD</th><th>HEAD</th><th>HEAD</th></tr></thead><tfoot><tr><td>FOOT</td><td>w261</td><td>FOOT</td></tr><tr><td>{IMG}<br>{IMG}<br>FOOT</td><td>w262</td><td>FOOT</td></tr></tfoot><tbody>{rows}</tbody></table>",
            filler(0)
        ),
    );
    let missing = not_once(&pdf, 260);
    assert_eq!(missing, BTreeMap::new(), "words lost or repeated");
    assert_eq!(pages(&pdf), 3, "Chrome: 3 pages");
    let images: usize = (1..=3).map(|p| images_on(&pdf, p)).sum();
    assert!(images >= 22, "20 body images and the 2 of the footer: {images}");
}
