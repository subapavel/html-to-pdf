//! No word of a multicol container is lost (F161, F165).
//!
//! F161: a paragraph that started in a column with less than one line of room was cut after its first line, moved to
//! the next column, and the slice of its second line overwrote it there, so `<p>w9</p><p>w14 … w20</p>` in
//! `columns:2` printed 2 of 8 words. A container with `column-span: all` children that does not fit its page lost
//! most of the content after the spanner (171 of 241 words). F165: the content after a multicol container inside a
//! `display: contents` wrapper was not moved by the container's height change (`propagate_height_delta` shifted
//! boxes, and a contents element has none).
//!
//! Chrome prints every word of each shape below (css-multicol-1 §6 `column-span`, §8.2 overflow into the next
//! page; css-break-3 §4.1 breaks between line boxes). Asserted on the text layer (`pdftotext -bbox`).

use htmltopdf::{RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>\
                 body{{margin:0;font-family:Inter;font-size:16px;line-height:20px}}\
                 p{{margin:0 0 8px}}\
                 </style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// One printed word: its 1-based page, the top of its box in pt and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    y_min: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-mclw-{}-{n}.pdf", std::process::id()));
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
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            let y_min = attrs
                .split_whitespace()
                .find_map(|a| a.strip_prefix("yMin="))
                .and_then(|v| v.trim_matches('"').parse().ok())
                .expect("word without yMin");
            found.push(Word {
                page,
                y_min,
                text: word.to_string(),
            });
        }
    }
    found
}

/// `w<from>` … `w<to-1>`, four digits each, joined by spaces.
fn ws(from: usize, to: usize) -> String {
    (from..to)
        .map(|i| format!("w{i:04}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every `wNNNN` word of `expected` is printed exactly once, and no other `wNNNN` word is.
fn assert_all_once(what: &str, found: &[Word], expected: &[String]) {
    let mut seen: Vec<&str> = found
        .iter()
        .map(|w| w.text.as_str())
        .filter(|t| t.len() == 5 && t.starts_with('w') && t[1..].bytes().all(|b| b.is_ascii_digit()))
        .collect();
    seen.sort_unstable();
    let mut want: Vec<&str> = expected.iter().map(String::as_str).collect();
    want.sort_unstable();
    let missing: Vec<&&str> = want.iter().filter(|w| !seen.contains(w)).collect();
    let mut dup = seen.clone();
    dup.dedup();
    assert!(
        seen == want,
        "{what}: printed {} of {} words; missing {:?}; printed twice: {}",
        seen.len(),
        want.len(),
        missing,
        seen.len() != dup.len()
    );
}

fn words(from: usize, to: usize) -> Vec<String> {
    (from..to).map(|i| format!("w{i:04}")).collect()
}

#[test]
fn a_paragraph_after_a_one_word_paragraph_keeps_its_words() {
    // The finding's shape: 2 of 8 words were printed.
    let body = format!(
        "<div style=\"columns:2;width:500px\"><p>w0009</p><p>{}</p></div>",
        ws(14, 21)
    );
    let mut want = vec!["w0009".to_string()];
    want.extend(words(14, 21));
    assert_all_once("one-word + 7-word paragraph", &printed_words(&render(&body)), &want);
}

#[test]
fn the_column_break_may_fall_after_any_line_of_a_paragraph() {
    // `k` one-word paragraphs, then a paragraph of a few lines: the balanced column break moves through every line
    // of the long paragraph (and the gaps between paragraphs) as `k` grows.
    for k in 0..16 {
        let mut body = String::from("<div style=\"columns:2;column-gap:20px;width:400px\">");
        let mut want = Vec::new();
        for i in 0..k {
            body.push_str(&format!("<p>w{i:04}</p>"));
            want.push(format!("w{i:04}"));
        }
        body.push_str(&format!("<p>{}</p></div><p>w0999</p>", ws(100, 124)));
        want.extend(words(100, 124));
        want.push("w0999".into());
        assert_all_once(
            &format!("{k} one-word paragraphs before a 24-word paragraph"),
            &printed_words(&render(&body)),
            &want,
        );
    }
}

#[test]
fn a_column_span_all_heading_in_a_container_over_two_pages_keeps_every_word() {
    // The finding's shape: 171 of 241 words were printed. Chrome: 3 pages, every word.
    let mut body = String::from("<div style=\"columns:2\"><p>w0000</p>");
    for i in 1..121 {
        body.push_str(&format!("<p>w{i:04}</p>"));
    }
    body.push_str("<h2 style=\"column-span:all;margin:8px 0\">w0500</h2>");
    for i in 121..241 {
        body.push_str(&format!("<p>w{i:04}</p>"));
    }
    body.push_str("</div><p>w0999</p>");
    let mut want = words(0, 241);
    want.push("w0500".into());
    want.push("w0999".into());
    let found = printed_words(&render(&body));
    assert_all_once("column-span: all over pages", &found, &want);
    // The spanner sits under every word before it on its page and above every word after it.
    let h = found.iter().find(|w| w.text == "w0500").unwrap();
    for w in &found {
        let Some(n) = w.text.strip_prefix('w').and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        if w.page == h.page && n < 121 {
            assert!(w.y_min < h.y_min, "{} ({:.1}pt) is not above the spanner ({:.1}pt)", w.text, w.y_min, h.y_min);
        } else if w.page == h.page && (121..241).contains(&n) {
            assert!(w.y_min > h.y_min, "{} ({:.1}pt) is not below the spanner ({:.1}pt)", w.text, w.y_min, h.y_min);
        }
        if n < 121 {
            assert!(w.page <= h.page, "{} is on page {}, after the spanner's page {}", w.text, w.page, h.page);
        } else if (121..241).contains(&n) {
            assert!(w.page >= h.page, "{} is on page {}, before the spanner's page {}", w.text, w.page, h.page);
        }
    }
}

#[test]
fn a_column_span_all_heading_on_one_page_keeps_every_word() {
    let mut body = String::from("<div style=\"columns:2\">");
    body.push_str(&format!("<p>{}</p>", ws(0, 30)));
    body.push_str("<h2 style=\"column-span:all\">w0500</h2>");
    body.push_str(&format!("<p>{}</p></div><p>w0999</p>", ws(30, 60)));
    let mut want = words(0, 60);
    want.push("w0500".into());
    want.push("w0999".into());
    assert_all_once("column-span: all on one page", &printed_words(&render(&body)), &want);
}

/// The top of `word` in a render of `body`.
fn top_of(body: &str, word: &str) -> (usize, f32) {
    let found = printed_words(&render(body));
    let w = found
        .iter()
        .find(|w| w.text == word)
        .unwrap_or_else(|| panic!("{word} is not printed"));
    (w.page, w.y_min)
}

#[test]
fn a_contents_wrapper_after_a_multicol_container_is_placed_under_it() {
    // F165: the wrapped `w0999` was drawn at 223 pt, the unwrapped one at 103 pt.
    let columns: String = (0..12).map(|i| format!("<p>w{i:04}</p>")).collect();
    let plain = format!("<div><div style=\"columns:3\">{columns}</div><p>w0999</p></div>");
    let wrapped = format!(
        "<div><div style=\"columns:3\">{columns}</div><div style=\"display:contents\"><p>w0999</p></div></div>"
    );
    let a = top_of(&plain, "w0999");
    let b = top_of(&wrapped, "w0999");
    assert!(
        a.0 == b.0 && (a.1 - b.1).abs() < 0.5,
        "the wrapped paragraph is at page {} {:.1}pt, the plain one at page {} {:.1}pt",
        b.0,
        b.1,
        a.0,
        a.1
    );
}

#[test]
fn a_long_multicol_container_before_a_contents_wrapper_keeps_its_words() {
    // F165: 108 of 228 words were printed.
    let body = format!(
        "<div><div style=\"columns:2\"><p>{}</p></div><div style=\"display:contents\"><p>{}</p></div></div>",
        ws(0, 120),
        ws(120, 228)
    );
    assert_all_once("multicol + wrapped paragraph", &printed_words(&render(&body)), &words(0, 228));
}
