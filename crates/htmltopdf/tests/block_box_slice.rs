//! A block box with a border, padding, background or shadow that is split across pages is sliced at its breaks (F96).
//!
//! `box-decoration-break: slice`, the initial value (css-break-3 §5.4): the box is laid out as if there were no break and
//! then cut at the page edge. The top border and radius belong to the first fragment only, the bottom border and radius to
//! the last only; the side borders and the background of every other fragment run to the page's content bottom
//! (css-break-3 §5.3: "its content box extends to fill any remaining fragmentainer extent"), and a `box-shadow` is drawn for
//! the unbroken box and sliced. What follows the box starts below its bottom padding and border.
//!
//! Before the fix the fragmenter closed the frame on every page: a bottom border and rounded bottom corners under the last
//! line of page 1, a top border and rounded top corners over the first line of page 2, a frame that stopped at the last
//! child when a child moved whole, and, when the box split only between whole children, a last frame that ended at its last
//! child so that what followed started 25 px too high (defect (c) of the plan).
//!
//! Chrome (Skia/PDF m153, the version of the committed references; A4, 15 mm, Inter, 14px/1.2) was measured on the same
//! HTML (`scratchpad/f96/probes`, plan `docs/plans/2026-11-02-f96-block-box-decoration-slice.md` §10). The frame is checked on
//! a 96 dpi raster, where 1 CSS px is one pixel, so no unit conversion is needed in the test.
//!
//! Tests 1 to 5 are red until the fork slices block boxes. Test 6 pins that the text layer of the cards that already split by
//! lines does not move (the change is paint only there), and the guards of test 7 pin today's closed frame for the shapes
//! that stay out of F96 (each becomes red when its filed item F96b-F96e is fixed).

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/1.2 Inter;margin:0}p{margin:0}";
/// The card of the plan: padding, a black border (so that the raster shows it) and a radius.
const CARD: &str = "padding:16px;border:1px solid #000;border-radius:8px;margin-bottom:24px";
/// The nested box of shape 4.
const INNER: &str =
    "padding:16px;border:1px solid #000;border-radius:6px;background:#fff;margin-bottom:8px";
const FILL: &str = "background:#f0f0f0";
const SHADOW: &str = "box-shadow:0 4px 8px rgba(0,0,0,.4)";
/// The last content row of an A4 page with 15 mm margins (content bottom 1065.8 px) and the first one (56.7 px).
const LAST_ROW: usize = 1065;
const FIRST_ROW: usize = 57;

fn fonts() -> Vec<Vec<u8>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    // read_dir order is not deterministic; registration order must be.
    paths.sort();
    paths.iter().map(|p| std::fs::read(p).unwrap()).collect()
}

fn render(body: &str) -> Vec<u8> {
    let mut b = Renderer::builder().strict_fonts(true);
    for f in fonts() {
        b = b.font_bytes(f).unwrap();
    }
    b.build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// A committed fixture, rendered like `scripts/render-fixtures.sh` does.
fn render_fixture(name: &str) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let html =
        std::fs::read_to_string(root.join("fixtures/invoices").join(name).join("index.html"))
            .unwrap();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css");
    for f in fonts() {
        b = b.font_bytes(f).unwrap();
    }
    b.build()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

// ---- shapes -------------------------------------------------------------------------------------------------------

fn spacer(px: u32) -> String {
    format!("<div style=\"height:{px}px\"></div>")
}

fn word_list(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clause `i`: a 3-line paragraph of 36 words that stays whole (`break-inside: avoid`).
fn clause(i: usize) -> String {
    format!(
        "<p style=\"margin-bottom:8px;break-inside:avoid\">{}</p>",
        word_list(i * 36, i * 36 + 36)
    )
}

fn clauses(from: usize, to: usize) -> String {
    (from..to).map(clause).collect::<Vec<_>>().join("\n")
}

fn after() -> &'static str {
    "<p>AFTER</p>"
}

/// P1: the card of fixture 06: one paragraph longer than the room under the card's top on page 1.
fn shape_p1() -> String {
    format!(
        "{}<section style=\"{CARD}\"><p>{}</p></section>{}",
        spacer(700),
        word_list(0, 500),
        after()
    )
}

/// P2: the same card with 14 clauses: the break falls between two clauses, the last clause of page 1 ends 18 px
/// above the content bottom.
fn shape_p2() -> String {
    format!(
        "{}<section style=\"{CARD}\">{}</section>{}",
        spacer(690),
        clauses(0, 14),
        after()
    )
}

/// P3: P2 with a background and a shadow.
fn shape_p3() -> String {
    format!(
        "{}<section style=\"{CARD};{FILL};{SHADOW}\">{}</section>{}",
        spacer(690),
        clauses(0, 14),
        after()
    )
}

/// P4: a second bordered box nested in the card, across the break: two clauses of it on page 1, the rest on page 2.
fn shape_p4() -> String {
    format!(
        "{}<section style=\"{CARD};{FILL}\">{}<div style=\"{INNER}\">{}</div>{}</section>{}",
        spacer(700),
        clauses(0, 2),
        clauses(2, 8),
        clauses(8, 12),
        after()
    )
}

// ---- text layer ----------------------------------------------------------------------------------------------------

struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-bbs-{}-{n}.pdf", std::process::id()));
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
                x_min: attr(attrs, "xMin"),
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(words: &[Word]) -> usize {
    words.iter().map(|w| w.page).max().unwrap_or(0)
}

fn after_y(words: &[Word]) -> (usize, f32) {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    (a[0].page, a[0].y_min)
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

/// Words per page, and an FNV-1a hash of `page|text|x|y` (pt, 2 decimals) of every word in reading order.
fn text_layer(pdf: &[u8]) -> (Vec<usize>, u64) {
    let words = printed_words(pdf);
    let mut per = vec![0; page_count(&words)];
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for w in &words {
        per[w.page - 1] += 1;
        for b in format!("{}|{}|{:.2}|{:.2}\n", w.page, w.text, w.x_min, w.y_min).bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    (per, hash)
}

// ---- raster --------------------------------------------------------------------------------------------------------

/// A page of the PDF as 96 dpi RGB pixels (1 pixel = 1 CSS px).
struct Raster {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Raster {
    fn of(pdf: &[u8], page: usize) -> Raster {
        let path = temp(pdf);
        let dir = std::env::temp_dir().join(format!(
            "htmltopdf-bbs-r-{}-{}",
            std::process::id(),
            path.file_stem().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let status = std::process::Command::new("pdftoppm")
            .args(["-r", "96", "-f", &page.to_string(), "-l", &page.to_string()])
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
        // P6\n<w> <h>\n255\n<data>
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
        assert_eq!(fields[0], "P6");
        let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
        Raster {
            w,
            h,
            rgb: bytes[pos..].to_vec(),
        }
    }

    /// The grey level (mean of the channels) of a pixel.
    fn grey(&self, x: usize, y: usize) -> u32 {
        let i = (y * self.w + x) * 3;
        (u32::from(self.rgb[i]) + u32::from(self.rgb[i + 1]) + u32::from(self.rgb[i + 2])) / 3
    }

    /// Whether a pixel of the 1 px black border column at `x` or `x + 1` is dark in row `y`.
    fn dark_at(&self, x: usize, y: usize) -> bool {
        self.grey(x, y).min(self.grey(x + 1, y)) < 200
    }

    /// The card's left border (56.7 px) is dark in row `y`.
    fn outer_left(&self, y: usize) -> bool {
        self.dark_at(56, y)
    }

    /// The card's right border (737.3 px) is dark in row `y`.
    fn outer_right(&self, y: usize) -> bool {
        self.dark_at(736, y)
    }

    /// The nested box's left border (73.7 px) is dark in row `y`.
    fn inner_left(&self, y: usize) -> bool {
        self.dark_at(73, y)
    }

    /// Groups of consecutive rows across which a horizontal rule (the top or the bottom border) runs: the first row of
    /// each group. A rule at a fractional y shows in two rows.
    fn rules(&self, from: usize) -> Vec<usize> {
        let rows: Vec<usize> = (from..self.h)
            .filter(|&y| (150..640).filter(|&x| self.grey(x, y) < 235).count() * 10 > 490 * 9)
            .collect();
        let mut groups: Vec<usize> = Vec::new();
        let mut last = usize::MAX - 1;
        for r in rows {
            if r != last + 1 {
                groups.push(r);
            }
            last = r;
        }
        groups
    }

    /// Whether every pixel of rows `rows` between x 80 and 700 is paper white (no shadow ink).
    fn clean(&self, rows: std::ops::Range<usize>) -> bool {
        rows.into_iter()
            .all(|y| (80..700).all(|x| self.grey(x, y) >= 253))
    }
}

// ---- tests 1 to 5: red until block boxes are sliced ---------------------------------------------------------------------

/// P1 (06's card): page 1 has the top border and the side borders to the content bottom, no bottom border; page 2 has side
/// borders from the content top, no top border, and the bottom border. Chrome: page 1 sides 762..1065, one rule (757);
/// page 2 sides 57..528, one rule (533).
#[test]
fn card_around_a_split_paragraph_is_open_at_the_break() {
    let pdf = render(&shape_p1());
    let (p1, p2) = (Raster::of(&pdf, 1), Raster::of(&pdf, 2));
    let r1 = p1.rules(0);
    assert_eq!(r1.len(), 1, "page 1 has its top border only (rules {r1:?})");
    assert!(r1[0] < 800, "that rule is the card's top at 757 ({r1:?})");
    let r2 = p2.rules(0);
    assert_eq!(
        r2.len(),
        1,
        "page 2 has its bottom border only (rules {r2:?})"
    );
    assert!(
        r2[0] > 300,
        "that rule is the bottom at 533, not a top border ({r2:?})"
    );
    assert!(
        (LAST_ROW - 3..=LAST_ROW).any(|y| p1.outer_left(y) && p1.outer_right(y)),
        "page 1's side borders run to the content bottom (row {LAST_ROW})"
    );
    for y in [FIRST_ROW + 1, FIRST_ROW + 3] {
        assert!(
            p2.outer_left(y) && p2.outer_right(y),
            "page 2's side borders start at the content top (row {y})"
        );
    }
}

/// P3: the background runs to the content bottom on page 1 and from the content top on page 2, and the shadow is not
/// drawn at the cut edges (Chrome: no ink in the margins).
#[test]
fn background_runs_to_the_cut_and_the_shadow_stops_there() {
    let pdf = render(&shape_p3());
    let (p1, p2) = (Raster::of(&pdf, 1), Raster::of(&pdf, 2));
    // x 66 is inside the card's padding, left of the text.
    let bg = |v: u32| (238..=242).contains(&v);
    assert!(
        bg(p1.grey(66, LAST_ROW - 3)),
        "page 1: the background is there 3 px above the content bottom (grey {})",
        p1.grey(66, LAST_ROW - 3)
    );
    assert!(
        bg(p2.grey(66, FIRST_ROW + 3)),
        "page 2: the background is there 3 px below the content top (grey {})",
        p2.grey(66, FIRST_ROW + 3)
    );
    assert!(
        p1.clean(LAST_ROW + 2..p1.h),
        "page 1: no shadow ink below the content bottom"
    );
    assert!(
        p2.clean(LAST_ROW + 2..p2.h) && p2.clean(0..FIRST_ROW - 2),
        "page 2: no shadow ink above the content top or below the content bottom"
    );
}

/// Corners: square at a cut (the border column reaches the content bottom on page 1, starts at the content top on page 2),
/// round at an unbroken corner (the pixel outside the radius stays white). Chrome: `(57, 1065)` and `(57, 57)` are border.
#[test]
fn corners_are_square_at_the_cut_and_round_elsewhere() {
    let pdf = render(&shape_p1());
    let (p1, p2) = (Raster::of(&pdf, 1), Raster::of(&pdf, 2));
    assert!(
        p1.outer_left(LAST_ROW) && p1.outer_right(LAST_ROW),
        "page 1: both bottom corners are square at the cut"
    );
    assert!(
        p2.outer_left(FIRST_ROW) && p2.outer_right(FIRST_ROW),
        "page 2: both top corners are square at the cut"
    );
    // The unbroken top corners (page 1) and bottom corners (page 2) keep their radius: the outermost pixel is paper.
    let top = p1.rules(0)[0];
    assert!(
        p1.grey(57, top + 1) >= 235,
        "page 1: the top-left corner is rounded (grey {} outside the arc)",
        p1.grey(57, top + 1)
    );
    let bottom = p2.rules(0).last().copied().unwrap();
    assert!(
        p2.grey(57, bottom - 1) >= 200,
        "page 2: the bottom-left corner is rounded (grey {} outside the arc)",
        p2.grey(57, bottom - 1)
    );
}

/// P2 (defect (c)): a box split between whole children closes its bottom inset on the last page, so what follows starts
/// where Chrome's does. Chrome (m153): `AFTER` on page 2 at 467.6 pt (ours before the fix: 448.6), measured on this very HTML
/// with the Inter TTF of `fixtures/fonts` embedded. Before the fix `AFTER` stood 19 pt (25 px: the last clause's 8 px
/// margin, 16 px padding and 1 px border) too high.
#[test]
fn what_follows_a_box_split_between_children_starts_below_its_bottom_inset() {
    const CHROME_AFTER: (usize, f32) = (2, 467.6);
    let pdf = render(&shape_p2());
    let words = printed_words(&pdf);
    assert_every_word_once(
        "P2",
        &words,
        &(0..14 * 36).map(|i| format!("w{i:04}")).collect::<Vec<_>>(),
    );
    let (page, y) = after_y(&words);
    assert_eq!(page, CHROME_AFTER.0, "AFTER is on Chrome's page");
    assert!(
        (y - CHROME_AFTER.1).abs() <= 1.5,
        "AFTER at {y:.1} pt, Chrome puts it at {:.1} pt",
        CHROME_AFTER.1
    );
    // The frame of page 1 runs to the content bottom although its last clause ends 18 px above it.
    let p1 = Raster::of(&pdf, 1);
    assert!(
        (LAST_ROW - 3..=LAST_ROW).any(|y| p1.outer_left(y)),
        "page 1: the side border runs to the content bottom, not to the last clause"
    );
}

/// P4: a bordered box nested in the card, across the break. Both boxes are open at the cut (sides to the content bottom
/// on page 1, from the content top on page 2); page 2 has the inner box's bottom border and the card's, nothing else.
/// Chrome: page 1 rules 757 and 954, page 2 rules 541 and 878 (the same shape with other clauses).
#[test]
fn nested_boxes_are_both_open_at_the_break() {
    let pdf = render(&shape_p4());
    let (p1, p2) = (Raster::of(&pdf, 1), Raster::of(&pdf, 2));
    let r1 = p1.rules(0);
    assert_eq!(
        r1.len(),
        2,
        "page 1 has the card's top and the nested box's top, no bottom border (rules {r1:?})"
    );
    let r2 = p2.rules(0);
    assert_eq!(
        r2.len(),
        2,
        "page 2 has the nested box's bottom and the card's bottom, no top border (rules {r2:?})"
    );
    assert!(
        r2[0] > 100,
        "no top border at the content top (rules {r2:?})"
    );
    assert!(
        (LAST_ROW - 3..=LAST_ROW).any(|y| p1.outer_left(y) && p1.inner_left(y)),
        "page 1: both side borders run to the content bottom"
    );
    for y in [FIRST_ROW + 1, FIRST_ROW + 3] {
        assert!(
            p2.outer_left(y) && p2.inner_left(y),
            "page 2: both side borders start at the content top (row {y})"
        );
    }
}

/// The frame of a box that is not on its last page ends at the page's content bottom (css-break-3 §5.3), not in the bottom
/// margin. Before F96 the frame of the first page of a box whose nested box split a paragraph ran to the page's bottom
/// edge (row 1122, 56 rows into the margin: the `fresh()`-tall frame). P4 and fixture 42 are both such shapes. Chrome: the
/// side borders end at row 1065 of 1123.
#[test]
fn outer_frame_ends_at_the_content_bottom_not_in_the_margin() {
    for (what, pdf) in [
        ("P4", render(&shape_p4())),
        ("42", render_fixture("42-terms-card-clauses")),
    ] {
        let p1 = Raster::of(&pdf, 1);
        // Non-white, because the border of 42 is light grey. The left edges of the card and of the nested box are 56.7 and
        // 73.7 px in both shapes.
        let inked = |x: usize, y: usize| p1.grey(x, y).min(p1.grey(x + 1, y)) < 250;
        assert!(
            (LAST_ROW - 3..=LAST_ROW).any(|y| inked(56, y)),
            "{what}: the card's side border reaches the content bottom"
        );
        // One pixel of slack for the antialiased last row.
        for y in LAST_ROW + 2..p1.h {
            assert!(
                !inked(56, y) && !inked(73, y),
                "{what}: a side border runs into the bottom margin (row {y}, content bottom {LAST_ROW})"
            );
        }
    }
}

// ---- test 6: paint only for the cards that split by lines -----------------------------------------------------------------

/// The text layer of the cards that already split by lines (P1, 06, 08, 16) is what it was before F96: the change moves
/// the frame, not a word. Words per page and a hash of every word's page, text and position to 0.01 pt, from the binary
/// of `main` at `ed60ea2` (fork `d5b4a994`).
#[test]
fn text_layer_of_line_split_cards_does_not_move() {
    // (name, words per page, hash)
    let expected: Vec<(&str, Vec<usize>, u64)> = vec![
        ("P1", vec![222, 279], 3411154536329250146),
        ("06", vec![453, 498], 12828538040360679227),
        ("08", vec![407, 473], 16485408555130982072),
        ("16", vec![467, 541], 16217713157611749195),
    ];
    let got: Vec<(&str, Vec<usize>, u64)> = [
        ("P1", render(&shape_p1())),
        ("06", render_fixture("06-long-notes")),
        ("08", render_fixture("08-terms-grid")),
        ("16", render_fixture("16-terms-card")),
    ]
    .into_iter()
    .map(|(name, pdf)| {
        let (per_page, hash) = text_layer(&pdf);
        (name, per_page, hash)
    })
    .collect();
    assert_eq!(
        got, expected,
        "words per page and hash of the word positions"
    );
}

// ---- test 7: guards that pin today's closed frame --------------------------------------------------------------------------

/// What the guards pin, per page: the groups of horizontal rules below row 150, the rows where the card's left border
/// starts and ends (the longest dark run of its column) and the first word's y (pt, 1 decimal; -1 without words).
type PageShape = (Vec<usize>, Option<(usize, usize)>, i32);

fn side_run(r: &Raster) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut y = 0;
    while y < r.h {
        if r.outer_left(y) {
            let s = y;
            while y < r.h && r.outer_left(y) {
                y += 1;
            }
            if best.is_none_or(|(a, b)| y - 1 - s > b - a) {
                best = Some((s, y - 1));
            }
        } else {
            y += 1;
        }
    }
    best
}

fn signature(pdf: &[u8]) -> (usize, Vec<PageShape>) {
    let words = printed_words(pdf);
    let pages = page_count(&words);
    let per_page = (1..=pages)
        .map(|p| {
            let first = words
                .iter()
                .filter(|w| w.page == p)
                .map(|w| w.y_min)
                .reduce(f32::min);
            let r = Raster::of(pdf, p);
            (
                r.rules(150),
                side_run(&r),
                first.map_or(-1, |y| (y * 10.0).round() as i32),
            )
        })
        .collect();
    (pages, per_page)
}

/// F96d: a table cell is sliced by Chrome and closed by us.
#[test]
fn guard_table_cell_keeps_its_closed_frame() {
    let pdf = render(&format!(
        "{}<table style=\"width:100%;border-collapse:collapse\"><tr><td style=\"border:1px solid #000;padding:16px 0\">{}</td></tr></table>{}",
        spacer(700),
        word_list(0, 1500),
        after()
    ));
    assert_eq!(
        signature(&pdf),
        (
            3,
            vec![
                (vec![757], Some((757, 1060)), 5798),
                (vec![], Some((57, 1064)), 541),
                (vec![677], Some((57, 677)), 541),
            ]
        ),
        "table cell"
    );
}

/// F96b: a box with a `height` is sliced by Chrome and closed by us.
#[test]
fn guard_box_with_a_height_keeps_its_closed_frame() {
    let pdf = render(&format!(
        "{}<section style=\"{CARD};height:3000px\">{}</section>{}",
        spacer(300),
        clauses(0, 6),
        after()
    ));
    assert_eq!(
        signature(&pdf),
        (
            5,
            vec![
                (vec![], None, -1),
                (vec![1064], Some((61, 1061)), 548),
                (vec![1064], Some((61, 1061)), -1),
                (vec![1064], Some((61, 1061)), -1),
                (vec![], Some((57, 62)), 650),
            ]
        ),
        "height box"
    );
}

/// F96c: an absolutely positioned card is sliced by Chrome and closed by us.
#[test]
fn guard_absolutely_positioned_card_keeps_its_closed_frame() {
    let pdf = render(&format!(
        "{}<div style=\"position:relative\"><div style=\"position:absolute;left:0;right:0;top:0;{CARD}\"><p>{}</p></div></div>{}",
        spacer(700),
        word_list(0, 500),
        after()
    ));
    assert_eq!(
        signature(&pdf),
        (
            2,
            vec![
                (vec![756, 1064], Some((761, 1061)), 5671),
                (vec![442], Some((61, 439)), 421),
            ]
        ),
        "abspos card"
    );
}

/// F96e: a flex row with two items, one bordered and stretched beside a tall paragraph.
#[test]
fn guard_flex_row_item_keeps_its_closed_frame() {
    let pdf = render(&format!(
        "{}<div style=\"display:flex;gap:16px\"><div style=\"{CARD}\">Beside</div><div><p>{}</p></div></div>{}",
        spacer(700),
        word_list(0, 1500),
        after()
    ));
    assert_eq!(
        signature(&pdf),
        (
            3,
            vec![
                (vec![], Some((761, 1122)), 5671),
                (vec![], None, 421),
                (vec![], Some((957, 959)), 421),
            ]
        ),
        "flex row"
    );
}
