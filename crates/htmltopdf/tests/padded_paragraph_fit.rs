//! What follows a body-level paragraph with a border, a padding or a block size of its own that fits its page starts
//! below the box, not over its last line (F30f-h).
//!
//! CSS 2.1 §10.6.3 / §8.1: the box is as tall as its lines plus the padding and border below them (or its `height` /
//! `min-height`). The fitting branch of the body-level line path returned the end of the lines as the cursor, so the
//! next sibling started `border + padding` (or the block size's extra) too high, over the last line; the frame was
//! right (it is painted from the layout size), which made the overlap visible.
//!
//! Every number is Chrome's (m141, A4, 15 mm, Inter, 14px/20px, `border:1px solid; padding:9px`, an `AFTER` paragraph
//! with no margin below), measured with `out/scratch/f30fh/{gen,run.sh,meas}.py` (plan `2026-10-29-f30f-h` §3). The
//! raster is taken at 72 dpi, where one pixel is one pt like the text layer's coordinates, so the test converts no units.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";
/// The card: a black border so that the raster shows it, a light background.
const CARD: &str = "border:1px solid #000;padding:9px;background:#eee";

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

/// `L1<br>L2<br>…`
fn lines(prefix: &str, n: usize) -> String {
    (1..=n)
        .map(|i| format!("{prefix}{i}"))
        .collect::<Vec<_>>()
        .join("<br>")
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
    let path = std::env::temp_dir().join(format!("htmltopdf-ppf-{}-{n}.pdf", std::process::id()));
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

fn after(words: &[Word]) -> &Word {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    a[0]
}

/// The rows (72 dpi: pt from the page top) where a horizontal rule crosses the card, of page 1.
fn rule_rows(pdf: &[u8]) -> Vec<usize> {
    let path = temp(pdf);
    let dir = std::env::temp_dir().join(format!(
        "htmltopdf-ppf-r-{}-{}",
        std::process::id(),
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let status = std::process::Command::new("pdftoppm")
        .args(["-r", "72", "-gray", "-f", "1", "-l", "1"])
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
    let px = &bytes[pos..];
    // The card spans 15 mm .. 580 pt; the background (#eee = 238) is not dark, the black rule is.
    (0..h)
        .filter(|&y| {
            let dark = (100..500).filter(|&x| px[y * w + x] < 200).count();
            dark * 10 > 400 * 9
        })
        .collect()
}

/// AFTER's top is Chrome's within a point.
fn assert_after_at(what: &str, body: &str, chrome_y: f32) {
    let pdf = render(body);
    let words = printed_words(&pdf);
    let a = after(&words);
    assert_eq!(a.page, 1, "{what}: AFTER is on page 1");
    assert!(
        (a.y_min - chrome_y).abs() <= 1.0,
        "{what}: AFTER at {:.1} pt, Chrome puts it at {chrome_y:.1} pt",
        a.y_min
    );
}

#[test]
fn a_card_with_border_and_padding_is_followed_below_its_frame() {
    // The finding's repro: 5 lines, border 1px, padding 9px; the lines end at 111.3 pt and AFTER was over the last one.
    assert_after_at(
        "border + padding, 5 lines",
        &format!("<div style=\"{CARD}\">{}</div><p>AFTER</p>", lines("L", 5)),
        133.8,
    );
}

#[test]
fn nothing_overlaps_and_the_bottom_border_is_below_the_last_line() {
    let pdf = render(&format!(
        "<div style=\"{CARD}\">{}</div><p>AFTER</p>",
        lines("L", 5)
    ));
    let words = printed_words(&pdf);
    let last = words.iter().find(|w| w.text == "L5").expect("L5");
    let a = after(&words);
    let rules = rule_rows(&pdf);
    let bottom = *rules.last().expect("the card has a bottom border");
    let top = rules[0];
    assert!(
        bottom > top + 50,
        "a top rule and a bottom rule below it (rows {rules:?})"
    );
    assert!(
        last.y_max <= bottom as f32,
        "L5 ends at {:.1} pt, the bottom border is at row {bottom}",
        last.y_max
    );
    assert!(
        a.y_min >= bottom as f32,
        "AFTER starts at {:.1} pt, above the bottom border (row {bottom}): it overlaps the card",
        a.y_min
    );
}

#[test]
fn the_shapes_that_leave_the_cursor_at_the_end_of_the_lines_agree_with_chrome() {
    let card = |style: &str, n: usize| {
        format!("<div style=\"{style}\">{}</div><p>AFTER</p>", lines("L", n))
    };
    // Two lines (a one-line card was right already: guard below).
    assert_after_at("two lines", &card(CARD, 2), 88.8);
    // Padding only, border only.
    assert_after_at(
        "padding only",
        &card("padding:9px;background:#eee", 5),
        132.3,
    );
    assert_after_at("border only", &card("border:1px solid #000", 5), 120.3);
    // `overflow:hidden` card (F30f-b's auto-height clip box).
    assert_after_at(
        "overflow:hidden",
        &card(&format!("overflow:hidden;{CARD}"), 5),
        133.8,
    );
    // A margin below the card is added to the frame, not to the lines.
    assert_after_at(
        "margin-bottom:24px",
        &card(&format!("{CARD};margin-bottom:24px"), 5),
        151.8,
    );
    // `height` and `min-height` taller than the lines (border-box, as with Tailwind's preflight).
    assert_after_at(
        "height:150px",
        &card(&format!("box-sizing:border-box;height:150px;{CARD}"), 5),
        156.3,
    );
    assert_after_at(
        "min-height:200px",
        &card(&format!("box-sizing:border-box;min-height:200px;{CARD}"), 5),
        193.8,
    );
    // The same without any padding or border.
    assert_after_at(
        "plain height:150px",
        &card("height:150px;background:#eee", 5),
        156.3,
    );
}

#[test]
fn two_cards_in_a_row_are_stacked_not_overlapped() {
    assert_after_at(
        "two cards",
        &format!(
            "<div style=\"{CARD};margin-bottom:6px\">{}</div>\
             <div style=\"border:1px solid #f00;padding:9px;background:#fee\">{}</div><p>AFTER</p>",
            lines("L", 3),
            lines("M", 3)
        ),
        168.3,
    );
}

#[test]
fn shapes_that_were_right_stay_where_they_were() {
    // One line (the block path), a flex item, a nested card, `break-inside: avoid`, a plain paragraph with a margin.
    assert_after_at(
        "one line",
        &format!("<div style=\"{CARD}\">L1</div><p>AFTER</p>"),
        73.8,
    );
    assert_after_at(
        "flex item",
        &format!(
            "<div style=\"display:flex\"><div style=\"{CARD}\">{}</div></div><p>AFTER</p>",
            lines("L", 5)
        ),
        133.8,
    );
    assert_after_at(
        "nested in a section",
        &format!(
            "<section><div style=\"{CARD}\">{}</div><p>AFTER</p></section>",
            lines("L", 5)
        ),
        133.8,
    );
    assert_after_at(
        "break-inside: avoid",
        &format!(
            "<div style=\"break-inside:avoid;{CARD}\">{}</div><p>AFTER</p>",
            lines("L", 5)
        ),
        133.8,
    );
    assert_after_at(
        "plain paragraph with a margin",
        &format!(
            "<div style=\"margin-bottom:24px;background:#eee\">{}</div><p>AFTER</p>",
            lines("L", 5)
        ),
        136.8,
    );
}

/// A card below an 800 px spacer, with `n` lines, followed by `AFTER`.
fn card_at_the_page_bottom(n: usize) -> Vec<Word> {
    printed_words(&render(&format!(
        "<div style=\"height:800px\"></div><div style=\"{CARD}\">{}</div><p>AFTER</p>",
        lines("L", n)
    )))
}

/// The distance from the card's last line to `AFTER`.
fn last_line_to_after(words: &[Word], last: &str) -> f32 {
    let l = words.iter().find(|w| w.text == last).expect("last line");
    after(words).y_min - l.y_min
}

#[test]
fn a_card_that_fits_and_one_that_spans_are_followed_alike() {
    // 8 lines fit the page under the spacer, 10 do not (F30f-d slices them 8 / 2): in both, AFTER is the card's bottom
    // inset below the last line, 22.5 pt in Chrome.
    let fits = card_at_the_page_bottom(8);
    assert_eq!(
        after(&fits).page,
        1,
        "the fitting card leaves room for AFTER"
    );
    let d_fit = last_line_to_after(&fits, "L8");
    let spans = card_at_the_page_bottom(10);
    assert_eq!(after(&spans).page, 2, "the card spans two pages");
    let d_span = last_line_to_after(&spans, "L10");
    assert!(
        (d_fit - 22.5).abs() <= 1.0 && (d_span - 22.5).abs() <= 1.0,
        "last line to AFTER: {d_fit:.1} pt when the card fits, {d_span:.1} pt when it spans; Chrome: 22.5 both"
    );
}

#[test]
fn a_card_that_fills_the_page_pushes_what_follows_to_the_next_one() {
    // 9 lines end the card at 793 pt, inside the content box (799.4); AFTER's line (15 pt) no longer fits and starts
    // page 2 at 43.8 pt, as in Chrome. Before, it was written over L9 on page 1.
    let words = card_at_the_page_bottom(9);
    let a = after(&words);
    assert_eq!(a.page, 2, "AFTER starts the next page");
    assert!(
        (a.y_min - 43.8).abs() <= 1.0,
        "AFTER at {:.1} pt, Chrome puts it at 43.8 pt",
        a.y_min
    );
    let l9 = words.iter().find(|w| w.text == "L9").expect("L9");
    assert_eq!(l9.page, 1, "all 9 lines stay on page 1");
}
