//! An `inline-block` / `inline-flex` taller than a page that shares its line with text: the text on the line sits at the
//! line's baseline, which is the baseline of the box's last line box for an `inline-block` (CSS 2.1 §10.8.1) and its first
//! baseline for an `inline-flex` (css-align-3 §9.1, css-inline-3 `baseline-source`), and is painted once on the page that
//! holds that baseline. css-break-3 §4.1 lets a UA that splits a line box too tall for a fragmentainer fragment the
//! inline-level boxes on it. Before F213b the line was painted once, on the page where the box starts, so the baseline
//! landed below that page's edge and the text of the line was printed nowhere (`START`, `TAIL`, `Lead words`). Plan:
//! docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md (section 7 has the probes behind every number).
//!
//! The page is A4 with 15 mm margins (content box 1009.13 px), `body { font: 14px/20px Inter }`. The box has 130 block
//! children `T000`..`T129` (2602 px with its border, 3 pages), 100 px wide. Chrome m153 numbers (`pdftotext -bbox`,
//! the settings of `scripts/reference/render.mjs`): `<p>START BOX TAIL</p><p>AFTER</p>` puts T000 at the top of page 1,
//! T129 at 464.6 pt on page 3, `START` and `TAIL` on page 3 at 464.6 pt (the baseline of T129) and AFTER on page 3 at
//! 480.3 pt. Chrome slices the box and prints the line at each page edge twice (`T050`); we print every word once
//! (ADR-0004), so the tests compare pages and the y of the words with each other, not the Chrome slice.
//!
//! Where Chrome and we differ and the tests do not pin Chrome (plan sections 2.2 and 6): Chrome moves a tall line that
//! has something above it on its page to the next page; a tall line that is the first of its paragraph starts in place
//! here (ADR-0004 A3). Chrome puts a `vertical-align: top` badge on the box's first page (P6).
//!
//! The tests named `r_*` are red before their step of the plan and are `#[ignore]`d with the step that turns them green;
//! the `g_*` tests are guards that hold before and after.

use htmltopdf::{RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// One printed word: its 1-based page, the top of its box in pt from the page top, and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    y_min: f32,
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

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";

/// `n` one-line block children `T000`, `T001`, ...
fn blocks(n: u32) -> String {
    (0..n)
        .map(|i| format!("<span style=\"display:block\">T{i:03}</span>"))
        .collect()
}

/// The tall `inline-block`: 100 px wide, 1 px border, `n` block children.
fn tall_box(n: u32) -> String {
    format!(
        "<span style=\"display:inline-block;width:100px;border:1px solid\">{}</span>",
        blocks(n)
    )
}

/// The same box made of text lines `L01<br>L02...`: the box itself is the inline formatting context.
fn tall_text_box(n: u32) -> String {
    let lines = (1..=n)
        .map(|i| format!("L{i:02}"))
        .collect::<Vec<_>>()
        .join("<br>");
    format!("<span style=\"display:inline-block;width:100px;border:1px solid\">{lines}</span>")
}

/// Every word `prefix`NNN with NNN in `from..to` is printed exactly once (a lost word or a repeated one fails).
fn assert_each_once(words: &[Word], prefix: &str, from: u32, to: u32, width: usize) {
    for i in from..to {
        find(words, &format!("{prefix}{i:0width$}"));
    }
}

/// `a` and `b` are on the same page and their tops are within `tol` pt of each other.
fn assert_level(words: &[Word], a: &str, b: &str, tol: f32) {
    let (wa, wb) = (find(words, a), find(words, b));
    assert_eq!(
        wa.page, wb.page,
        "`{a}` is on page {} and `{b}` on page {}: they share the line baseline",
        wa.page, wb.page
    );
    assert!(
        (wa.y_min - wb.y_min).abs() <= tol,
        "`{a}` is at {:.1} pt and `{b}` at {:.1} pt: not on one baseline",
        wa.y_min,
        wb.y_min
    );
}

#[test]
#[ignore = "red until F213b step 3 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t1_one_line_carrier_text_sits_on_the_baseline_page_of_the_box() {
    // P1: `<p>START BOX TAIL</p><p>AFTER</p>`, also after a one-line paragraph. Chrome (after `S` it moves the whole line
    // to page 2, which we do not copy: the box starts in place): START and TAIL level with T129 (464.6 pt, page 3),
    // AFTER at 480.3 pt below them, T050 printed twice. Main printed T000..T129 once and START / TAIL nowhere: the line
    // was painted once, on page 1, and its baseline is on page 3.
    for (label, before) in [("page top", ""), ("after a paragraph", "<p>S</p>")] {
        let pdf = render(&format!(
            "{before}<p>START {} TAIL</p><p>AFTER</p>",
            tall_box(130)
        ));
        let words = printed_words(&pdf);
        assert_each_once(&words, "T", 0, 130, 3);
        assert_level(&words, "START", "T129", 0.5);
        assert_level(&words, "TAIL", "T129", 0.5);
        let (last, after) = (find(&words, "T129"), find(&words, "AFTER"));
        assert_eq!(
            after.page, last.page,
            "{label}: AFTER follows on the last page of the box"
        );
        assert!(
            after.y_min > last.y_min + 10.0,
            "{label}: AFTER ({:.1} pt) is below the line ({:.1} pt)",
            after.y_min,
            last.y_min
        );
    }
}

#[test]
#[ignore = "red until F213b step 3 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t1_the_baseline_is_where_the_last_line_box_is_even_when_a_margin_pushes_it_on() {
    // P2b: the box is 1200 px high and its only child, `BOXLOW`, has `margin-top: 1100px`, so its line is on page 2.
    // Chrome puts START and TAIL at 112.1 pt on page 2 next to BOXLOW and AFTER at 187.8 pt (the exact y differs here:
    // Chrome keeps the margin at the page edge, we truncate it, css-break-3 section 5.2). Main printed BOXLOW and AFTER
    // and no START / TAIL: the line was painted on page 1.
    let pdf = render(
        "<p>START <span style=\"display:inline-block;width:100px;height:1200px;border:1px solid\">\
         <span style=\"display:block;margin-top:1100px\">BOXLOW</span></span> TAIL</p><p>AFTER</p>",
    );
    let words = printed_words(&pdf);
    assert_eq!(find(&words, "BOXLOW").page, 2, "the baseline is on page 2");
    assert_level(&words, "START", "BOXLOW", 0.5);
    assert_level(&words, "TAIL", "BOXLOW", 0.5);
    assert!(
        find(&words, "AFTER").y_min > find(&words, "BOXLOW").y_min,
        "AFTER follows the line"
    );
}

#[test]
#[ignore = "red until F213b step 3 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t4_f213d_b_a_fixed_box_with_a_short_child_keeps_its_line_and_pushes_the_next_one_below_it() {
    // P2 / F213d-b: `<p>START <box height 1200px, one child BOXTOP> TAIL</p><p>AFTER</p>` at a page top. Chrome: START,
    // BOXTOP and TAIL at 44.6 pt on page 1, the box goes on to page 2 and AFTER is at 187.8 pt on page 2, below the end
    // of the box (1202 px). Main prints the three words (F183c) but puts AFTER at 43.6 pt on page 2, over the rest of
    // the box. After a one-line paragraph Chrome moves the whole line to page 2 (START at 44.6 pt, AFTER on page 3 at
    // 187.8 pt); the box starts in place here, the same words on page 1 and AFTER where the box ends.
    let tall = "<span style=\"display:inline-block;width:100px;height:1200px;border:1px solid\">\
                <span style=\"display:block\">BOXTOP</span></span>";
    let pdf = render(&format!("<p>START {tall} TAIL</p><p>AFTER</p>"));
    let words = printed_words(&pdf);
    assert_eq!(find(&words, "START").page, 1);
    assert_level(&words, "START", "BOXTOP", 0.5);
    assert_level(&words, "TAIL", "BOXTOP", 0.5);
    let after = find(&words, "AFTER");
    assert_eq!(after.page, 2, "AFTER is on page 2, where the box ends");
    assert!(
        (after.y_min - 187.8).abs() <= 2.0,
        "AFTER is at {:.1} pt, Chrome has it at 187.8 pt below the end of the box",
        after.y_min
    );
    assert_eq!(page_count(&pdf), 2);
}

#[test]
#[ignore = "red until F213b step 3 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t6_an_inline_flex_box_puts_its_line_on_the_page_of_its_first_baseline() {
    // P5b: `inline-flex` 1200 px high, `align-items: flex-end`, one item `FLEXITEM` at the bottom of the box, so its first
    // baseline is on page 2. Chrome: START, FLEXITEM and TAIL at 172.1 pt on page 2, AFTER at 187.8 pt. Main printed
    // FLEXITEM and AFTER and no START / TAIL (they were painted on page 1, below the edge of nothing: the line is
    // there but its baseline is not).
    let pdf = render(
        "<p>START <span style=\"display:inline-flex;align-items:flex-end;width:100px;height:1200px;border:1px solid\">\
         <span>FLEXITEM</span></span> TAIL</p><p>AFTER</p>",
    );
    let words = printed_words(&pdf);
    assert_eq!(
        find(&words, "FLEXITEM").page,
        2,
        "the first baseline is on page 2"
    );
    assert_level(&words, "START", "FLEXITEM", 0.5);
    assert_level(&words, "TAIL", "FLEXITEM", 0.5);
    assert!(
        find(&words, "AFTER").y_min > find(&words, "FLEXITEM").y_min,
        "AFTER follows the line"
    );
}

#[test]
fn g_t6_an_inline_flex_column_keeps_its_line_on_the_first_page() {
    // P5: `inline-flex` column of 130 items sharing its line. Its first baseline is the first item's, on the first page:
    // Chrome has START, TAIL and T000 at 44.6 pt on page 1 and AFTER at 480.3 pt on page 3. Main agrees on the page of
    // START and TAIL and keeps it (the barrier of `inline-flex` is not touched, plan section 3.2).
    let items: String = (0..130).map(|i| format!("<span>T{i:03}</span>")).collect();
    let pdf = render(&format!(
        "<p>START <span style=\"display:inline-flex;flex-direction:column;width:100px;border:1px solid\">{items}</span> TAIL</p><p>AFTER</p>"
    ));
    let words = printed_words(&pdf);
    assert_each_once(&words, "T", 0, 130, 3);
    assert_eq!(find(&words, "START").page, 1);
    assert_level(&words, "START", "T000", 0.5);
    assert_level(&words, "TAIL", "T000", 0.5);
    assert_eq!(find(&words, "AFTER").page, 3);
}

#[test]
#[ignore = "red until F213b step 4 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t2_a_multi_line_paragraph_prints_the_text_of_the_tall_line_level_with_the_box_end() {
    // P3: `<div style="width:300px">Lead words before BOX trailing words after the box in text.</div><p>AFTER</p>`. Chrome:
    // the box is sliced over pages 1 to 3, `Lead` and `trailing` are at 464.6 pt on page 3 level with T129 (the line
    // holds them all: 300 px is enough), the second line of the div is at 480.3 pt and AFTER at 495.3 pt on page 3.
    // Main printed T000..T052 and none of the words of the div (plan section 1).
    let pdf = render(&format!(
        "<div style=\"width:300px\">Lead words before {} trailing words after the box in text.</div><p>AFTER</p>",
        tall_box(130)
    ));
    let words = printed_words(&pdf);
    assert_each_once(&words, "T", 0, 130, 3);
    for w in ["Lead", "words", "before", "trailing"] {
        let hits: Vec<_> = words.iter().filter(|x| x.text == w).collect();
        assert!(!hits.is_empty(), "`{w}` is printed");
    }
    assert_level(&words, "Lead", "T129", 0.5);
    assert_level(&words, "trailing", "T129", 0.5);
    let after = find(&words, "AFTER");
    assert_eq!(after.page, find(&words, "T129").page);
    assert!(
        after.y_min > find(&words, "T129").y_min + 25.0,
        "AFTER is below the second line of the div"
    );
}

#[test]
#[ignore = "red until F213b step 4 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t3_a_tall_line_that_follows_other_lines_starts_a_fresh_page() {
    // P3: the tall line is the 3rd of 4 (`X1<br>X2<br>Lead words before BOX trailing words<br>X4`). Chrome: X1 and X2 on
    // page 1, T000 at the top of page 2 (the tall line does not start under X2), `Lead` and `trailing` on page 4 at
    // 464.6 pt level with T129, X4 at 495.3 pt below them. Main printed X1, X2 and T000..T052 on page 2 and lost the rest.
    let pdf = render(&format!(
        "<div style=\"width:300px\">X1 first line<br>X2 second line<br>Lead words before {} trailing words<br>X4 last line</div><p>AFTER</p>",
        tall_box(130)
    ));
    let words = printed_words(&pdf);
    assert_each_once(&words, "T", 0, 130, 3);
    assert_eq!(find(&words, "X1").page, 1);
    assert_eq!(find(&words, "X2").page, 1);
    let t0 = find(&words, "T000");
    assert_eq!(t0.page, 2, "the tall line starts a fresh page");
    assert!(
        t0.y_min < 50.0,
        "T000 is at the top of page 2 ({:.1} pt)",
        t0.y_min
    );
    assert_level(&words, "Lead", "T129", 0.5);
    assert_level(&words, "trailing", "T129", 0.5);
    let (x4, last) = (find(&words, "X4"), find(&words, "T129"));
    assert_eq!(x4.page, last.page);
    assert!(x4.y_min > last.y_min + 25.0, "X4 is below the tall line");
}

#[test]
#[ignore = "red until F213b step 5 (plan docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md)"]
fn r_t5_text_lines_inside_the_box_are_printed_once_and_the_line_is_on_the_last_one() {
    // P4: `<p>START <box of 80 text lines L01..L80> TAIL</p><p>AFTER</p>`. Chrome: 3 pages (a blank one follows), L01
    // at the top of page 1, L51 on both pages 1 and 2, L80 at 472.1 pt on page 2, START and TAIL level with it and
    // AFTER at 487.8 pt. Main printed L01..L53 and nothing else of the box, START or TAIL (the box shares its line,
    // so it is not entered: the barrier of ADR-0004, plan step 5).
    let pdf = render(&format!(
        "<p>START {} TAIL</p><p>AFTER</p>",
        tall_text_box(80)
    ));
    let words = printed_words(&pdf);
    for i in 1..=80 {
        find(&words, &format!("L{i:02}"));
    }
    assert_level(&words, "START", "L80", 0.5);
    assert_level(&words, "TAIL", "L80", 0.5);
    assert!(find(&words, "AFTER").y_min > find(&words, "L80").y_min);
}
