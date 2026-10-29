//! A body-level paragraph with a border or padding that spans pages is sliced at its breaks (F30f-d).
//!
//! `box-decoration-break: slice`, the initial value (css-break-3 §5.4): the box is laid out as if there were no break and
//! then cut at the page edge, so the lines of a continuation page start at the content box top, there is no top border or
//! padding there, and a page that is not the last has no bottom border (its side borders run to the content box bottom).
//! Before the fix the line path fitted the lines as if they started at the top of the border box while the renderer
//! painted them `inset_top` lower: the last line of every page but the last stood in the bottom margin (or under the
//! clip of an `overflow` box, where it was cut off the picture and stayed in the text layer), the frame closed
//! `inset_top + inset_bottom` too early on the last page, and what followed the box started over its last line.
//!
//! Every number is Chrome's (m141, A4, 15 mm, Inter, 14px/1.2, 1500 words `w0000 …`, `padding:16px; border:1px solid;
//! border-radius:8px`, `AFTER` below), measured with `out/scratch/f30fd/{gen1,meas,tab}.py` (plan §7). The frame is
//! checked on a 96 dpi raster (1 CSS px = 1 pixel there, so no unit conversion is needed in the test).

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/1.2 Inter;margin:0}p{margin:0}";
/// The card of the repro (c286), with a black border so that the raster shows it.
const CARD: &str = "padding:16px;border:1px solid #000;border-radius:8px";
/// The content box of A4 with 15 mm margins, in pt from the page top: its top and bottom.
const CONTENT_TOP: f32 = 42.52;
const CONTENT_BOTTOM: f32 = 799.37;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn names(from: usize, to: usize) -> Vec<String> {
    (from..to).map(|i| format!("w{i:04}")).collect()
}

fn words(n: usize) -> String {
    names(0, n).join(" ")
}

struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-pps-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf);
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

fn page_count(words: &[Word]) -> usize {
    words.iter().map(|w| w.page).max().unwrap_or(0)
}

/// How many `wNNNN` words are printed on each page.
fn per_page(words: &[Word]) -> Vec<usize> {
    let mut per = vec![0; page_count(words)];
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        per[w.page - 1] += 1;
    }
    per
}

/// How many lines (distinct `y_min`, rounded to 0.1 pt) of `wNNNN` words are printed on each page.
fn lines_per_page(words: &[Word]) -> Vec<usize> {
    (1..=page_count(words))
        .map(|p| {
            let mut ys: Vec<i32> = words
                .iter()
                .filter(|w| w.page == p && w.text.starts_with('w'))
                .map(|w| (w.y_min * 10.0).round() as i32)
                .collect();
            ys.sort_unstable();
            ys.dedup();
            ys.len()
        })
        .collect()
}

fn after(words: &[Word]) -> (usize, f32) {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    (a[0].page, a[0].y_min)
}

/// A page of the PDF as 96 dpi grey pixels (1 pixel = 1 CSS px): `(width, height, rows of bytes)`.
fn raster(pdf: &[u8], page: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let dir = std::env::temp_dir().join(format!(
        "htmltopdf-pps-r-{}-{}",
        std::process::id(),
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let status = std::process::Command::new("pdftoppm")
        .args([
            "-r",
            "96",
            "-gray",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
        ])
        .arg(&path)
        .arg(dir.join("p"))
        .status()
        .expect("pdftoppm (poppler-utils) is required for this test");
    assert!(status.success(), "pdftoppm failed");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .expect("pdftoppm wrote no page")
        .unwrap()
        .path();
    let bytes = std::fs::read(&file).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);
    // P5\n<w> <h>\n255\n<data>
    let mut fields = Vec::new();
    let mut pos = 0;
    while fields.len() < 4 {
        while bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        while !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(String::from_utf8_lossy(&bytes[start..pos]).into_owned());
    }
    pos += 1;
    assert_eq!(fields[0], "P5");
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    (w, h, bytes[pos..].to_vec())
}

/// What the frame of the card looks like on one page, in pixel rows.
struct Frame {
    /// The last row (inclusive) of the left border's longest run, `None` when the page has no side border.
    side_bottom: Option<usize>,
    /// The first row of that run.
    side_top: Option<usize>,
    /// Rows where a horizontal rule (the top or the bottom border) crosses the card.
    rules: Vec<usize>,
    /// The last row with text ink (dark pixels inside the card, away from the border).
    last_ink: Option<usize>,
}

fn frame(pdf: &[u8], page: usize) -> Frame {
    let (w, h, px) = raster(pdf, page);
    let at = |x: usize, y: usize| px[y * w + x];
    // The left border: 15 mm = 56.7 px, 1 px wide, so pixels 56 and 57.
    let side: Vec<bool> = (0..h).map(|y| at(56, y).min(at(57, y)) < 200).collect();
    let mut best: Option<(usize, usize)> = None;
    let mut y = 0;
    while y < h {
        if side[y] {
            let s = y;
            while y < h && side[y] {
                y += 1;
            }
            if best.is_none_or(|(a, b)| y - 1 - s > b - a) {
                best = Some((s, y - 1));
            }
        } else {
            y += 1;
        }
    }
    let rules: Vec<usize> = (0..h)
        .filter(|&y| {
            let dark = (150..640).filter(|&x| at(x, y) < 235).count();
            dark * 10 > 490 * 9
        })
        .collect();
    // Text below the card's bottom border (the `AFTER` paragraph) is not the card's last line: only rows above the lowest
    // rule on the lower half of the page count when there is one.
    let card_end = rules
        .iter()
        .copied()
        .filter(|&r| r > 200)
        .max()
        .unwrap_or(h);
    let last_ink = (0..card_end)
        .rev()
        .filter(|&y| (80..700).any(|x| at(x, y) < 110))
        .find(|&y| !(150..640).all(|x| at(x, y) < 235));
    Frame {
        side_bottom: best.map(|b| b.1),
        side_top: best.map(|b| b.0),
        rules,
        last_ink,
    }
}

fn assert_every_word_once(what: &str, words: &[Word], expected: &[String]) {
    let printed: Vec<&str> = words
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.text.as_str())
        .collect();
    assert_eq!(
        printed,
        expected.iter().map(String::as_str).collect::<Vec<_>>(),
        "{what}: every word once, in order"
    );
}

/// Every printed word is inside the page's content box (page 1 ends at 799.4 pt).
fn assert_inside_content_box(what: &str, words: &[Word]) {
    for w in words {
        assert!(
            w.y_max <= CONTENT_BOTTOM + 0.2 && w.y_min >= CONTENT_TOP - 0.7,
            "{what}: `{}` on page {} at {:.1}..{:.1} pt is outside the content box ({CONTENT_TOP}..{CONTENT_BOTTOM})",
            w.text,
            w.page,
            w.y_min,
            w.y_max
        );
    }
}

fn first_line_y(words: &[Word], page: usize) -> f32 {
    words
        .iter()
        .filter(|w| w.page == page && w.text.starts_with('w'))
        .map(|w| w.y_min)
        .fold(f32::MAX, f32::min)
}

/// The frame of a two-page card: page 1 has the top border and side borders to the content box bottom and no bottom
/// border; page 2 has side borders from the content box top, no top border, and the bottom border below the last line.
fn assert_sliced_frame(what: &str, pdf: &[u8], pages: usize) {
    let last = pages;
    for page in 1..=last {
        let f = frame(pdf, page);
        let (side_top, side_bottom) = (
            f.side_top
                .unwrap_or_else(|| panic!("{what}: page {page} has no side border")),
            f.side_bottom.unwrap(),
        );
        if page == 1 {
            // The top border: a rule within a pixel or two of the first line's frame top (card y).
            assert!(
                f.rules.iter().any(|&r| r < 400),
                "{what}: page 1 has its top border (rules {:?})",
                f.rules
            );
            assert!(
                f.rules.iter().all(|&r| r < 400),
                "{what}: page 1 has no bottom border (rules {:?})",
                f.rules
            );
            let ink = f.last_ink.expect("page 1 has text");
            assert!(
                side_bottom >= ink,
                "{what}: page 1's side border ends at row {side_bottom}, above the last text row {ink}: the last line is outside the frame"
            );
            // The frame of a page that is not the last runs to the content box bottom (1065.8 px).
            assert!(
                side_bottom >= 1062,
                "{what}: page 1's side border ends at row {side_bottom}, Chrome's runs to the content box bottom (row 1065)"
            );
        } else if page < last {
            assert!(
                f.rules.is_empty(),
                "{what}: page {page} has neither a top nor a bottom border (rules {:?})",
                f.rules
            );
            assert!(
                side_top <= 60 && side_bottom >= 1062,
                "{what}: page {page}'s side border spans the content box ({side_top}..{side_bottom})"
            );
        } else {
            assert!(
                f.rules.iter().all(|&r| r > 200),
                "{what}: the last page has no top border (rules {:?})",
                f.rules
            );
            assert!(
                side_top <= 60,
                "{what}: the last page's side border starts at the content box top (row {side_top})"
            );
            let ink = f.last_ink.expect("the last page has text");
            let bottom = f
                .rules
                .iter()
                .copied()
                .find(|&r| r > ink)
                .unwrap_or_else(|| panic!("{what}: the last page has no bottom border below its text (rules {:?}, last text row {ink})", f.rules));
            assert!(
                bottom - ink <= 24,
                "{what}: the bottom border is {} px below the last text row (padding 16 px)",
                bottom - ink
            );
        }
    }
}

// ── The repro: c286 ─────────────────────────────────────────────────────────────────────────────────────────────────

fn c286(extra: &str, spacer: u32) -> Vec<u8> {
    let pre = if spacer > 0 {
        format!("<div style=\"height:{spacer}px;background:#eee\"></div>")
    } else {
        String::new()
    };
    render(&format!(
        "{pre}<div style=\"{extra}{CARD}\">{}</div><p>AFTER</p>",
        words(1500)
    ))
}

#[test]
fn a_clipping_card_with_direct_text_keeps_every_line_inside_its_page_and_frame() {
    // Main: 776 / 724, the 13 words of the last line of page 1 at 798-811 pt (the content box ends at 799.4 and the
    // clip hid them), page 2's last line one line too high and the bottom padding missing (AFTER at 735.8 pt).
    // Chrome: 763 / 737.
    let pdf = c286("overflow:hidden;", 0);
    let w = printed_words(&pdf);
    assert_eq!(per_page(&w), vec![763, 737], "words per page (Chrome)");
    assert_every_word_once("c286", &w, &names(0, 1500));
    assert_inside_content_box("c286", &w);
    assert_sliced_frame("c286", &pdf, 2);
}

#[test]
fn a_plain_card_with_direct_text_keeps_every_line_inside_its_page_and_frame() {
    // Without `overflow` the last line of page 1 was painted in the bottom margin (visible, where an `@bottom-*`
    // footer sits), the frame closed above the last line of page 2 and AFTER started over it.
    let pdf = c286("", 0);
    let w = printed_words(&pdf);
    assert_eq!(per_page(&w), vec![763, 737], "words per page (Chrome)");
    assert_every_word_once("plain", &w, &names(0, 1500));
    assert_inside_content_box("plain", &w);
    assert_sliced_frame("plain", &pdf, 2);
    let (page, y) = after(&w);
    assert_eq!(page, 2, "AFTER on page 2");
    assert!(
        (y - 760.8).abs() <= 1.5,
        "AFTER at {y:.1} pt, Chrome 760.8 (main 735.8)"
    );
}

#[test]
fn the_continuation_page_starts_at_the_content_box_top() {
    // Chrome: the first line of page 2 at 42.3 pt (the content box top is 42.5), no top border and no padding there.
    // The same with and without `overflow`, and for a `<p>` with the padding.
    for (what, pdf) in [
        ("overflow", c286("overflow:hidden;", 0)),
        ("plain", c286("", 0)),
        (
            "p",
            render(&format!(
                "<p style=\"{CARD}\">{}</p><p>AFTER</p>",
                words(1500)
            )),
        ),
    ] {
        let w = printed_words(&pdf);
        let y = first_line_y(&w, 2);
        assert!(
            (y - 42.3).abs() <= 1.2,
            "{what}: page 2's first line at {y:.1} pt, Chrome 42.3"
        );
        let (page, y) = after(&w);
        assert_eq!(page, 2, "{what}: AFTER on page 2");
        assert!(
            (y - 760.8).abs() <= 1.5,
            "{what}: AFTER at {y:.1} pt, Chrome 760.8"
        );
    }
}

#[test]
fn a_card_that_starts_below_something_is_sliced_where_it_starts() {
    // Chrome starts the card where it is and slices it: 300 px above it 530 / 798 / 172 words and AFTER on page 3 at
    // 231.3 pt; 700 px 223 / 773 / 504 and AFTER at 533.6. Main moved the whole card to page 2 (0 words on page 1).
    for (spacer, expected, lines, after_y) in [
        (300, vec![530, 798, 172], vec![41, 60, 14], 231.3),
        (700, vec![223, 773, 504], vec![17, 60, 38], 533.6),
    ] {
        for overflow in ["overflow:hidden;", ""] {
            let what = format!("spacer {spacer} {overflow}");
            let pdf = c286(overflow, spacer);
            let w = printed_words(&pdf);
            // Lines per page are Chrome's exactly. Words per page are within 6: our line breaks differ from Chrome's by one
            // word in a few lines (sub-pixel advances; main's nested `<div><p>` has the same 530 / 793 / 177), which moves
            // five words across the page break without moving a line.
            assert_eq!(lines_per_page(&w), lines, "{what}: lines per page (Chrome)");
            for (got, want) in per_page(&w).iter().zip(&expected) {
                assert!(
                    got.abs_diff(*want) <= 6,
                    "{what}: words per page {:?}, Chrome {expected:?}",
                    per_page(&w)
                );
            }
            assert_every_word_once(&what, &w, &names(0, 1500));
            assert_inside_content_box(&what, &w);
            let (page, y) = after(&w);
            assert_eq!(page, 3, "{what}: AFTER on page 3");
            assert!(
                (y - after_y).abs() <= 1.5,
                "{what}: AFTER at {y:.1} pt, Chrome {after_y}"
            );
            for p in 2..=3 {
                let y = first_line_y(&w, p);
                assert!(
                    (y - 42.3).abs() <= 1.2,
                    "{what}: page {p}'s first line at {y:.1} pt, Chrome 42.3"
                );
            }
        }
    }
}

// ── Guards: shapes the fix must not touch (numbers are main's, pinned) ──────────────────────────────────────────────

/// `(words per page, y_min of the first and y_max of the last word of each page, AFTER)`, rounded to 0.1 pt.
type Shape = (Vec<usize>, Vec<(f32, f32)>, (usize, f32));

fn shape(pdf: &[u8]) -> Shape {
    let w = printed_words(pdf);
    let pages = page_count(&w);
    let spans = (1..=pages)
        .map(|p| {
            let on: Vec<&Word> = w
                .iter()
                .filter(|x| x.page == p && x.text.starts_with('w'))
                .collect();
            let min = on.iter().map(|x| x.y_min).fold(f32::MAX, f32::min);
            if on.is_empty() {
                return (0.0, 0.0);
            }
            let max = on.iter().map(|x| x.y_max).fold(0.0, f32::max);
            ((min * 10.0).round() / 10.0, (max * 10.0).round() / 10.0)
        })
        .collect();
    let (ap, ay) = after(&w);
    (per_page(&w), spans, (ap, (ay * 10.0).round() / 10.0))
}

#[test]
fn a_padded_paragraph_that_fits_its_page_is_laid_out_as_before() {
    let pdf = render(&format!(
        "<div style=\"{CARD}\">{}</div><p>AFTER</p>",
        words(300)
    ));
    let got = shape(&pdf);
    assert_eq!(got, (vec![300], vec![(54.8, 345.1)], (1, 333.1)), "{got:?}");
}

#[test]
fn a_padded_paragraph_with_break_inside_avoid_is_laid_out_as_before() {
    let pdf = render(&format!(
        "<div style=\"height:700px;background:#eee\"></div><div style=\"{CARD};break-inside:avoid\">{}</div><p>AFTER</p>",
        words(700)
    ));
    let got = shape(&pdf);
    assert_eq!(
        got,
        (vec![0, 700], vec![(0.0, 0.0), (54.8, 747.8)], (2, 760.6)),
        "{got:?}"
    );
}

#[test]
fn an_unpadded_paragraph_keeps_its_page_break_rule() {
    // F30c: an unpadded body-level paragraph that starts mid-page and does not fit is not routed through the new path.
    let pdf = render(&format!(
        "<div style=\"height:700px;background:#eee\"></div><div>{}</div><p>AFTER</p>",
        words(1500)
    ));
    let got = shape(&pdf);
    assert_eq!(
        got,
        (
            vec![0, 801, 699],
            vec![(0.0, 0.0), (42.1, 798.1), (42.1, 672.1)],
            (3, 672.8)
        ),
        "{got:?}"
    );
}

// ── Fixture 29 ──────────────────────────────────────────────────────────────────────────────────────────────────────

fn fixture_29() -> Vec<u8> {
    fixture_29_with(
        "rounded-lg border border-gray-300 p-4 mb-6 text-sm",
        "rounded-lg border border-gray-300 p-4 mb-6 text-sm",
    )
}

/// Fixture 29 with the card's classes replaced.
fn fixture_29_with(card: &str, replacement: &str) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let html =
        std::fs::read_to_string(root.join("fixtures/invoices/29-card-direct-text/index.html"))
            .unwrap();
    assert!(html.contains(card), "the card of fixture 29 has {card}");
    let html = html.replace(card, replacement);
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .unwrap();
    for face in ["Regular", "Medium", "SemiBold", "Bold", "ExtraBold"] {
        b = b
            .font_file(root.join(format!("fixtures/fonts/Inter-{face}.ttf")))
            .unwrap();
    }
    b.build()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("fixture 29")
}

#[test]
fn fixture_29_prints_the_card_on_two_pages_like_chrome() {
    // Chrome (reference/chrome.pdf): 2 pages, 497 / 505 words. Main: 3 pages (the card moved to page 2: 84 / 684 / 238
    // words), page 2's last line at 804 pt, 5 pt below the content box.
    let ours = printed_words(&fixture_29());
    let chrome = printed_words(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/invoices/29-card-direct-text/reference/chrome.pdf"),
        )
        .unwrap(),
    );
    assert_eq!(
        page_count(&ours),
        2,
        "pages (Chrome {})",
        page_count(&chrome)
    );
    let count = |w: &[Word], p: usize| w.iter().filter(|x| x.page == p).count();
    for p in 1..=2 {
        assert!(
            count(&ours, p).abs_diff(count(&chrome, p)) <= 7,
            "words on page {p}: {} vs Chrome {}",
            count(&ours, p),
            count(&chrome, p)
        );
    }
    assert_inside_content_box("fixture 29", &ours);
    // The card's first line on page 2 is the first word of that page, at the content box top like Chrome's.
    let top = |w: &[Word]| {
        w.iter()
            .filter(|x| x.page == 2)
            .map(|x| x.y_min)
            .fold(f32::MAX, f32::min)
    };
    assert!(
        (top(&ours) - top(&chrome)).abs() <= 1.2,
        "page 2's first line at {:.1} pt, Chrome {:.1}",
        top(&ours),
        top(&chrome)
    );
}

/// Words and lines (distinct `y_min`, rounded to 1 pt) on each page, of every word of the document.
fn page_shape(words: &[Word]) -> (Vec<usize>, Vec<usize>) {
    let pages = page_count(words);
    let count = |p: usize| words.iter().filter(|w| w.page == p).count();
    let lines = |p: usize| {
        let mut ys: Vec<i32> = words
            .iter()
            .filter(|w| w.page == p)
            .map(|w| w.y_min.round() as i32)
            .collect();
        ys.sort_unstable();
        ys.dedup();
        ys.len()
    };
    (
        (1..=pages).map(count).collect(),
        (1..=pages).map(lines).collect(),
    )
}

#[test]
fn tailwind_cards_with_direct_text_have_chromes_lines_per_page() {
    // F30f-d, plan test 4: the card of fixture 29 in the shapes of the plan's `t16/A`, `B` and `E` (the `overflow-hidden`,
    // `text-xs leading-snug` and `text-[13px] leading-[1.3]` cards) and three more (a 16 px `leading-none` card, a thin
    // `p-1` one, a `p-6 leading-relaxed` one). Chrome m141 (`out/scratch/f30fd/v29/*.html`, rendered with
    // `chrome.mjs`): words per page and lines per page of the whole document. Main moved the card to page 2 in all of
    // them (84 words on page 1 and a page more than Chrome, or 84 / 891 / 30 against Chrome's 665 / 337).
    let base = "rounded-lg border border-gray-300 p-4 mb-6 text-sm";
    let shapes: [(&str, String, [usize; 3], [usize; 3]); 6] = [
        (
            "A overflow-hidden",
            base.replace("rounded-lg", "overflow-hidden rounded-lg"),
            [497, 505, 0],
            [43, 40, 0],
        ),
        (
            "B text-xs leading-snug",
            base.replace("text-sm", "text-xs leading-snug"),
            [665, 337, 0],
            [49, 25, 0],
        ),
        (
            "E text-[13px] leading-[1.3]",
            base.replace("text-sm", "text-[13px] leading-[1.3]"),
            [549, 453, 0],
            [45, 35, 0],
        ),
        (
            "F text-base leading-none",
            base.replace("text-sm", "text-base leading-none"),
            [375, 504, 124],
            [38, 42, 13],
        ),
        (
            "G p-1 text-xs",
            base.replace("p-4", "p-1").replace("text-sm", "text-xs"),
            [723, 279, 0],
            [51, 20, 0],
        ),
        (
            "H p-6 leading-relaxed",
            base.replace("p-4", "p-6")
                .replace("text-sm", "text-sm leading-relaxed"),
            [423, 545, 34],
            [39, 42, 4],
        ),
    ];
    for (what, classes, chrome_words, chrome_lines) in shapes {
        let words = printed_words(&fixture_29_with(base, &classes));
        let (w, l) = page_shape(&words);
        let pages = chrome_words.iter().filter(|&&n| n > 0).count();
        assert_eq!(
            w.len(),
            pages,
            "{what}: pages {w:?}, Chrome {chrome_words:?}"
        );
        assert_eq!(l, chrome_lines[..pages], "{what}: lines per page (Chrome)");
        for (got, want) in w.iter().zip(chrome_words) {
            assert!(
                got.abs_diff(want) <= 7,
                "{what}: words per page {w:?}, Chrome {chrome_words:?}"
            );
        }
        assert_inside_content_box(what, &words);
    }
}
