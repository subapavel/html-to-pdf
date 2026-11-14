//! A fixed-size atomic inline in the text of a flex item counts in the item's height and keeps its width (F217).
//!
//! Taffy lays a flex item out in `SizingMode::ContentSize` ("the caller already applied this node's own size, so
//! ignore `size`"). Blitz handed that mode on to the inline boxes in the item's lines, so a non-replaced
//! `inline-block` measured without its `width` / `height` / `min-height`: the row ended at its text line and the block
//! after it was painted over the box (a QR box in a payment row). CSS 2.1 §10.3.9 and §10.6.6 give an `inline-block`
//! its specified size; §10.8.1 puts its margin box into the line box; css-flexbox-1 §9.4 step 7 lays the item out "as
//! if it were an in-flow block-level box", which suspends nothing of its descendants. Plan:
//! docs/plans/2026-11-03-f217-fixed-inline-block-in-flex-item.md.
//!
//! The page is A4 with 15 mm margins, `body { font: 14px/20px Inter }`. Every case is
//! `<p>BEFORE</p> row <p>AFTER</p>` where the row is a flex container with a 128 px label `Platba:` and an item that
//! holds `Zaplaťte QR kódem BOX nebo převodem.` (BOX in the 112 px cases has a 1 px border: a 114 px margin box).
//! `dy` is the distance of the top of `AFTER` from the top of `BEFORE` in pt. The spec gives a lower bound that is
//! independent of either engine: one text line (15 pt) plus the margin box of the box (0.75 pt per px). The y of a word
//! is not compared with Chrome's directly, because the glyph boxes of the two renderers differ; `dy` is (both
//! words are the same height in both). Chrome constants are m153's, measured with `scripts/reference/render.mjs`'s
//! settings and `pdftotext -bbox` on the plan's probes.
//!
//! The `r*` tests are red before the fix; the `g*` tests are guards that hold before and after (the shapes that
//! never went wrong; byte equality of their PDFs before and after is checked by the sweep of the plan, section 4.4).

use htmltopdf::{RenderOptions, Renderer};

const RED60: &[u8] = include_bytes!("data/red60.png");

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";

/// Chrome m153, probe `d.html`: `AFTER` minus `BEFORE`, 145.16 - 40.91 pt.
const CHROME_DY_D: f32 = 104.25;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inter() -> Vec<u8> {
    let path = root().join("fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .image("red60.png", RED60.to_vec())
        .strict_images(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

#[derive(Clone, Debug)]
struct Word {
    x_min: f32,
    y_min: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-fib-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
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
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push(Word {
                x_min: attr(attrs, "xMin"),
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

/// The single printed `text`; panics when it is lost or printed twice.
fn find(words: &[Word], text: &str) -> Word {
    let hits: Vec<&Word> = words.iter().filter(|w| w.text == text).collect();
    assert_eq!(
        hits.len(),
        1,
        "`{text}` must be printed exactly once (words: {:?})",
        words.iter().map(|w| &w.text).collect::<Vec<_>>()
    );
    hits[0].clone()
}

/// `<p>BEFORE</p>` + a flex row (`container` is the container's style, `item` the style of the item that holds `box_html`)
/// + `<p>AFTER</p>`, and the words printed.
fn row(container: &str, item: &str, box_html: &str) -> Vec<Word> {
    printed_words(&render(&format!(
        "<p>BEFORE</p><div style=\"display:flex;gap:16px;{container}\">\
         <div style=\"width:128px;flex-shrink:0\">Platba:</div>\
         <div style=\"{item}\">Zaplaťte QR kódem {box_html} nebo převodem.</div></div><p>AFTER</p>"
    )))
}

fn dy(words: &[Word]) -> f32 {
    find(words, "AFTER").y_min - find(words, "BEFORE").y_min
}

/// One text line above the row plus the box's margin box of `box_px` CSS px.
fn at_least(box_px: f32) -> f32 {
    15.0 + box_px * 0.75
}

fn assert_clears(words: &[Word], box_px: f32) {
    let d = dy(words);
    assert!(
        d >= at_least(box_px) - 0.3,
        "AFTER is {d:.1} pt below BEFORE; one line and a {box_px} px box need at least {:.1} pt",
        at_least(box_px)
    );
}

const BOX: &str = "display:inline-block;width:112px;height:112px;border:1px solid #000";

// --- R: the finding ----------------------------------------------------------------------------------------------

#[test]
fn r1_an_empty_fixed_box_in_a_flex_item_counts_in_the_row() {
    // Probe d.html. Chrome: AFTER 104.25 pt below BEFORE. Before the fix 30.0 pt: the next line was painted over the box.
    // After it: 100.5 pt, the lower bound exactly. The 3.75 pt (5 px) to Chrome is the strut's descent under the baseline
    // that the line box of an `inline-block` lacks here (F183b): the block path has the same gap on main (probe e.html,
    // the same markup in blocks: Chrome 119.25 pt, ours 115.5 pt). So the test is the bound and a ceiling, not Chrome's number.
    let words = row("", "", &format!("<span style=\"{BOX}\"></span>"));
    assert_clears(&words, 114.0);
    let d = dy(&words);
    assert!(
        d <= CHROME_DY_D + 2.0,
        "dy {d:.1} pt, Chrome {CHROME_DY_D} pt: the row is too tall"
    );
}

#[test]
fn r2_a_fixed_box_with_text_counts_in_the_row() {
    // No Chrome number: Chrome seats this box on the baseline of its inner text line (F183c), which is not what we do.
    let words = row("", "", &format!("<span style=\"{BOX}\">QR</span>"));
    assert_clears(&words, 114.0);
}

#[test]
fn r3_a_min_height_counts_in_the_row() {
    let words = row(
        "",
        "",
        "<span style=\"display:inline-block;min-height:112px;border:1px solid #000\">QR</span>",
    );
    assert_clears(&words, 114.0);
}

#[test]
fn r4_an_inline_flex_box_counts_in_the_row() {
    let words = row(
        "",
        "",
        "<span style=\"display:inline-flex;width:112px;height:112px;border:1px solid #000\">QR</span>",
    );
    assert_clears(&words, 114.0);
}

#[test]
fn r5_a_fixed_width_is_kept_in_a_flex_item() {
    // 300 px = 225 pt between the start of `kódem` and the start of `nebo` at least (the box and two spaces and the word).
    let words = row(
        "",
        "",
        "<span style=\"display:inline-block;width:300px;border:1px solid #000\">QR</span>",
    );
    let gap = find(&words, "nebo").x_min - find(&words, "kódem").x_min;
    assert!(
        gap >= 225.0,
        "kódem to nebo is {gap:.1} pt, the box alone is 225.75 pt"
    );
}

#[test]
fn r6_the_same_in_a_column_flex_item() {
    // The label is its own item above: the item with the box is the second one.
    let words = printed_words(&render(&format!(
        "<p>BEFORE</p><div style=\"display:flex;flex-direction:column\">\
         <div>Platba:</div><div>Zaplaťte QR kódem <span style=\"{BOX}\"></span> nebo převodem.</div></div><p>AFTER</p>"
    )));
    // Two items: one text line (15 pt) of the label, then the second item with the box (85.5 pt and a line).
    let d = dy(&words);
    assert!(
        d >= 15.0 + 15.0 + 85.5 - 0.3,
        "AFTER is {d:.1} pt below BEFORE; two lines' worth and the box need at least 115.5 pt"
    );
}

#[test]
fn r7_fixture_46_payment_rows_end_below_their_boxes() {
    // Fixture 46: two payment rows, a 112 px QR box (padding, border, 64 px svg) and an empty 112 px stamp box. The
    // paragraph after each row starts at least a box (84 pt) below the row's label word.
    let name = "46-qr-flex-payment-row";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let mut b = Renderer::builder()
        .strict_fonts(true)
        .strict_images(true)
        .css_file(root().join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css");
    let mut fonts: Vec<_> = std::fs::read_dir(root().join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    let pdf = b
        .build()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let words = printed_words(&pdf);
    for (label, after) in [("Platba:", "Variabilní"), ("Razítko:", "Děkujeme")] {
        let d = find(&words, after).y_min - find(&words, label).y_min;
        assert!(
            d >= 84.0,
            "`{after}` is {d:.1} pt below `{label}`; the 112 px box alone is 84 pt"
        );
    }
}

// --- G: guards, the shapes that were right before --------------------------------------------------------------

#[test]
fn g1_the_same_text_in_a_block_is_unchanged() {
    // Probe c.html: label and text are blocks. Ours was right (the line holds the box).
    let words = printed_words(&render(&format!(
        "<p>BEFORE</p><div><div>Platba:</div><div>Zaplaťte QR kódem <span style=\"{BOX}\">QR</span> nebo převodem.</div></div><p>AFTER</p>"
    )));
    let d = dy(&words);
    assert!(d >= 15.0 + 15.0 + 85.5 - 0.3, "dy {d:.1} pt");
}

#[test]
fn g2_an_auto_height_box_in_a_flex_item_is_unchanged() {
    // Probe b.html: padding 50 px top and bottom, border 1 px, one text line: 122 px.
    let words = row(
        "",
        "",
        "<span style=\"display:inline-block;width:112px;padding:50px 0;border:1px solid #000\">QR</span>",
    );
    assert_clears(&words, 122.0);
}

#[test]
fn g3_an_img_with_a_height_in_a_flex_item_is_unchanged() {
    let words = row("", "", "<img src=\"red60.png\" style=\"height:112px\">");
    assert_clears(&words, 112.0);
}

#[test]
fn g4_a_grid_item_is_unchanged() {
    let words = printed_words(&render(&format!(
        "<p>BEFORE</p><div style=\"display:grid;grid-template-columns:128px auto;gap:16px\">\
         <div>Platba:</div><div>Zaplaťte QR kódem <span style=\"{BOX}\"></span> nebo převodem.</div></div><p>AFTER</p>"
    )));
    assert_clears(&words, 114.0);
}

#[test]
fn g5_a_table_cell_is_unchanged() {
    let words = printed_words(&render(&format!(
        "<p>BEFORE</p><table style=\"border-collapse:separate;border-spacing:0\"><tr>\
         <td style=\"width:128px;padding:0;vertical-align:top\">Platba:</td>\
         <td style=\"padding:0\">Zaplaťte QR kódem <span style=\"{BOX}\"></span> nebo převodem.</td></tr></table><p>AFTER</p>"
    )));
    assert_clears(&words, 114.0);
}

#[test]
fn g6_a_paragraph_in_the_item_is_unchanged() {
    // The text in a `<p>` inside the item: the paragraph is the inline root, laid out by the block path.
    let words = printed_words(&render(&format!(
        "<p>BEFORE</p><div style=\"display:flex;gap:16px\"><div style=\"width:128px;flex-shrink:0\">Platba:</div>\
         <div><p>Zaplaťte QR kódem <span style=\"{BOX}\"></span> nebo převodem.</p></div></div><p>AFTER</p>"
    )));
    assert_clears(&words, 114.0);
}
