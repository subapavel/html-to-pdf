//! A paragraph taller than a page in an `inline-block` or a clipping (`overflow` not `visible`) wrapper keeps all of its
//! text (F30f).
//!
//! The fragmenter split such a paragraph between its line boxes nowhere: it placed the `<p>` whole, the lines below the
//! page were drawn below it and what landed in the bottom margin was printed, the rest lost (386 of 4000 words). An
//! `inline-block` that does split (direct text, a body-level one) was painted only by the paragraph that carries it, on
//! the page of that paragraph's single line, so its later pages were empty.
//!
//! Spec: css-break-3 §2 and §4.1 (what a UA may treat as monolithic: `overflow: auto` / `scroll`, `overflow: hidden`
//! with a definite height, inline-level boxes that establish an independent formatting context), §4.4 (breaks between
//! line boxes, orphans and widows); css-overflow-3 §3 (a fragmented scroll container clips each fragment). Every
//! expectation was measured with headless Chromium (m141) first; the numbers are in the F30f plan §7. Chrome fragments
//! a clipping wrapper between lines, so the words are identical. It **slices** an `inline-block` instead: the line that
//! straddles the page edge is printed on both pages, half on each, and an `inline-block` that is not first on its page
//! starts the next one. We split between line boxes (ADR-0004), so every word is printed once, at most one line per
//! page differs from Chrome and the page count is Chrome's where the box starts a page.
//!
//! The page is A4 with 15 mm margins (content box 1009.1 px) and 20 px lines unless a test says otherwise. Words are
//! found with `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

/// A4 height in pt and the 15 mm page margin.
const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// A glyph box (ascent + descent) may stick out of its line box by a few pt below the content box.
const SLACK_BELOW_PT: f32 = 5.0;
const SLACK_ABOVE_PT: f32 = 2.0;

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}p{margin:0;line-height:20px}";
const HEAD_NORMAL: &str =
    "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:normal;margin:0}p{margin:0}";

/// The `inline-block` of R1, alone on its line.
const IB: &str = "display:inline-block;width:100%";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    render_with(HEAD, body)
}

fn render_with(head_style: &str, body: &str) -> Vec<u8> {
    let tailwind = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tailwind/tailwind.css");
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .css_file(tailwind)
        .unwrap()
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head_style}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Words `word<from> … word<to - 1>`.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The 4000 words of R1.
fn w() -> String {
    words(0, 4000)
}

/// One printed word: its 1-based page, left edge, the top and bottom of its box in pt (from the page top) and its text.
#[derive(Clone)]
struct Word {
    page: usize,
    x_min: f32,
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

/// The word `text` (first occurrence).
fn find(pdf: &[u8], text: &str) -> Word {
    printed_words(pdf)
        .into_iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("{text} is not printed"))
}

/// How many times `text` is printed.
fn count_of(pdf: &[u8], text: &str) -> usize {
    printed_words(pdf)
        .into_iter()
        .filter(|w| w.text == text)
        .count()
}

/// How many `wordNNNNN` are printed on each page, from page 1.
fn words_per_page(pdf: &[u8]) -> Vec<usize> {
    let mut per = vec![0; page_count(pdf)];
    for w in printed_words(pdf) {
        if w.text.starts_with("word") {
            per[w.page - 1] += 1;
        }
    }
    per
}

/// Pages of `pdf` with no printed word on them.
fn blank_pages(pdf: &[u8]) -> Vec<usize> {
    let printed = printed_words(pdf);
    (1..=page_count(pdf))
        .filter(|p| !printed.iter().any(|w| w.page == *p))
        .collect()
}

/// `n` words `wordNNNNN`, each exactly once, in order, inside the content box.
fn assert_every_word_once(what: &str, pdf: &[u8], n: usize) {
    let found: Vec<Word> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .collect();
    let first_diff = found
        .iter()
        .enumerate()
        .position(|(i, w)| w.text != format!("word{i:05}"));
    assert!(
        found.len() == n && first_diff.is_none(),
        "{what}: every word exactly once, in order: printed {} of {n}, first difference at index {first_diff:?}",
        found.len()
    );
    let bottom = PAGE_H_PT - MARGIN_PT + SLACK_BELOW_PT;
    let top = MARGIN_PT - SLACK_ABOVE_PT;
    for w in &found {
        assert!(
            w.y_max <= bottom,
            "{what}: {} on page {} ends at {:.1}pt, below the content box ({bottom:.1}pt)",
            w.text,
            w.page,
            w.y_max
        );
        assert!(
            w.y_min >= top,
            "{what}: {} on page {} starts at {:.1}pt, above the content box ({top:.1}pt)",
            w.text,
            w.page,
            w.y_min
        );
    }
    assert!(
        found.windows(2).all(|w| w[0].page <= w[1].page),
        "{what}: pages out of order"
    );
}

// ── A1: R1, an inline-block alone on its line ────────────────────────────────────────────────────────────────────────

#[test]
fn a_paragraph_in_an_inline_block_keeps_every_word() {
    // R1: 386 of 4000 words on 1 page before. Chrome slices the box: all words on 12 pages (the line at each break
    // twice, half on each page).
    let pdf = render(&format!("<div style=\"{IB}\"><p>{}</p></div>", w()));
    assert_every_word_once("A1", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

#[test]
fn direct_text_in_an_inline_block_is_printed_on_every_page() {
    // A1t: the line splitter already split this box (it holds its text directly), but only the paragraph that carries it
    // painted it, on page 1: 351 words on 12 pages, 11 of them empty.
    let pdf = render(&format!("<div style=\"{IB}\">{}</div>", w()));
    assert_every_word_once("A1t", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
    assert!(blank_pages(&pdf).is_empty());
}

#[test]
fn an_inline_block_with_line_height_normal_keeps_every_word() {
    // A1n: the validator's own shape (19.2 px advance, 20 px boxes).
    let pdf = render_with(
        HEAD_NORMAL,
        &format!("<div style=\"{IB}\"><p>{}</p></div>", w()),
    );
    assert_every_word_once("A1n", &pdf, 4000);
}

// ── A2: auto width, centred ──────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_inline_block_of_auto_width_keeps_every_word() {
    let pdf = render(&format!(
        "<div style=\"display:inline-block\"><p>{}</p></div>",
        w()
    ));
    assert_every_word_once("A2", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

#[test]
fn a_centred_inline_block_keeps_its_x_on_every_page() {
    // A2c: 60 % wide, centred (its text is left-aligned, so the first word of a page is the box's left edge). Every page
    // of the box starts at the same x (the box has one position; a continuation
    // page is drawn at the geometry's x, which relies on Taffy and Parley agreeing). Chrome: 20 pages.
    let pdf = render(&format!(
        "<div style=\"text-align:center\"><div style=\"display:inline-block;width:60%;text-align:left\"><p>{}</p></div></div>",
        w()
    ));
    assert_every_word_once("A2c", &pdf, 4000);
    let found = printed_words(&pdf);
    let x_of = |page: usize| {
        found
            .iter()
            .filter(|w| w.page == page && w.text.starts_with("word"))
            .min_by(|a, b| {
                a.y_min
                    .total_cmp(&b.y_min)
                    .then(a.x_min.total_cmp(&b.x_min))
            })
            .expect("a word on the page")
            .x_min
    };
    let first = x_of(1);
    for page in 2..=page_count(&pdf) {
        assert!(
            (x_of(page) - first).abs() < 0.5,
            "page {page}: the box starts at x = {:.1}pt, on page 1 at {first:.1}pt",
            x_of(page)
        );
    }
    let pages = page_count(&pdf);
    assert!(
        (20..=21).contains(&pages),
        "{pages} pages, Chrome 20 (it slices)"
    );
}

// ── A3, A4: the carrier of the box ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_inline_block_after_a_heading_keeps_every_word() {
    // A3: the carrier is an anonymous block below the heading. Chrome moves the box whole to page 2 (13 pages, a
    // monolithic box that does not fit starts the next page); we start it below the heading (ADR-0004).
    let pdf = render(&format!(
        "<h1 style=\"margin:0;font-size:24px\">Podmínky</h1><div style=\"{IB}\"><p>{}</p></div>",
        w()
    ));
    assert_every_word_once("A3", &pdf, 4000);
    assert_eq!(count_of(&pdf, "Podmínky"), 1);
    assert_eq!(find(&pdf, "Podmínky").page, 1);
    assert!(blank_pages(&pdf).is_empty());
}

#[test]
fn an_inline_block_in_a_bordered_box_keeps_every_word_and_the_frame_on_every_page() {
    // A4: the carrier is the bordered `div`. The frame closes on every page it touches (F96, Chrome slices it).
    let pdf = render(&format!(
        "<div style=\"border:2px solid #f00;padding:8px\"><div style=\"{IB}\"><p>{}</p></div></div>",
        w()
    ));
    assert_every_word_once("A4", &pdf, 4000);
    assert!(blank_pages(&pdf).is_empty());
    for page in 1..=page_count(&pdf) {
        let top = first_red_row(&pdf, page).unwrap_or_else(|| panic!("no frame on page {page}"));
        // 15 mm = 28.3 px at 48 dpi.
        assert!(
            (top as f32 - 28.3).abs() <= 2.0,
            "page {page}: the frame starts at row {top}, the content top is row 28"
        );
    }
}

#[test]
fn an_inline_block_under_a_body_margin_stays_in_the_page_area() {
    // A4b: the continuation is drawn at body-relative geometry; the body offset is cancelled once (F92).
    let pdf = render_with(
        &format!("{HEAD}body{{margin:8px}}"),
        &format!("<div style=\"{IB}\"><p>{}</p></div>", w()),
    );
    assert_every_word_once("A4b", &pdf, 4000);
    assert!(blank_pages(&pdf).is_empty());
    let found = printed_words(&pdf);
    // The first line of every page sits at the same y: no page is shifted by the body offset.
    let tops: Vec<f32> = (2..=page_count(&pdf))
        .map(|p| {
            found
                .iter()
                .filter(|w| w.page == p && w.text.starts_with("word"))
                .map(|w| w.y_min)
                .fold(f32::MAX, f32::min)
        })
        .collect();
    let spread = tops.iter().cloned().fold(f32::MIN, f32::max)
        - tops.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread < 1.0, "first lines of pages 2.. at {tops:?}");
}

// ── A5: an inline-block that shares its line (guard) ────────────────────────────────────────────────────────────────

#[test]
fn an_inline_block_sharing_its_line_is_left_as_it_was() {
    // The box is not alone on its line: the carrier line holds text too. Chrome slices (20 pages); F30f-b. Left as
    // main printed it: the lines that fit page 1 and nothing after (the warning names the case, see
    // `unsplit_paragraph_warning.rs`).
    let pdf = render(&format!(
        "<p>Úvod <span style=\"display:inline-block;width:60%\">{}</span> konec</p>",
        w()
    ));
    assert_eq!(page_count(&pdf), 1);
    assert_eq!(words_per_page(&pdf), vec![212], "main: 212 words");
}

// ── A6, A7, A8: clipping wrappers ────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_paragraph_in_an_overflow_hidden_wrapper_keeps_every_word() {
    // R2: 386 of 4000 words on 1 page before. Chrome fragments the box between lines: 12 pages.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden\"><p>{}</p></div>",
        w()
    ));
    assert_every_word_once("A6", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

#[test]
fn direct_text_in_an_overflow_hidden_box_keeps_every_word() {
    // A6t: the child side of the splitter (the clipping box holds its text itself).
    let pdf = render(&format!(
        "<div><div style=\"overflow:hidden\">{}</div></div>",
        w()
    ));
    assert_every_word_once("A6t", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

#[test]
fn an_overflow_hidden_wrapper_with_line_height_normal_keeps_every_word() {
    let pdf = render_with(
        HEAD_NORMAL,
        &format!("<div style=\"overflow:hidden\"><p>{}</p></div>", w()),
    );
    assert_every_word_once("A6n", &pdf, 4000);
}

#[test]
fn a_paragraph_in_an_overflow_clip_wrapper_keeps_every_word() {
    let pdf = render(&format!(
        "<div style=\"overflow:clip\"><p>{}</p></div>",
        w()
    ));
    assert_every_word_once("A7", &pdf, 4000);
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

#[test]
fn scroll_containers_are_fragmented_in_print_like_hidden_ones() {
    // A8: Chrome prints every word of an `overflow: auto` / `scroll` / `overflow-x: auto` / `overflow-x: hidden` box on
    // 12 pages (no slicing, no clipping): `HasUnsplittableScrollingOverflow` is false when printing.
    for style in [
        "overflow:auto",
        "overflow:scroll",
        "overflow-x:auto",
        "overflow-x:hidden",
    ] {
        let pdf = render(&format!("<div style=\"{style}\"><p>{}</p></div>", w()));
        assert_every_word_once(&format!("A8 {style}"), &pdf, 4000);
        assert_eq!(page_count(&pdf), 12, "{style} (Chrome: 12)");
    }
}

// ── A9: a clip that hides content (guards) ───────────────────────────────────────────────────────────────────────────

#[test]
fn a_clip_box_shorter_than_its_paragraph_keeps_hiding_it() {
    // `height: 600px`: Chrome prints 211 words on 1 page. The box does not fragment (its content does not end inside
    // it) and the clip still hides what is below it: no ink below the box.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden;height:600px\"><p>{}</p></div>",
        w()
    ));
    assert_eq!(page_count(&pdf), 1);
    assert_eq!(
        words_per_page(&pdf),
        vec![372],
        "main: 372 words in the text layer"
    );
    let box_bottom_pt = MARGIN_PT + 600.0 * 0.75;
    assert_eq!(
        ink_rows_below(&pdf, 1, box_bottom_pt + 2.0),
        0,
        "ink below the clip box"
    );
}

#[test]
fn a_clip_box_with_a_max_height_below_its_paragraph_is_left_as_it_was() {
    // `max-height: 2000px`: Chrome prints 701 words on 2 pages (the box fragments, clipped at 2000 px: F30f-b); main
    // printed 372 on page 1. Unchanged here.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden;max-height:2000px\"><p>{}</p></div>",
        w()
    ));
    assert_eq!(page_count(&pdf), 1);
    assert_eq!(words_per_page(&pdf), vec![372], "main: 372 words");
}

// ── A10: wrappers that split already ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn flow_root_and_a_hidden_wrapper_of_a_block_keep_splitting() {
    let flow_root = render(&format!(
        "<div style=\"display:flow-root\"><p>{}</p></div>",
        w()
    ));
    assert_every_word_once("A10 flow-root", &flow_root, 4000);
    assert_eq!(page_count(&flow_root), 12, "Chrome: 12");
    let nested = render(&format!(
        "<div style=\"overflow:hidden\"><div><p>{}</p></div></div>",
        w()
    ));
    assert_every_word_once("A10 hidden > div > p", &nested, 4000);
    assert_eq!(page_count(&nested), 12, "Chrome: 12");
}

// ── A11: the shape of fixture 05 ─────────────────────────────────────────────────────────────────────────────────────

/// A table of `rows` rows of three cells, `wordNNNNN` each, in a rounded clipping wrapper (Tailwind classes).
fn card_table(rows: usize) -> String {
    let cells: String = (0..rows)
        .map(|i| {
            format!(
                "<tr><td>word{:05}</td><td>word{:05}</td><td>word{:05}</td></tr>",
                3 * i,
                3 * i + 1,
                3 * i + 2
            )
        })
        .collect();
    format!(
        "<div class=\"overflow-hidden rounded-xl ring-1 ring-gray-200\"><table class=\"w-full\">{cells}</table></div>"
    )
}

#[test]
fn a_clip_wrapper_of_a_short_table_is_untouched() {
    let pdf = render(&card_table(3));
    assert_eq!(page_count(&pdf), 1);
    assert_eq!(words_per_page(&pdf), vec![9]);
}

#[test]
fn a_clip_wrapper_of_a_long_table_keeps_every_cell() {
    // 60 rows: Chrome 2 pages, 126 + 54 cells; ours 135 + 45 (a row is 16.5 px here, 18 px there; not F30f). Main
    // printed all of them; F30f does not touch tables.
    let pdf = render(&card_table(60));
    // pdftotext reads a table by column, so the order is not the source order: compare the set.
    let mut found: Vec<String> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .map(|w| w.text)
        .collect();
    found.sort();
    let expected: Vec<String> = (0..180).map(|i| format!("word{i:05}")).collect();
    assert_eq!(found, expected, "A11: every cell once");
    assert_eq!(page_count(&pdf), 2, "Chrome: 2");
}

// ── A12: F30g's guard under a clip ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_abspos_stamp_in_a_clip_wrapper_adds_neither_a_page_nor_a_copy() {
    // The stamp is out of flow and the wrapper clips: the abspos pass leaves it as it was (F95 b; Chrome prints it
    // once on page 1, we do not). The text is split and nothing else changes.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden;position:relative\"><p>{}</p>\
         <div style=\"position:absolute;top:0;right:0\">ZAPLACENO</div></div>",
        w()
    ));
    assert_every_word_once("A12", &pdf, 4000);
    assert!(count_of(&pdf, "ZAPLACENO") <= 1);
    assert!(blank_pages(&pdf).is_empty());
    assert_eq!(page_count(&pdf), 12, "Chrome: 12");
}

// ── A13: an inline-block in a clip scope ─────────────────────────────────────────────────────────────────────────────

#[test]
fn an_inline_block_in_a_clipping_card_keeps_every_word() {
    // The continuation is painted from `draw_under_clip`'s walk.
    let pdf = render(&format!(
        "<div class=\"overflow-hidden rounded-lg border p-4\"><div style=\"{IB}\"><p>{}</p></div></div>",
        w()
    ));
    assert_every_word_once("A13", &pdf, 4000);
    assert!(blank_pages(&pdf).is_empty());
}

// ── pixels ────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// The page at 48 dpi as `(width, height, RGB bytes)`.
fn page_pixels(pdf: &[u8], page: usize) -> (usize, usize, Vec<u8>) {
    let path = temp(pdf);
    let root = path.with_extension("page");
    let out = std::process::Command::new("pdftoppm")
        .args([
            "-r",
            "48",
            "-singlefile",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
        ])
        .arg(&path)
        .arg(&root)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftoppm failed");
    let ppm = root.with_extension("page.ppm");
    let data = std::fs::read(&ppm).expect("pdftoppm wrote no PPM");
    let _ = std::fs::remove_file(&ppm);
    // "P6\n<w> <h>\n255\n" and then w * h RGB triples.
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
    assert_eq!(fields[0], "P6");
    (
        fields[1].parse().unwrap(),
        fields[2].parse().unwrap(),
        data[pos..].to_vec(),
    )
}

/// The first row (48 dpi, from the page top) with a strongly red pixel on 1-based `page`.
fn first_red_row(pdf: &[u8], page: usize) -> Option<usize> {
    let (width, _, data) = page_pixels(pdf, page);
    data.chunks_exact(width * 3).position(|row| {
        row.chunks_exact(3)
            .any(|px| px[0] > 200 && px[1] < 60 && px[2] < 60)
    })
}

/// How many pixel rows below `y_pt` (from the page top) on `page` hold dark ink.
fn ink_rows_below(pdf: &[u8], page: usize, y_pt: f32) -> usize {
    let (width, _, data) = page_pixels(pdf, page);
    let first = (y_pt / 72.0 * 48.0).ceil() as usize;
    data.chunks_exact(width * 3)
        .skip(first)
        .filter(|row| {
            row.chunks_exact(3)
                .any(|px| px[0] < 128 && px[1] < 128 && px[2] < 128)
        })
        .count()
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-wl-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
