//! The `::before` / `::after` of an inline element nested in another inline element are boxes like any other (F66i, F159(d)).
//!
//! CSS 2.1 §12.1: the generated content behaves "as if" an element had been inserted just inside the host, wherever
//! the host is in the tree. Blitz created the pseudo-element boxes of an inline run only one level down, so
//! `<div><span>a <span class=t></span> w</span></div>` printed `a w` where Chrome prints `a tx w`, with no warning.
//!
//! Every Chrome number is m141 (A4, 15 mm, Inter 14px/20px, an `AFTER` paragraph below, `out/scratch/f66i/gen.py`); the text
//! layer's coordinates are pt, so the test converts no units. Two oracles per case:
//! (a) the words in order with x and y within 1 pt of Chrome;
//! (b) the render at depth n has the words, x and y of the render at depth 1 within 0.01 pt (Chrome gives both the same layout;
//!     this holds with or without Chrome numbers, and it is the only oracle where our depth 1 itself differs from Chrome:
//!     an inline-block root sits 3.5 pt low, bold and italic hosts need faces the test does not register).
//! Plan: `docs/plans/2026-10-30-f66i-nested-inline-pseudos.md`.

use htmltopdf::{RenderOptions, Renderer};

const HEAD: &str = concat!(
    "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p,ul{margin:0;padding:0}",
    "table{margin:0;padding:0;border-spacing:0}td{padding:0}",
    ".b::before{content:'B1'}.a::after{content:'A1'}.ab::before{content:'B1'}.ab::after{content:'A1'}",
    ".star::after{content:'*'}.amount::after{content:' Kč'}",
    ".bx::before{content:'';display:inline-block;width:6px;height:6px;background:#000}",
    ".pdp::before{content:'p';padding:0 4px;background:#ccc}.bdp::before{content:'p';border-left:3px solid #000}",
    ".pr::before{content:\"Pred \"}",
    ".t::before{content:'tx'}.t2::after{content:'ty'}.tb::before{content:'';border-right:1px solid}",
    ".tp::after{content:'';padding-right:4px;display:inline-block}.te::after{content:'';margin-right:4px}",
    ".tl::before{content:'';padding-left:4px}",
    ".x::after{content:'';padding-right:4px}.y::before{content:'';padding-right:4px}.v::before{content:'';border-right:1px solid}",
    ".cn{counter-reset:c 4}.cn::before{content:counter(c)}",
    ".img::before{content:url(data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==)}",
);

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn page(body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{HEAD}</style></head><body>{body}</body></html>"
    )
}

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(&page(body), &RenderOptions::default())
        .unwrap_or_else(|e| panic!("render of {body:?}: {e}"))
}

fn render_tagged(body: &str) -> Vec<u8> {
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &page(body),
            &RenderOptions {
                tagged: true,
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("tagged render of {body:?}: {e}"))
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x_min: f32,
    y_min: f32,
    text: String,
}

fn tmp(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("htmltopdf-np-{}-{n}-{name}", std::process::id()))
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = tmp("words.pdf");
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
                x_min: attr(attrs, "xMin"),
                y_min: attr(attrs, "yMin"),
                text: word.to_string(),
            });
        }
    }
    found
}

fn words_of(body: &str) -> Vec<Word> {
    printed_words(&render(body))
}

fn describe(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| format!("{}@{:.1},{:.1}", w.text, w.x_min, w.y_min))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One expected word: text, x (None where our x is known to differ from Chrome's for a reason outside this finding), y.
type Expect = (&'static str, Option<f32>, f32);

/// Oracle (a): the words in order, x and y within 1 pt of Chrome's.
fn assert_like_chrome(what: &str, words: &[Word], expect: &[Expect]) {
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    let want: Vec<&str> = expect.iter().map(|e| e.0).collect();
    assert_eq!(
        texts,
        want,
        "{what}: the printed words, ours: {}",
        describe(words)
    );
    for (w, (text, x, y)) in words.iter().zip(expect) {
        assert_eq!(w.page, 1, "{what}: `{text}` is on page 1");
        if let Some(x) = x {
            assert!(
                (w.x_min - x).abs() <= 1.0,
                "{what}: `{text}` at x {:.1}, Chrome puts it at {x} ({})",
                w.x_min,
                describe(words)
            );
        }
        assert!(
            (w.y_min - y).abs() <= 1.0,
            "{what}: `{text}` at y {:.1}, Chrome puts it at {y} ({})",
            w.y_min,
            describe(words)
        );
    }
}

/// Oracle (b): same words, x and y as another render, within 0.01 pt.
fn assert_same_layout(what: &str, got: &[Word], reference: &[Word]) {
    assert_eq!(
        got.iter().map(|w| &w.text).collect::<Vec<_>>(),
        reference.iter().map(|w| &w.text).collect::<Vec<_>>(),
        "{what}: the printed words, got: {} / depth 1: {}",
        describe(got),
        describe(reference)
    );
    for (g, r) in got.iter().zip(reference) {
        assert!(
            g.page == r.page
                && (g.x_min - r.x_min).abs() <= 0.01
                && (g.y_min - r.y_min).abs() <= 0.01,
            "{what}: `{}` at {:.2},{:.2}, at depth 1 {:.2},{:.2}",
            g.text,
            g.x_min,
            g.y_min,
            r.x_min,
            r.y_min
        );
    }
}

const AFTER: Expect = ("AFTER", Some(42.7), 58.8);
const AFTER_P: &str = "<p>AFTER</p>";

/// A one-line expectation: the words at y 43.8, then `AFTER` below.
fn line(items: &[(&'static str, f32)]) -> Vec<Expect> {
    let mut v: Vec<Expect> = items.iter().map(|(t, x)| (*t, Some(*x), 43.8)).collect();
    v.push(AFTER);
    v
}

/// `n - 1` wrappers (`tag`) around `inner`.
fn nest(n: usize, inner: &str, tag: &str) -> String {
    format!(
        "{}{inner}{}",
        format!("<{tag}>").repeat(n - 1),
        format!("</{tag}>").repeat(n - 1)
    )
}

/// `a <span class=host>text</span> w`, `n - 1` spans deep.
fn mid(host: &str, text: &str, n: usize) -> String {
    nest(n, &format!("a <span class={host}>{text}</span> w"), "span")
}

#[test]
fn the_pseudo_of_a_nested_span_is_printed() {
    // The repro: `a tx w`, Chrome and depth 1 alike. Main printed `a w`.
    let want = line(&[("a", 42.7), ("tx", 51.6), ("w", 63.7)]);
    for n in [1, 2] {
        let body = format!("<div>{}</div>{AFTER_P}", mid("t", "", n));
        assert_like_chrome(&format!("depth {n}"), &words_of(&body), &want);
    }
}

#[test]
fn before_and_after_at_depth_two_to_four() {
    // (class, host text, Chrome words)
    let cases: [(&str, &str, [(&str, f32); 3]); 6] = [
        ("b", "", [("a", 42.7), ("B1", 51.6), ("w", 65.7)]),
        ("a", "", [("a", 42.7), ("A1", 51.6), ("w", 65.7)]),
        ("ab", "", [("a", 42.7), ("B1A1", 51.6), ("w", 76.9)]),
        ("b", "h", [("a", 42.7), ("B1h", 51.6), ("w", 71.9)]),
        ("a", "h", [("a", 42.7), ("hA1", 51.6), ("w", 71.9)]),
        ("ab", "h", [("a", 42.7), ("B1hA1", 51.6), ("w", 83.1)]),
    ];
    for (class, text, chrome) in cases {
        let want = line(&chrome);
        let depth1 = words_of(&format!("<div>{}</div>{AFTER_P}", mid(class, text, 1)));
        assert_like_chrome(&format!(".{class} {text:?} depth 1"), &depth1, &want);
        for n in 2..=4 {
            let what = format!(".{class} host text {text:?} at depth {n}");
            let got = words_of(&format!("<div>{}</div>{AFTER_P}", mid(class, text, n)));
            assert_like_chrome(&what, &got, &want);
            assert_same_layout(&what, &got, &depth1);
        }
    }
}

/// The roots of a line layout: each wraps `inner`.
fn roots() -> Vec<(&'static str, fn(&str) -> String, bool)> {
    fn div(s: &str) -> String {
        format!("<div>{s}</div>")
    }
    fn p(s: &str) -> String {
        format!("<p>{s}</p>")
    }
    fn td(s: &str) -> String {
        format!("<table><tr><td>{s}</td></tr></table>")
    }
    fn li(s: &str) -> String {
        format!("<ul><li style='list-style:none'>{s}</li></ul>")
    }
    fn flex(s: &str) -> String {
        format!("<div style='display:flex'><div>{s}</div></div>")
    }
    fn grid(s: &str) -> String {
        format!("<div style='display:grid'><div>{s}</div></div>")
    }
    fn inline_block(s: &str) -> String {
        format!("<div><span style='display:inline-block'>{s}</span></div>")
    }
    // The last flag: our depth 1 is Chrome's (an inline-block box sits 3.5 pt low in ours, a separate gap).
    vec![
        ("div", div, true),
        ("p", p, true),
        ("td", td, true),
        ("li", li, true),
        ("flex item", flex, true),
        ("grid item", grid, true),
        ("inside an inline-block", inline_block, false),
    ]
}

#[test]
fn every_root_kind() {
    for (name, wrap, like_chrome) in roots() {
        for (class, text, chrome) in [
            ("ab", "", [("a", 42.7), ("B1A1", 51.6), ("w", 76.9)]),
            ("b", "h", [("a", 42.7), ("B1h", 51.6), ("w", 71.9)]),
        ] {
            let depth1 = words_of(&format!("{}{AFTER_P}", wrap(&mid(class, text, 1))));
            if like_chrome {
                assert_like_chrome(&format!("{name} depth 1"), &depth1, &line(&chrome));
            }
            for n in 2..=3 {
                let what = format!("root {name}, .{class} {text:?}, depth {n}");
                let got = words_of(&format!("{}{AFTER_P}", wrap(&mid(class, text, n))));
                if like_chrome {
                    assert_like_chrome(&what, &got, &line(&chrome));
                } else {
                    // x is Chrome's; y is the inline-block's own offset, taken from depth 1.
                    for (w, (t, x)) in got.iter().zip(&chrome) {
                        assert_eq!(&w.text, t, "{what}: {}", describe(&got));
                        assert!((w.x_min - x).abs() <= 1.0, "{what}: {}", describe(&got));
                    }
                }
                assert_same_layout(&what, &got, &depth1);
            }
        }
    }
}

#[test]
fn hosts_that_templates_use() {
    // An outer element of each kind around the host. `b`, `strong` and `em` need faces the test does not register
    // (bold, italic), so they only have to equal depth 1.
    for (tag, chrome) in [
        ("a", true),
        ("label", true),
        ("b", false),
        ("strong", false),
        ("em", false),
        ("span", true),
    ] {
        let open = if tag == "a" {
            "<a href=\"https://x.example\">"
        } else {
            &format!("<{tag}>")
        };
        let inner = format!("{open}a <span class=ab>h</span> w</{tag}>");
        let depth1 = words_of(&format!("<div>a <span class=ab>h</span> w</div>{AFTER_P}"));
        let got = words_of(&format!("<div>{inner}</div>{AFTER_P}"));
        if chrome {
            assert_like_chrome(
                tag,
                &got,
                &line(&[("a", 42.7), ("B1hA1", 51.6), ("w", 83.1)]),
            );
        }
        assert_same_layout(tag, &got, &depth1);
    }
    // The hosts themselves are inline elements of other kinds (depth 2).
    for tag in ["a", "b", "strong", "em", "label", "i"] {
        let depth1 = words_of(&format!(
            "<div>a <{tag} class=ab>h</{tag}> w</div>{AFTER_P}"
        ));
        let got = words_of(&format!(
            "<div><span>a <{tag} class=ab>h</{tag}> w</span></div>{AFTER_P}"
        ));
        assert_same_layout(&format!("host <{tag}>"), &got, &depth1);
        assert!(
            got.iter().any(|w| w.text == "B1hA1"),
            "host <{tag}>: {}",
            describe(&got)
        );
    }
    // A required-field marker: `label span::after{content:'*'}`.
    assert_like_chrome(
        "label span::after",
        &words_of(&format!(
            "<div><label>Jméno <span class=star></span> w</label></div>{AFTER_P}"
        )),
        &line(&[("Jméno", 42.7), ("*", 79.5), ("w", 87.7)]),
    );
    assert_like_chrome(
        "label > span > span::after",
        &words_of(&format!(
            "<div><label for=x>Jméno<span class=req><span class=star></span></span></label> next</div>{AFTER_P}"
        )),
        &line(&[("Jméno*", 42.7), ("next", 84.8)]),
    );
    // A currency after an amount, in a table cell, at depth 1, 2 and 3.
    for n in 1..=3 {
        let cell = nest(n, "<span class=amount>1 250,00</span>", "span");
        let body = format!(
            "<table style='width:300px'><tr><td>Položka</td><td style='text-align:right'>{cell}</td></tr></table>{AFTER_P}"
        );
        let got = words_of(&body);
        assert_like_chrome(
            &format!("amount, depth {n}"),
            &got,
            &[
                ("Položka", Some(42.7), 43.8),
                AFTER,
                ("1", Some(209.9), 43.8),
                ("250,00", Some(217.2), 43.8),
                ("Kč", Some(255.1), 43.8),
            ],
        );
    }
}

#[test]
fn a_box_without_text() {
    // An inline-block box from `content:''`: the next word moves by its 6 px, at every depth.
    let want = line(&[("a", 42.7), ("w", 59.1)]);
    let depth1 = words_of(&format!("<div>{}</div>{AFTER_P}", mid("bx", "", 1)));
    assert_like_chrome("inline-block box, depth 1", &depth1, &want);
    for n in 2..=3 {
        let got = words_of(&format!("<div>{}</div>{AFTER_P}", mid("bx", "", n)));
        assert_like_chrome(&format!("inline-block box, depth {n}"), &got, &want);
        assert_same_layout(&format!("inline-block box, depth {n}"), &got, &depth1);
    }
    // Padding and a border on the generated inline box: the text is there at every depth, laid out as at depth 1. (Padding and
    // borders of inline elements move no text in ours at depth 1 either: a separate gap, so Chrome's x is not asserted.)
    for class in ["pdp", "bdp"] {
        let depth1 = words_of(&format!("<div>{}</div>{AFTER_P}", mid(class, "", 1)));
        assert!(
            depth1.iter().any(|w| w.text == "p"),
            "{class}: {}",
            describe(&depth1)
        );
        for n in 2..=3 {
            let got = words_of(&format!("<div>{}</div>{AFTER_P}", mid(class, "", n)));
            assert_same_layout(&format!(".{class}, depth {n}"), &got, &depth1);
        }
    }
}

#[test]
fn trailing_space_in_content() {
    // `content: "Pred "` keeps its space (F154 W10): the next word is 26 pt on.
    let want = line(&[("a", 42.7), ("Pred", 51.6), ("x", 77.6), ("w", 86.3)]);
    let depth1 = words_of(&format!(
        "<div>{}</div>{AFTER_P}",
        nest(1, "a <span class=pr>x</span> w", "span")
    ));
    assert_like_chrome("depth 1", &depth1, &want);
    for n in 2..=3 {
        let got = words_of(&format!(
            "<div>{}</div>{AFTER_P}",
            nest(n, "a <span class=pr>x</span> w", "span")
        ));
        assert_like_chrome(&format!("depth {n}"), &got, &want);
    }
}

#[test]
fn f66j_pseudo_after_a_br() {
    // F66j: the pseudo-element after the `<br>` inside a span. Once it exists the line is Chrome's (AFTER at 73.8, main 58.6).
    let shapes: [(&str, &str, &str, &[Expect]); 9] = [
        ("before, end padding", "", "y", &[("a", Some(42.7), 43.8)]),
        ("after, end padding", "", "x", &[("a", Some(42.7), 43.8)]),
        ("before, end border", "", "v", &[("a", Some(42.7), 43.8)]),
        // Inline padding is not laid out in ours (`a` is at 42.5, Chrome 45.7): a separate gap, so x of `a` is not asserted.
        (
            "p2_Apl_t",
            "padding-left:4px",
            "t",
            &[("a", None, 43.8), ("tx", Some(42.7), 58.8)],
        ),
        (
            "p2_Apl_t2",
            "padding-left:4px",
            "t2",
            &[("a", None, 43.8), ("ty", Some(42.7), 58.8)],
        ),
        ("p2_Apl_tb", "padding-left:4px", "tb", &[("a", None, 43.8)]),
        ("p2_Apl_te", "padding-left:4px", "te", &[("a", None, 43.8)]),
        ("p2_Apl_tl", "padding-left:4px", "tl", &[("a", None, 43.8)]),
        ("p2_Apl_tp", "padding-left:4px", "tp", &[("a", None, 43.8)]),
    ];
    for (name, style, class, head) in shapes {
        let body = format!(
            "<div><span style=\"{style}\">a<br><span class={class}></span></span></div>{AFTER_P}"
        );
        let mut want: Vec<Expect> = head.to_vec();
        want.push(("AFTER", Some(42.7), 73.8));
        assert_like_chrome(name, &words_of(&body), &want);
    }
}

#[test]
fn tagged_nested_pseudo_text() {
    // The new text lies in the paragraph's marked content; the tagged file is valid and `pdftotext` finds it.
    let bodies = [
        format!("<div>{}</div>{AFTER_P}", mid("t", "", 2)),
        format!("<div><label>Jméno <span class=star></span> w</label></div>{AFTER_P}"),
        format!(
            "<table><tr><td><span class=price><span class=amount>1 250,00</span></span></td></tr></table>{AFTER_P}"
        ),
    ];
    for (body, needle) in bodies.iter().zip(["tx", "*", "Kč"]) {
        let pdf = render_tagged(body);
        let (input, qdf) = (tmp("in.pdf"), tmp("qdf.pdf"));
        std::fs::write(&input, &pdf).unwrap();
        let check = std::process::Command::new("qpdf")
            .arg("--check")
            .arg(&input)
            .output()
            .expect("qpdf is required for this test and was not found in PATH");
        assert!(
            check.status.success(),
            "qpdf --check of {body:?}: {}{}",
            String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr)
        );
        let status = std::process::Command::new("qpdf")
            .args(["--qdf", "--object-streams=disable"])
            .arg(&input)
            .arg(&qdf)
            .status()
            .expect("qpdf");
        assert!(status.success(), "qpdf --qdf failed");
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&qdf);
        let words = printed_words(&pdf);
        assert!(
            words.iter().any(|w| w.text.contains(needle)),
            "`{needle}` is in the text layer of the tagged file: {}",
            describe(&words)
        );
    }
}

// Guards: shapes that work today and must go on working.

#[test]
fn guards_depth_one_anonymous_block_and_contents_chain() {
    // Depth 1.
    let want = line(&[("a", 42.7), ("tx", 51.6), ("w", 63.7)]);
    assert_like_chrome(
        "depth 1",
        &words_of(&format!("<div>{}</div>{AFTER_P}", mid("t", "", 1))),
        &want,
    );
    // The inline run sits next to a block sibling: the root is an anonymous block.
    assert_like_chrome(
        "anonymous block",
        &words_of(&format!(
            "<div><span>a <span class=t></span> w</span><div>blk</div></div>{AFTER_P}"
        )),
        &[
            ("a", Some(42.7), 43.8),
            ("tx", Some(51.6), 43.8),
            ("w", Some(63.7), 43.8),
            ("blk", Some(42.7), 58.8),
            ("AFTER", Some(42.7), 73.8),
        ],
    );
    // Every element between the root and the host's parent is `display: contents` (F150).
    assert_like_chrome(
        "display: contents chain",
        &words_of(&format!(
            "<div><span style='display:contents'><span style='display:contents'>a <span class=t></span> w</span></span></div>{AFTER_P}"
        )),
        &want,
    );
    // A pseudo-element under `display:none` inside an anonymous block is not drawn.
    assert_like_chrome(
        "display:none under an anonymous block",
        &words_of(&format!(
            "<div><span>a <span style='display:none'><span class=t></span></span> w</span><div>blk</div></div>{AFTER_P}"
        )),
        &[
            ("a", Some(42.7), 43.8),
            ("w", Some(51.6), 43.8),
            ("blk", Some(42.7), 58.8),
            ("AFTER", Some(42.7), 73.8),
        ],
    );
}

#[test]
fn contents_beyond_a_string_is_laid_out_as_at_depth_one() {
    // `<q>` (open-quote / close-quote, not supported: separate finding), `counter()` and `url()` at depth 2 give what
    // depth 1 gives: no panic, same words, same x.
    for (what, host) in [
        ("q", "<q>x</q>"),
        ("counter()", "<span class=cn>h</span>"),
        ("url()", "<span class=img>h</span>"),
    ] {
        let at = |n: usize| {
            words_of(&format!(
                "<div>{}</div>{AFTER_P}",
                nest(n, &format!("a {host} w"), "span")
            ))
        };
        let depth1 = at(1);
        for n in 2..=3 {
            assert_same_layout(&format!("{what}, depth {n}"), &at(n), &depth1);
        }
    }
}
