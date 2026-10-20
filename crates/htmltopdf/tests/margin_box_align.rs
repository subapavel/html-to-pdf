//! Content of an `@page` margin box is aligned in its box by the css-page-3 UA defaults (F103, with F110 folded in):
//! `vertical-align: middle` (`top` / `bottom` for the `@left-top` / `@left-bottom` families) and the `text-align` of
//! the table in css-page-3 §4.2 (`center` for `-center`, `right` for `-right` and for the left corners, `left` for
//! `-left` and for the right corners). An author declaration beats the default. The content used to be painted from
//! the top left corner of its box.
//!
//! Every case is one of the step-0 probes P1-P14 of docs/plans/2026-10-20-f103-margin-box-vertical-align.md. The
//! `CHROME_*` constants are what Chromium (Playwright, print to PDF, `pdftotext -bbox`) printed for the same
//! document, so the test needs no browser. Positions are in pt from the top left of the page.
//!
//! Tolerances: 0.5 pt against Chrome. Two documented exceptions, both Chrome's own pixel snapping and not ours:
//! - a bottom band of 15 mm is placed 0.88 pt lower in Chrome than the mm geometry gives (the same box in a 10, 20
//!   and 30 mm band is within 0.3 pt; the top boxes are exact), so the 15 mm bottom cases allow 1 pt;
//! - Chrome's content area is 0.74 pt wider on the right than 210 mm - 2 * 15 mm (its right-aligned text ends at
//!   553.50, the mm geometry gives 552.76), so right edges allow 1 pt.
//!
//! Where Chrome and css-page-3 / CSS 2.1 §17.5.3 disagree we follow the spec: Chrome centres content that is taller
//! than its box (P7, it overflows at both ends), a table cell grows and starts at the top.

use std::path::{Path, PathBuf};

use htmltopdf::{RenderOptions, Renderer};

const PAGE_H: f32 = 841.89;
const MM: f32 = 72.0 / 25.4;

/// One word of the PDF: page (1-based) and its box in pt from the top left.
#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    text: String,
}

impl Word {
    fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
    fn cy(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn temp(pdf: &[u8], ext: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-mbalign-{}-{}.{ext}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            let attr = |name: &str| -> f32 {
                attrs
                    .split_whitespace()
                    .find_map(|a| a.strip_prefix(name))
                    .and_then(|v| v.strip_prefix("=\""))
                    .and_then(|v| v.trim_end_matches('"').parse::<f32>().ok())
                    .unwrap_or_else(|| panic!("word without {name}"))
            };
            found.push(Word {
                page,
                x0: attr("xMin"),
                y0: attr("yMin"),
                x1: attr("xMax"),
                y1: attr("yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

/// The words of a margin box probe: everything but the body text of the probe page.
fn margin_words(pdf: &[u8]) -> Vec<Word> {
    words(pdf)
        .into_iter()
        .filter(|w| {
            !matches!(
                w.text.as_str(),
                "Body" | "text" | "of" | "the" | "page." | "One" | "Two" | "Three"
            )
        })
        .collect()
}

fn word(all: &[Word], text: &str) -> Word {
    all.iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("word {text:?} is not in the PDF; found {all:?}"))
        .clone()
}

/// `page` are the `@page` properties, `boxes` the margin boxes inside it, `css` the rest of the stylesheet.
fn render_with(page: &str, boxes: &str, css: &str, body: &str, tagged: bool) -> Vec<u8> {
    let r = root();
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>@page{{{page}{boxes}}}{css}</style></head>\
         <body class=\"font-sans text-[10pt]\">{body}</body></html>"
    );
    let mut b = Renderer::builder()
        .css(std::fs::read_to_string(r.join("fixtures/tailwind/tailwind.css")).unwrap())
        .strict_fonts(true);
    let mut fonts: Vec<PathBuf> = std::fs::read_dir(r.join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    let opts = RenderOptions {
        lang: Some("cs".into()),
        tagged,
        ..Default::default()
    };
    b.build().render(&html, &opts).expect("render")
}

fn render(page: &str, boxes: &str, body: &str) -> Vec<u8> {
    render_with(page, boxes, "", body, false)
}

const BODY: &str = "<p>Body text of the page.</p>";

/// `@pos { content: "text"; font-family: Inter; font-size: 9pt; extra }`, as in the probes.
fn bx(pos: &str, content: &str, extra: &str) -> String {
    format!("@{pos}{{content:{content};font-family:Inter,sans-serif;font-size:9pt;{extra}}}")
}

fn near(got: f32, want: f32, tol: f32, what: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: {got:.2}, want {want:.2} +- {tol}"
    );
}

/// Vertical centre of the word against Chrome's and against the centre of the band `[top, bottom]`.
fn centred_in_band(w: &Word, chrome_cy: f32, band: (f32, f32), tol: f32) {
    near(
        w.cy(),
        chrome_cy,
        tol,
        &format!("`{}` centre y vs Chrome", w.text),
    );
    // The glyph box of a 9 pt Inter line sits 0.5 pt above the line box centre (ascent 1.03, descent 1.58 of 13.5).
    near(
        w.cy(),
        (band.0 + band.1) / 2.0 - 0.54,
        1.0,
        &format!("`{}` centre y vs the band", w.text),
    );
}

const TOP15: (f32, f32) = (0.0, 15.0 * MM);
const BOTTOM15: (f32, f32) = (PAGE_H - 15.0 * MM, PAGE_H);

// --- P1 / P2 --------------------------------------------------------------------------------------------------

/// Chrome P1: TL1 / TC1 / TR1 at y 15.28..26.17. Bottom P2: 815.53..826.42.
const CHROME_P1_CY: f32 = 20.725;
const CHROME_P2_CY: f32 = 820.975;

#[test]
fn t01_top_boxes_centred() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("top-left", "\"TL1\"", ""),
            bx("top-center", "\"TC1\"", ""),
            bx("top-right", "\"TR1\"", "")
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    for t in ["TL1", "TC1", "TR1"] {
        centred_in_band(&word(&all, t), CHROME_P1_CY, TOP15, 0.5);
    }
}

#[test]
fn t02_bottom_boxes_centred() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("bottom-left", "\"BL1\"", ""),
            bx("bottom-center", "\"BC1\"", ""),
            bx("bottom-right", "\"BR1\"", "")
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    for t in ["BL1", "BC1", "BR1"] {
        // 1 pt: Chrome's 15 mm bottom band is 0.88 pt lower than the mm geometry (module doc).
        centred_in_band(&word(&all, t), CHROME_P2_CY, BOTTOM15, 1.0);
    }
}

// --- P3: the band height decides ------------------------------------------------------------------------------

/// Chrome P3: (margin mm, top centre y, bottom centre y) of `TC` / `BC`.
const CHROME_P3: [(f32, f32, f32); 3] = [
    (10.0, 13.975, 827.725),
    (20.0, 28.225, 813.475),
    (30.0, 42.475, 799.225),
];

#[test]
fn t03_centring_follows_margin_height() {
    for (mm, top_cy, bottom_cy) in CHROME_P3 {
        let pdf = render(
            &format!("size:A4;margin:{mm}mm;"),
            &format!(
                "{}{}",
                bx("top-center", "\"TCX\"", ""),
                bx("bottom-center", "\"BCX\"", "")
            ),
            BODY,
        );
        let all = margin_words(&pdf);
        centred_in_band(&word(&all, "TCX"), top_cy, (0.0, mm * MM), 0.5);
        centred_in_band(
            &word(&all, "BCX"),
            bottom_cy,
            (PAGE_H - mm * MM, PAGE_H),
            0.5,
        );
    }
}

// --- P4: the line height decides ------------------------------------------------------------------------------

#[test]
fn t04_centring_follows_font_size() {
    let pdf = render(
        "size:A4;margin:15mm;",
        "@top-left{content:\"S7\";font-family:Inter;font-size:7pt}\
         @top-center{content:\"S9\";font-family:Inter;font-size:9pt}\
         @top-right{content:\"S14\";font-family:Inter;font-size:14pt}",
        BODY,
    );
    let all = margin_words(&pdf);
    // Chrome P4: S7 16.47..24.94, S9 15.28..26.17, S14 11.94..28.88.
    near(word(&all, "S7").cy(), 20.705, 0.5, "7 pt");
    near(word(&all, "S9").cy(), 20.725, 0.5, "9 pt");
    near(word(&all, "S14").cy(), 20.41, 0.5, "14 pt");
}

// --- P5: several lines ----------------------------------------------------------------------------------------

#[test]
fn t05_multi_line_block_centred() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("top-center", "\"A\\A B\"", "white-space:pre"),
            bx(
                "top-left",
                "\"Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod\"",
                ""
            ),
            bx("top-right", "\"TR5\"", "")
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    let (a, b) = (word(&all, "A"), word(&all, "B"));
    // Chrome P5: A 8.53..19.42, B 22.03..32.92; the wrapped third-width box has the same two lines.
    near(a.y0, 8.53, 0.5, "first line top");
    near(b.y1, 32.92, 0.5, "last line bottom");
    let (top_gap, bottom_gap) = (a.y0, TOP15.1 - b.y1);
    assert!(
        (top_gap - bottom_gap).abs() <= 1.5,
        "gap above the first line {top_gap:.2}, below the last {bottom_gap:.2}"
    );
    near(word(&all, "Lorem").y0, 8.53, 0.5, "wrapped box, first line");
    near(
        word(&all, "eiusmod").y0,
        22.03,
        0.5,
        "wrapped box, second line",
    );
    near(
        word(&all, "TR5").cy(),
        20.725,
        0.5,
        "single line next to two",
    );
}

// --- P6: the author's vertical-align wins ---------------------------------------------------------------------

/// Half-leading of a 9 pt line in 13.5 pt (1.31) plus 0.5.
const SLACK: f32 = 1.31 + 0.5;

#[test]
fn t06_author_vertical_align_honoured() {
    // Top band, Chrome P6a/P6b: top, baseline, 4px and super start at the top (yMin 1.03), bottom ends at 41.17,
    // middle centres (15.28..26.17).
    let top = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("top-left", "\"VT\"", "vertical-align:top"),
            bx("top-center", "\"VB\"", "vertical-align:bottom"),
            bx("top-right", "\"VBL\"", "vertical-align:baseline"),
        ),
        BODY,
    );
    let all = margin_words(&top);
    for t in ["VT", "VBL"] {
        let w = word(&all, t);
        assert!(
            w.y0 <= TOP15.0 + SLACK,
            "{t} starts at {:.2}, want the band top",
            w.y0
        );
    }
    let vb = word(&all, "VB");
    assert!(
        vb.y1 >= TOP15.1 - SLACK - 0.3,
        "bottom ends at {:.2}",
        vb.y1
    );
    let top2 = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("top-left", "\"V4\"", "vertical-align:4px"),
            bx("top-center", "\"VM\"", "vertical-align:middle"),
            bx("top-right", "\"VSUP\"", "vertical-align:super"),
        ),
        BODY,
    );
    let all = margin_words(&top2);
    for t in ["V4", "VSUP"] {
        let w = word(&all, t);
        assert!(
            w.y0 <= TOP15.0 + SLACK,
            "{t} starts at {:.2}, want the band top",
            w.y0
        );
    }
    near(word(&all, "VM").cy(), 20.725, 0.5, "explicit middle");

    // Bottom band (P6c/P6d). Chrome puts `top` 1.91 below the band top, i.e. the 0.88 pt of module doc.
    let bottom = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("bottom-left", "\"VT\"", "vertical-align:top"),
            bx("bottom-center", "\"VB\"", "vertical-align:bottom"),
            bx("bottom-right", "\"VBL\"", "vertical-align:baseline"),
        ),
        BODY,
    );
    let all = margin_words(&bottom);
    for t in ["VT", "VBL"] {
        let w = word(&all, t);
        assert!(
            w.y0 <= BOTTOM15.0 + SLACK,
            "{t} starts at {:.2}, want the band top",
            w.y0
        );
    }
    let vb = word(&all, "VB");
    assert!(
        vb.y1 >= BOTTOM15.1 - SLACK - 0.3,
        "bottom ends at {:.2}",
        vb.y1
    );
}

// --- P7: more lines than fit ----------------------------------------------------------------------------------

/// What main printed for P7 (`L1 L2` at the top of the 10 mm band, `M1 M2` at the top of the bottom one): the
/// lines that do not fit are dropped without a warning, and that stays (non-goal of the plan). Chrome centres the
/// overflow (L1 and L2 above the page, L3 L4 L5 at 8.53 / 22.03 / 35.53); css-page-3 / CSS 2.1 §17.5.3 start at
/// the top, and so do we.
const MAIN_P7: [&str; 4] = ["L1", "L2", "M1", "M2"];

#[test]
fn t07_overflow_starts_at_box_top() {
    let pdf = render(
        "size:A4;margin:10mm;",
        &format!(
            "{}{}",
            bx(
                "top-center",
                "\"L1\\A L2\\A L3\\A L4\\A L5\"",
                "white-space:pre"
            ),
            bx(
                "bottom-center",
                "\"M1\\A M2\\A M3\\A M4\\A M5\"",
                "white-space:pre"
            )
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    let mut printed: Vec<&str> = all.iter().map(|w| w.text.as_str()).collect();
    printed.sort();
    let mut want = MAIN_P7.to_vec();
    want.sort();
    assert_eq!(printed, want, "the lines that fit are the lines printed");
    near(
        word(&all, "L1").y0,
        1.03,
        1.0,
        "top box: first line at the band top",
    );
    near(
        word(&all, "M1").y0,
        PAGE_H - 10.0 * MM + 1.03,
        1.0,
        "bottom box: first line at the band top",
    );
}

// --- P8: the side boxes ---------------------------------------------------------------------------------------

#[test]
fn t08_side_boxes_top_middle_bottom() {
    let pdf = render(
        "size:A4;margin:15mm 25mm;",
        &format!(
            "{}{}{}{}{}{}",
            bx("left-top", "\"LT\"", ""),
            bx("left-middle", "\"LM\"", ""),
            bx("left-bottom", "\"LB\"", ""),
            bx("right-top", "\"RT\"", ""),
            bx("right-middle", "\"RM\"", ""),
            bx("right-bottom", "\"RB\"", ""),
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    // Chrome P8: LT/RT y 43.78..54.67, LM/RM 415.78..426.67, LB/RB 787.78..798.67; x centre 35.24 / 559.84 (the
    // 25 mm side margins are 70.87 pt wide: text-align centre). Chrome's middle and bottom side boxes sit 0.7 and
    // 1.2 pt lower than the mm geometry (its content area ends at 800.02, not 799.37; the same snapping as the
    // 15 mm bottom band, module doc), and Blitz rounds the box height to whole px (up to 0.4 pt), hence the
    // tolerances.
    for (l, r, cy, tol) in [
        ("LT", "RT", 49.225, 0.5),
        ("LM", "RM", 421.225, 1.0),
        ("LB", "RB", 793.225, 1.25),
    ] {
        let (l, r) = (word(&all, l), word(&all, r));
        near(l.cy(), cy, tol, &format!("`{}` y", l.text));
        near(r.cy(), cy, tol, &format!("`{}` y", r.text));
        near(l.cx(), 35.24, 0.5, &format!("`{}` x", l.text));
        near(r.cx(), 559.84, 0.5, &format!("`{}` x", r.text));
    }
}

// --- P9: the corners ------------------------------------------------------------------------------------------

#[test]
fn t09_corner_boxes() {
    let pdf = render(
        "size:A4;margin:15mm 25mm;",
        &format!(
            "{}{}{}{}",
            bx("top-left-corner", "\"TLC\"", ""),
            bx("top-right-corner", "\"TRC\"", ""),
            bx("bottom-left-corner", "\"BLC\"", ""),
            bx("bottom-right-corner", "\"BRC\"", ""),
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    // Chrome P9: TLC 53.27..70.49, BLC 53.20..70.50 (right aligned in the 70.87 pt corner), TRC / BRC start at 524.41
    // (left aligned), all at y 15.28..26.17 / 815.53..826.42.
    for t in ["TLC", "BLC"] {
        near(word(&all, t).x1, 70.49, 0.5, &format!("`{t}` right edge"));
    }
    for t in ["TRC", "BRC"] {
        near(word(&all, t).x0, 524.41, 0.5, &format!("`{t}` left edge"));
    }
    for t in ["TLC", "TRC"] {
        centred_in_band(&word(&all, t), CHROME_P1_CY, TOP15, 0.5);
    }
    for t in ["BLC", "BRC"] {
        centred_in_band(&word(&all, t), CHROME_P2_CY, BOTTOM15, 1.0);
    }
}

// --- P13 / F110: the centre box -------------------------------------------------------------------------------

/// Chrome P13: CENTRE13 alone and between LEFT13 / RIGHT13: 275.51..320.50; RIGHT13 ends at 553.50.
const CHROME_CENTRE_CX: f32 = 298.005;

#[test]
fn t10_centre_box_centred_horizontally() {
    let alone = render(
        "size:A4;margin:15mm;",
        &bx("bottom-center", "\"CENTRE13\"", ""),
        BODY,
    );
    near(
        word(&margin_words(&alone), "CENTRE13").cx(),
        CHROME_CENTRE_CX,
        0.5,
        "alone",
    );
    let three = render(
        "size:A4;margin:15mm;",
        &format!(
            "{}{}{}",
            bx("bottom-left", "\"LEFT13\"", ""),
            bx("bottom-center", "\"CENTRE13\"", ""),
            bx("bottom-right", "\"RIGHT13\"", "")
        ),
        BODY,
    );
    let all = margin_words(&three);
    near(
        word(&all, "CENTRE13").cx(),
        CHROME_CENTRE_CX,
        0.5,
        "between left and right",
    );
    near(
        word(&all, "LEFT13").x0,
        42.52,
        0.5,
        "left box starts at the content edge",
    );
    near(
        word(&all, "RIGHT13").x1,
        553.50,
        1.0,
        "right box ends at the content edge",
    );
}

// --- P14: the shape of fixture 02 -----------------------------------------------------------------------------

#[test]
fn t11_right_box_ends_at_content_edge() {
    let pdf = render(
        "size:A4;margin:15mm 15mm 20mm 15mm;",
        &format!(
            "{}{}",
            bx("bottom-right", "\"Strana 1 / 1\"", ""),
            bx("bottom-left", "\"Faktura left\"", "")
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    // Chrome P14: "Faktura" starts at 42.52, "Strana 1 / 1" ends at 553.50; both at y 808.03..818.92.
    near(word(&all, "Faktura").x0, 42.52, 0.5, "left box");
    let strana: Vec<&Word> = all
        .iter()
        .filter(|w| w.y0 > 780.0 && w.x0 > 400.0)
        .collect();
    assert!(!strana.is_empty(), "no Strana in {all:?}");
    let end = strana.iter().map(|w| w.x1).fold(0.0, f32::max);
    near(end, 553.50, 1.0, "right box ends at the right content edge");
    near(
        word(&all, "Faktura").cy(),
        813.475,
        0.5,
        "20 mm band, centred",
    );
}

// --- P12: author text-align wins ------------------------------------------------------------------------------

#[test]
fn t12_author_text_align_wins() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &bx("bottom-center", "\"BC12\"", "text-align:left"),
        BODY,
    );
    near(
        word(&margin_words(&pdf), "BC12").x0,
        42.52,
        0.5,
        "author `text-align: left` on a centre box",
    );
}

// --- P10: element() footer ------------------------------------------------------------------------------------

const RUNNING_PAGE: &str = "size:A4;margin:15mm;";
const RUNNING_BOX: &str = "@bottom-center{content:element(paticka)}";
const RUNNING_CSS: &str = ".paticka{position:running(paticka)}";
const RUNNING_BODY: &str = "<footer class=\"paticka border-t border-gray-300 pt-2 text-xs text-gray-500\">Footer <a href=\"https://www.example.cz\">link RUN10</a></footer><p>Body text of the page.</p>";

/// Link annotation rectangles `[x0, y0, x1, y1]` in PDF user space (origin bottom left). Annotation dictionaries
/// are not compressed.
fn link_rects(pdf: &[u8]) -> Vec<[f32; 4]> {
    let text = String::from_utf8_lossy(pdf);
    let mut found = Vec::new();
    for (i, _) in text.match_indices("/Subtype /Link") {
        let rest = &text[i..];
        if let Some(r) = rest.find("/Rect [") {
            let nums: Vec<f32> = rest[r + 7..]
                .split(']')
                .next()
                .unwrap()
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            found.push([nums[0], nums[1], nums[2], nums[3]]);
        }
    }
    found
}

#[test]
fn t13_running_footer_centred() {
    let pdf = render_with(RUNNING_PAGE, RUNNING_BOX, RUNNING_CSS, RUNNING_BODY, false);
    let all = margin_words(&pdf);
    let w = word(&all, "RUN10");
    // Main printed this footer with its top edge on the band top (the link word at y 806.40). Its block is the
    // 0.75 pt rule + 6 pt padding + one 12 pt line = 18.75 pt, centred in the 42.52 pt band: 11.89 pt lower.
    // (Chrome cannot show an element(); the shape cannot be compared with its string box.)
    near(
        w.y0,
        806.40 + (15.0 * MM - 18.75) / 2.0,
        0.75,
        "footer line top",
    );
    let rects = link_rects(&pdf);
    assert_eq!(rects.len(), 1, "one link annotation: {rects:?}");
    let r = rects[0];
    let (top, bottom) = (PAGE_H - r[3], PAGE_H - r[1]);
    assert!(
        r[0] <= w.x0 + 0.5 && r[2] >= w.x1 - 0.5 && top <= w.y0 + 0.5 && bottom >= w.y1 - 0.5,
        "link rect x {:.2}..{:.2} y {:.2}..{:.2} must contain the word {:?}",
        r[0],
        r[2],
        top,
        bottom,
        w
    );
}

// --- P11: counter(page) ---------------------------------------------------------------------------------------

#[test]
fn t14_counter_footer_same_y_every_page() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &bx(
            "bottom-center",
            "\"Strana \" counter(page) \" / \" counter(pages)",
            "",
        ),
        "<p>One</p><p style=\"break-before:page\">Two</p><p style=\"break-before:page\">Three</p>",
    );
    let all = margin_words(&pdf);
    for page in 1..=3 {
        let on: Vec<&Word> = all
            .iter()
            .filter(|w| w.page == page && w.y0 > 700.0)
            .collect();
        assert_eq!(on.len(), 4, "page {page}: {on:?}");
        for w in &on {
            near(
                w.cy(),
                CHROME_P2_CY,
                1.0,
                &format!("page {page} `{}` y", w.text),
            );
        }
        // Chrome P11: the string spans 274.20..321.80 on page 1, centre 298.0 on every page.
        let (x0, x1) = (
            on.iter().map(|w| w.x0).fold(f32::MAX, f32::min),
            on.iter().map(|w| w.x1).fold(0.0, f32::max),
        );
        near(
            (x0 + x1) / 2.0,
            298.0,
            0.5,
            &format!("page {page} string centre"),
        );
    }
}

// --- P10bg: background and border fill the band ---------------------------------------------------------------

/// Gray value of the pixel at `(x, y)` of the page rasterised at `dpi`.
fn gray_pixel(pdf: &[u8], dpi: u32, x: u32, y: u32) -> u8 {
    let path = temp(pdf, "pdf");
    let out = std::process::Command::new("pdftoppm")
        .args(["-gray", "-r", &dpi.to_string(), "-f", "1", "-l", "1"])
        .args([
            "-x",
            &x.to_string(),
            "-y",
            &y.to_string(),
            "-W",
            "1",
            "-H",
            "1",
        ])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftoppm failed");
    // Binary PGM: "P5\n1 1\n255\n" then the byte.
    *out.stdout.last().expect("pixel")
}

#[test]
fn t15_margin_box_background_fills_band() {
    let pdf = render(
        "size:A4;margin:15mm;",
        &bx(
            "top-center",
            "\"BG10\"",
            "border-bottom:1px solid #000;background:#eee",
        ),
        BODY,
    );
    // 288 dpi: 1 pt = 4 px. The band is 0..42.52 pt = 0..170 px; the rule is the last 3 px of it. x = 300 pt.
    let x = 1200;
    for y in [4, 40, 100, 160] {
        let g = gray_pixel(&pdf, 288, x, y);
        assert!(
            (230..=245).contains(&g),
            "y = {y} px: gray {g}, want the #eee background"
        );
    }
    let g = gray_pixel(&pdf, 288, x, 168);
    assert!(g < 60, "y = 168 px: gray {g}, want the black bottom border");
    let g = gray_pixel(&pdf, 288, x, 200);
    assert!(
        g > 250,
        "y = 200 px (below the band): gray {g}, want the white page"
    );
}

// --- tagged structure -----------------------------------------------------------------------------------------

#[test]
fn t16_tagged_struct_count_unchanged() {
    let pdf = render_with(RUNNING_PAGE, RUNNING_BOX, RUNNING_CSS, RUNNING_BODY, true);
    let text = String::from_utf8_lossy(&pdf);
    // Main (84f15a4): Document + P. Margin content is not tagged (F20); the wrapper must not add Table / TR / TD.
    assert_eq!(text.matches("/StructElem").count(), 2);
    for tag in ["/S /Table", "/S /TR", "/S /TD", "/S /TH"] {
        assert!(!text.contains(tag), "{tag} appeared in the structure tree");
    }
}

// --- guards found while moving the fork's goldens ---------------------------------------------------------------

#[test]
fn t17_leader_keeps_the_page_number_inside_the_box() {
    // `leader()` makes the box a flex container whose fill item holds 400 characters (`flex: 1; min-width: 0`).
    // In a cell that took its width from its content the fill stretched the cell and pushed the number out of the
    // box (fork VRT `gcpm/leader-toc`). The number stays at the right edge of the box: A5, 50 pt margins.
    let pdf = render(
        "size:A5;margin:50pt;",
        &bx(
            "top-right",
            "\"Introduction\" leader(dotted) counter(page)",
            "",
        ),
        BODY,
    );
    let all = margin_words(&pdf);
    let n = word(&all, "1");
    let right_edge = 419.53 - 50.0;
    assert!(
        n.x1 <= right_edge + 1.0 && n.x1 >= right_edge - 1.0,
        "page number ends at {:.2}, want the box edge {right_edge:.2}",
        n.x1
    );
    near(
        word(&all, "Introduction").x0,
        50.0,
        1.0,
        "title at the left edge",
    );
}

#[test]
fn t18_author_margin_on_a_margin_box_does_not_hide_it() {
    // A table cell has no margins; Blitz places one with a `margin-top` outside its row and the box was not painted
    // at all (fork example `header-footer-split`, `@top-right { margin-top: 12px }`). The margin is ignored and the
    // text stays centred in the band, like the same box without it.
    let plain = render(
        "size:A4;margin:15mm;",
        &bx("top-right", "\"MARG\"", ""),
        BODY,
    );
    let y = word(&margin_words(&plain), "MARG").cy();
    for decl in [
        "margin-top:12px",
        "margin:5px 3px 4px 2px",
        "margin-bottom:9px",
    ] {
        let pdf = render(
            "size:A4;margin:15mm;",
            &bx("top-right", "\"MARG\"", decl),
            BODY,
        );
        near(
            word(&margin_words(&pdf), "MARG").cy(),
            y,
            0.5,
            &format!("`{decl}`: the box is painted where it is without it"),
        );
    }
}
