//! The document's `@page` decides the page size and margin (F6, `@page` half). The renderer used to force A4 portrait
//! with 15 mm margins, so `@page { size: A5 }` printed as A4 and `@page { margin: 0 }` kept its 15 mm (css-page-3,
//! "Page context" and the `size` property). Now the caller's explicit `RenderOptions::page` settings win per property,
//! then the document's `:first` / `:left` / `:right` rules, then its plain `@page` rule, then A4 and 15 mm.
//!
//! Asserted on the PDF's MediaBoxes (`pdfinfo -box`) and on word positions (`pdftotext -bbox`), tolerance 0.1 pt unless
//! a test says otherwise. Plan: docs/plans/2026-10-19-f6-page-size-margins.md.

use std::sync::{Mutex, Once};

use htmltopdf::{Margins, Page, PageSize, RenderOptions, Renderer};

const A4_W: f32 = 595.28;
const A4_H: f32 = 841.89;
/// 15 mm in pt: the default margin.
const MM15: f32 = 42.52;

/// A word printed in the PDF with its 1-based page and its box in pt from the top left of the page.
#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn document(css: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{css}</style></head><body>{body}</body></html>"
    )
}

fn render_with(css: &str, body: &str, opts: &RenderOptions) -> Vec<u8> {
    Renderer::builder()
        .build()
        .render(&document(css, body), opts)
        .expect("render")
}

fn render(css: &str, body: &str) -> Vec<u8> {
    render_with(css, body, &RenderOptions::default())
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-pagecss-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8], to_stdout: bool) -> String {
    let path = temp(pdf);
    let mut cmd = std::process::Command::new(tool);
    cmd.args(args).arg(&path);
    if to_stdout {
        cmd.arg("-");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{tool} (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `(width, height)` of every page's MediaBox in pt.
fn media_boxes(pdf: &[u8]) -> Vec<(f32, f32)> {
    poppler("pdfinfo", &["-f", "1", "-l", "9999", "-box"], pdf, false)
        .lines()
        .filter(|l| l.contains("MediaBox:"))
        .map(|l| {
            let n: Vec<f32> = l
                .split("MediaBox:")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            (n[2] - n[0], n[3] - n[1])
        })
        .collect()
}

fn attr(attrs: &str, name: &str) -> f32 {
    attrs
        .split_whitespace()
        .find_map(|a| a.strip_prefix(&format!("{name}=\"")))
        .and_then(|v| v.trim_end_matches('"').parse().ok())
        .unwrap_or_else(|| panic!("word without {name}"))
}

/// Every word of the PDF in extraction order (page by page, top to bottom).
fn words(pdf: &[u8]) -> Vec<Word> {
    let text = poppler("pdftotext", &["-bbox"], pdf, true);
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

fn first_word(pdf: &[u8], text: &str) -> Word {
    words(pdf)
        .into_iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("word {text:?} is not in the PDF"))
}

fn assert_size(pdf: &[u8], want: (f32, f32), what: &str) {
    let boxes = media_boxes(pdf);
    assert!(!boxes.is_empty(), "{what}: no page");
    for (i, (w, h)) in boxes.iter().enumerate() {
        assert!(
            (w - want.0).abs() <= 0.1 && (h - want.1).abs() <= 0.1,
            "{what}: page {} is {w} x {h} pt, want {} x {}",
            i + 1,
            want.0,
            want.1
        );
    }
}

fn assert_near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= 0.1, "{what}: {got} pt, want {want}");
}

/// One `<p>` of `Line001<br>Line002<br>…`, 20 px per line. (Inline text directly in `<body>` that is taller than a
/// page is cut off at the page edge, unrelated to F6, so the lines sit in a paragraph.)
fn lines(n: usize) -> String {
    let text = (1..=n)
        .map(|i| format!("Line{i:03}"))
        .collect::<Vec<_>>()
        .join("<br>");
    format!("<p>{text}</p>")
}

const BODY0: &str = "body { margin: 0 } p { margin: 0 } ";
const LINES20: &str = "body { margin: 0; line-height: 20px } ";

// --- T1 -------------------------------------------------------------------------------------------------------

#[test]
fn t1_size_a5_makes_an_a5_page() {
    let pdf = render("@page { size: A5 }", "<p>Ahoj</p>");
    assert_size(&pdf, (419.53, 595.28), "size: A5");
}

// --- T2 -------------------------------------------------------------------------------------------------------

#[test]
fn t2_orientation_after_the_keyword() {
    let pdf = render("@page { size: A4 landscape }", "<p>Ahoj</p>");
    assert_size(&pdf, (A4_H, A4_W), "size: A4 landscape");
}

#[test]
fn t2_orientation_before_the_keyword() {
    let pdf = render("@page { size: landscape A5 }", "<p>Ahoj</p>");
    assert_size(&pdf, (595.28, 419.53), "size: landscape A5");
}

#[test]
fn t2_portrait_before_the_keyword() {
    let pdf = render("@page { size: portrait A5 }", "<p>Ahoj</p>");
    assert_size(&pdf, (419.53, 595.28), "size: portrait A5");
}

#[test]
fn t2_other_keywords_of_the_table() {
    for (css, want) in [
        ("A3", (841.89, 1190.55)),
        ("B5", (498.90, 708.66)),
        ("B4", (708.66, 1000.63)),
        ("jis-b5", (515.91, 728.50)),
        ("jis-b4", (728.50, 1031.81)),
        ("letter", (612.0, 792.0)),
        ("legal", (612.0, 1008.0)),
        ("ledger", (792.0, 1224.0)),
    ] {
        let pdf = render(&format!("@page {{ size: {css} }}"), "<p>Ahoj</p>");
        assert_size(&pdf, want, &format!("size: {css}"));
    }
}

// --- T3 -------------------------------------------------------------------------------------------------------

#[test]
fn t3_two_lengths() {
    let pdf = render("@page { size: 210mm 99mm }", "<p>Ahoj</p>");
    assert_size(&pdf, (595.28, 280.63), "size: 210mm 99mm");
}

#[test]
fn t3_one_length_is_a_square() {
    let pdf = render("@page { size: 5in }", "<p>Ahoj</p>");
    assert_size(&pdf, (360.0, 360.0), "size: 5in");
}

// --- T4 -------------------------------------------------------------------------------------------------------

#[test]
fn t4_an_invalid_size_keeps_the_earlier_valid_one() {
    let pdf = render("@page { size: A5; size: Foo }", "<p>Ahoj</p>");
    assert_size(&pdf, (419.53, 595.28), "size: A5; size: Foo");
}

#[test]
fn t4_an_unknown_keyword_alone_leaves_the_default() {
    let pdf = render("@page { size: Foo }", "<p>Ahoj</p>");
    assert_size(&pdf, (A4_W, A4_H), "size: Foo");
}

// --- T5 -------------------------------------------------------------------------------------------------------

#[test]
fn t5_margin_zero_puts_text_at_the_page_corner() {
    let pdf = render(
        &format!("@page {{ margin: 0 }} {BODY0}"),
        "<p>Ahoj svete</p>",
    );
    let w = first_word(&pdf, "Ahoj");
    assert!(w.x_min < 1.0, "first word starts at x = {} pt", w.x_min);
    assert!(w.y_min < 3.0, "first word starts at y = {} pt", w.y_min);
}

// --- T6 -------------------------------------------------------------------------------------------------------

#[test]
fn t6_four_value_margin() {
    let css = format!("@page {{ margin: 10mm 20mm 30mm 40mm }} {LINES20}");
    let pdf = render(&css, &lines(120));
    let all = words(&pdf);
    assert!(media_boxes(&pdf).len() >= 2, "a multi-page text");
    assert_near(all[0].x_min, 113.39, "left margin 40 mm");
    assert!(all[0].y_min >= 28.0, "top 10 mm: {}", all[0].y_min);
    let limit = A4_H - 85.04;
    for w in &all {
        assert!(
            w.y_max <= limit + 3.0,
            "{} on page {} ends at {} pt, below the 30 mm bottom margin ({limit})",
            w.text,
            w.page,
            w.y_max
        );
    }
    // The page holds fewer lines than the 15 mm default would give it.
    let last_on_first = all.iter().filter(|w| w.page == 1).count();
    let default_fit = ((A4_H - 2.0 * MM15) / 15.0).floor() as usize;
    assert!(
        last_on_first < default_fit,
        "{last_on_first} lines on page 1"
    );
}

#[test]
fn t6_longhand_after_the_shorthand() {
    let css = format!("@page {{ margin: 10mm; margin-left: 25mm }} {BODY0}");
    let pdf = render(&css, "<p>Ahoj</p>");
    assert_near(first_word(&pdf, "Ahoj").x_min, 70.87, "margin-left: 25mm");
}

// --- T7: what the caller sets wins -------------------------------------------------------------------------------

fn with_page(page: Page) -> RenderOptions {
    RenderOptions {
        page,
        ..RenderOptions::default()
    }
}

#[test]
fn t7_caller_size_beats_the_document() {
    let pdf = render_with(
        "@page { size: A5 }",
        "<p>Ahoj</p>",
        &with_page(Page {
            size: Some(PageSize::A4),
            ..Page::default()
        }),
    );
    assert_size(&pdf, (A4_W, A4_H), "caller A4 over size: A5");
}

#[test]
fn t7_caller_size_keeps_its_own_orientation() {
    let pdf = render_with(
        "@page { size: A5 portrait }",
        "<p>Ahoj</p>",
        &with_page(Page {
            size: Some(PageSize::A4.landscape()),
            ..Page::default()
        }),
    );
    assert_size(
        &pdf,
        (A4_H, A4_W),
        "caller A4 landscape over size: A5 portrait",
    );
    // ... and a portrait caller size stays portrait under a landscape document.
    let pdf = render_with(
        "@page { size: A4 landscape }",
        "<p>Ahoj</p>",
        &with_page(Page {
            size: Some(PageSize::A4),
            ..Page::default()
        }),
    );
    assert_size(&pdf, (A4_W, A4_H), "caller A4 over size: A4 landscape");
}

#[test]
fn t7_caller_size_leaves_the_document_margin() {
    let pdf = render_with(
        &format!("@page {{ size: A5; margin: 25mm }} {BODY0}"),
        "<p>Ahoj</p>",
        &with_page(Page {
            size: Some(PageSize::A4),
            ..Page::default()
        }),
    );
    assert_near(
        first_word(&pdf, "Ahoj").x_min,
        70.87,
        "document margin 25 mm",
    );
}

// --- T8 ---------------------------------------------------------------------------------------------------------

#[test]
fn t8_caller_margin_beats_the_document() {
    let pdf = render_with(
        &format!("@page {{ margin: 0 }} {BODY0}"),
        "<p>Ahoj</p>",
        &with_page(Page {
            margin: Some(Margins::uniform_mm(15.0)),
            ..Page::default()
        }),
    );
    assert_near(
        first_word(&pdf, "Ahoj").x_min,
        MM15,
        "caller 15 mm over margin: 0",
    );
}

#[test]
fn t8_caller_margin_per_side() {
    let pdf = render_with(
        &format!("@page {{ margin: 0 }} {BODY0}"),
        "<p>Ahoj</p>",
        &with_page(Page {
            margin: Some(Margins {
                top_mm: 30.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 40.0,
            }),
            ..Page::default()
        }),
    );
    let w = first_word(&pdf, "Ahoj");
    assert_near(w.x_min, 113.39, "caller left 40 mm");
    assert!(
        w.y_min >= 83.0 && w.y_min < 95.0,
        "caller top 30 mm: {}",
        w.y_min
    );
}

#[test]
fn page_size_landscape_is_idempotent() {
    assert_eq!(
        PageSize::A4.landscape(),
        PageSize::A4.landscape().landscape()
    );
    assert_eq!(PageSize::A4.landscape().width_mm, 297.0);
    assert_eq!(PageSize::A5.landscape().height_mm, 148.0);
}

// --- T9 (guard) -----------------------------------------------------------------------------------------------

#[test]
fn t9_no_page_rule_is_a4_with_15mm() {
    let pdf = render(BODY0, "<p>Ahoj</p>");
    assert_size(&pdf, (A4_W, A4_H), "no @page");
    assert_near(
        first_word(&pdf, "Ahoj").x_min,
        MM15,
        "no @page, left margin",
    );
}

// --- T10 ------------------------------------------------------------------------------------------------------

#[test]
fn t10_size_alone_keeps_the_15mm_margin() {
    let pdf = render(&format!("@page {{ size: A5 }} {BODY0}"), "<p>Ahoj</p>");
    assert_size(&pdf, (419.53, 595.28), "size: A5");
    assert_near(
        first_word(&pdf, "Ahoj").x_min,
        MM15,
        "only size: left margin",
    );
}

#[test]
fn t10_one_side_alone_keeps_the_other_three() {
    let pdf = render(
        &format!("@page {{ margin-top: 30mm }} {BODY0}"),
        "<p>Ahoj</p>",
    );
    let w = first_word(&pdf, "Ahoj");
    assert_near(w.x_min, MM15, "only margin-top: left margin");
    assert!(w.y_min >= 83.0 && w.y_min < 95.0, "top 30 mm: {}", w.y_min);
}

// --- T11 ------------------------------------------------------------------------------------------------------

#[test]
fn t11_margin_box_sits_in_the_bottom_band_of_the_page_size() {
    let css = format!(
        "@page {{ size: A5; margin: 10mm; @bottom-center {{ content: \"PATICKA\"; font-size: 8pt }} }} {BODY0}"
    );
    let pdf = render(&css, "<p>Ahoj</p>");
    assert_size(&pdf, (419.53, 595.28), "A5 with a margin box");
    let w = first_word(&pdf, "PATICKA");
    let band = 595.28 - 28.35;
    assert!(
        w.y_min >= band - 1.0 && w.y_max <= 595.28,
        "margin box text is at {}..{} pt, want inside {band}..595.28",
        w.y_min,
        w.y_max
    );
    // (The text starts at the left margin edge, 28.35 pt, as on A4: a centre margin box is not centred, F110; change this when it is.)
    assert_near(w.x_min, 28.35, "margin box starts at the left margin edge");
}

// --- T12 ------------------------------------------------------------------------------------------------------

#[test]
fn t12_a_taller_first_page_margin_loses_no_line() {
    let css = format!("@page :first {{ margin-top: 50mm }} {LINES20}");
    let pdf = render(&css, &lines(100));
    let all = words(&pdf);
    let pages = media_boxes(&pdf).len();
    assert!(pages >= 3, "{pages} pages");
    for i in 1..=100 {
        let name = format!("Line{i:03}");
        assert_eq!(
            all.iter().filter(|w| w.text == name).count(),
            1,
            "{name} must be printed once"
        );
    }
    let first = all.iter().find(|w| w.text == "Line001").unwrap();
    assert!(first.y_min >= 140.0, "50 mm top on page 1: {}", first.y_min);
    let edge = A4_H - MM15;
    for w in &all {
        assert!(
            w.y_max <= edge + 3.0,
            "{} on page {} ends at {}",
            w.text,
            w.page,
            w.y_max
        );
    }
}

#[test]
#[ignore = "F109: every page is paginated against page 1's content height, so a :first rule with a smaller margin pushes later pages into their bottom margin"]
fn t12b_a_smaller_first_page_margin_overflows_later_pages() {
    let css =
        format!("@page {{ margin: 40mm 15mm }} @page :first {{ margin: 5mm 15mm }} {LINES20}");
    let pdf = render(&css, &lines(120));
    // Pages 2 and up have the 40 mm margins; page 1's own (5 mm) bottom edge is further down.
    let edge = A4_H - 113.39;
    for w in words(&pdf).into_iter().filter(|w| w.page >= 2) {
        assert!(
            w.y_max <= edge + 3.0,
            "{} on page {} ends at {}",
            w.text,
            w.page,
            w.y_max
        );
    }
}

// --- T13 ------------------------------------------------------------------------------------------------------

#[test]
fn t13_a_named_page_rule_is_dropped_and_the_page_stays_a4() {
    let pdf = render(
        &format!("@page cover {{ size: A5 }} {BODY0}"),
        "<p>Ahoj</p>",
    );
    assert_size(&pdf, (A4_W, A4_H), "@page cover { size: A5 }");
}

// --- Warnings: boxes that differ from page 1's, and margins that leave no room -----------------------------------
//
// The log is global to the process and the tests run in parallel, so every test looks for a message made unique by
// the sizes in its own document.

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.level() <= log::Level::Warn
            && (r.target().starts_with("fulgur") || r.target().starts_with("htmltopdf"))
        {
            LOG.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

fn capture_warnings() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::set_boxed_logger(Box::new(Capture)).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
}

/// The captured warnings that contain `needle`.
fn warnings_with(needle: &str) -> Vec<String> {
    LOG.lock()
        .unwrap()
        .iter()
        .filter(|m| m.contains(needle))
        .cloned()
        .collect()
}

#[test]
fn w1_a_smaller_later_page_size_warns_that_pages_follow_page_1s_box() {
    capture_warnings();
    // A3 first page, A5 for the rest: 299 lines fill 4 pages against the A3 box, of which most is lost on A5 pages
    // (F109). The page size says what happened; the output is not changed.
    let css = format!(
        "@page {{ size: A5; margin: 10mm }} @page :first {{ size: A3; margin: 10mm }} {LINES20}"
    );
    let body: String = (1..=299)
        .map(|i| format!("<p>Line{i:03} lorem ipsum</p>"))
        .collect();
    render(&css, &body);
    let w = warnings_with("page 1's content box (785.2 x 1133.9 pt)");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("362.8 x 538.6 pt"), "{}", w[0]);
    assert!(
        w[0].contains("smaller, so content may be clipped or lost"),
        "{}",
        w[0]
    );
    assert!(w[0].contains("F109"), "{}", w[0]);
}

#[test]
fn w1_a_larger_later_box_warns_too() {
    capture_warnings();
    // The letterhead shape (t12): later pages have room page 1's box does not.
    let css = format!("@page :first {{ margin-top: 50mm }} {LINES20}");
    render(&css, &lines(100));
    let w = warnings_with("page 1's content box (510.2 x 657.6 pt)");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("larger, so the page is filled only as far as page 1's box"),
        "{}",
        w[0]
    );
}

#[test]
fn w1_pages_with_one_box_say_nothing() {
    capture_warnings();
    let css = format!("@page {{ size: 5in; margin: 7mm }} {LINES20}");
    let pdf = render(&css, &lines(100));
    assert!(media_boxes(&pdf).len() >= 2);
    assert!(warnings_with("(320.2 x 320.2 pt)").is_empty());
    assert!(warnings_with("page 1's content box (320.2").is_empty());
}

#[test]
fn w2_a_margin_larger_than_the_page_warns() {
    capture_warnings();
    let pdf = render(
        &format!("@page {{ size: A5; margin: 200mm }} {BODY0}"),
        "<p>Ahoj</p>",
    );
    assert_size(&pdf, (419.53, 595.28), "margin 200mm");
    let w = warnings_with("(page 419.5 x 595.3 pt, margins 566.9 / 566.9 / 566.9 / 566.9 pt)");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("the margins leave no room"), "{}", w[0]);
}

#[test]
fn w2_a_caller_margin_larger_than_the_page_warns() {
    capture_warnings();
    render_with(
        "@page { size: 99mm 98mm }",
        "<p>Ahoj</p>",
        &with_page(Page {
            margin: Some(Margins::uniform_mm(60.0)),
            ..Page::default()
        }),
    );
    let w = warnings_with("(page 280.6 x 277.8 pt, margins 170.1 / 170.1 / 170.1 / 170.1 pt)");
    assert_eq!(w.len(), 1, "{w:?}");
}
