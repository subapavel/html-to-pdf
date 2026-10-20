//! `display: contents` in Blitz's box construction (F150, F153). Plan:
//! docs/plans/2026-10-20-f150-display-contents-construction.md.
//!
//! An element with `display: contents` generates no box; its children (and `::before` / `::after`) generate boxes as
//! if they were the children of its parent (css-display-3 §2.5). Blitz used to compute the layout children of each
//! child of such an element instead of placing the child: a `<p>Faktura</p>` inside it was never laid out and its
//! text silently vanished (F153), and mixed content with a whitespace text node before it freed a box that was
//! still listed, which panicked in `resolve` with "invalid key" (F150).
//!
//! Every case renders A4 with the default 15 mm margins, Inter bundled, strict fonts. Two oracles:
//!
//! - a wrapper without styles must not move anything: the words of the document with `<w>` (a `display: contents`
//!   `<div>`) or `<s>` (a `display: contents` `<span>`) wrappers have the same page and box (0.01 pt) as the words of
//!   the same document with the wrappers removed;
//! - Chrome: C1, C4 and C7 are also checked against positions measured in Chrome (relative to the first word, 1.5 pt;
//!   the absolute offset differs by a constant because Chrome uses the host's Inter, we the bundled one).
//!
//! Positions come from `pdftotext -bbox`, colours from `pdftoppm`.

use htmltopdf::{RenderOptions, Renderer};

/// Same word, wrapped and unwrapped.
const EPS_PT: f32 = 0.01;
/// Against Chrome.
const CHROME_PT: f32 = 1.5;

const CSS: &str = "body { margin: 0; font-family: Inter; line-height: 20px } p { margin: 0 } \
    ul, ol { margin: 0 } li { line-height: 20px }";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn renderer(css: &str) -> Renderer {
    Renderer::builder()
        .font_bytes(inter())
        .expect("Inter")
        .strict_fonts(true)
        .css(format!("{CSS} {css}"))
        .build()
}

fn try_render(css: &str, body: &str) -> htmltopdf::Result<Vec<u8>> {
    let html = format!("<!doctype html><html><body>{body}</body></html>");
    renderer(css).render(&html, &RenderOptions::default())
}

fn render(css: &str, body: &str) -> Vec<u8> {
    try_render(css, body).expect("render")
}

/// `<w>` and `<s>` become `display: contents` wrappers.
fn wrapped(body: &str) -> String {
    body.replace("<w>", "<div style=\"display:contents\">")
        .replace("</w>", "</div>")
        .replace("<s>", "<span style=\"display:contents\">")
        .replace("</s>", "</span>")
}

/// `<w>` and `<s>` are removed.
fn plain(body: &str) -> String {
    body.replace("<w>", "")
        .replace("</w>", "")
        .replace("<s>", "")
        .replace("</s>", "")
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    text: String,
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-dc-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> Vec<u8> {
    let path = temp(pdf);
    let mut cmd = std::process::Command::new(tool);
    cmd.args(args).arg(&path);
    if tool == "pdftotext" {
        cmd.arg("-");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{tool} (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    out.stdout
}

/// Every word of the PDF in extraction order (page by page, top to bottom).
fn words(pdf: &[u8]) -> Vec<Word> {
    let text = String::from_utf8_lossy(&poppler("pdftotext", &["-bbox"], pdf)).into_owned();
    let mut page = 0;
    let mut found = Vec::new();
    let attr = |attrs: &str, name: &str| -> f32 {
        attrs
            .split_whitespace()
            .find_map(|a| a.strip_prefix(&format!("{name}=\"")))
            .and_then(|v| v.trim_end_matches('"').parse().ok())
            .unwrap_or_else(|| panic!("word without {name}"))
    };
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push(Word {
                page,
                x0: attr(attrs, "xMin"),
                y0: attr(attrs, "yMin"),
                x1: attr(attrs, "xMax"),
                y1: attr(attrs, "yMax"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn page_count(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(&poppler("pdfinfo", &[], pdf))
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

fn texts(words: &[Word]) -> Vec<&str> {
    words.iter().map(|w| w.text.as_str()).collect()
}

/// The words grouped into lines: a new line starts when `y0` moves by more than 2 pt (page changes start one too).
fn lines(words: &[Word]) -> Vec<Vec<&Word>> {
    let mut out: Vec<Vec<&Word>> = Vec::new();
    for w in words {
        match out.last_mut() {
            Some(line) if line[0].page == w.page && (line[0].y0 - w.y0).abs() <= 2.0 => {
                line.push(w)
            }
            _ => out.push(vec![w]),
        }
    }
    out
}

fn line_texts(words: &[Word]) -> Vec<String> {
    lines(words)
        .iter()
        .map(|l| {
            l.iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// The words of `a` and `b` are the same, at the same page and box (`EPS_PT`).
fn assert_same_boxes(what: &str, a: &[u8], b: &[u8]) {
    let (wa, wb) = (words(a), words(b));
    assert_eq!(texts(&wa), texts(&wb), "{what}: the words differ");
    for (x, y) in wa.iter().zip(&wb) {
        let near = |p: f32, q: f32| (p - q).abs() <= EPS_PT;
        assert!(
            x.page == y.page
                && near(x.x0, y.x0)
                && near(x.y0, y.y0)
                && near(x.x1, y.x1)
                && near(x.y1, y.y1),
            "{what}: {} moved: wrapped {x:?}, plain {y:?}",
            x.text
        );
    }
}

/// The oracle: `body` (with `<w>` / `<s>` wrappers) prints every `expected` word once, in order, and has the same
/// boxes as the document without the wrappers. Returns the wrapped document's PDF.
fn assert_wrapper_is_invisible(what: &str, css: &str, body: &str, expected: &[&str]) -> Vec<u8> {
    let with = render(css, &wrapped(body));
    let without = render(css, &plain(body));
    let found = words(&with);
    assert_eq!(texts(&found), expected, "{what}: words printed");
    assert_same_boxes(what, &with, &without);
    assert_eq!(page_count(&with), page_count(&without), "{what}: pages");
    with
}

/// `positions[i]` is the offset `(dx, dy)` in pt of word `i` from word 0 in Chrome.
fn assert_like_chrome(what: &str, pdf: &[u8], positions: &[(f32, f32)]) {
    let found = words(pdf);
    assert_eq!(found.len(), positions.len(), "{what}: word count");
    for (w, (dx, dy)) in found.iter().zip(positions) {
        let (gx, gy) = (w.x0 - found[0].x0, w.y0 - found[0].y0);
        assert!(
            (gx - dx).abs() <= CHROME_PT && (gy - dy).abs() <= CHROME_PT,
            "{what}: {} is at ({gx:.1}, {gy:.1}) from the first word, Chrome has ({dx:.1}, {dy:.1})",
            w.text
        );
    }
}

// --- C1-C3: blocks and inline content placed through a wrapper ------------------------------------------------------

#[test]
fn c1_blocks_in_a_wrapper_before_a_block() {
    let body = "<w><p>Faktura</p><p>Celkem</p></w><p>end</p>";
    let pdf = assert_wrapper_is_invisible("C1", "", body, &["Faktura", "Celkem", "end"]);
    // Chrome 148: one word per line, 15 pt apart.
    assert_like_chrome("C1", &pdf, &[(0.0, 0.0), (0.0, 15.0), (0.0, 30.0)]);
    assert_eq!(lines(&words(&pdf)).len(), 3);
}

#[test]
fn c2_blocks_in_a_wrapper_after_a_block() {
    let body = "<p>start</p><w><p>Faktura</p><p>Celkem</p></w>";
    assert_wrapper_is_invisible("C2", "", body, &["start", "Faktura", "Celkem"]);
}

#[test]
fn c3_inline_content_of_a_wrapper_then_a_block() {
    let body = "<div><w><span>Faktura</span> <b>Celkem</b></w><p>y</p></div>";
    let pdf = assert_wrapper_is_invisible("C3", "", body, &["Faktura", "Celkem", "y"]);
    assert_eq!(line_texts(&words(&pdf)), ["Faktura Celkem", "y"]);
}

// --- C4, C5: runs of inline content across the boundary -------------------------------------------------------------

#[test]
fn c4_a_run_of_inline_content_crosses_the_boundary() {
    let body = "<div>a <s>b <i>c</i></s> d<p>e</p></div>";
    let pdf = assert_wrapper_is_invisible("C4", "", body, &["a", "b", "c", "d", "e"]);
    assert_eq!(line_texts(&words(&pdf)), ["a b c d", "e"]);
    // Chrome: a b c d on one line, e below.
    assert_like_chrome(
        "C4",
        &pdf,
        &[
            (0.0, 0.0),
            (10.2, 0.0),
            (20.9, 0.0),
            (31.1, 0.0),
            (0.0, 15.0),
        ],
    );
}

#[test]
fn c5_three_nested_wrappers_with_mixed_content() {
    let body = "<div>w1 <w>w2 <w><p>w3</p> w4 <s>w5 <w> <p>w6</p> </w> w7</s></w> w8</w> <p>w9</p> w10</div>";
    let pdf = assert_wrapper_is_invisible(
        "C5",
        "",
        body,
        &["w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10"],
    );
    assert_eq!(
        line_texts(&words(&pdf)),
        ["w1 w2", "w3", "w4 w5", "w6", "w7 w8", "w9", "w10"]
    );
}

// --- C6: the wrapper is the only child of body ----------------------------------------------------------------------

#[test]
fn c6a_a_wrapper_as_the_only_child_of_body_stacks_its_blocks() {
    let body = "<w><p>Faktura</p><p>Celkem</p></w>";
    let pdf = assert_wrapper_is_invisible("C6a", "", body, &["Faktura", "Celkem"]);
    // Before F153 body became an inline root and the paragraphs sat side by side.
    assert_eq!(line_texts(&words(&pdf)), ["Faktura", "Celkem"]);
}

#[test]
fn c6b_inline_only_content_keeps_body_an_inline_root() {
    // F111: body stays the inline formatting root, so a tall run still splits at line boxes.
    let body = "<w><span>Faktura</span> <b>Celkem</b></w>";
    let pdf = assert_wrapper_is_invisible("C6b", "", body, &["Faktura", "Celkem"]);
    assert_eq!(line_texts(&words(&pdf)), ["Faktura Celkem"]);
    let long: String = (1..=200)
        .map(|i| format!("Line{i:03}"))
        .collect::<Vec<_>>()
        .join("<br>");
    let pdf = render("", &wrapped(&format!("<w>{long}</w>")));
    let found = words(&pdf);
    assert_eq!(found.len(), 200);
    assert_eq!(page_count(&pdf), 4, "200 lines of 20 px on A4");
}

// --- C7, C8: flex and grid -------------------------------------------------------------------------------------------

#[test]
fn c7_flex_items_and_text_through_a_wrapper() {
    let body = "<div style=\"display:flex;gap:10px\"><w><div>one</div><div>two</div></w>tx <div>three</div></div>";
    let pdf = assert_wrapper_is_invisible("C7", "", body, &["one", "two", "tx", "three"]);
    assert_eq!(line_texts(&words(&pdf)), ["one two tx three"]);
    // Chrome: one two tx three in a row.
    assert_like_chrome(
        "C7",
        &pdf,
        &[(0.0, 0.0), (28.8, 0.0), (57.2, 0.0), (75.2, 0.0)],
    );
}

#[test]
fn c8_grid_cells_and_text_through_a_wrapper() {
    let body = "<div style=\"display:grid;grid-template-columns:100px 100px;gap:4px\">\
        <w><div>g1</div><div>g2</div></w>loose <div>g4</div></div>";
    let pdf = assert_wrapper_is_invisible("C8", "", body, &["g1", "g2", "loose", "g4"]);
    // 2 x 2: the text is one anonymous item.
    assert_eq!(line_texts(&words(&pdf)), ["g1 g2", "loose g4"]);
}

// --- C9: tables ------------------------------------------------------------------------------------------------------

#[test]
fn c9_tables_and_wrappers() {
    // A wrapper around the row group, and inside a cell.
    let body = "<table><w><tbody><tr><td>cell1</td><td>cell2</td></tr></tbody></w></table>";
    assert_wrapper_is_invisible("C9a", "", body, &["cell1", "cell2"]);
    let body = "<table><tr><td><w><p>inner</p></w></td></tr></table>";
    assert_wrapper_is_invisible("C9b", "", body, &["inner"]);
    // An orphan table (a `display: table` wrapper that is itself `contents`): the words are placed once
    // (the geometry of the orphan case is F50b).
    let body = "<div style=\"display:table;display:contents\"><div style=\"display:table-row\">\
        <div style=\"display:table-cell;padding:0 8px\">orphan</div></div></div><p>after</p>";
    let pdf = render("", body);
    assert_eq!(texts(&words(&pdf)), ["orphan", "after"]);
}

// --- C10: list items ---------------------------------------------------------------------------------------------------

#[test]
fn c10_list_items_in_a_wrapper_are_numbered_by_the_list() {
    let body = "<ol><w><li>Prvni</li><li>Druhy</li></w><li>Treti</li></ol>";
    let pdf = assert_wrapper_is_invisible(
        "C10",
        "",
        body,
        &["1.", "Prvni", "2.", "Druhy", "3.", "Treti"],
    );
    assert_eq!(
        line_texts(&words(&pdf)),
        ["1. Prvni", "2. Druhy", "3. Treti"]
    );
}

// --- C11: the style of the wrapper -------------------------------------------------------------------------------------

/// The darkest pixel (smallest r+g+b) of the page-1 raster at 96 dpi inside `[x0, x1] x [y0, y1]` (pt).
fn darkest(pdf: &[u8], x0: f32, y0: f32, x1: f32, y1: f32) -> (u8, u8, u8) {
    let ppm = poppler("pdftoppm", &["-r", "96", "-f", "1", "-l", "1"], pdf);
    // P6\n<w> <h>\n255\n<data>
    let mut parts = ppm.splitn(4, |&b| b == b'\n');
    assert_eq!(parts.next().unwrap(), b"P6");
    let dims = String::from_utf8_lossy(parts.next().unwrap()).into_owned();
    let (w, _h) = dims.split_once(' ').unwrap();
    let w: usize = w.parse().unwrap();
    let _max = parts.next().unwrap();
    let data = parts.next().unwrap();
    let px = |v: f32| (v * 96.0 / 72.0).floor() as usize;
    let mut best = (255u8, 255u8, 255u8);
    for y in px(y0)..=px(y1) {
        for x in px(x0)..=px(x1) {
            let i = (y * w + x) * 3;
            let p = (data[i], data[i + 1], data[i + 2]);
            if u32::from(p.0) + u32::from(p.1) + u32::from(p.2)
                < u32::from(best.0) + u32::from(best.1) + u32::from(best.2)
            {
                best = p;
            }
        }
    }
    best
}

#[test]
fn c11_a_styled_inline_wrapper_gives_its_text_its_style() {
    let body = "<p>a <span style=\"display:contents;font-size:30px;color:red\">B</span> c</p>";
    let pdf = render("", body);
    let found = words(&pdf);
    assert_eq!(texts(&found), ["a", "B", "c"]);
    let b = &found[1];
    // 30 px is 22.5 pt: the glyph box of Inter is about 1.21 em tall.
    let height = b.y1 - b.y0;
    assert!(
        (24.0..=30.0).contains(&height),
        "B is {height:.1} pt tall, a 30 px glyph box is about 27 pt"
    );
    let (r, g, bl) = darkest(&pdf, b.x0, b.y0, b.x1, b.y1);
    assert!(
        r > 200 && g < 80 && bl < 80,
        "the darkest pixel of B is ({r}, {g}, {bl}), not red"
    );
    // The same for a wrapper in an anonymous block (mixed content).
    let body =
        "<div>a <span style=\"display:contents;font-size:30px;color:red\">B</span> c<p>d</p></div>";
    let pdf = render("", body);
    let found = words(&pdf);
    let b = found.iter().find(|w| w.text == "B").expect("B");
    assert_eq!(found.len(), 4);
    assert!((24.0..=30.0).contains(&(b.y1 - b.y0)));
    let (r, g, bl) = darkest(&pdf, b.x0, b.y0, b.x1, b.y1);
    assert!(r > 200 && g < 80 && bl < 80, "({r}, {g}, {bl}) is not red");
}

// --- C12: pseudo-elements -----------------------------------------------------------------------------------------------

#[test]
fn c12_before_and_after_of_a_wrapper() {
    let css = ".x::before { content: \"Pred\" } .x::after { content: \"Po\" }";
    // Block context: the pseudo-elements are inline, so they and the blocks around them take one anonymous block
    // each; Chrome prints Pred / blk / Po on three lines.
    let pdf = render(
        css,
        "<div class=\"x\" style=\"display:contents\"><p>blk</p></div>",
    );
    assert_eq!(line_texts(&words(&pdf)), ["Pred", "blk", "Po"]);
    // Inline context (the wrapper is the only content of its parent).
    let pdf = render(
        css,
        "<div><span class=\"x\" style=\"display:contents\">inl</span></div>",
    );
    assert_eq!(line_texts(&words(&pdf)), ["PredinlPo"]);
    // Mixed context.
    let pdf = render(
        css,
        "<div>m <span class=\"x\" style=\"display:contents\">mix <i>i</i></span><p>q</p></div>",
    );
    assert_eq!(line_texts(&words(&pdf)), ["m Predmix iPo", "q"]);
}

// --- C13: a running element in a wrapper ---------------------------------------------------------------------------------

#[test]
fn c13_a_running_header_in_a_wrapper_is_in_the_margin_only() {
    let css = ".run { position: running(hdr) } \
        @page { size: A4; margin: 25mm 15mm; @top-center { content: element(hdr); font-size: 8pt } }";
    let body = "<w><div class=\"run\">RUNHDR</div><p>one</p></w><p>two</p>";
    let with = render(css, &wrapped(body));
    let without = render(css, &plain(body));
    let found = words(&with);
    assert_eq!(found.iter().filter(|w| w.text == "RUNHDR").count(), 1);
    let hdr = found.iter().find(|w| w.text == "RUNHDR").unwrap();
    assert!(
        hdr.y1 < 25.0 / 25.4 * 72.0,
        "the header is in the top margin: {hdr:?}"
    );
    let flow: Vec<&Word> = found.iter().filter(|w| w.text != "RUNHDR").collect();
    assert_eq!(
        flow.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_same_boxes("C13", &with, &without);
}

// --- C14: many lines across pages --------------------------------------------------------------------------------------

#[test]
fn c14_a_long_wrapper_paginates_like_the_unwrapped_content() {
    let mut inner = String::new();
    let mut expected = Vec::new();
    for i in 1..=150 {
        if i % 7 == 0 {
            inner.push_str(&format!("run{i:03} <b>bold{i:03}</b> "));
            expected.push(format!("run{i:03}"));
            expected.push(format!("bold{i:03}"));
        } else {
            inner.push_str(&format!("<p>Line{i:03}</p>"));
            expected.push(format!("Line{i:03}"));
        }
    }
    let body = format!("<w>{inner}</w>");
    let with = render("", &wrapped(&body));
    let without = render("", &plain(&body));
    let found = words(&with);
    assert_eq!(
        texts(&found),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_same_boxes("C14", &with, &without);
    assert_eq!(page_count(&with), page_count(&without));
    assert!(page_count(&with) >= 3, "about 3 pages");
}

// --- C17: multicol (F150 review: the multicol walks of fulgur read the DOM children, not the box tree) ----------------

#[test]
fn c17_a_wrapper_in_a_multicol_container_keeps_the_columns() {
    let css = ".mc { columns: 2; column-gap: 20px }";
    let paragraphs =
        |pre: &str, n: usize| -> String { (0..n).map(|i| format!("<p>{pre}{i}</p>")).collect() };
    let cases: Vec<(&str, String, Vec<String>)> = vec![
        (
            "only child",
            format!("<div class=\"mc\"><w>{}</w></div>", paragraphs("m", 10)),
            (0..10).map(|i| format!("m{i}")).collect(),
        ),
        (
            "with a plain sibling",
            format!(
                "<div class=\"mc\"><w>{}</w><p>sibling</p></div>",
                paragraphs("m", 8)
            ),
            (0..8)
                .map(|i| format!("m{i}"))
                .chain(["sibling".to_string()])
                .collect(),
        ),
        (
            "two wrappers",
            format!(
                "<div class=\"mc\"><w><div>{}</div></w><w><div>{}</div></w></div>",
                paragraphs("m", 5),
                paragraphs("n", 5)
            ),
            (0..5)
                .map(|i| format!("m{i}"))
                .chain((0..5).map(|i| format!("n{i}")))
                .collect(),
        ),
        (
            "nested wrappers",
            format!("<div class=\"mc\"><w><w>{}</w></w></div>", paragraphs("m", 12)),
            (0..12).map(|i| format!("m{i}")).collect(),
        ),
        (
            "inline content",
            "<div class=\"mc\"><w>one two <b>three</b> four five six seven eight nine ten</w></div>"
                .to_string(),
            ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"]
                .map(String::from)
                .to_vec(),
        ),
        (
            "taller than a page",
            format!(
                "<div class=\"mc\"><w>{}</w></div>",
                (0..200).map(|i| format!("<p>l{i:03}</p>")).collect::<String>()
            ),
            (0..200).map(|i| format!("l{i:03}")).collect(),
        ),
    ];
    for (what, body, expected) in cases {
        let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
        let pdf = assert_wrapper_is_invisible(&format!("C17 {what}"), css, &body, &expected);
        // Two columns: the words sit at two different x positions.
        let mut xs: Vec<i32> = words(&pdf).iter().map(|w| w.x0.round() as i32).collect();
        xs.sort_unstable();
        xs.dedup();
        assert!(
            what == "inline content" || xs.len() >= 2,
            "{what}: one column only: {xs:?}"
        );
    }
}

// --- C15, C16: the panic and the root ---------------------------------------------------------------------------------

#[test]
fn c15_f150_repros_do_not_panic() {
    let cases: &[(&str, &[&str])] = &[
        // a: the end-of-loop cleanup.
        ("<p>x</p> <w><div><p>y</p></div></w>", &["x", "y"]),
        // b: the cleanup before a block sibling.
        (
            "<div> <w><div><p>y</p></div></w><p>z</p></div>",
            &["y", "z"],
        ),
        // c: an orphan table in a wrapper.
        (
            "<p>x</p> <table style=\"display:contents\"><tr></tr></table>",
            &["x"],
        ),
        // c': with content.
        (
            "<p>x</p> <table style=\"display:contents\"><tr><td>cell</td></tr></table>",
            &["x", "cell"],
        ),
        // Newlines and indents between every tag.
        (
            "\n  <p>x</p>\n  <w>\n    <div>\n      <p>y</p>\n    </div>\n  </w>\n  <p>z</p>\n  <w>\n    <div> t </div>\n  </w>\n",
            &["x", "y", "z", "t"],
        ),
        // An inline holding a block, after whitespace.
        (
            "<p>x</p> <w><span><div>y</div></span></w> <p>z</p>",
            &["x", "y", "z"],
        ),
        // A list in a wrapper.
        ("<p>x</p> <w><ul><li>y</li></ul></w>", &["x", "•", "y"]),
    ];
    for (body, expected) in cases {
        let html = wrapped(body);
        let pdf =
            try_render("", &html).unwrap_or_else(|e| panic!("render of {html:?} failed: {e}"));
        let found = words(&pdf);
        let mut got: Vec<&str> = texts(&found);
        got.sort_unstable();
        let mut want: Vec<&str> = expected.to_vec();
        want.sort_unstable();
        assert_eq!(got, want, "{html:?}");
    }
}

#[test]
fn c16_display_contents_on_body() {
    let html = "<!doctype html><html><body style=\"display:contents\"><p>Faktura</p><p>Celkem</p></body></html>";
    let pdf = renderer("")
        .render(html, &RenderOptions::default())
        .expect("render");
    assert_eq!(line_texts(&words(&pdf)), ["Faktura", "Celkem"]);
}

// --- Fuzz: random trees with and without wrappers -----------------------------------------------------------------------

/// xorshift64*: no dependency, the same trees on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

struct Gen {
    rng: Rng,
    word: usize,
}

impl Gen {
    fn word(&mut self) -> String {
        self.word += 1;
        format!("w{:04}", self.word)
    }

    /// Whitespace between two siblings: a space, or a newline with indentation (all collapse to one space).
    fn ws(&mut self) -> &'static str {
        ["", " ", "\n  ", " ", "\n\t "][self.rng.below(5) as usize]
    }

    /// Inline content that starts and ends with a word or an element, never with whitespace: Parley trims the
    /// whitespace at the edges of a span (F154), which would glue words together in wrapped and plain alike.
    fn inline_tight(&mut self, depth: u32) -> String {
        let n = 1 + self.rng.below(3);
        let mut out = String::new();
        for i in 0..n {
            if i > 0 {
                out.push_str(if self.rng.chance(20) { "\n  " } else { " " });
            }
            let pick = if depth == 0 { 0 } else { self.rng.below(6) };
            match pick {
                0..=2 => out.push_str(&self.word()),
                3 => out.push_str(&format!("<span>{}</span>", self.inline_tight(depth - 1))),
                4 => out.push_str(&format!("<b>{}</b>", self.inline_tight(depth - 1))),
                _ => out.push_str(&format!("<s>{}</s>", self.inline_tight(depth - 1))),
            }
        }
        out
    }

    /// A run of inline content in a block context: it has whitespace at both ends, so two runs, or a run and a
    /// word of a neighbouring element, are never glued together (that would be the same in Chrome).
    fn inline_run(&mut self, depth: u32) -> String {
        format!(" {} ", self.inline_tight(depth))
    }

    /// Children of a block, flex or grid container.
    fn blocks(&mut self, depth: u32) -> String {
        let n = 1 + self.rng.below(4);
        let mut out = String::new();
        for _ in 0..n {
            out.push_str(self.ws());
            let pick = if depth == 0 { 0 } else { self.rng.below(14) };
            match pick {
                0 | 1 => out.push_str(&format!("<p>{}</p>", self.inline_tight(2))),
                // Mixed content: raw text among blocks.
                2 | 3 => out.push_str(&self.inline_run(2)),
                4 | 5 => out.push_str(&format!("<div>{}</div>", self.blocks(depth - 1))),
                6 | 7 => out.push_str(&format!("<w>{}</w>", self.blocks(depth - 1))),
                8 => out.push_str(&format!(
                    "<div style=\"display:flex;flex-wrap:wrap;gap:12px\">{}</div>",
                    self.blocks(depth - 1)
                )),
                9 => out.push_str(&format!(
                    "<div style=\"display:grid;grid-template-columns:90px 90px;gap:12px\">{}</div>",
                    self.blocks(depth - 1)
                )),
                10 => out.push_str(&self.table(depth - 1)),
                11 => out.push_str(&self.list(depth - 1)),
                _ => out.push_str(&format!("<w><w>{}</w></w>", self.blocks(depth - 1))),
            }
        }
        out
    }

    /// The content of a multicol container: paragraphs, plain divs and wrappers (the flow of F30d carries lines and
    /// block heights; other content in columns has gaps of its own, F30d-b).
    fn column_blocks(&mut self, depth: u32) -> String {
        let n = 1 + self.rng.below(4);
        let mut out = String::new();
        for _ in 0..n {
            out.push_str(self.ws());
            let pick = if depth == 0 { 0 } else { self.rng.below(6) };
            match pick {
                0..=2 => out.push_str(&format!("<p>{}</p>", self.inline_tight(2))),
                3 => out.push_str(&format!("<div>{}</div>", self.column_blocks(depth - 1))),
                _ => out.push_str(&format!("<w>{}</w>", self.column_blocks(depth - 1))),
            }
        }
        out
    }

    fn list(&mut self, depth: u32) -> String {
        let tag = if self.rng.chance(50) { "ul" } else { "ol" };
        let n = 1 + self.rng.below(3);
        let mut items = String::new();
        for _ in 0..n {
            items.push_str(self.ws());
            let li = if self.rng.chance(60) {
                format!("<li>{}</li>", self.inline_tight(1))
            } else {
                format!("<li>{}</li>", self.blocks(depth.min(1)))
            };
            if self.rng.chance(35) {
                items.push_str(&format!("<w>{li}</w>"));
            } else {
                items.push_str(&li);
            }
        }
        format!("<{tag}>{items}</{tag}>")
    }

    /// A `display: table` made of divs (a `<table>` tag would make html5ever move a `<div>` out of it).
    fn table(&mut self, depth: u32) -> String {
        let mut rows = String::new();
        for _ in 0..1 + self.rng.below(2) {
            let mut cells = String::new();
            for _ in 0..1 + self.rng.below(2) {
                let cell = format!(
                    "<div style=\"display:table-cell;padding:0 8px\">{}</div>",
                    self.blocks(depth.min(1))
                );
                cells.push_str(self.ws());
                if self.rng.chance(25) {
                    cells.push_str(&format!("<w>{cell}</w>"));
                } else {
                    cells.push_str(&cell);
                }
            }
            let row = format!("<div style=\"display:table-row\">{cells}</div>");
            rows.push_str(self.ws());
            if self.rng.chance(25) {
                rows.push_str(&format!("<w>{row}</w>"));
            } else {
                rows.push_str(&row);
            }
        }
        format!("<div style=\"display:table;width:100%;table-layout:fixed\">{rows}</div>")
    }
}

/// Keeps flex items and cells from growing past the page edge (words there are not printed, which is not what
/// the fuzz test looks for).
const FUZZ_CSS: &str = "p { max-width: 150px }";

/// The seeds of the fuzz test: `DISPLAY_CONTENTS_FUZZ_SEEDS` overrides the 64 of CI.
fn fuzz_seeds() -> u64 {
    std::env::var("DISPLAY_CONTENTS_FUZZ_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64)
}

/// Renders the seeds `from..to`: no panic, every word once, and the boxes of the wrapped and of the plain document
/// are the same.
fn fuzz(from: u64, to: u64) {
    let mut failures = Vec::new();
    for seed in from..to {
        let mut g = Gen {
            rng: Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1),
            word: 0,
        };
        let mut body = g.blocks(4);
        // A multicol container at the top level (nested in flex, tables or other multicol containers the F30d flow
        // has gaps of its own that are not about wrappers, F30d-b).
        let has_columns = g.rng.chance(40);
        if has_columns {
            let columns = g.column_blocks(3);
            body.push_str(&format!(
                "<div style=\"columns:2;column-gap:20px\">{columns}</div>"
            ));
        }
        let expected = g.word;
        // `DISPLAY_CONTENTS_FUZZ_DUMP=<dir>` writes every body (with the `<w>` / `<s>` markers), for bisecting.
        if let Ok(dir) = std::env::var("DISPLAY_CONTENTS_FUZZ_DUMP") {
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{seed:04}.txt")),
                &body,
            )
            .expect("write dump");
        }
        let with = match try_render(FUZZ_CSS, &wrapped(&body)) {
            Ok(pdf) => pdf,
            Err(e) => {
                failures.push(format!("seed {seed}: wrapped render failed: {e}\n{body}"));
                continue;
            }
        };
        let without = match try_render(FUZZ_CSS, &plain(&body)) {
            Ok(pdf) => pdf,
            Err(e) => {
                failures.push(format!("seed {seed}: plain render failed: {e}\n{body}"));
                continue;
            }
        };
        let (a, b) = (words(&with), words(&without));
        // The tokens of each document, each at most once (the numbers of a list are words too: dropped).
        let tokens_of = |found: &[Word]| -> Vec<String> {
            let mut seen: Vec<String> = found
                .iter()
                .filter(|w| w.text.starts_with('w') && w.text.len() == 5)
                .map(|w| w.text.clone())
                .collect();
            seen.sort_unstable();
            seen
        };
        let (seen_a, seen_b) = (tokens_of(&a), tokens_of(&b));
        let mut dedup = seen_a.clone();
        dedup.dedup();
        if dedup.len() != seen_a.len() {
            failures.push(format!("seed {seed}: a word is printed twice\n{body}"));
        }
        // A word that runs past the right edge of the page is not printed: in a few trees (a `flex-wrap` row in a
        // table cell, F155) the plain document does that too. The wrapper must not change which words are lost.
        if seen_a != seen_b {
            let lost: Vec<&String> = seen_b.iter().filter(|w| !seen_a.contains(w)).collect();
            failures.push(format!(
                "seed {seed}: wrapped prints {} words, plain {}; only plain prints {lost:?}\n{body}",
                seen_a.len(),
                seen_b.len()
            ));
        }
        // (A multicol container loses words of its own in the plain document too, F161: no floor there.)
        if !has_columns && seen_b.len() * 100 < expected * 97 {
            failures.push(format!(
                "seed {seed}: plain prints {} of {expected} words\n{body}",
                seen_b.len()
            ));
        }
        // Boxes of the tokens, by text.
        let tokens =
            |ws: &[Word]| -> std::collections::BTreeMap<String, (usize, f32, f32, f32, f32)> {
                ws.iter()
                    .filter(|w| w.text.starts_with('w') && w.text.len() == 5)
                    .map(|w| (w.text.clone(), (w.page, w.x0, w.y0, w.x1, w.y1)))
                    .collect()
            };
        let (ta, tb) = (tokens(&a), tokens(&b));
        for (word, p) in &ta {
            if let Some(q) = tb.get(word) {
                let near = |x: f32, y: f32| (x - y).abs() <= EPS_PT;
                if !(p.0 == q.0
                    && near(p.1, q.1)
                    && near(p.2, q.2)
                    && near(p.3, q.3)
                    && near(p.4, q.4))
                {
                    failures.push(format!(
                        "seed {seed}: {word} moved: wrapped {p:?}, plain {q:?}\n{body}"
                    ));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} seeds failed:\n{}",
        failures.len(),
        to - from,
        failures.join("\n---\n")
    );
}

#[test]
fn fuzz_wrappers_never_panic_lose_words_or_move_anything() {
    fuzz(0, fuzz_seeds());
}

/// The long run: `cargo test --release --test display_contents -- --ignored fuzz_long`.
#[test]
#[ignore = "2000 seeds, about two minutes"]
fn fuzz_long() {
    fuzz(1000, 3000);
}
