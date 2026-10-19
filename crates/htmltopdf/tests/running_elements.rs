//! An element taken out of the flow by `position: running(name)` is shown by `element(name)` in a margin box and
//! nowhere else (CSS GCPM 3 §2.2). F102: when the rule came from an inline `<style>` the engine never hid the
//! element, so it was painted a second time in the body (and its `<a>` got a second link annotation).
//!
//! Chrome does not implement `position: running()`, so the reference of every case is the same document with
//! the running elements deleted from the body (`with_running = false`, "X_ref"). Every case asserts:
//!
//! - (a) X and X_ref have the same page count and the same words at the same boxes (0.5 pt) inside the
//!   content area, so the hidden element takes no room;
//! - (b) the running text is never inside the content area;
//! - (c) the running text is in the margin band on exactly the pages the case lists;
//! - (d) X has as many link annotations as X_ref plus one per margin box that shows a link.
//!
//! The renderer's page margin (15 mm) wins over `@page { margin }` (F6), so the content area is the one the
//! default page gives, not the one the documents' own `@page` rule asks for. Positions come from
//! `pdftotext -bbox`, link and image counts from the file itself and `pdfimages -list`.

use std::path::{Path, PathBuf};

use htmltopdf::{RenderOptions, Renderer};

const PAGE_H_PT: f32 = 841.89;
const MARGIN_PT: f32 = 15.0 / 25.4 * 72.0;
/// Positions of the same word in two renders may differ by this much.
const EPS_PT: f32 = 0.5;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
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

impl Word {
    fn centre_y(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
    /// Inside the content area of the default page (between the top and bottom margin).
    fn in_content(&self) -> bool {
        (MARGIN_PT..=PAGE_H_PT - MARGIN_PT).contains(&self.centre_y())
    }
    fn in_margin(&self) -> bool {
        !self.in_content()
    }
}

fn temp(pdf: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-running-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    path
}

fn words(pdf: &[u8]) -> Vec<Word> {
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

fn pages(pdf: &[u8]) -> usize {
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
        .expect("page count")
}

fn count(pdf: &[u8], needle: &str) -> usize {
    let needle = needle.as_bytes();
    pdf.windows(needle.len()).filter(|w| *w == needle).count()
}

/// Link annotations in the file (the annotation dictionaries are not compressed).
fn link_count(pdf: &[u8]) -> usize {
    count(pdf, "/Subtype /Link")
}

/// Images drawn on each page (`pdfimages -list`), index 0 = page 1.
fn images_per_page(pdf: &[u8], pages: usize) -> Vec<usize> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfimages")
        .arg("-list")
        .arg(&path)
        .output()
        .expect("pdfimages (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout);
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

/// How a case is rendered.
#[derive(Clone, Default)]
struct Setup {
    /// Stylesheet given to the renderer (`--css`); empty for none.
    css: String,
    /// Tailwind compiled sheet appended to `css` (R4).
    tailwind: bool,
    tagged: bool,
    /// Bundled images by name.
    images: Vec<(&'static str, PathBuf)>,
}

fn render(html: &str, setup: &Setup) -> Vec<u8> {
    let r = root();
    // `F102_DUMP_DIR=<dir>` writes every rendered HTML and its renderer stylesheet, for the Chrome probes.
    if let Ok(dir) = std::env::var("F102_DUMP_DIR") {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::fs::write(Path::new(&dir).join(format!("{n:03}.html")), html).expect("write dump");
        std::fs::write(Path::new(&dir).join(format!("{n:03}.css")), &setup.css)
            .expect("write dump");
    }
    let mut css = String::new();
    if setup.tailwind {
        css.push_str(&std::fs::read_to_string(r.join("fixtures/tailwind/tailwind.css")).unwrap());
    }
    css.push_str(&setup.css);
    let mut b = Renderer::builder();
    if !css.is_empty() {
        b = b.css(css);
    }
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
    for (name, path) in &setup.images {
        b = b.image(*name, std::fs::read(path).unwrap());
    }
    let opts = RenderOptions {
        lang: Some("cs".into()),
        tagged: setup.tagged,
        ..Default::default()
    };
    b.build().render(html, &opts).expect("render")
}

/// What the case expects from the margin boxes: the text prefix of one running element and the pages
/// (1-based) where it is printed in the margin band.
struct Shows {
    token: &'static str,
    pages: Vec<usize>,
    /// The margin box prints a link on each of those pages.
    link: bool,
}

/// The checks (a) to (d) for one case: `x` is the document with the running elements, `x_ref` without.
fn check(name: &str, x: &[u8], x_ref: &[u8], shows: &[Shows], hidden: &[&str]) {
    let wx = words(x);
    let wr = words(x_ref);
    assert_eq!(
        pages(x),
        pages(x_ref),
        "{name}: page count differs from X_ref"
    );

    // (a) the content area is the one of the document without the running element.
    let cx: Vec<&Word> = wx.iter().filter(|w| w.in_content()).collect();
    let cr: Vec<&Word> = wr.iter().filter(|w| w.in_content()).collect();
    let texts = |v: &[&Word]| v.iter().map(|w| w.text.clone()).collect::<Vec<_>>();
    if texts(&cx) != texts(&cr) {
        let first = cx
            .iter()
            .zip(&cr)
            .position(|(a, b)| a.text != b.text)
            .unwrap_or(cx.len().min(cr.len()));
        panic!(
            "{name}: (a) the words of the content area differ from X_ref ({} against {} words); first difference at word {first}: X has {:?}, X_ref has {:?}",
            cx.len(),
            cr.len(),
            cx.get(first),
            cr.get(first)
        );
    }
    for (a, b) in cx.iter().zip(&cr) {
        let d = (a.x0 - b.x0)
            .abs()
            .max((a.y0 - b.y0).abs())
            .max((a.x1 - b.x1).abs())
            .max((a.y1 - b.y1).abs());
        assert!(
            a.page == b.page && d <= EPS_PT,
            "{name}: (a) `{}` is at page {} ({:.2},{:.2}) in X but page {} ({:.2},{:.2}) in X_ref",
            a.text,
            a.page,
            a.x0,
            a.y0,
            b.page,
            b.x0,
            b.y0
        );
    }

    // (b) the running text is never in the content area.
    for s in shows {
        if let Some(w) = wx
            .iter()
            .find(|w| w.text.starts_with(s.token) && w.in_content())
        {
            panic!(
                "{name}: (b) `{}` is painted in the content area of page {} at ({:.2},{:.2})",
                w.text, w.page, w.x0, w.y0
            );
        }
    }
    for token in hidden {
        if let Some(w) = wx.iter().find(|w| w.text.starts_with(token)) {
            panic!(
                "{name}: (b) `{}` is printed on page {} at ({:.2},{:.2}), it should be nowhere",
                w.text, w.page, w.x0, w.y0
            );
        }
    }

    // (c) the margin band shows it on exactly the listed pages.
    let mut margin_links = 0;
    for s in shows {
        let mut on: Vec<usize> = wx
            .iter()
            .filter(|w| w.text.starts_with(s.token) && w.in_margin())
            .map(|w| w.page)
            .collect();
        on.dedup();
        assert_eq!(
            on, s.pages,
            "{name}: (c) pages with `{}` in the margin",
            s.token
        );
        if s.link {
            margin_links += s.pages.len();
        }
    }

    // (d) the link annotations of X_ref plus one per margin box that prints a link.
    assert_eq!(
        link_count(x),
        link_count(x_ref) + margin_links,
        "{name}: (d) link annotations (X_ref has {})",
        link_count(x_ref)
    );
}

const FONT: &str = "body { font-family: Inter; font-size: 10pt } p { margin: 0 }";

/// 110 table rows, about three pages.
fn rows(n: usize) -> String {
    let mut s = String::from(
        "<table style=\"width:100%;border-collapse:collapse\"><thead><tr><th>Popis</th><th>Cena</th></tr></thead><tbody>",
    );
    for i in 1..=n {
        s += &format!(
            "<tr><td>Položka č. {i} — žluťoučký kůň</td><td style=\"text-align:right\">{} Kč</td></tr>",
            i * 123
        );
    }
    s + "</tbody></table>"
}

/// `n` one-line paragraphs of 20 px.
fn lines(from: usize, to: usize) -> String {
    (from..=to)
        .map(|i| format!("<p style=\"line-height:20px\">Radek {i} žluťoučký kůň</p>"))
        .collect()
}

fn doc(css: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"cs\"><head><meta charset=\"utf-8\"><style>{css}</style></head><body>{body}</body></html>"
    )
}

// ---------------------------------------------------------------------------------------------------------
// R1: a running header with a link, the rule in an inline <style> or in the renderer's stylesheet.

const R1_RULES: &str = ".run { position: running(hdr) }";
const R1_PAGE: &str =
    "@page { size: A4; margin: 25mm 15mm; @top-center { content: element(hdr); font-size: 8pt } }";

fn r1_html(with_running: bool, inline: bool) -> String {
    let css = if inline {
        format!("{FONT} {R1_RULES} {R1_PAGE}")
    } else {
        FONT.to_string()
    };
    let run = if with_running {
        "<div class=\"run\">RUNHDR Faktura <a href=\"https://example.cz/f/1\">2026-0099</a> · <b>Příliš</b></div>"
    } else {
        ""
    };
    doc(
        &css,
        &format!("{run}<h1>Faktura 2026-0099</h1>{}", rows(110)),
    )
}

fn r1_setup(inline: bool) -> Setup {
    Setup {
        css: if inline {
            String::new()
        } else {
            format!("{R1_RULES} {R1_PAGE}")
        },
        ..Default::default()
    }
}

fn r1_shows() -> Vec<Shows> {
    vec![Shows {
        token: "RUNHDR",
        pages: vec![1, 2, 3],
        link: true,
    }]
}

#[test]
fn r1_running_header_is_painted_only_in_the_margin_inline_style() {
    let s = r1_setup(true);
    let x = render(&r1_html(true, true), &s);
    let x_ref = render(&r1_html(false, true), &s);
    assert_eq!(pages(&x_ref), 3, "the case must span three pages");
    check("R1 inline", &x, &x_ref, &r1_shows(), &[]);
}

#[test]
fn r1_running_header_is_painted_only_in_the_margin_renderer_css() {
    let s = r1_setup(false);
    let x = render(&r1_html(true, false), &s);
    let x_ref = render(&r1_html(false, false), &s);
    check("R1 css", &x, &x_ref, &r1_shows(), &[]);
}

#[test]
fn r1_the_rule_in_either_place_gives_the_same_words() {
    let a = render(&r1_html(true, true), &r1_setup(true));
    let b = render(&r1_html(true, false), &r1_setup(false));
    let key = |pdf: &[u8]| {
        words(pdf)
            .into_iter()
            .map(|w| format!("{}:{}:{:.1}:{:.1}", w.page, w.text, w.x0, w.y0))
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&a), key(&b));
}

// ---------------------------------------------------------------------------------------------------------
// R2: three running elements, each with a link.

fn r2_html(with_running: bool) -> String {
    let css = format!(
        "{FONT} .h {{ position: running(hdr) }} .f {{ position: running(ftr) }} .i {{ position: running(ico) }} \
         @page {{ @top-center {{ content: element(hdr) }} @bottom-center {{ content: element(ftr) }} \
         @bottom-left {{ content: element(ico) }} }}"
    );
    let run = if with_running {
        "<div class=\"h\">RUNHDR <a href=\"https://example.cz/h\">hlavicka</a></div>\
         <div class=\"f\">RUNFTR <a href=\"https://example.cz/f\">paticka</a></div>\
         <div class=\"i\">RUNICO <a href=\"https://example.cz/i\">ico</a></div>"
    } else {
        ""
    };
    doc(&css, &format!("{run}{}", rows(110)))
}

#[test]
fn r2_header_footer_and_a_third_running_element() {
    let s = Setup::default();
    let x = render(&r2_html(true), &s);
    let x_ref = render(&r2_html(false), &s);
    let all = |token| Shows {
        token,
        pages: vec![1, 2, 3],
        link: true,
    };
    check(
        "R2",
        &x,
        &x_ref,
        &[all("RUNHDR"), all("RUNFTR"), all("RUNICO")],
        &[],
    );
}

// ---------------------------------------------------------------------------------------------------------
// R3: element(name, first-except), and an element whose source is on page 2.

fn r3_html(with_running: bool) -> String {
    let css = format!(
        "{FONT} .h {{ position: running(hdr) }} .k {{ position: running(kap) }} \
         @page {{ @top-center {{ content: element(hdr, first-except) }} @bottom-center {{ content: element(kap, first) }} }}"
    );
    let (h, k) = if with_running {
        (
            "<div class=\"h\">RUNHDR záhlaví</div>",
            "<div class=\"k\">RUNKAP kapitola</div>",
        )
    } else {
        ("", "")
    };
    doc(&css, &format!("{h}{}{k}{}", lines(1, 70), lines(71, 130)))
}

#[test]
fn r3_first_except_and_a_source_on_page_two() {
    let s = Setup::default();
    let x = render(&r3_html(true), &s);
    let x_ref = render(&r3_html(false), &s);
    assert_eq!(pages(&x_ref), 3);
    check(
        "R3",
        &x,
        &x_ref,
        &[
            Shows {
                token: "RUNHDR",
                pages: vec![2, 3],
                link: false,
            },
            Shows {
                token: "RUNKAP",
                pages: vec![2, 3],
                link: false,
            },
        ],
        &[],
    );
}

// ---------------------------------------------------------------------------------------------------------
// R4: a footer with a table, a bundled PNG and Tailwind flex; and a `display` the author sets on it.

const R4_PAGE: &str = "@page { @bottom-center { content: element(f) } }";

fn r4_html(with_running: bool, rule: &str) -> String {
    let css = format!("{FONT} {rule} {R4_PAGE}");
    let run = if with_running {
        "<footer class=\"run flex justify-between text-xs\">\
         <img src=\"logo.png\" style=\"height:20px;width:20px\" alt=\"\">\
         <table><tr><td>RUNIC IČO 12345678</td><td>IBAN CZ65 0800 0000 1920 0014 5399</td></tr></table>\
         <a href=\"https://www.example.cz\">www.example.cz</a></footer>"
    } else {
        ""
    };
    doc(&css, &format!("{run}{}", lines(1, 75)))
}

fn r4_setup(css: &str) -> Setup {
    Setup {
        css: css.to_string(),
        tailwind: true,
        images: vec![(
            "logo.png",
            root().join("fixtures/invoices/03-logo-qr/logo.png"),
        )],
        ..Default::default()
    }
}

fn r4_shows() -> Vec<Shows> {
    vec![Shows {
        token: "RUNIC",
        pages: vec![1, 2],
        link: true,
    }]
}

#[test]
fn r4_footer_with_table_image_and_flex() {
    let rule = ".run { position: running(f) }";
    let s = r4_setup("");
    let x = render(&r4_html(true, rule), &s);
    let x_ref = render(&r4_html(false, rule), &s);
    assert_eq!(pages(&x_ref), 2);
    check("R4", &x, &x_ref, &r4_shows(), &[]);
    // An image in the margin copy is not painted at all today (F107: the margin-box documents are converted
    // without the image assets), so this is 0 and 0; once F107 is fixed it is 1 and 1. Either way the image is
    // not painted a second time in the body and both pages agree.
    let n = images_per_page(&x, 2);
    assert!(n[0] <= 1 && n[0] == n[1], "images per page {n:?}");
    assert_eq!(images_per_page(&x_ref, 2), vec![0, 0]);
}

#[test]
fn r4_author_display_in_the_same_rule() {
    let rule = ".run { position: running(f); display: flex }";
    let s = r4_setup("");
    let x = render(&r4_html(true, rule), &s);
    let x_ref = render(&r4_html(false, rule), &s);
    check("R4 same-rule display", &x, &x_ref, &r4_shows(), &[]);
    let n = images_per_page(&x, 2);
    assert!(
        n[0] <= 1 && n[0] == n[1],
        "images per page {n:?} (see F107 above)"
    );
}

/// `display: block !important` beats the engine's `display: none` (an author important declaration wins over a
/// normal one in the style attribute, Cascade 5 §6.4), so the element keeps its box in the layout and still takes
/// its room; what must hold is that it is not painted, linked or tagged a second time. (a) is therefore not
/// asserted for this case. The element holds text itself: the text of an element nested in a running element has
/// no geometry and is never painted in the body, with or without the fix.
#[test]
fn r4_author_important_display_is_not_painted() {
    let s = Setup {
        css: format!(".run {{ position: running(f); display: block !important }} {R4_PAGE}"),
        ..Default::default()
    };
    let html = |with: bool| {
        let run = if with {
            "<div class=\"run\">RUNP <a href=\"https://example.cz/p\">paticka</a></div>"
        } else {
            ""
        };
        doc(FONT, &format!("{run}{}", lines(1, 75)))
    };
    let x = render(&html(true), &s);
    let x_ref = render(&html(false), &s);
    let wx = words(&x);
    let on: Vec<usize> = wx
        .iter()
        .filter(|w| w.text.starts_with("RUNP") && w.in_margin())
        .map(|w| w.page)
        .collect();
    assert_eq!(
        on,
        vec![1, 2],
        "the margin box prints the footer on both pages"
    );
    for w in wx.iter().filter(|w| w.text.starts_with("RUNP")) {
        assert!(
            w.in_margin(),
            "RUNP is painted in the body: page {} ({:.1},{:.1})",
            w.page,
            w.x0,
            w.y0
        );
    }
    assert_eq!(
        link_count(&x),
        link_count(&x_ref) + 2,
        "one link per margin box"
    );
}

// ---------------------------------------------------------------------------------------------------------
// R5: nested running elements.

fn r5_html(with_running: bool) -> String {
    let css = format!(
        "{FONT} .r {{ position: running(hdr) }} .in {{ position: running(inner) }} \
         @page {{ @top-center {{ content: element(hdr) }} }}"
    );
    let run = if with_running {
        "<header><div><div class=\"r\">RUNOUT <span class=\"in\">RUNINNER</span> <a href=\"https://example.cz/o\">odkaz</a></div></div></header>"
    } else {
        "<header><div></div></header>"
    };
    doc(&css, &format!("{run}{}", lines(1, 130)))
}

#[test]
fn r5_nested_running_elements() {
    let s = Setup::default();
    let x = render(&r5_html(true), &s);
    let x_ref = render(&r5_html(false), &s);
    check(
        "R5",
        &x,
        &x_ref,
        &[
            Shows {
                token: "RUNOUT",
                pages: vec![1, 2, 3],
                link: true,
            },
            // The inner element is not registered (`walk_tree` does not look into a running element), so it
            // stays inside the copy of the outer one, and is nowhere in the body.
            Shows {
                token: "RUNINNER",
                pages: vec![1, 2, 3],
                link: false,
            },
        ],
        &[],
    );
}

// ---------------------------------------------------------------------------------------------------------
// R6: between two paragraphs of one section that splits over pages 1-2.

fn r6_html(with_running: bool) -> String {
    let css = format!(
        "{FONT} .x {{ position: running(x) }} @page {{ @top-center {{ content: element(x, first) }} }} \
         section p {{ line-height: 20px; height: 40px }}"
    );
    let p = |a: usize, b: usize| {
        (a..=b)
            .map(|i| format!("<p>Odstavec {i} žluťoučký kůň úpěl ďábelské ódy a nic víc</p>"))
            .collect::<String>()
    };
    let run = if with_running {
        "<div class=\"x\">RUNX poznámka</div>"
    } else {
        ""
    };
    doc(
        &css,
        &format!("<section>{}{run}{}</section>", p(1, 30), p(31, 42)),
    )
}

#[test]
fn r6_between_the_paragraphs_of_a_split_section() {
    let s = Setup::default();
    let x = render(&r6_html(true), &s);
    let x_ref = render(&r6_html(false), &s);
    assert_eq!(pages(&x_ref), 2, "the section must split over two pages");
    check(
        "R6",
        &x,
        &x_ref,
        &[Shows {
            token: "RUNX",
            pages: vec![2],
            link: false,
        }],
        &[],
    );
}

// ---------------------------------------------------------------------------------------------------------
// R7: tagged output.

#[test]
fn r7_tagged_output_has_no_structure_for_the_running_source() {
    let mut s = r1_setup(true);
    s.tagged = true;
    let x = render(&r1_html(true, true), &s);
    let x_ref = render(&r1_html(false, true), &s);
    check("R7", &x, &x_ref, &r1_shows(), &[]);
    assert_eq!(
        count(&x, "/Type /StructElem"),
        count(&x_ref, "/Type /StructElem"),
        "structure elements of X and X_ref"
    );
    assert_eq!(
        count(&x, "/S /Link"),
        0,
        "the body has no link, the margin link is untagged"
    );
}

// ---------------------------------------------------------------------------------------------------------
// R8: a running element nothing places.

#[test]
fn r8_running_element_that_no_element_places_is_not_displayed() {
    let css = format!("{FONT} .x {{ position: running(x) }}");
    let html = |with: bool| {
        let run = if with {
            "<div class=\"x\">RUNX <a href=\"https://example.cz/x\">odkaz</a></div>"
        } else {
            ""
        };
        doc(&css, &format!("{run}<h1>Faktura</h1>{}", lines(1, 60)))
    };
    let s = Setup::default();
    let x = render(&html(true), &s);
    let x_ref = render(&html(false), &s);
    check("R8", &x, &x_ref, &[], &["RUNX"]);
    assert_eq!(link_count(&x), 0);
    assert_eq!(link_count(&x_ref), 0);
}
