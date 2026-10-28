//! A clip box shorter than its content shows what it should hide, or writes it into the PDF, or loses what it should
//! show (F30f-b, the clip part).
//!
//! A box clips its block axis when its computed `overflow-y` is not `visible` (css-overflow-3 §3). Given a `height` or
//! `max-height` below its content, main did one of three things: it revealed the hidden lines (a body-level box with
//! direct text: 12 pages, 4000 words), it kept the `<p>` whole and wrote every glyph of it although the clip hides most
//! of them (372 words in the text layer, `pdftotext`, copy and search read text nobody sees), and it never advanced a
//! page for a `max-height` box taller than a page (701 words in Chrome, 372 here), nor put what follows the box at the
//! box's bottom.
//!
//! Spec: css-overflow-3 §3 (the clip is the padding box), css-break-3 §2 and §4.4 (a UA may treat `overflow: hidden` with a
//! definite height as monolithic; `max-height` must fragment; breaks between line boxes, orphans and widows 2),
//! CSS 2.1 §11.1.1. No spec says what a clipped line does to a PDF's text layer: Chrome writes none for a line box wholly
//! outside the clip and ours does the same (ADR-0009). Every number below is Chrome's (m141, A4, 15 mm, Inter, 20 px
//! lines, `W` = 4000 words of 7 per line), measured with `out/scratch/f30f-b/probe.mjs` (F30f-b plan §7).
//!
//! Every case puts `<p>AFTER</p>` after the box. AFTER is printed once, at the bottom of the box, within 2 pt of
//! Chrome's y.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}p{margin:0;line-height:20px}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_with(body: &str, options: RenderOptions) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &options,
        )
        .expect("render")
}

fn render(body: &str) -> Vec<u8> {
    render_with(body, RenderOptions::default())
}

/// Words `word<from> … word<to - 1>`.
fn words(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The 4000 words.
fn w() -> String {
    words(0, 4000)
}

const AFTER: &str = "<p>AFTER</p>";

/// One printed word: its 1-based page, the top of its box in pt from the page top, and its text.
struct Word {
    page: usize,
    y_min: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-cb-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> String {
    let path = temp(pdf);
    let out = std::process::Command::new(tool)
        .args(args)
        .arg(&path)
        .arg("-")
        .output()
        .unwrap_or_else(|_| panic!("{tool} (poppler-utils) is required for this test"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let text = poppler("pdftotext", &["-bbox"], pdf);
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
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// The URLs of the link annotations, from `pdfinfo -url`.
fn link_urls(pdf: &[u8]) -> Vec<String> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfinfo")
        .arg("-url")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().last())
        .filter(|u| u.starts_with("http"))
        .map(str::to_string)
        .collect()
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

/// Chrome's numbers: `pages`, the words on each page (±7 each, one line), and where AFTER is printed (page, y in pt, ±2).
fn like_chrome(what: &str, pdf: &[u8], per_page: &[usize], after: (usize, f32)) {
    assert_eq!(
        words_per_page(pdf).len(),
        per_page.len(),
        "{what}: pages (Chrome {})",
        per_page.len()
    );
    let got = words_per_page(pdf);
    for (k, (g, c)) in got.iter().zip(per_page).enumerate() {
        assert!(
            g.abs_diff(*c) <= 7,
            "{what}: words on page {}: {g}, Chrome {c} (all pages {got:?} vs {per_page:?})",
            k + 1
        );
    }
    let afters: Vec<Word> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text == "AFTER")
        .collect();
    assert_eq!(afters.len(), 1, "{what}: AFTER is printed once");
    assert_eq!(afters[0].page, after.0, "{what}: AFTER on page (Chrome)");
    assert!(
        (afters[0].y_min - after.1).abs() <= 2.0,
        "{what}: AFTER at {:.1} pt, Chrome {:.1}",
        afters[0].y_min,
        after.1
    );
}

/// A box of `style` holding the 4000 words directly, then AFTER.
fn direct(style: &str) -> String {
    format!("<div style=\"{style}\">{}</div>{AFTER}", w())
}

/// A box of `style` holding one `<p>` of the 4000 words, then AFTER.
fn with_p(style: &str) -> String {
    format!("<div style=\"{style}\"><p>{}</p></div>{AFTER}", w())
}

// ── C1: R1, a clip box shorter than its content ─────────────────────────────────────────────────────────────────────

#[test]
fn a_body_level_clip_box_with_direct_text_shows_only_what_its_height_shows() {
    // R1: 12 pages and 4000 words before (the clip was cut at every page's own fragment, so it hid nothing). Chrome: 15
    // lines of 7 words, one page.
    let pdf = render(&direct("overflow:hidden;height:300px"));
    like_chrome("C1", &pdf, &[105], (1, 268.1));
}

#[test]
fn a_clip_box_with_a_paragraph_shows_only_what_its_height_shows() {
    // C1p: the text layer held 372 words and AFTER stood on page 2.
    let pdf = render(&with_p("overflow:hidden;height:300px"));
    like_chrome("C1p", &pdf, &[105], (1, 268.1));
}

#[test]
fn a_clip_box_with_a_nested_paragraph_shows_only_what_its_height_shows() {
    // C1n: box > div > p. Main revealed all 4000 words on 12 pages.
    let pdf = render(&format!(
        "<div style=\"overflow:hidden;height:300px\"><div><p>{}</p></div></div>{AFTER}",
        w()
    ));
    like_chrome("C1n", &pdf, &[105], (1, 268.1));
}

// ── C2: R2, a flex clip box ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_flex_clip_box_writes_only_the_visible_lines() {
    // R2: the picture was right, the text layer held 372 words.
    let pdf = render(&with_p("display:flex;overflow:hidden;height:300px"));
    like_chrome("C2", &pdf, &[105], (1, 268.1));
}

// ── C3: R3, a clip box taller than a few lines but shorter than the page ────────────────────────────────────────────

#[test]
fn a_clip_box_of_600px_writes_only_the_visible_lines() {
    // R3: 372 words in the text layer and AFTER on page 2. Chrome: 30 lines (211 words; the 30th is cut by the box
    // edge: 600 / 20 = 30 lines of 7 words, one of them short).
    let pdf = render(&with_p("overflow:hidden;height:600px"));
    like_chrome("C3", &pdf, &[211], (1, 493.1));
}

// ── C4: R4, max-height ──────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_max_height_clip_box_taller_than_a_page_spans_two_pages() {
    // R4: 372 words on one page and AFTER on page 2 before. Chrome fragments a `max-height` box (css-break-3 §2) and
    // clips each fragment: 2000 px = 1009 + 991.
    let pdf = render(&with_p("overflow:hidden;max-height:2000px"));
    like_chrome("C4", &pdf, &[351, 350], (2, 785.6));
}

#[test]
fn a_max_height_clip_box_with_direct_text_spans_two_pages() {
    // C4t: main revealed all 4000 words on 12 pages.
    let pdf = render(&direct("overflow:hidden;max-height:2000px"));
    like_chrome("C4t", &pdf, &[351, 350], (2, 785.6));
}

// ── C5, C6: a box taller than a page, a box that straddles the page bottom ──────────────────────────────────────────

#[test]
fn a_clip_box_of_3000px_spans_three_pages() {
    let pdf = render(&direct("overflow:hidden;height:3000px"));
    like_chrome("C5 direct", &pdf, &[351, 350, 344], (3, 778.1));
    let pdf = render(&with_p("overflow:hidden;height:3000px"));
    like_chrome("C5 p", &pdf, &[351, 350, 344], (3, 778.1));
}

#[test]
fn a_clip_box_that_straddles_the_page_bottom_is_fragmented() {
    // The box starts 900 px down: 5 lines on page 1 (109 px), 10 on page 2 (the box's remaining 191 px, the last line cut).
    let spacer = "<div style=\"height:900px\"></div>";
    let pdf = render(&format!(
        "{spacer}{}",
        direct("overflow:hidden;height:300px")
    ));
    like_chrome("C6 direct", &pdf, &[35, 70], (2, 185.6));
    let pdf = render(&format!(
        "{spacer}{}",
        with_p("overflow:hidden;height:300px")
    ));
    like_chrome("C6 p", &pdf, &[35, 70], (2, 185.6));
}

// ── C7: the other clipping values ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn clip_auto_and_scroll_boxes_behave_like_hidden_ones() {
    // Chrome fragments `auto` and `scroll` boxes in print like `hidden` ones and writes the same words.
    for value in ["clip", "auto", "scroll"] {
        let pdf = render(&direct(&format!("overflow:{value};height:300px")));
        like_chrome(&format!("C7 {value} direct"), &pdf, &[105], (1, 268.1));
        let pdf = render(&with_p(&format!("overflow:{value};height:600px")));
        like_chrome(&format!("C7 {value} p"), &pdf, &[211], (1, 493.1));
        let pdf = render(&with_p(&format!("overflow:{value};max-height:2000px")));
        like_chrome(&format!("C7 {value} max"), &pdf, &[351, 350], (2, 785.6));
    }
}

// ── C8: which axis counts ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn overflow_y_alone_clips_the_block_axis() {
    for style in ["overflow-y:hidden", "overflow-y:clip", "overflow-x:hidden"] {
        // `overflow-x: hidden` computes `overflow-y: visible` to `auto` (css-overflow-3 §3).
        let pdf = render(&direct(&format!("{style};height:300px")));
        like_chrome(&format!("C8 {style}"), &pdf, &[105], (1, 268.1));
    }
}

#[test]
fn overflow_x_clip_alone_hides_nothing_in_the_block_axis() {
    // `overflow-x: clip` leaves `overflow-y` visible: the text runs on over 12 pages, every word once. (Chrome puts AFTER
    // under the 300 px box, on top of the overflowing text of page 1; ours puts it below the text.)
    let pdf = render(&direct("overflow-x:clip;height:300px"));
    assert_eq!(page_count(&pdf), 12, "Chrome: 12 pages");
    let printed: Vec<String> = printed_words(&pdf)
        .into_iter()
        .filter(|w| w.text.starts_with("word"))
        .map(|w| w.text)
        .collect();
    let expected: Vec<String> = (0..4000).map(|i| format!("word{i:05}")).collect();
    assert_eq!(printed, expected, "every word once, in order");
}

// ── C9: a partly visible line ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_partly_visible_line_is_written() {
    // 310 px: the 16th line starts at 300 px and 10 px of it show. Chrome writes it (112 words = 16 lines).
    let pdf = render(&direct("overflow:hidden;height:310px"));
    like_chrome("C9 direct", &pdf, &[112], (1, 275.6));
    let pdf = render(&with_p("overflow:hidden;height:310px"));
    like_chrome("C9 p", &pdf, &[112], (1, 275.6));
}

// ── C10: a link in the hidden part ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_link_in_the_hidden_part_is_not_written() {
    let body = format!(
        "<div style=\"overflow:hidden;height:300px\"><p>{} <a href=\"https://example.com/visible\">VISLINK</a> {} \
         <a href=\"https://example.com/hidden\">HIDLINK</a> {}</p></div>{AFTER}",
        words(0, 56),
        words(56, 200),
        words(200, 300)
    );
    let pdf = render(&body);
    let printed: Vec<String> = printed_words(&pdf).into_iter().map(|w| w.text).collect();
    assert!(
        printed.iter().any(|t| t == "VISLINK"),
        "the visible link text is written"
    );
    assert!(
        !printed.iter().any(|t| t == "HIDLINK"),
        "the hidden link text is not written"
    );
    // Chrome: one annotation.
    assert_eq!(
        link_urls(&pdf),
        vec!["https://example.com/visible".to_string()],
        "Chrome: only the visible link has an annotation"
    );
}

// ── Guards ──────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn tagged_renders_of_clip_boxes_succeed() {
    // A paragraph clipped out completely leaves a structure element without content; it must not fail the render.
    let tagged = || RenderOptions {
        tagged: true,
        lang: Some("cs".into()),
        ..Default::default()
    };
    for (what, body) in [
        ("C1", direct("overflow:hidden;height:300px")),
        ("C3", with_p("overflow:hidden;height:600px")),
        ("C4", with_p("overflow:hidden;max-height:2000px")),
    ] {
        let pdf = render_with(&body, tagged());
        assert!(pdf.starts_with(b"%PDF"), "{what}: tagged render");
    }
}

// ── Auto-height clip boxes are content-sized boxes (plan §7, deviation from the first version) ─────────────────────────
//
// A clip box with no `height` and no `max-height` hides nothing: it is as tall as its content. The first version of the
// fragmenter judged "content reaches past the box" from the line boxes, and Blitz's line boxes are taller than their
// pitch when the line height is below the font's ascent + descent (16 px Inter in 12 px lines: 20 px boxes, 12 px apart),
// so the last line box "reached past" the box and the box was cut and clipped like a fixed one: words lost.

const SMALL_HEAD: &str = "@page{size:A4;margin:15mm}p{margin:0}";

fn render_styled(head: &str, body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Every word of `expected` once, in order, in the text layer (the words all start with `w`).
fn assert_every_word_once(what: &str, pdf: &[u8], expected: &[String]) {
    let printed: Vec<String> = printed_words(pdf)
        .into_iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.text)
        .collect();
    let missing: Vec<&String> = expected.iter().filter(|e| !printed.contains(e)).collect();
    assert!(
        missing.is_empty(),
        "{what}: {} of {} words are not in the text layer (first: {:?})",
        missing.len(),
        expected.len(),
        missing.first()
    );
    assert_eq!(printed, expected, "{what}: every word once, in order");
}

/// `w0000 w0001 …`: the words of the validator's repros, ten or so to a line of the card.
fn short_names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("w{i:04}")).collect()
}

#[test]
fn an_auto_height_clip_box_with_lines_smaller_than_the_font_loses_no_word() {
    // c318 / c300: `overflow:hidden` and nothing else, 1500 words over a page break. Main: 1500 words; the first version
    // of the clip part: 1488 (a 20 px last line box in a 12 px pitch "reached past" the box). Chrome: 1500, 2 pages.
    for line_height in ["12px", "0.9"] {
        let pdf = render_styled(
            &format!("{SMALL_HEAD}body{{font:16px/{line_height} Inter;margin:0}}"),
            &format!(
                "<div style=\"overflow:hidden\">{}</div>{AFTER}",
                short_names(1500).join(" ")
            ),
        );
        assert_eq!(page_count(&pdf), 2, "lh {line_height}: Chrome 2 pages");
        assert_every_word_once(&format!("lh {line_height}"), &pdf, &short_names(1500));
    }
}

#[test]
fn an_auto_height_card_with_direct_text_loses_no_word_in_the_text_layer() {
    // c286: `14px/1.2`, `overflow:hidden;padding:16px;border:1px solid;border-radius:8px`, 1500 direct words. Main and
    // Chrome: 1500; the first version: 1484 (the writer's band dropped the last line of each page fragment: main already
    // cut it in the picture).
    let pdf = render_styled(
        &format!("{SMALL_HEAD}body{{font:14px/1.2 Inter;margin:0}}"),
        &format!(
            "<div style=\"overflow:hidden;padding:16px;border:1px solid #ccc;border-radius:8px\">{}</div>{AFTER}",
            short_names(1500).join(" ")
        ),
    );
    assert_eq!(page_count(&pdf), 2, "Chrome: 2 pages");
    assert_every_word_once("card", &pdf, &short_names(1500));
}

#[test]
fn an_auto_height_card_after_a_spacer_loses_no_word() {
    // s12_40: a bordered, padded `overflow:hidden` card (h2 + p, `line-height: 0.7`) that starts 700 px down a page
    // without margins. Chrome and main: 1187 words; the first version: 762.
    let pdf = render_styled(
        "@page{size:A4;margin:0}body{font:16px/0.7 Inter;margin:0}p{margin:0 0 4px}",
        &format!(
            "<div style=\"height:700px;background:#eee\"></div><div style=\"overflow:hidden;border-radius:8px;\
             border:1px solid #ccc;padding:16px\"><h2 style=\"margin:0 0 6px\">H40</h2><p>{}</p></div>",
            short_names(1187).join(" ")
        ),
    );
    assert_every_word_once("s12_40", &pdf, &short_names(1187));
}

#[test]
fn an_auto_height_clip_box_that_fits_the_page_is_laid_out_as_a_box_without_the_clip() {
    // c/auto-lineheight-small: `overflow:hidden;line-height:5px` on 16 px text, one page. The first version put AFTER at
    // 729 pt (the box was placed at its own height); main puts it at 740 pt, where the same box without `overflow` has
    // it (Chrome: 726; that gap is the line boxes' height, not this change). A document with no fixed-size box does not
    // move.
    let layout = |style: &str| {
        let pdf = render_styled(
            &format!("{SMALL_HEAD}body{{font:16px/20px Inter;margin:0}}"),
            &format!(
                "<div style=\"{style}line-height:5px\">{}</div>{AFTER}",
                short_names(600).join(" ")
            ),
        );
        assert_eq!(page_count(&pdf), 1);
        printed_words(&pdf)
            .into_iter()
            .find(|w| w.text == "AFTER")
            .expect("AFTER is printed")
            .y_min
    };
    let (clipped, plain) = (layout("overflow:hidden;"), layout(""));
    assert!(
        (clipped - plain).abs() < 0.1,
        "AFTER at {clipped:.1} pt with `overflow:hidden`, {plain:.1} pt without"
    );
}

#[test]
fn a_clip_box_with_a_height_still_drops_the_lines_it_hides_when_lines_are_small() {
    // The other side: a fixed height below the content keeps ADR-0009's rule whatever the line height. 16 px text in a
    // 12 px pitch, `height:300px`: 25 lines of 12 px are visible, 12 words each at most.
    let pdf = render_styled(
        &format!("{SMALL_HEAD}body{{font:16px/12px Inter;margin:0}}"),
        &format!(
            "<div style=\"overflow:hidden;height:300px\">{}</div>{AFTER}",
            short_names(1500).join(" ")
        ),
    );
    assert_eq!(page_count(&pdf), 1);
    let n = printed_words(&pdf)
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .count();
    assert!(
        (60..=400).contains(&n),
        "a 300 px box shows 25 lines, not the 1500 words: {n}"
    );
}

// ── A clip box with a `height` and padding or a border that straddles a page (vrt-validator, second pass) ─────────────

/// A 700 / 800 / 900 / 960 px spacer, then a clip box of `style` holding one `<p>` of the 4000 words (or the words
/// directly), then AFTER: the box starts `spacer` px down a page of 1009 px and straddles its bottom.
fn straddling(spacer: usize, style: &str, direct: bool) -> Vec<u8> {
    let inner = if direct {
        w()
    } else {
        format!("<p>{}</p>", w())
    };
    render(&format!(
        "<div style=\"height:{spacer}px;background:#eee\"></div>\
         <div style=\"overflow:hidden;{style}\">{inner}</div>{AFTER}"
    ))
}

#[test]
fn a_padded_clip_box_with_a_paragraph_that_straddles_a_page_keeps_the_lines_of_its_first_page() {
    // `padding:16px;height:300px` (content-box: 332 px tall). Main moved the box to page 3 (4 pages, 371 words in the
    // text layer, the clip showing a part); the first version of the clip part kept the box on page 1 and moved its
    // paragraph whole to page 2: an empty frame on page 1, 14 words in the PDF where Chrome has 105. The paragraph is
    // split where it is, like the same box with the words directly in it (the `direct` rows, which were right). Chrome:
    // words per page and AFTER (m141, A4, 15 mm, 20 px lines; `out/scratch/f30fbv3/probe3.mjs`).
    for (spacer, per_page, after) in [
        (700, vec![98, 7], 59.6),
        (800, vec![63, 42], 134.6),
        (900, vec![28, 77], 209.6),
        (960, vec![0, 112], 292.1),
    ] {
        for direct in [false, true] {
            let pdf = straddling(spacer, "padding:16px;height:300px", direct);
            like_chrome(
                &format!("padding 16, spacer {spacer}, direct {direct}"),
                &pdf,
                &per_page,
                (2, after),
            );
        }
    }
}

#[test]
fn a_bordered_clip_box_with_a_paragraph_that_straddles_a_page_keeps_the_lines_of_its_first_page() {
    // `border:2px solid;height:300px`: the paragraph sat 2 px below the box's top, so it was not "first in the box" and
    // moved whole to page 2 (66 words in the PDF at spacer 800 against Chrome's 105).
    for (spacer, per_page, after) in [
        (800, vec![70, 35], 113.6),
        (900, vec![35, 70], 188.6),
        (960, vec![14, 91], 233.6),
    ] {
        let pdf = straddling(spacer, "border:2px solid #888;height:300px", false);
        like_chrome(
            &format!("border 2, spacer {spacer}"),
            &pdf,
            &per_page,
            (2, after),
        );
    }
}

#[test]
fn a_clip_box_with_a_top_padding_or_a_border_box_sizing_keeps_the_lines_of_its_first_page() {
    // `padding-top:16px` alone (13 words in the PDF at spacer 700, Chrome 98) and `box-sizing:border-box` with padding
    // and border.
    let pdf = straddling(700, "padding-top:16px;height:300px", false);
    like_chrome("padding-top 16, spacer 700", &pdf, &[98, 0], (2, 47.6));
    let pdf = straddling(900, "padding-top:16px;height:300px", false);
    like_chrome("padding-top 16, spacer 900", &pdf, &[28, 70], (2, 197.6));
    let pdf = straddling(
        900,
        "padding:16px;border:2px solid #888;box-sizing:border-box;height:300px",
        false,
    );
    like_chrome("border-box, spacer 900", &pdf, &[28, 69], (2, 185.6));
}

#[test]
fn a_padded_box_inside_a_clip_box_that_straddles_a_page_keeps_the_lines_of_its_first_page() {
    // The validator's `a_in_c_pad`: `overflow:hidden;height:300px;padding:20px` > `overflow:hidden;padding:10px` > `<p>`.
    // The inner box is content-sized (its padding sits above the paragraph), the outer one clips it; the paragraph moved
    // whole to page 2 and page 1 showed an empty frame, where main showed the lines (and Chrome shows 85 / 12 words).
    for (spacer, per_page, after) in [(700, vec![85, 12], 65.6), (880, vec![31, 66], 200.6)] {
        let pdf = render(&format!(
            "<div style=\"height:{spacer}px;background:#eee\"></div>\
             <div style=\"overflow:hidden;height:300px;padding:20px\">\
             <div style=\"overflow:hidden;padding:10px\"><p>{}</p></div></div>{AFTER}",
            w()
        ));
        like_chrome(
            &format!("nested, spacer {spacer}"),
            &pdf,
            &per_page,
            (2, after),
        );
    }
}

#[test]
fn a_table_cell_with_a_height_and_overflow_hidden_is_not_a_clip_box_of_that_height() {
    // `height` on a table cell is a minimum (CSS 2.1 §17.5.3): the cell grows with its content, so it is not a box
    // whose block size the author constrains, and the clip semantics of this change do not apply to it. Chrome grows the
    // cell and prints all 4000 words on 13 pages (70 / 350 / ..., AFTER on page 13): a gap of its own (F51, F30f-b
    // "limits"), not closed here; what this pins is that the output stays what main's was (371 words in the text layer,
    // 2 pages, AFTER on page 2 at 269.4 pt) instead of the half-line clipped 35 words the first version wrote.
    let pdf = render(&format!(
        "<div style=\"height:800px;background:#eee\"></div><table style=\"width:100%\"><tr>\
         <td style=\"height:300px;overflow:hidden\"><p>{}</p></td></tr></table>{AFTER}",
        w()
    ));
    let per_page = words_per_page(&pdf);
    assert_eq!(per_page.len(), 2, "pages (main's): {per_page:?}");
    assert!(
        per_page[1] >= 300,
        "words on page 2: {} (main 371; the first version 35)",
        per_page[1]
    );
    let after = printed_words(&pdf)
        .into_iter()
        .find(|w| w.text == "AFTER")
        .expect("AFTER is printed");
    assert_eq!(after.page, 2);
    assert!(
        (after.y_min - 269.4).abs() <= 2.0,
        "AFTER at {:.1} pt (main 269.4, the first version 111.9)",
        after.y_min
    );
}

// ── Shapes this change does not fragment (vrt-validator, third pass) ─────────────────────────────────────────────────

/// An 880 / 700 px spacer, then a clip box of `style` holding `inner`, then AFTER.
fn clip_box(spacer: usize, style: &str, inner: &str) -> Vec<u8> {
    render(&format!(
        "<div style=\"height:{spacer}px;background:#eee\"></div>\
         <div style=\"overflow:hidden;{style}\">{inner}</div>{AFTER}"
    ))
}

/// The output of main for a shape: words in the text layer on each page, and where AFTER is (page, y in pt, ±0.5).
/// `chrome_words` is how many of the first words Chrome shows (page 1 and page 2 together): every one of them has to be
/// in the PDF, however far from Chrome's page the shape puts them.
fn like_main(what: &str, pdf: &[u8], per_page: &[usize], after: (usize, f32), chrome_words: usize) {
    assert_eq!(
        words_per_page(pdf),
        per_page,
        "{what}: words per page (main's)"
    );
    let printed = printed_words(pdf);
    let afters: Vec<&Word> = printed.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(afters.len(), 1, "{what}: AFTER is printed once");
    assert_eq!(afters[0].page, after.0, "{what}: AFTER on page (main's)");
    assert!(
        (afters[0].y_min - after.1).abs() <= 0.5,
        "{what}: AFTER at {:.1} pt, main {:.1}",
        afters[0].y_min,
        after.1
    );
    for i in 0..chrome_words {
        let word = format!("word{i:05}");
        assert!(
            printed.iter().any(|w| w.text == word),
            "{what}: {word} is shown by Chrome and missing from the PDF"
        );
    }
}

/// One shape a constrained clip box can hold that this change leaves to main, with main's numbers.
struct MainShape {
    what: &'static str,
    spacer: usize,
    style: &'static str,
    inner: String,
    per_page: Vec<usize>,
    after: (usize, f32),
    chrome_words: usize,
}

#[test]
fn a_constrained_clip_box_whose_content_is_not_one_paragraph_at_its_top_is_what_main_made_of_it() {
    // The clip semantics of this change (the box is fragmented by its own block size, the writer drops what lies below
    // its clip) are written for a box whose content is direct text, or one paragraph at its content top (plan 7, "Supported
    // shapes"). The fragmenter splits only the first thing on a strip (F30c): with anything above the paragraph the
    // paragraph moved whole and was clipped, and lines Chrome shows on page 1 were not in the PDF (the third pass:
    // hp_pad 14 words against main's 371 and Chrome's 98). Such a box keeps main's output until F30c (finding F30f-f).
    // Numbers: main's binary (fork 516558cc) on these documents; Chrome m141 shows 98 / 105 / 98 / 101 / 84 / 98 words.
    let w4 = w();
    let img = "<img style=\"display:block\" width=\"120\" height=\"60\" \
               src=\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'>\
               <rect width='120' height='60' fill='red'/></svg>\">";
    let cases: Vec<MainShape> = vec![
        MainShape {
            what: "heading above the paragraph",
            spacer: 700,
            style: "height:300px;padding:16px",
            inner: format!("<h2 style=\"margin:0\">HEAD</h2><p>{w4}</p>"),
            per_page: vec![0, 371, 0],
            after: (3, 42.9),
            chrome_words: 98,
        },
        MainShape {
            what: "second paragraph",
            spacer: 700,
            style: "height:300px;padding:16px",
            inner: format!(
                "<p style=\"margin:0 0 14px\">{w4}</p><p style=\"margin:0 0 14px\">{}</p>",
                words(4000, 4100)
            ),
            per_page: vec![0, 0, 371, 100],
            after: (4, 267.9),
            chrome_words: 105,
        },
        MainShape {
            what: "the paragraph's own margin",
            spacer: 880,
            style: "height:300px",
            inner: format!("<p style=\"margin:14px 0\">{w4}</p>"),
            per_page: vec![0, 372, 0],
            after: (3, 42.9),
            chrome_words: 98,
        },
        MainShape {
            what: "float above the paragraph",
            spacer: 700,
            style: "height:300px;padding:16px",
            inner: format!(
                "<div style=\"float:left;width:100px;height:80px;background:#ccc\"></div><p>{w4}</p>"
            ),
            per_page: vec![0, 371, 0],
            after: (3, 42.9),
            chrome_words: 101,
        },
        MainShape {
            what: "image above the paragraph",
            spacer: 700,
            style: "height:300px;padding:16px",
            inner: format!("{img}<p>{w4}</p>"),
            per_page: vec![0, 371, 0],
            after: (3, 42.9),
            chrome_words: 84,
        },
        MainShape {
            what: "heading and direct text",
            spacer: 880,
            style: "height:300px;padding:16px",
            inner: format!("<h2 style=\"margin:0\">HEAD</h2>{w4}"),
            per_page: vec![0, 371, 0],
            after: (3, 42.9),
            chrome_words: 98,
        },
        MainShape {
            what: "padding in percent (Taffy resolves the vertical part to 0, plan 7)",
            spacer: 880,
            style: "height:300px;padding:5%",
            inner: format!("<p>{w4}</p>"),
            per_page: vec![0, 322, 0],
            after: (3, 42.9),
            chrome_words: 96,
        },
    ];
    for c in cases {
        let pdf = clip_box(c.spacer, c.style, &c.inner);
        like_main(c.what, &pdf, &c.per_page, c.after, c.chrome_words);
    }
}

#[test]
fn a_paragraph_that_avoids_a_break_inside_a_padded_clip_box_is_what_main_made_of_it() {
    // `break-inside: avoid` on the paragraph (150 words: it fits a page): Chrome moves it whole to page 2 and shows 112
    // words there. The first version of the clip part split it (98 / 14, farther from Chrome than main, which has 150
    // words on page 2 and AFTER at 291.9 pt, Chrome 292.1); a box with such a paragraph is not a supported shape.
    let pdf = clip_box(
        700,
        "height:300px;padding:16px",
        &format!("<p style=\"break-inside:avoid\">{}</p>", words(0, 150)),
    );
    like_main("avoid", &pdf, &[0, 150], (2, 291.9), 112);
}

#[test]
fn clip_boxes_in_a_grid_keep_what_main_made_of_them() {
    // Two grid items with `overflow:hidden;height:256px;padding:16px`, one holding a heading and a paragraph, one a
    // paragraph, 880 px down the page. A clip box is a supported shape only in block flow: the fragmenter lifts the clip
    // only below a block container, so with the second item "supported" the pair came out worse than main in the
    // validator's `tw_clip_grid_card.s880` (3 pages, twice the ink; Chrome and main 2). Main: 300 words on page 2,
    // AFTER at 258.9 pt (Chrome: 27 / 48 words, AFTER 161.6 pt; the fork before the narrowing was close to it, F30f-f).
    let item = |inner: &str| {
        format!("<div style=\"overflow:hidden;height:256px;padding:16px\">{inner}</div>")
    };
    let pdf = render(&format!(
        "<div style=\"height:880px;background:#eee\"></div>\
         <div style=\"display:grid;grid-template-columns:1fr 1fr;gap:16px\">{}{}</div>{AFTER}",
        item(&format!(
            "<h3 style=\"margin:0\">HEAD</h3><p>{}</p>",
            words(0, 150)
        )),
        item(&format!("<p>{}</p>", words(0, 150))),
    ));
    like_main("grid", &pdf, &[0, 300], (2, 258.9), 0);
}

// ── Fixture 28 ──────────────────────────────────────────────────────────────────────────────────────────────────────

fn fixture_28() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("fixtures/invoices/28-notes-box-clip");
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
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
        .expect("fixture 28")
}

#[test]
fn fixture_28_writes_the_words_chrome_writes() {
    // The note is longer than its 112 px box and a body-direct child: main moved the box to page 2 and wrote (and showed)
    // all of it, 491 words on 2 pages.
    let ours = fixture_28();
    let reference = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/invoices/28-notes-box-clip/reference/chrome.pdf"),
    )
    .unwrap();
    assert_eq!(page_count(&ours), 1, "Chrome: 1 page");
    let words =
        |pdf: &[u8]| -> Vec<String> { printed_words(pdf).into_iter().map(|w| w.text).collect() };
    let (ours, chrome) = (words(&ours), words(&reference));
    // A one-line wrapping difference would move a word at most; the words themselves are the same.
    assert_eq!(
        ours.len(),
        chrome.len(),
        "words in the text layer: {} vs Chrome {}",
        ours.len(),
        chrome.len()
    );
    assert_eq!(ours, chrome);
}
