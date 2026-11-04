//! An `inline-block` that is alone on its line, with a short block child, is moved to the next page whole when it does
//! not fit the room and fits a page; it is not left on page 1 with its content painted below the page edge (F213d).
//!
//! css-break-3 §4.1 and §4.4: there is no break point inside a line box, and a UA may treat an inline-level box that
//! establishes a formatting context (`inline-block`, `inline-table`) as monolithic; monolithic content that does not fit
//! the room is pushed to the next fragmentainer through the Class A break in front of its line, and only monolithic
//! content taller than a fragmentainer is sliced. CSS 2.1 §13.3.3: breaks occur between block boxes and between line
//! boxes, never inside a line box. Before the fix the fragmenter entered the carrier `<p>` because the box's first row
//! fits the room, left it whole on page 1 and painted the box and its text below the page edge: nothing of it was
//! printed and nothing was reported. Plan: docs/plans/2026-11-03-f213d-alone-inline-block-lost.md.
//!
//! The page is A4 with 15 mm margins (content box 1009.13 px), `body { font: 14px/20px Inter }`. `room(R)` is a block
//! that leaves R px below it. `alone(..)` is `<p>BOX</p><p>AFTER</p>`: a 150 px wide `inline-block` with a 1 px border
//! alone in its paragraph. Every page number is Chrome's (version of the committed references, measured with the
//! settings of `scripts/reference/render.mjs` and `pdftotext -bbox`, plan section 7). The y positions are not compared
//! with Chrome's: ours seats an inline box bottom-on-baseline (F183c), so a fixed-height box and its line sit lower.
//!
//! The tests named `r_*` are red before the fix; the `g_*` tests are guards that hold before and after.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";
/// Bottom of the content box in pt: 841.89 - 15 mm.
const CONTENT_BOTTOM_PT: f32 = 841.89 - 15.0 / 25.4 * 72.0;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// One printed word: its 1-based page, the top and bottom of its box in pt from the page top, and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
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

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-aibl-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// The single printed `text`: panics when it is not printed (a lost word) or printed twice.
fn find(words: &[Word], text: &str) -> Word {
    let hits: Vec<&Word> = words.iter().filter(|w| w.text == text).collect();
    assert_eq!(
        hits.len(),
        1,
        "`{text}` must be printed exactly once, found {} (all words: {:?})",
        hits.len(),
        words.iter().map(|w| (w.page, &w.text)).collect::<Vec<_>>()
    );
    hits[0].clone()
}

fn render_css(page_css: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{page_css}body{{font:14px/20px Inter;margin:0}}p{{margin:0}}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
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

/// A block that leaves `r` px of room below it on page 1 (the content box is 1009.13 px).
fn room(r: u32) -> String {
    format!("<div style=\"height:{}px\"></div>", 1009 - r)
}

const BOXTOP: &str = "<span style=\"display:block\">BOXTOP</span>";

/// `n` one-line block children `L01`, `L02`, ...
fn blocks(n: u32) -> String {
    (1..=n)
        .map(|i| format!("<span style=\"display:block\">L{i:02}</span>"))
        .collect()
}

/// `n` lines of text `L01<br>L02...` (the box itself is the inline formatting context).
fn br_lines(n: u32) -> String {
    (1..=n)
        .map(|i| format!("L{i:02}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn texts(prefix: &str, from: u32, to: u32) -> Vec<String> {
    (from..=to).map(|i| format!("{prefix}{i:02}")).collect()
}

/// An atomic box (`tag` is its `display`) 150 px wide with a 1 px border; `height` is `Some(px)` or auto.
fn boxed(tag: &str, height: Option<u32>, extra: &str, inner: &str) -> String {
    let h = height.map(|h| format!("height:{h}px;")).unwrap_or_default();
    format!(
        "<span style=\"display:{tag};width:150px;{h}border:1px solid #000;{extra}\">{inner}</span>"
    )
}

/// `<p>BOX</p><p>AFTER</p>`.
fn alone(box_html: &str) -> String {
    format!("<p>{box_html}</p><p>AFTER</p>")
}

/// Every word of `texts` is printed once, on page `page`, inside the content box.
fn assert_all_on_page(words: &[Word], page: usize, names: &[&str]) {
    for t in names {
        let w = find(words, t);
        assert_eq!(
            w.page, page,
            "`{t}` is on page {}, Chrome has it on page {page}",
            w.page
        );
        assert!(
            w.y_max <= CONTENT_BOTTOM_PT + 5.0,
            "`{t}` ends at {:.1} pt, below the content box ({CONTENT_BOTTOM_PT:.1} pt)",
            w.y_max
        );
    }
}

// --- red before the fix ------------------------------------------------------------------------------------------------

#[test]
fn r_a_fixed_height_box_alone_on_its_line_moves_to_the_next_page() {
    // Q1: `room(R)<p>[300 px box with a block child BOXTOP]</p><p>AFTER</p>`. Chrome: for every R below 302 the box
    // moves to page 2 (BOXTOP at 41.7 pt, AFTER 267.4 pt), 2 pages. Main: the box is painted below the page edge for R
    // from 21 px (the box's first row: border and one line) up to 199; BOXTOP is printed nowhere.
    for r in [60, 120, 199] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed("inline-block", Some(300), "", BOXTOP))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert!(
            find(&words, "AFTER").y_min > find(&words, "BOXTOP").y_min,
            "room {r}: AFTER follows the line that holds the box"
        );
        assert_eq!(page_count(&pdf), 2, "room {r}: two pages like Chrome");
    }
}

#[test]
fn r_the_same_with_vertical_align_and_text_align() {
    // Q8 / Q9. Chrome: page 2 for `vertical-align` top / middle / bottom and for `text-align` right / center.
    for (name, para_style, box_style) in [
        ("vertical-align:top", "", "vertical-align:top"),
        ("vertical-align:middle", "", "vertical-align:middle"),
        ("text-align:right", "text-align:right", ""),
        ("text-align:center", "text-align:center", ""),
    ] {
        let pdf = render(&format!(
            "{}<p style=\"{para_style}\">{}</p><p>AFTER</p>",
            room(120),
            boxed("inline-block", Some(300), box_style, BOXTOP)
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "{name}: two pages like Chrome");
    }
}

#[test]
fn r_a_box_with_several_block_children_moves_whole() {
    // Q4: the shape of fixture 44. A fixed box of 196 px: a caption, a 72 px `<svg>` with margins, a name. Chrome: for
    // every room below 198 px the whole box is on page 2 (CAPTION 41.7 pt, NAME 122.7 pt). Main: for room 40 and 100 the
    // box is cut (CAPTION on page 1, NAME on page 2), for 130 NAME is painted below the edge, for 150 to 195 the box is
    // left on page 1 with its bottom in the page margin.
    let svg = "<svg style=\"display:block;margin:8px 0\" width=\"72\" height=\"72\" viewBox=\"0 0 100 100\">\
        <circle cx=\"50\" cy=\"50\" r=\"40\" fill=\"none\" stroke=\"#000\"/></svg>";
    let inner = format!(
        "<span style=\"display:block\">CAPTION</span>{svg}<span style=\"display:block\">NAME</span>"
    );
    for r in [40, 100, 130, 150, 170, 190] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed("inline-block", Some(196), "", &inner))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["CAPTION", "NAME", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "room {r}: two pages like Chrome");
    }
}

#[test]
fn r_the_same_in_a_section_a_card_a_cell_and_a_flex_item() {
    // Q6, room 120. Chrome: BOXTOP and AFTER on page 2 in every nesting. In `<section><p>x</p>...</section>` `x` stays
    // on page 1 (707.7 pt) and the paragraph with the box moves.
    let inner = format!(
        "<p>{}</p><p>AFTER</p>",
        boxed("inline-block", Some(300), "", BOXTOP)
    );
    let card = "border:1px solid #ccc;padding:16px;border-radius:8px";
    let cases: Vec<(&str, String, bool)> = vec![
        ("section", format!("<section>{inner}</section>"), false),
        (
            "card",
            format!("<div style=\"{card}\">{inner}</div>"),
            false,
        ),
        (
            "overflow-hidden card",
            format!("<div style=\"overflow:hidden;{card}\">{inner}</div>"),
            false,
        ),
        (
            "section after a paragraph",
            format!("<section><p>x</p>{inner}</section>"),
            true,
        ),
        (
            "table cell",
            format!(
                "<table style=\"border-collapse:collapse;width:100%\"><tr><td>{inner}</td></tr></table>"
            ),
            false,
        ),
        (
            "flex item",
            format!("<div style=\"display:flex\"><div style=\"flex:1\">{inner}</div></div>"),
            false,
        ),
    ];
    for (name, html, keeps_x) in cases {
        let pdf = render(&format!("{}{html}", room(120)));
        let words = printed_words(&pdf);
        if keeps_x {
            assert_all_on_page(&words, 1, &["x"]);
        }
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "{name}: two pages like Chrome");
    }
}

#[test]
fn r_an_inline_table_alone_that_fits_a_page_moves_whole() {
    // Q7: an `inline-table` built from `display:table-row` / `table-cell` spans, 15 rows (300 px), room 120. Chrome: all
    // 15 rows and AFTER on page 2. Main splits it between rows (5 rows on page 1, 10 on page 2).
    let rows: String = (1..=15)
        .map(|i| {
            format!(
                "<span style=\"display:table-row\"><span style=\"display:table-cell\">L{i:02}</span></span>"
            )
        })
        .collect();
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-table", None, "", &rows))
    ));
    let words = printed_words(&pdf);
    let mut names: Vec<String> = texts("L", 1, 15);
    names.push("AFTER".into());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_all_on_page(&words, 2, &names);
    assert_eq!(page_count(&pdf), 2, "two pages like Chrome");
}

#[test]
fn r_an_auto_height_box_that_fits_a_page_moves_whole() {
    // Q3: an auto-height box of 15 block children (302 px), room 120. Chrome moves it whole: L01 to L15 and AFTER on page
    // 2 (L01 41.7 pt, AFTER 267.4 pt). It is monolithic and fits a fresh page (css-break-3 §4.4). Main enters it and
    // splits it between its children: L01 to L05 on page 1, L06 to L15 on page 2 (ADR-0004, which this finding amends
    // for boxes that fit a page). The same holds for `min-height` and for an auto box with a tall padding.
    let mut names: Vec<String> = texts("L", 1, 15);
    names.push("AFTER".into());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-block", None, "", &blocks(15)))
    ));
    let words = printed_words(&pdf);
    assert_all_on_page(&words, 2, &names);
    assert_eq!(page_count(&pdf), 2, "two pages like Chrome");

    // A fixed 300 px box filled by 15 block children: the same.
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-block", Some(300), "", &blocks(15)))
    ));
    let words = printed_words(&pdf);
    assert_all_on_page(&words, 2, &names);

    // `min-height: 300px` with one short child, and an auto box with `padding-bottom: 250px`: Chrome page 2 for both.
    for (name, extra) in [
        ("min-height", "min-height:300px"),
        ("padding-bottom", "padding-bottom:250px"),
    ] {
        let pdf = render(&format!(
            "{}{}",
            room(120),
            alone(&boxed("inline-block", None, extra, BOXTOP))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "{name}: two pages like Chrome");
    }
}

// --- guards: green before and after ------------------------------------------------------------------------------------

#[test]
fn g_a_box_whose_first_row_does_not_fit_the_room_already_moves() {
    // Q1 at room 10 (and 20): the box's first row (21 px) does not fit, so main moves the carrier. Chrome: page 2.
    for r in [10, 20] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed("inline-block", Some(300), "", BOXTOP))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "room {r}");
    }
}

#[test]
fn g_a_box_that_fits_the_room_stays_on_page_one() {
    // Q2 at h = 100 (box 102 px, room 120): Chrome keeps it on page 1 (BOXTOP 708.4 pt) and puts AFTER on page 2.
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-block", Some(100), "", BOXTOP))
    ));
    let words = printed_words(&pdf);
    assert_all_on_page(&words, 1, &["BOXTOP"]);
    assert_all_on_page(&words, 2, &["AFTER"]);
}

#[test]
fn g_an_inline_flex_box_alone_moves_whole() {
    // Q7: F213 moves an `inline-flex` box (it is not an inline-block). Chrome: page 2, at room 120 and 10.
    for r in [10, 120] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed("inline-flex", Some(300), "", BOXTOP))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "room {r}");
    }
}

#[test]
fn g_a_box_that_shares_its_line_keeps_f213() {
    // Q5, room 120. Chrome: page 2 in all four. `Úvod<br>BOX` has the box on line 2 of 2.
    for (name, para, words_on_2) in [
        (
            "text, box, text",
            format!(
                "START {} TAIL",
                boxed("inline-block", Some(300), "", BOXTOP)
            ),
            vec!["START", "BOXTOP", "TAIL", "AFTER"],
        ),
        (
            "box, text",
            format!("{} TAIL", boxed("inline-block", Some(300), "", BOXTOP)),
            vec!["BOXTOP", "TAIL", "AFTER"],
        ),
        (
            "text, box",
            format!("START {}", boxed("inline-block", Some(300), "", BOXTOP)),
            vec!["START", "BOXTOP", "AFTER"],
        ),
        (
            "the box on line 2 of 2",
            format!("Úvod<br>{}", boxed("inline-block", Some(300), "", BOXTOP)),
            vec!["Úvod", "BOXTOP", "AFTER"],
        ),
    ] {
        let pdf = render(&format!("{}<p>{para}</p><p>AFTER</p>", room(120)));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &words_on_2);
        assert_eq!(page_count(&pdf), 2, "{name}: two pages like Chrome");
    }
}

#[test]
fn g_a_box_taller_than_a_page_with_block_lines_prints_every_line_once() {
    // Q2 / Q3 shape that ADR-0004 keeps: an auto-height box of 70 block lines (1402 px, taller than the 1009 px page), room
    // 120. Chrome slices it (lines 1 to 50 on page 2, the rest on page 3; one line is printed twice at the slice). Main
    // splits it between its lines: every line once, 5 on page 1. The fix must not move this: it does not fit a page.
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-block", None, "", &blocks(70)))
    ));
    let words = printed_words(&pdf);
    for t in texts("L", 1, 70)
        .iter()
        .map(String::as_str)
        .chain(["AFTER"])
    {
        let _ = find(&words, t);
    }
    assert_eq!(
        find(&words, "L01").page,
        1,
        "ADR-0004: the box is split between its lines, the first lines stay on page 1"
    );
}

#[test]
fn g_a_box_whose_content_overflows_it_visibly_prints_every_line_once() {
    // Q3: a fixed 300 px box with 40 block children (800 px of content overflowing it visibly, which fits a page), room
    // 120. Chrome moves it whole (L01 to L40 on page 2). Main splits the overflowing content between its lines (5 on page
    // 1, 35 on page 2): every line once, so none is lost. After the fix it must stay on this path: moving the box alone
    // would leave its overflow behind (plan section 3, `content_ends_inside`).
    let pdf = render(&format!(
        "{}{}",
        room(120),
        alone(&boxed("inline-block", Some(300), "", &blocks(40)))
    ));
    let words = printed_words(&pdf);
    for t in texts("L", 1, 40)
        .iter()
        .map(String::as_str)
        .chain(["AFTER"])
    {
        let _ = find(&words, t);
    }
}

#[test]
fn g_a_box_of_text_lines_moves_whole_on_main_already() {
    // Q3: an auto box whose content is text lines (`L01<br>L02...`, the box is the inline formatting context), 15 lines,
    // room 120 and 10. Chrome: page 2. Main already moves it whole, because the carrier path does not enter a box that
    // has no block child. (A fixed 300 px box with 40 such lines loses L01 to L22 above the top of page 2 on main, a
    // different bug of the F183c family: the overflow is seated above the box.)
    let mut names: Vec<String> = texts("L", 1, 15);
    names.push("AFTER".into());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    for r in [10, 120] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed("inline-block", None, "", &br_lines(15)))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &names);
    }
}

#[test]
fn g_a_fixed_height_inline_table_alone_moves_whole_already() {
    // Q7: an `inline-table` of 300 px with one `table-row` / `table-cell` child, room 120 and 10. Chrome: page 2. Main
    // already moves it (its first row, a 300 px table box, does not fit); an auto-height one of 15 rows is the red test.
    for r in [10, 120] {
        let pdf = render(&format!(
            "{}{}",
            room(r),
            alone(&boxed(
                "inline-table",
                Some(300),
                "",
                "<span style=\"display:table-row\"><span style=\"display:table-cell\">BOXTOP</span></span>"
            ))
        ));
        let words = printed_words(&pdf);
        assert_all_on_page(&words, 2, &["BOXTOP", "AFTER"]);
        assert_eq!(page_count(&pdf), 2, "room {r}");
    }
}

// --- F213d review round 1: the last pixel of a page, and a margin below the last child -------------------------------

/// Every `L01`..`Lnn` and `AFTER` printed once.
fn assert_every_line_printed(words: &[Word], n: u32, what: &str) {
    for i in 1..=n {
        let t = format!("L{i:02}");
        let hits = words.iter().filter(|w| w.text == t).count();
        assert_eq!(hits, 1, "{what}: `{t}` is printed {hits} times");
    }
    let hits = words.iter().filter(|w| w.text == "AFTER").count();
    assert_eq!(hits, 1, "{what}: `AFTER` is printed {hits} times");
}

#[test]
fn r_a_box_in_the_last_pixel_of_a_page_loses_no_line_in_any_wrapper() {
    // Validator HIGH-1: an alone box whose outer height is within about 1.2 px above the content height (1009.13 px) did not
    // fit a page, but the one pixel of slack of the strip slicer counted it as fitting: the carrier was moved and a
    // `section`, grid, flex or table-cell wrapper placed it whole, below the page edge. Every line of the box was lost; main
    // splits the box between its lines and prints all of them. Chrome puts the box on page 2 (its fragmentainer is 1010 px).
    // The box is a 300 px wide `border-box` with 30 block lines, 30 px of room.
    let lines: String = (1..=30)
        .map(|i| format!("<span style=\"display:block\">L{i:02}</span>"))
        .collect();
    let wrappers: [(&str, &str); 5] = [
        ("body", "{}"),
        ("section", "<section><p>{}</p></section>"),
        ("flex", "<div style=\"display:flex\"><div>{}</div></div>"),
        ("grid", "<div style=\"display:grid\"><div>{}</div></div>"),
        ("td", "<table><tr><td>{}</td></tr></table>"),
    ];
    for height in ["267.1mm", "267.3mm", "1010px", "1011px", "1009px", "267mm"] {
        for (name, wrapper) in wrappers {
            let bx = format!(
                "<span style=\"display:inline-block;width:300px;box-sizing:border-box;border:1px solid #000;height:{height}\">{lines}</span>"
            );
            let html = wrapper.replace("{}", &bx);
            let pdf = render(&format!("{}{html}<p>AFTER</p>", room(30)));
            let words = printed_words(&pdf);
            assert_every_line_printed(&words, 30, &format!("{name}, height {height}"));
        }
    }
}

#[test]
fn r_an_auto_box_whose_last_child_has_a_bottom_margin_moves_whole() {
    // Validator MEDIUM-1: Blitz's auto height omits the margin below the last in-flow child, so `content_ends_inside` saw the
    // content overflow the box by that margin and the box was split between its children; Chrome moves it whole (a margin
    // is blank space, and an `inline-block` is a BFC root). Room 100 px; a default `<ul>` of 10 items (232 px, 16 px margins),
    // and 8 paragraphs with `margin: 0 0 16px`.
    let ul = format!(
        "<ul>{}</ul>",
        (1..=10)
            .map(|i| format!("<li>L{i:02}</li>"))
            .collect::<String>()
    );
    let ps: String = (1..=8)
        .map(|i| format!("<p style=\"margin:0 0 16px\">L{i:02}</p>"))
        .collect();
    for (name, inner, n) in [("ul", ul, 10), ("p with a margin below", ps, 8)] {
        let pdf = render(&format!(
            "{}<div><span style=\"display:inline-block;width:300px\">{inner}</span></div><p>AFTER</p>",
            room(100)
        ));
        let words = printed_words(&pdf);
        assert_every_line_printed(&words, n, name);
        for i in 1..=n {
            let t = format!("L{i:02}");
            assert_eq!(
                find(&words, &t).page,
                2,
                "{name}: `{t}` is on page 1, Chrome moves the box whole to page 2"
            );
        }
    }
}

#[test]
fn r_a_page_tall_box_behind_a_margined_ancestor_at_the_page_top_loses_no_line() {
    // Review round 2 (HIGH-1b): an auto-height box about a page tall, first on the page (no room made before it), behind
    // `ul > li`, `blockquote > p`, a `div` with a margin, a paragraph with `margin-top`, `dl > dd` or `figure`. The margin that
    // collapses through the body makes the first page's strip smaller; a box that fills a page cannot move off the first page, so it
    // was placed whole and its last one or two lines were painted below the edge (with a warning). Main splits it and prints
    // every line. `@page { margin: 0 }`: the content box is the sheet, 1122.5 px. The box is `n` blocks of 20 px and a spacer,
    // 1 px under the page, 4 px under it and 1 px over it.
    let page_h = 297.0 / 25.4 * 96.0;
    let wrappers: [(&str, &str); 6] = [
        ("ul > li", "<ul><li>{}</li></ul>"),
        ("blockquote > p", "<blockquote><p>{}</p></blockquote>"),
        (
            "div with a margin",
            "<div style=\"margin:16px 0\"><p>{}</p></div>",
        ),
        ("p with margin-top", "<p style=\"margin-top:24px\">{}</p>"),
        ("dl > dd", "<dl><dd>{}</dd></dl>"),
        ("figure", "<figure><p>{}</p></figure>"),
    ];
    for delta in [-1.0_f32, -4.0, 1.0] {
        let h = page_h + delta;
        let n = (h / 20.0).floor() as u32;
        let pad = h - n as f32 * 20.0 - 2.0;
        let lines: String = (0..n)
            .map(|i| format!("<span style=\"display:block\">F{i:02}</span>"))
            .collect();
        let bx = format!(
            "<span style=\"display:inline-block;width:200px;border:1px solid #000\">{lines}<span style=\"display:block;height:{pad:.2}px\"></span></span>"
        );
        for (name, wrapper) in wrappers {
            let pdf = render_css(
                "@page{size:A4;margin:0}",
                &format!("{}<p>AFTER</p>", wrapper.replace("{}", &bx)),
            );
            let words = printed_words(&pdf);
            for i in 0..n {
                let t = format!("F{i:02}");
                let hits = words.iter().filter(|w| w.text == t).count();
                assert_eq!(
                    hits, 1,
                    "{name}, box {delta:+} px from the page: `{t}` is printed {hits} times"
                );
            }
        }
    }
}

#[test]
#[ignore = "F183c: red until step 3 (Blitz reports the baseline of the box, fulgur stops moving it a second time)"]
fn f183c_the_y_of_the_moved_boxes_is_chromes() {
    // The y that this file leaves uncompared "because of F183c" (see the header), with Chrome m153's numbers measured with
    // the bundled Inter TTF through `@font-face`, so that `pdftotext` reports the metrics of our font (plan section 7).
    // Room 120, box moved to page 2; within 1 px (0.75 pt). `(name, html, [(word, y on page 2)])`.
    let l15: Vec<(String, f32)> = (1..=15)
        .map(|i| (format!("L{i:02}"), 44.6 + 15.0 * (i - 1) as f32))
        .collect();
    type Expect = Vec<(String, f32)>;
    let mut cases: Vec<(&str, String, Expect)> = vec![
        (
            "Q1: 300 px box, a block child",
            alone(&boxed("inline-block", Some(300), "", BOXTOP)),
            vec![("BOXTOP".into(), 44.6), ("AFTER".into(), 270.3)],
        ),
        (
            "Q7: 300 px inline-table",
            alone(&boxed(
                "inline-table",
                Some(300),
                "",
                "<span style=\"display:table-row\"><span style=\"display:table-cell\">BOXTOP</span></span>",
            )),
            vec![("BOXTOP".into(), 44.6), ("AFTER".into(), 270.3)],
        ),
    ];
    let mut text_lines = l15.clone();
    text_lines.push(("AFTER".into(), 270.3));
    cases.push((
        "Q3: auto box of 15 text lines",
        alone(&boxed("inline-block", None, "", &br_lines(15))),
        text_lines.clone(),
    ));
    cases.push((
        "Q4: auto box of 15 block children",
        alone(&boxed("inline-block", None, "", &blocks(15))),
        text_lines,
    ));
    let mut bad = Vec::new();
    for (name, html, expect) in cases {
        let words = printed_words(&render(&format!("{}{html}", room(120))));
        for (t, y) in expect {
            let w = find(&words, &t);
            if w.page != 2 || (w.y_min - y).abs() > 0.75 {
                bad.push(format!(
                    "{name}: `{t}` at {:.1} pt on page {}, Chrome {y:.1} pt on page 2",
                    w.y_min, w.page
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "differences from Chrome m153:\n{}",
        bad.join("\n")
    );
}
