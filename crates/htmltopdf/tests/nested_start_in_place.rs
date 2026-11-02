//! A paragraph nested in a block that does not fit the room below it starts there and is split between its lines when
//! its first two lines fit (F30c-n, the nested half of F30c; the body-level half is `pre_advance_split.rs`).
//!
//! css-break-3 §4.1: breaks between siblings (Class A), between line boxes (Class B) and between a container's edge and
//! its children (Class C) are all break opportunities; nesting depth does not rank them. §4.4: with `orphans` / `widows`
//! at 2 a paragraph may start on a page when two of its lines fit and two can be left for the next one. Before F30c-n a
//! paragraph inside `<section>`, a `<li>`, a padded `<section>` or an abspos box moved to the next page whole when it did
//! not fit the room, although the same paragraph in `<body>` starts in place: the same text paged differently depending
//! on whether it had a wrapper.
//!
//! Every number is Chrome's (m141, A4, 15 mm, Inter, `14px/21px`, `p { margin: 0 }`, lines `w0000<br>w0001…`, `AFTER` in
//! a `<p>` below), measured with `out/scratch/f30cn/{gen,sum}.py` (plan §8). Shape A wraps the spacer and the paragraph
//! in one `<section>` (the paragraph follows a sibling inside its wrapper); shape B puts the paragraph first in its own
//! `<section>` after a spacer outside it. A page holds 48 such lines (1008 px against Chrome's 1010 px; ours is 1009.13 px,
//! so the tests keep a room that is 42 px to 43 px or more than 1 px from a line boundary, F30f-c).
//!
//! Where Chrome is not matched, the test says so and pins ours: a 3-line paragraph with two lines of room (F30c-w) and a
//! paragraph in a table cell (F30c-t, a cell is its row's unit).

use htmltopdf::{RenderOptions, Renderer};

/// The content box of A4 with 15 mm margins, in pt from the page top: its bottom.
const CONTENT_BOTTOM: f32 = 799.37;
/// One line of 21 px in pt.
const LINE: f32 = 15.75;

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_with(sheet: bool, body: &str) -> Vec<u8> {
    let head = "@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}";
    let mut b = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true);
    if sheet {
        b = b
            .css_file(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../fixtures/tailwind/tailwind.css"),
            )
            .unwrap();
    }
    b.build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn render(body: &str) -> Vec<u8> {
    render_with(false, body)
}

/// `w0000<br>w0001<br>…` from word `from` (no trailing `<br>`).
fn lines(n: usize, from: usize) -> String {
    (from..from + n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join("<br>")
}

/// `w0000 w0001 …` (wrapped by the browser).
fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn spacer(h: usize) -> String {
    format!("<div style=\"height:{h}px\"></div>")
}

struct Word {
    page: usize,
    y_min: f32,
    y_max: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-slo-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
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

/// How many `wNNNN` words (one per line here) are printed on each page.
fn per_page(words: &[Word]) -> Vec<usize> {
    let pages = words.iter().map(|w| w.page).max().unwrap_or(0);
    let mut per = vec![0; pages];
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        per[w.page - 1] += 1;
    }
    per
}

fn after(words: &[Word]) -> (usize, f32) {
    let a: Vec<&Word> = words.iter().filter(|w| w.text == "AFTER").collect();
    assert_eq!(a.len(), 1, "AFTER is printed once");
    (a[0].page, a[0].y_min)
}

/// The page and y of one `wNNNN` word.
fn at(words: &[Word], text: &str) -> (usize, f32) {
    let w: Vec<&Word> = words.iter().filter(|w| w.text == text).collect();
    assert_eq!(w.len(), 1, "{text} is printed once");
    (w[0].page, w[0].y_min)
}

/// No glyph of a `wNNNN` word is below `limit` pt.
fn assert_inside(words: &[Word], limit: f32) {
    for w in words.iter().filter(|w| w.text.starts_with('w')) {
        assert!(
            w.y_max <= limit,
            "{} on page {} ends at {:.1} pt, below {limit:.1} pt",
            w.text,
            w.page,
            w.y_max
        );
    }
}

/// `AFTER` is on `page` at `y` pt (±1 pt).
fn assert_after(words: &[Word], page: usize, y: f32) {
    let (p, got) = after(words);
    assert_eq!(p, page, "AFTER on page {p}");
    assert!(
        (got - y).abs() <= 1.0,
        "AFTER at {got:.1} pt, Chrome {y:.1} pt"
    );
}

/// Every word of `0..n` is printed once, in order, and no line stands below the content box. Lines of a `<br>` paragraph
/// keep their 21 px step on each page, so no fragment is placed over another.
fn assert_lines_in_order(words: &[Word], n: usize) {
    let names: Vec<&str> = words
        .iter()
        .filter(|w| w.text.starts_with('w'))
        .map(|w| w.text.as_str())
        .collect();
    let want: Vec<String> = (0..n).map(|i| format!("w{i:04}")).collect();
    assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>());
    assert_inside(words, CONTENT_BOTTOM);
}

fn assert_line_step(words: &[Word]) {
    let ws: Vec<&Word> = words.iter().filter(|w| w.text.starts_with('w')).collect();
    for pair in ws.windows(2) {
        if pair[0].page == pair[1].page {
            let step = pair[1].y_min - pair[0].y_min;
            assert!(
                (step - LINE).abs() < 0.3,
                "{} to {} on page {}: {step:.2} pt, expected {LINE}",
                pair[0].text,
                pair[1].text,
                pair[0].page
            );
        }
    }
}

/// An opaque `w` × `h` RGB PNG of one colour (stored deflate blocks).
fn solid_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let row: Vec<u8> = std::iter::once(0)
        .chain(rgb.iter().copied().cycle().take(3 * w as usize))
        .collect();
    let raw = row.repeat(h as usize);
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// Images drawn on each page (`pdfimages -list`), index 0 = page 1.
fn images_per_page(pdf: &[u8], pages: usize) -> Vec<usize> {
    let text = poppler("pdfimages", &["-list"], pdf);
    let mut per = vec![0; pages];
    for line in text.lines().skip(2) {
        if let Some(p) = line
            .split_whitespace()
            .next()
            .and_then(|p| p.parse::<usize>().ok())
            && (1..=pages).contains(&p)
        {
            per[p - 1] += 1;
        }
    }
    per
}

fn link_pages(pdf: &[u8]) -> Vec<(usize, String)> {
    let text = poppler("pdfinfo", &["-url"], pdf);
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let page = it.next()?.parse().ok()?;
            let _kind = it.next()?;
            Some((page, it.next()?.to_string()))
        })
        .collect()
}

fn poppler(tool: &str, args: &[&str], pdf: &[u8]) -> String {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-nsp-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new(tool)
        .args(args)
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("{tool} (poppler-utils) is required for this test: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{tool} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn render_images(body: &str) -> Vec<u8> {
    let head = "@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}";
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .image("dot.png", solid_png(10, 10, [255, 0, 0]))
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{head}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn section(inner: &str) -> String {
    format!("<section>{inner}</section>")
}

/// The first line of a continuation page: the content top (42.52 pt) plus the half-leading of the line (Chrome: 44.58 pt).
const FIRST_LINE_Y: f32 = 44.58;

fn assert_y(words: &[Word], text: &str, page: usize, y: f32) {
    let (p, got) = at(words, text);
    assert_eq!(p, page, "{text} on page {p}");
    assert!(
        (got - y).abs() <= 1.0,
        "{text} at {got:.1} pt, Chrome {y:.1} pt"
    );
}

// ── 1. A second long paragraph in a section starts below the first one's tail ─────────────────────────────────────

/// Chrome: `(first, second, lines per page, AFTER page, AFTER y)`, counted over both paragraphs, as for the body-level
/// twins. At `55154081` the cases with a second paragraph of 47 or 48 lines were 47 / 2 / 47, 47 / 2 / 48 / 0, ...
#[test]
fn a_second_long_paragraph_in_a_section_starts_below_the_first() {
    let cases: [(usize, usize, &[usize], usize, f32); 10] = [
        (49, 47, &[47, 47, 2], 3, 76.1),
        (49, 48, &[47, 48, 2], 3, 76.1),
        (49, 49, &[47, 48, 3], 3, 91.8),
        (49, 50, &[47, 48, 4], 3, 107.6),
        (49, 60, &[47, 48, 14], 3, 265.1),
        (50, 47, &[48, 47, 2], 3, 76.1),
        (50, 48, &[48, 48, 2], 3, 76.1),
        (50, 49, &[48, 48, 3], 3, 91.8),
        (50, 50, &[48, 48, 4], 3, 107.6),
        (50, 60, &[48, 48, 14], 3, 265.1),
    ];
    for (first, second, per, page, y) in cases {
        // Shape A: both paragraphs in one section.
        let words = printed_words(&render(&format!(
            "<section><p>{}</p><p>{}</p></section><p>AFTER</p>",
            lines(first, 0),
            lines(second, first)
        )));
        assert_eq!(per_page(&words), per, "two_{first}_{second}_A");
        assert_lines_in_order(&words, first + second);
        assert_line_step(&words);
        assert_after(&words, page, y);
        // Shape B: the second one first in its own section.
        let words = printed_words(&render(&format!(
            "<p>{}</p>{}<p>AFTER</p>",
            lines(first, 0),
            section(&format!("<p>{}</p>", lines(second, first)))
        )));
        assert_eq!(per_page(&words), per, "two_{first}_{second}_B");
        assert_lines_in_order(&words, first + second);
        assert_after(&words, page, y);
    }
}

// ── 2. A paragraph below a spacer in a section starts where two lines fit ─────────────────────────────────────────

fn shapes(h: usize, paragraph: &str) -> [(&'static str, String); 2] {
    [
        (
            "A",
            format!(
                "{}<p>AFTER</p>",
                section(&format!("{}{paragraph}", spacer(h)))
            ),
        ),
        (
            "B",
            format!("{}{}<p>AFTER</p>", spacer(h), section(paragraph)),
        ),
    ]
}

/// A 30-line paragraph. Chrome: H = 650 gives 17 / 13 (`AFTER` on page 2 at 249.3 pt), H = 966 gives 2 / 28 (485.6 pt); at
/// H = 975 one line of room is left and the paragraph moves whole (517.1 pt). At `55154081`: 0 / 30 for all three.
#[test]
fn a_paragraph_below_a_spacer_in_a_section_starts_where_two_lines_fit() {
    for (h, per, y) in [
        (650, [17, 13], 249.3),
        (966, [2, 28], 485.6),
        (975, [0, 30], 517.1),
    ] {
        for (shape, body) in shapes(h, &format!("<p>{}</p>", lines(30, 0))) {
            let words = printed_words(&render(&body));
            assert_eq!(per_page(&words), per, "H={h} shape {shape}");
            assert_lines_in_order(&words, 30);
            assert_line_step(&words);
            assert_after(&words, 2, y);
        }
    }
}

/// A 4-line paragraph with three lines of room: Chrome 2 / 2, `AFTER` on page 2 at 76.1 pt; with 5 lines 3 / 2 at 945.
/// A 30-line one with the same room: 3 / 27 (469.8 pt).
#[test]
fn three_lines_of_room_start_a_four_line_paragraph_in_a_section_and_leave_it_two_lines() {
    for (n, h, per, y) in [
        (4, 945, [2, 2], 76.1),
        (5, 945, [3, 2], 76.1),
        (30, 945, [3, 27], 469.8),
    ] {
        for (shape, body) in shapes(h, &format!("<p>{}</p>", lines(n, 0))) {
            let words = printed_words(&render(&body));
            assert_eq!(per_page(&words), per, "{n} lines, H={h}, shape {shape}");
            assert_lines_in_order(&words, n);
            assert_after(&words, 2, y);
        }
    }
}

/// 100 words, 960 px spacer (two lines of room): Chrome puts 27 words on page 1. 600 words after 650 px: 232 / 368.
#[test]
fn a_running_text_paragraph_in_a_section_starts_on_page_one() {
    for (shape, body) in shapes(960, &format!("<p>{}</p>", words(100))) {
        let per = per_page(&printed_words(&render(&body)));
        assert_eq!(per.len(), 2, "{shape}: {per:?}");
        assert!(
            per[0].abs_diff(27) <= 14 && per[0] >= 2,
            "{shape}: {per:?}, Chrome 27 / 73"
        );
    }
    for (shape, body) in shapes(650, &format!("<p>{}</p>", words(600))) {
        let w = printed_words(&render(&body));
        let per = per_page(&w);
        assert_eq!(per.len(), 2, "{shape}: {per:?}");
        assert!(
            per[0].abs_diff(232) <= 14,
            "{shape}: page 1 has {} words, Chrome 232",
            per[0]
        );
        assert_eq!(per[0] + per[1], 600);
        assert_lines_in_order(&w, 600);
    }
}

// ── 3. A section with a heading, then a long paragraph ────────────────────────────────────────────────────────────

/// `<section><h1>Title</h1><p>46 lines</p></section>`: Chrome puts the heading and 44 lines on page 1 and 2 lines on page 2
/// (`AFTER` 76.1 pt); 48 lines give 45 / 3 (91.8 pt). At `55154081` the heading was alone on page 1.
#[test]
fn a_heading_and_a_long_paragraph_in_a_section() {
    for (n, per, y) in [(46, [44, 2], 76.1), (48, [45, 3], 91.8)] {
        let words = printed_words(&render(&format!(
            "<section><h1>Title</h1><p>{}</p></section><p>AFTER</p>",
            lines(n, 0)
        )));
        assert_eq!(per_page(&words), per, "{n} lines");
        assert_eq!(at(&words, "Title").0, 1);
        assert_lines_in_order(&words, n);
        assert_after(&words, 2, y);
    }
}

/// Chrome does not keep a heading with the lines below it, in a section either: with room for the heading and 0 or 1 line
/// the paragraph moves whole (the heading stays alone on page 1), with room for 2 or 3 lines it starts there.
#[test]
fn a_heading_in_a_section_is_left_alone_when_fewer_than_two_lines_fit_below_it() {
    for (h, per) in [
        (973, [0, 30]),
        (953, [0, 30]),
        (933, [2, 28]),
        (913, [3, 27]),
    ] {
        let words = printed_words(&render(&format!(
            "<section>{}<h2 style=\"font:700 18px/27px Inter;margin:0\">Heading</h2><p>{}</p></section><p>AFTER</p>",
            spacer(h),
            lines(30, 0)
        )));
        assert_eq!(per_page(&words), per, "room after H={h}");
        assert_eq!(
            at(&words, "Heading").0,
            1,
            "H={h}: the heading is on page 1"
        );
        assert_lines_in_order(&words, 30);
    }
}

// ── 4. A padded paragraph slices its top inset on page one ────────────────────────────────────────────────────────

/// `<p style="padding:8px;border:1px solid">`, 30 lines after 650 px. Chrome: 16 / 14, `AFTER` at 271.8 pt, the first line of
/// page 2 at the content top with no top padding. At `55154081`: 0 / 30 (`AFTER` at 530.3 pt).
#[test]
fn a_padded_nested_paragraph_slices_its_top_inset_on_page_one() {
    for (shape, body) in shapes(
        650,
        &format!(
            "<p style=\"padding:8px;border:1px solid #888\">{}</p>",
            lines(30, 0)
        ),
    ) {
        let words = printed_words(&render(&body));
        assert_eq!(per_page(&words), [16, 14], "shape {shape}");
        assert_lines_in_order(&words, 30);
        assert_y(&words, "w0016", 2, FIRST_LINE_Y);
        assert_after(&words, 2, 271.8);
    }
}

// ── 5. A padded section breaks inside its paragraph ───────────────────────────────────────────────────────────────

/// `<section style="padding:20px;border:1px solid"><h2/><p>46 lines</p></section>`: the paragraph fits a page but not the
/// room below the heading and the padding (48 px above it); Chrome 44 / 2 (`AFTER` 91.8 pt). The frame of the section is
/// not asserted (F96). At `55154081`: 0 / 46. (With 16 px of padding the 46 lines end on a line boundary of Chrome's
/// 1010 px page, which ours, 1009.13 px, does not reach; F30f-c.)
#[test]
fn a_padded_section_breaks_inside_its_paragraph() {
    let words = printed_words(&render(&format!(
        "<section style=\"padding:20px;border:1px solid #888\"><h2 style=\"font:700 18px/27px Inter;margin:0\">Heading</h2><p>{}</p></section><p>AFTER</p>",
        lines(46, 0)
    )));
    assert_eq!(per_page(&words), [44, 2]);
    assert_eq!(at(&words, "Heading").0, 1);
    assert_lines_in_order(&words, 46);
    assert_after(&words, 2, 91.8);
}

// ── 6. A long list item starts in place ───────────────────────────────────────────────────────────────────────────

/// Forty one-line items and a 12-line one: Chrome puts 8 of its lines on page 1 and 4 on page 2 (`AFTER` 107.6 pt). Its
/// marker is on page 1. At `55154081`: the item moved whole (0 / 12).
#[test]
fn a_long_list_item_starts_in_place() {
    let shorts: String = (0..40).map(|i| format!("<li>s{i:02}</li>")).collect();
    let words = printed_words(&render(&format!(
        "<ul style=\"list-style:disc;padding-left:16px;margin:0\">{shorts}<li>{}</li></ul><p>AFTER</p>",
        lines(12, 0)
    )));
    assert_eq!(per_page(&words), [8, 4]);
    assert_lines_in_order(&words, 12);
    assert_after(&words, 2, 107.6);
    assert_y(&words, "w0008", 2, FIRST_LINE_Y);
    let markers: Vec<usize> = words
        .iter()
        .filter(|w| w.text == "•")
        .map(|w| w.page)
        .collect();
    assert_eq!(markers.len(), 41, "one marker per item");
    assert!(markers.iter().all(|&p| p == 1), "{markers:?}");
}

// ── 7. Margins collapse through the wrapper ───────────────────────────────────────────────────────────────────────

/// `<section style="margin-top:24px"><p style="margin-top:12px">`: the margins collapse to 24 px. After a 640 px spacer
/// Chrome starts the paragraph at 542.6 pt (16 / 14, `AFTER` 265.1 pt); after 920 px, 3 / 27 (469.8 pt). At `55154081`:
/// 0 / 30.
#[test]
fn margins_collapse_through_the_wrapper() {
    for (h, per, first, y) in [
        (640, [16, 14], Some(542.6), 265.1),
        (920, [3, 27], None, 469.8),
    ] {
        let words = printed_words(&render(&format!(
            "{}<section style=\"margin-top:24px\"><p style=\"margin-top:12px\">{}</p></section><p>AFTER</p>",
            spacer(h),
            lines(30, 0)
        )));
        assert_eq!(per_page(&words), per, "H={h}");
        assert_lines_in_order(&words, 30);
        if let Some(first) = first {
            assert_y(&words, "w0000", 1, first);
        }
        assert_y(
            &words,
            "w0016",
            2,
            if h == 640 { FIRST_LINE_Y } else { 249.3 },
        );
        assert_after(&words, 2, y);
    }
}

// ── 8. Links and inline images follow their lines ─────────────────────────────────────────────────────────────────

/// Links on lines 5 and 25 and (in the first round) a 10 px image on lines 3 and 27 of a 30-line paragraph after 650 px
/// (17 / 13): the first link and image are on page 1, the second link and image on page 2. Green at `55154081` with the
/// images (a paragraph with inline boxes takes another path there), red without them.
#[test]
fn links_and_images_of_a_nested_paragraph_are_on_the_page_of_their_line() {
    for images in [true, false] {
        let content: Vec<String> = (0..30)
            .map(|i| {
                let mut w = format!("w{i:04}");
                if i == 4 || i == 24 {
                    w = format!("<a href=\"https://example.com/l{i}\">{w}</a>");
                }
                if images && (i == 2 || i == 26) {
                    w.push_str("<img src=\"dot.png\" width=\"10\" height=\"10\">");
                }
                w
            })
            .collect();
        let pdf = render_images(&format!(
            "<section>{}<p>{}</p></section><p>AFTER</p>",
            spacer(650),
            content.join("<br>")
        ));
        let words = printed_words(&pdf);
        assert_eq!(per_page(&words), [17, 13], "images {images}");
        assert_lines_in_order(&words, 30);
        assert_eq!(
            link_pages(&pdf),
            [
                (1, "https://example.com/l4".to_string()),
                (2, "https://example.com/l24".to_string())
            ]
        );
        assert_eq!(
            images_per_page(&pdf, 2),
            if images { [1, 1] } else { [0, 0] }
        );
    }
}

// ── 9. Guards: shapes that must keep their output ─────────────────────────────────────────────────────────────────

/// A 3-line paragraph with two lines of room: Chrome keeps `orphans` and gives up `widows` (2 / 1, `AFTER` 60.3 pt); ours
/// moves it whole (0 / 3, 91.6 pt), in a section as at body level. F30c-w.
#[test]
fn a_nested_three_line_paragraph_moves_whole_where_chrome_gives_up_the_widows_rule() {
    for (shape, body) in shapes(966, &format!("<p>{}</p>", lines(3, 0))) {
        let words = printed_words(&render(&body));
        assert_eq!(
            per_page(&words),
            [0, 3],
            "ours, shape {shape}; Chrome 2 / 1"
        );
        assert_after(&words, 2, 91.6);
    }
}

/// `break-inside: avoid` on a nested paragraph that fits a page holds: Chrome 0 / 30 (`AFTER` 517.1 pt), from a `style`
/// attribute and from the bundled sheet's utility.
#[test]
fn an_avoid_paragraph_in_a_section_moves_whole() {
    for sheet in [false, true] {
        let attrs = if sheet {
            "class=\"break-inside-avoid\""
        } else {
            "style=\"break-inside:avoid\""
        };
        for (shape, body) in shapes(966, &format!("<p {attrs}>{}</p>", lines(30, 0))) {
            let words = printed_words(&render_with(sheet, &body));
            assert_eq!(per_page(&words), [0, 30], "sheet {sheet}, shape {shape}");
            assert_after(&words, 2, 516.8);
        }
    }
}

/// A relaxed `avoid` paragraph (60 lines, taller than a page) starts the next page: Chrome 0 / 48 / 12, `AFTER` 233.6 pt.
#[test]
fn a_relaxed_avoid_paragraph_in_a_section_still_starts_the_next_page() {
    let words = printed_words(&render(&format!(
        "<section>{}<p style=\"break-inside:avoid\">{}</p></section><p>AFTER</p>",
        spacer(300),
        lines(60, 0)
    )));
    assert_eq!(per_page(&words), [0, 48, 12]);
    assert_after(&words, 3, 233.6);
}

/// An `avoid` section that fits a page moves whole: Chrome 0 / 30 (517.1 pt).
#[test]
fn an_avoid_section_that_fits_a_page_moves_whole() {
    let words = printed_words(&render(&format!(
        "{}<section style=\"break-inside:avoid\"><p>{}</p></section><p>AFTER</p>",
        spacer(650),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [0, 30]);
    assert_after(&words, 2, 516.8);
}

/// A paragraph in a table cell is its row's unit (F183): Chrome splits it 16 / 14 (`AFTER` 265.8 pt), ours moves the row
/// whole (0 / 30). Pinned; filed as F30c-t.
#[test]
fn a_paragraph_in_a_table_cell_keeps_moving_with_its_row_until_f30c_t() {
    let words = printed_words(&render(&format!(
        "<table style=\"border-collapse:collapse;width:100%\"><tr><td>{}<h3 style=\"font:700 14px/21px Inter;margin:0\">Heading</h3><p>{}</p></td></tr></table><p>AFTER</p>",
        spacer(650),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [0, 30], "ours; Chrome 16 / 14");
    assert_eq!(
        at(&words, "Heading").0,
        1,
        "ours; the heading stays on page 1"
    );
}

/// A paragraph in a flex item is F30b's and not touched here: Chrome splits it 15 / 15 (`AFTER` 280.8 pt), ours moves it
/// whole (0 / 30, 516.8 pt), as before. Pinned so that the fix does not change it by accident.
#[test]
fn a_paragraph_in_a_flex_item_keeps_its_page() {
    let words = printed_words(&render(&format!(
        "<div style=\"display:flex\"><div>{}<h2 style=\"font:700 18px/27px Inter;margin:0\">Heading</h2><p>{}</p></div></div><p>AFTER</p>",
        spacer(650),
        lines(30, 0)
    )));
    assert_eq!(per_page(&words), [0, 30], "ours; Chrome 15 / 15");
    assert_after(&words, 2, 516.8);
}

// ── 10. A relaxed avoid ancestor ──────────────────────────────────────────────────────────────────────────────────

/// `<section style="break-inside:avoid">` of three 30-line paragraphs (taller than a page) after 300 px: Chrome starts it
/// on page 2 and fills that page (0 / 48 / 42, `AFTER` 706.1 pt). At `55154081` each paragraph moved whole (30 / 30 / 30).
#[test]
fn a_relaxed_avoid_section_starts_the_next_page_and_splits_in_place() {
    let words = printed_words(&render(&format!(
        "{}<section style=\"break-inside:avoid\"><p>{}</p><p>{}</p><p>{}</p></section><p>AFTER</p>",
        spacer(300),
        lines(30, 0),
        lines(30, 30),
        lines(30, 60)
    )));
    assert_eq!(per_page(&words), [0, 48, 42]);
    assert_lines_in_order(&words, 90);
    assert_after(&words, 3, 706.1);
}

/// A forced `break-before: page` on the paragraph that is first in a wrapper is the paragraph's start: Chrome moves the
/// wrapper to page 2 (0 / 48, `AFTER` on page 3 at 44.6 pt after its own break). Found by the sweep (`c_h_h_ba_n48_B`):
/// the wrapper must not be entered for its first two lines.
#[test]
fn a_forced_break_before_the_first_paragraph_of_a_wrapper_is_not_started_in_place() {
    let words = printed_words(&render(&format!(
        "<p>PRE</p><section><p style=\"break-before:page\">{}</p></section><p style=\"break-before:page\">AFTER</p>",
        lines(48, 0)
    )));
    assert_eq!(per_page(&words), [0, 48, 0]);
    assert_after(&words, 3, 44.6);
}
