//! F107: an `<img>` in a running element is painted in the margin box.
//!
//! GCPM 3 section 2.2 puts the element into the margin box as that box's content, so an `<img>` in it is ordinary
//! content: the engine must give the margin-box documents the bundled images (otherwise `convert_image` finds no
//! bundle and draws nothing) and the natural size of every `<img>` (otherwise an image without `width` and
//! `height` lays out 0 wide, in the measure stage and in the render stage alike; an SVG is never drawn).
//! css-page-3 section 5.3.2 sizes the boxes of an edge from the max-content width of their contents, so the
//! logo's width is part of what `@top-left` measures. Plan: docs/plans/2026-11-04-f107-image-in-running-element.md.
//!
//! Chrome has no `running()`, so the numbers are the spec arithmetic of the plan: A4 with `margin: 15mm` has a top
//! band of 56.69 px, a box of that height centres its content (F103), `@top-left` starts at x = 56.69 px and
//! `@top-right` ends at 793.7 - 56.69 px. Pixels come from `pdftoppm` at 96 dpi (1 device px = 1 CSS px), words
//! from `pdftotext -bbox`, scaled to CSS px by the ratio of the page's width in the output to `PAGE_W` (no px/pt constant of
//! our own).

use std::path::{Path, PathBuf};
use std::process::Command;

use htmltopdf::{RenderOptions, Renderer};

const RED64X32: &[u8] = include_bytes!("data/red64x32.png");
const ZNACKA: &[u8] = include_bytes!("data/znacka96x32.svg");

const CHILD_ENV: &str = "HTMLTOPDF_F107_CHILD";
const CHILD_TEST: &str = "zz_child_process";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// 15 mm in CSS px: the top band, and the left and right margins.
const BAND: f64 = 15.0 * 96.0 / 25.4;
/// The A4 page width in CSS px.
const PAGE_W: f64 = 210.0 * 96.0 / 25.4;

const RULES: &str = "body { margin: 0; font: 14px Inter } p { margin: 0 } \
     .hlavicka { position: running(hlavicka) } .znacka { position: running(znacka) } \
     @page { size: A4; margin: 15mm; @top-left { content: element(hlavicka) } \
     @top-right { content: element(znacka) } }";

/// Two pages: each block is 900 px high and the content area is 1010 px.
const BODY: &str =
    "<div style=\"height:900px\">Strana jedna</div><div style=\"height:900px\">Strana dva</div>";

fn doc(running: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"cs\"><head><meta charset=\"utf-8\"><style>{RULES}</style></head>\
         <body>{running}{body}</body></html>"
    )
}

fn renderer(strict: bool, bundled: bool) -> Renderer {
    let mut b = Renderer::builder()
        .css_file(root().join("fixtures/tailwind/tailwind.css"))
        .unwrap()
        .strict_fonts(strict)
        .strict_images(strict);
    let mut fonts: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect();
    fonts.sort();
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    if bundled {
        b = b
            .image("logo.png", RED64X32.to_vec())
            .image("znacka.svg", ZNACKA.to_vec());
    }
    b.build()
}

fn render(html: &str) -> Vec<u8> {
    renderer(true, true)
        .render(
            html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("the document renders")
}

fn temp(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-f107-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn tool(name: &str, args: &[&str], pdf: &[u8]) -> Vec<u8> {
    let path = temp(pdf);
    let mut cmd = Command::new(name);
    cmd.args(args).arg(&path);
    if name == "pdftotext" {
        // Text to stdout, not to a file next to the PDF.
        cmd.arg("-");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|_| panic!("{name} (poppler-utils) is required for this test"));
    let _ = std::fs::remove_file(&path);
    out.stdout
}

struct Page {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Page {
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.w + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

/// Page `n` (1-based) at 96 dpi.
fn raster_page(pdf: &[u8], n: usize) -> Page {
    let n = n.to_string();
    let data = tool("pdftoppm", &["-r", "96", "-f", &n, "-l", &n], pdf);
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while data[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while !data[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..at]).into_owned());
    }
    at += 1;
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    Page {
        w,
        h,
        rgb: data[at..at + w * h * 3].to_vec(),
    }
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}
fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 80 && p[1] < 80
}

#[derive(Debug)]
struct Ink {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

fn bbox(page: &Page, pred: fn([u8; 3]) -> bool) -> Option<Ink> {
    let mut b: Option<Ink> = None;
    for y in 0..page.h {
        for x in 0..page.w {
            if pred(page.at(x, y)) {
                let r = b.get_or_insert(Ink {
                    x0: x,
                    x1: x + 1,
                    y0: y,
                    y1: y + 1,
                });
                r.x0 = r.x0.min(x);
                r.x1 = r.x1.max(x + 1);
                r.y1 = y + 1;
            }
        }
    }
    b
}

#[track_caller]
fn near(got: usize, want: f64, tol: f64, what: &str) {
    assert!(
        (got as f64 - want).abs() <= tol,
        "{what}: {got} px, expected {want:.1} +- {tol}"
    );
}

#[derive(Debug, Clone)]
struct Word {
    page: usize,
    x0: f64,
    y0: f64,
    text: String,
}

/// Every word of the document with its box in CSS px (`pdftotext -bbox` reports the box in its own unit; the page
/// width it reports for the A4 page gives the ratio).
fn words(pdf: &[u8]) -> Vec<Word> {
    let out = String::from_utf8_lossy(&tool("pdftotext", &["-bbox"], pdf)).into_owned();
    let attr = |tag: &str, name: &str| -> f64 {
        let key = format!("{name}=\"");
        let from = tag.find(&key).unwrap() + key.len();
        tag[from..from + tag[from..].find('"').unwrap()]
            .parse()
            .unwrap()
    };
    let mut page = 0;
    let mut scale = 1.0;
    let mut v = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.starts_with("<page ") {
            page += 1;
            scale = PAGE_W / attr(line, "width");
        } else if line.starts_with("<word ") {
            let text = line[line.find('>').unwrap() + 1..line.rfind("</word>").unwrap()]
                .replace("&amp;", "&");
            v.push(Word {
                page,
                x0: attr(line, "xMin") * scale,
                y0: attr(line, "yMin") * scale,
                text,
            });
        }
    }
    v
}

/// `pdfimages -list` rows as `(page, object id)`.
fn image_objects(pdf: &[u8]) -> Vec<(usize, usize)> {
    String::from_utf8_lossy(&tool("pdfimages", &["-list"], pdf))
        .lines()
        .skip(2)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            // page num type width height color comp bpc enc interp object ID ...; a soft mask is not an image
            (f.get(2) == Some(&"image")).then_some(())?;
            Some((f.first()?.parse().ok()?, f.get(10)?.parse().ok()?))
        })
        .collect()
}

fn per_page(objs: &[(usize, usize)], pages: usize) -> Vec<usize> {
    (1..=pages)
        .map(|p| objs.iter().filter(|o| o.0 == p).count())
        .collect()
}

const LOGO: &str =
    "<div class=\"hlavicka\"><img src=\"logo.png\" style=\"height:32px\" alt=\"\"></div>";
const MARK: &str = "<div class=\"znacka flex justify-end\"><img src=\"znacka.svg\" alt=\"\"></div>";

/// I1: the sized PNG of the R4 footer is painted on both pages (the tightened R4 is in `running_elements.rs`);
/// here the same with the plan's header shape.
#[test]
fn i1_png_with_both_sizes_is_painted_on_each_page() {
    let html = doc(
        "<div class=\"hlavicka\"><img src=\"logo.png\" width=\"64\" height=\"32\" alt=\"\"></div>",
        BODY,
    );
    let pdf = render(&html);
    assert_eq!(per_page(&image_objects(&pdf), 2), vec![1, 1]);
}

/// I2: an unsized PNG at `height: 32px` in `@top-left`: 64 x 32 px, centred in the band, from the left margin.
#[test]
fn i2_unsized_png_is_painted_at_its_ratio_and_centred() {
    let pdf = render(&doc(LOGO, BODY));
    assert_eq!(per_page(&image_objects(&pdf), 2), vec![1, 1]);
    for page in [1, 2] {
        let p = raster_page(&pdf, page);
        let ink = bbox(&p, is_red).unwrap_or_else(|| panic!("page {page}: no red logo"));
        let top = (BAND - 32.0) / 2.0;
        near(ink.y0, top, 1.0, &format!("page {page} logo top"));
        near(ink.y1, top + 32.0, 1.0, &format!("page {page} logo bottom"));
        near(ink.x0, BAND, 1.0, &format!("page {page} logo left"));
        near(
            ink.x1 - ink.x0,
            64.0,
            1.0,
            &format!("page {page} logo width"),
        );
    }
}

/// I3: an SVG `<img>` in `@top-right`, with text in a bundled font and nothing from the host.
#[test]
fn i3_svg_img_is_painted_with_bundled_font() {
    let pdf = render(&doc(MARK, BODY));
    for page in [1, 2] {
        let p = raster_page(&pdf, page);
        let ink = bbox(&p, is_blue).unwrap_or_else(|| panic!("page {page}: no blue mark"));
        near(
            ink.x1,
            PAGE_W - BAND,
            1.5,
            &format!("page {page} mark right"),
        );
        near(
            ink.x1 - ink.x0,
            96.0,
            1.5,
            &format!("page {page} mark width"),
        );
        near(
            ink.y0,
            (BAND - 32.0) / 2.0,
            1.5,
            &format!("page {page} mark top"),
        );
        near(
            ink.y1 - ink.y0,
            32.0,
            1.5,
            &format!("page {page} mark height"),
        );
        assert!(
            words(&pdf).iter().any(|w| w.page == page
                && w.text == "ISO"
                && w.y0 < BAND
                && w.x0 > (PAGE_W - BAND - 96.0)),
            "page {page}: the text of the mark is not in the band"
        );
    }
    let fonts = String::from_utf8_lossy(&tool("pdffonts", &[], &pdf)).into_owned();
    for line in fonts.lines().skip(2) {
        let name = line.split_whitespace().next().unwrap_or("");
        let name = name.split_once('+').map_or(name, |(_, n)| n);
        assert!(
            name.starts_with("Inter"),
            "a font that is not bundled: {line}"
        );
    }
}

/// I4: logo and name in a Tailwind flex header: the name is on one line and starts right of the logo plus the gap,
/// which shows that the measure stage counts the image.
#[test]
fn i4_logo_and_name_in_a_flex_header() {
    let header = "<header class=\"hlavicka flex items-center gap-4 text-sm\">\
         <img src=\"logo.png\" style=\"height:32px\" alt=\"\"><span>Příkladná integrace s.r.o.</span></header>";
    let pdf = render(&doc(header, BODY));
    for page in [1, 2] {
        let w: Vec<Word> = words(&pdf)
            .into_iter()
            .filter(|w| w.page == page && w.y0 < BAND)
            .collect();
        let name: Vec<&Word> = w
            .iter()
            .filter(|w| ["Příkladná", "integrace", "s.r.o."].contains(&w.text.as_str()))
            .collect();
        assert_eq!(name.len(), 3, "page {page}: the name is {w:?}");
        let y = name[0].y0;
        assert!(
            name.iter().all(|w| (w.y0 - y).abs() < 1.0),
            "page {page}: the name wraps: {name:?}"
        );
        let want = BAND + 64.0 + 16.0;
        assert!(
            name[0].x0 >= want - 1.0,
            "page {page}: the name starts at {:.1} px, left of the logo and the gap ({want:.1} px)",
            name[0].x0
        );
        let ink = bbox(&raster_page(&pdf, page), is_red).expect("logo");
        near(ink.x1 - ink.x0, 64.0, 1.0, "logo width");
    }
}

/// I5: a `src` that is not bundled paints nothing and does not fail (non-strict, as for a body `<img>`).
#[test]
fn i5_unbundled_src_paints_nothing() {
    let html = doc(
        "<div class=\"hlavicka\"><img src=\"jinde.png\" style=\"height:32px\" alt=\"\"> Text</div>",
        BODY,
    );
    let pdf = renderer(false, true)
        .render(&html, &RenderOptions::default())
        .expect("renders");
    assert!(image_objects(&pdf).is_empty());
    assert!(bbox(&raster_page(&pdf, 1), is_red).is_none());
    assert!(words(&pdf).iter().any(|w| w.text == "Text" && w.y0 < BAND));
}

fn determinism_doc() -> String {
    doc(&format!("{LOGO}{MARK}"), BODY)
}

/// The child of I6: renders the document and writes it where the parent says.
#[test]
fn zz_child_process() {
    let Ok(out) = std::env::var(CHILD_ENV) else {
        return;
    };
    std::fs::write(out, render(&determinism_doc())).unwrap();
}

fn run_child(out: &Path, rayon_threads: u32) -> Vec<u8> {
    let o = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--test-threads=1", "--nocapture"])
        .env(CHILD_ENV, out)
        .env("RAYON_NUM_THREADS", rayon_threads.to_string())
        .output()
        .expect("spawn the test binary");
    assert!(
        o.status.success(),
        "child failed:\n{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    std::fs::read(out).unwrap()
}

/// I6: two renders, and one and four rayon threads, give the same bytes. Whether the two pages share one image
/// object is printed (`--nocapture`); two objects would be a size finding, not a failure.
#[test]
fn i6_deterministic_across_renders_and_threads() {
    let html = determinism_doc();
    let a = render(&html);
    assert_eq!(a, render(&html), "two renders in one process");
    let dir = std::env::temp_dir().join(format!("htmltopdf-f107-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let one = run_child(&dir.join("one.pdf"), 1);
    let four = run_child(&dir.join("four.pdf"), 4);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(one, four, "1 thread against 4 threads");
    assert_eq!(a, one, "this process against a fresh one");
    let objs = image_objects(&a);
    let mut ids: Vec<usize> = objs.iter().map(|o| o.1).collect();
    ids.sort();
    ids.dedup();
    eprintln!(
        "F107 I6: {} image rows, {} distinct image objects: {objs:?}",
        objs.len(),
        ids.len()
    );
}

/// I7: the words of the content area are the ones of the same document without the header.
#[test]
fn i7_content_area_is_unchanged() {
    let with = render(&doc(&format!("{LOGO}{MARK}"), BODY));
    let without = render(&doc("", BODY));
    let content = |pdf: &[u8]| -> Vec<(usize, String, i64, i64)> {
        words(pdf)
            .into_iter()
            .filter(|w| w.y0 >= BAND - 6.0)
            .map(|w| {
                (
                    w.page,
                    w.text,
                    (w.x0 * 100.0).round() as i64,
                    (w.y0 * 100.0).round() as i64,
                )
            })
            .collect()
    };
    let (a, b) = (content(&with), content(&without));
    assert!(!a.is_empty());
    assert_eq!(a, b);
}
