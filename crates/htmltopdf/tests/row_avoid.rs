//! `break-inside: avoid` on a table cell or on its row keeps a row that fits a page on one page (F83d, table part).
//!
//! css-break-3 section 3.3 / 4.4: a break between rows is a class A break point and is preferred to one inside a row;
//! `avoid` is dropped only when the row cannot fit a fragmentainer. `tr` has no box of its own in the fragmenter (Blitz
//! gives `tbody` and `tr` no size), so the rule is read from the row's cells and from the `tr` that holds them. Before
//! the fix the cell was split between its lines whenever it was the first thing in its table: every cell of a table
//! without a border, and, since F183 (plan section 7.6), every cell of a bordered or collapsed table too. Chrome moves the
//! row to the next page, and so does the fix, for separated and collapsed borders alike.
//!
//! Every case renders an A4 page with `@page { margin: 0 }`, `body { margin: 0; font: 16px Inter }` after a grey filler
//! `div` of the given height, so the table starts in the last 120 px of page 1 and its row (a 60 px image and 50 words, or
//! more) does not fit the rest. The numbers (words and ink per page) are Chrome's (headless Chromium via Playwright, same
//! HTML), measured the same way. Plan: docs/plans/2026-10-27-f183-inline-img-strut.md section 7.6.

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

fn render(css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_file(root().join("fixtures/fonts/Inter-Regular.ttf"))
        .unwrap()
        .image("red60.png", RED60.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>@page {{ size: A4; margin: 0 }} body {{ margin: 0; font: 16px Inter }} {css}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("the document renders")
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-rowavoid-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

/// The words `w001`.. on `page` (1-based) as numbers; empty for a page that does not exist.
fn words_on(pdf: &[u8], page: usize) -> Vec<u32> {
    let path = temp(pdf);
    let out = Command::new("pdftotext")
        .args(["-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|w| w.strip_prefix('w')?.parse().ok())
        .collect()
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

/// Rows `[first, last + 1)` of black ink (the cell borders) on `page` at 96 dpi, or `None` when there is none.
fn black_rows(pdf: &[u8], page: usize) -> Option<(usize, usize)> {
    let path = temp(pdf);
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while data[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while !data[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..at]).into_owned());
    }
    at += 1;
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let rgb = &data[at..at + w * h * 3];
    let black = |y: usize| (0..w).any(|x| rgb[(y * w + x) * 3..][..3].iter().all(|&c| c < 60));
    let ys: Vec<usize> = (0..h).filter(|&y| black(y)).collect();
    Some((*ys.first()?, ys.last()? + 1))
}

fn words(from: u32, n: u32) -> String {
    (from..from + n)
        .map(|i| format!("w{i:03}"))
        .collect::<Vec<_>>()
        .join(" ")
}

const IMG60: &str = "<img src=\"red60.png\" width=\"60\" height=\"60\">";
const IMG20: &str = "<img src=\"red60.png\" width=\"20\" height=\"20\">";
const COLLAPSED: &str = "table { border-collapse: collapse } td { padding: 5px; border: 1px solid #000; vertical-align: top }";
const SEPARATED: &str = "table { border-collapse: separate; border-spacing: 0 } td { padding: 5px; border: 1px solid #000; vertical-align: top }";

fn spacer(px: usize) -> String {
    format!("<div style=\"height: {px}px; background: #ccc\"></div>")
}

fn one_row(table_css: &str, rule: &str, filler: usize) -> Vec<u8> {
    render(
        &format!("{table_css} {rule}"),
        &format!(
            "{}<table><tr><td>{IMG60} {}</td></tr></table>",
            spacer(filler),
            words(1, 50)
        ),
    )
}

#[test]
fn a1_avoid_on_a_row_or_a_cell_moves_the_row_to_page_2() {
    // Chrome: nothing of the table on page 1, all 50 words on page 2, the first border row at 0 (collapsed or separated
    // borders, `tr` or `td`, `break-inside` or the legacy `page-break-inside`; filler 1000: the first lines would fit
    // and the cell used to be split 32 | 18 words; filler 1040: the 60 px image would not).
    for table in [COLLAPSED, SEPARATED] {
        for rule in [
            "tr { break-inside: avoid }",
            "td { break-inside: avoid }",
            "tr { page-break-inside: avoid }",
            "td { page-break-inside: avoid }",
        ] {
            for filler in [1000, 1040] {
                let pdf = one_row(table, rule, filler);
                let label = format!("{rule} / {} / {filler}", &table[..30]);
                assert_eq!(pages(&pdf), 2, "{label}");
                assert_eq!(words_on(&pdf, 1), Vec::<u32>::new(), "{label}: page 1");
                assert_eq!(
                    words_on(&pdf, 2),
                    (1..=50).collect::<Vec<_>>(),
                    "{label}: page 2"
                );
                assert_eq!(black_rows(&pdf, 1), None, "{label}: no border on page 1");
                let (top, _) = black_rows(&pdf, 2).expect("the cell border on page 2");
                assert!(top <= 1, "{label}: the row starts page 2, border at {top}");
            }
        }
    }
}

#[test]
fn a2_without_avoid_the_row_is_still_split_between_its_lines() {
    // Guard: `break-inside: auto` (the default) splits the cell as Chrome does, 32 words and the image on page 1, 18 on
    // page 2 (filler 1000, collapsed borders).
    let pdf = one_row(COLLAPSED, "", 1000);
    assert_eq!(words_on(&pdf, 1), (1..=32).collect::<Vec<_>>());
    assert_eq!(words_on(&pdf, 2), (33..=50).collect::<Vec<_>>());
    let pdf = one_row(COLLAPSED, "td { break-inside: auto }", 1000);
    assert_eq!(words_on(&pdf, 1), (1..=32).collect::<Vec<_>>());
}

#[test]
fn a3_six_avoid_rows_move_from_the_first_whole() {
    // `tr { break-inside: avoid }`, six rows of 50 words and a 20 px image after 1042 px: Chrome puts nothing on page
    // 1 and all 300 words on page 2 (collapsed and separated borders). The first row used to be split 33 | 17.
    for table in [COLLAPSED, SEPARATED] {
        reset_words();
        let rows: String = (0..6)
            .map(|_| {
                format!(
                    "<tr><td>{} {IMG20} {}</td></tr>",
                    next_words(30),
                    next_words(20)
                )
            })
            .collect();
        let pdf = render(
            &format!("{table} tr {{ break-inside: avoid }}"),
            &format!("{}<table>{rows}</table>", spacer(1042)),
        );
        assert_eq!(words_on(&pdf, 1), Vec::<u32>::new(), "{}", &table[..30]);
        assert_eq!(
            words_on(&pdf, 2),
            (1..=300).collect::<Vec<_>>(),
            "{}",
            &table[..30]
        );
        assert_eq!(black_rows(&pdf, 1), None);
    }
}

#[test]
fn a4_cells_level_with_the_avoid_cell_follow_it() {
    // A row of two cells, `tr` avoid or only the second cell: both cells go to page 2 (Chrome: 60 words on page 2, the
    // row starts at 0).
    for rule in [
        "tr { break-inside: avoid }",
        "td + td { break-inside: avoid }",
    ] {
        let pdf = render(
            &format!("{COLLAPSED} {rule}"),
            &format!(
                "{}<table><tr><td>{IMG60} {}</td><td>{IMG20} {}</td></tr></table>",
                spacer(1000),
                words(1, 50),
                words(51, 10)
            ),
        );
        assert_eq!(words_on(&pdf, 1), Vec::<u32>::new(), "{rule}");
        // pdftotext reads the shorter cell first, so compare as a set.
        let mut on_2 = words_on(&pdf, 2);
        on_2.sort_unstable();
        assert_eq!(on_2, (1..=60).collect::<Vec<_>>(), "{rule}");
        assert_eq!(black_rows(&pdf, 1), None, "{rule}");
        assert!(black_rows(&pdf, 2).unwrap().0 <= 1, "{rule}");
    }
}

#[test]
fn a5_only_the_row_that_straddles_moves() {
    // Three rows (a 20 px image and 30 words each) after 960 px: the first two stay on page 1 (60 words), the third
    // moves to page 2 (Chrome 60 | 30).
    reset_words();
    let rows: String = (0..3)
        .map(|_| format!("<tr><td>{IMG20} {}</td></tr>", next_words(30)))
        .collect();
    let pdf = render(
        &format!("{COLLAPSED} tr {{ break-inside: avoid }}"),
        &format!("{}<table>{rows}</table>", spacer(960)),
    );
    assert_eq!(words_on(&pdf, 1), (1..=60).collect::<Vec<_>>());
    assert_eq!(words_on(&pdf, 2), (61..=90).collect::<Vec<_>>());
    assert!(black_rows(&pdf, 2).unwrap().0 <= 1);
}

#[test]
fn a6_a_row_taller_than_a_page_is_still_split_and_loses_nothing() {
    // `avoid` cannot be honoured for a row that does not fit a page (css-break-3 section 4.4). The cell (a 60 px image and
    // 1300 words, about 1.4 pages) is split between its lines, nothing is lost. Chrome starts the row on page 2 (F83b's
    // rule for a box taller than a page, not done for rows); ours starts it where the table is, as before this change.
    for rule in ["tr { break-inside: avoid }", "td { break-inside: avoid }"] {
        let pdf = render(
            &format!("{SEPARATED} {rule}"),
            &format!(
                "{}<table><tr><td>{IMG60} {}</td></tr></table>",
                spacer(100),
                words(1, 1300)
            ),
        );
        let all: Vec<u32> = (1..=pages(&pdf)).flat_map(|p| words_on(&pdf, p)).collect();
        assert_eq!(all, (1..=1300).collect::<Vec<_>>(), "{rule}");
        assert!(pages(&pdf) >= 2, "{rule}");
    }
}

#[test]
fn a7_avoid_row_under_a_repeated_header_is_unchanged() {
    // Guard: a table with a `thead` and avoid rows after 960 px: the header and the first row stay (40 words), the second
    // row moves under the repeated header (80 words on page 2). Chrome 40 | 80.
    reset_words();
    let rows: String = (0..3)
        .map(|_| format!("<tr><td>{IMG20} {}</td></tr>", next_words(40)))
        .collect();
    let pdf = render(
        &format!("{COLLAPSED} tr {{ break-inside: avoid }}"),
        &format!(
            "{}<table><thead><tr><td>Head</td></tr></thead><tbody>{rows}</tbody></table>",
            spacer(960)
        ),
    );
    assert_eq!(words_on(&pdf, 1), (1..=40).collect::<Vec<_>>());
    assert_eq!(words_on(&pdf, 2), (41..=120).collect::<Vec<_>>());
}

thread_local! {
    static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

fn reset_words() {
    NEXT.with(|n| n.set(1));
}

/// The next `n` words of a running count, as the sweeps number them.
fn next_words(n: u32) -> String {
    let from = NEXT.with(|c| c.replace(c.get() + n));
    words(from, n)
}
