//! An `inline-block` sits on its line by the baseline of its last line box, not by its bottom edge (F183c, with F183b).
//!
//! CSS 2.1 section 10.8.1: the baseline of an `inline-block` is the baseline of its last line box in the normal flow;
//! when it has no in-flow line box, or its `overflow` is not `visible`, its bottom margin edge. css-align-3 section 9.1
//! (`baseline-source: auto`): the first baseline for `inline-flex`, `inline-grid` and `inline-table`. Every line box starts
//! with a strut (CSS 2.1 section 10.8), also the box's own. Parley 0.6 put every inline box bottom-on-baseline and Blitz
//! copied that position into the box's layout, but fulgur then painted the box lower by the free space under its last
//! line, so the recorded geometry and the paint disagreed (fixtures 43, 44, 45). Plan:
//! docs/plans/2026-11-04-f183c-inline-block-baseline.md, numbers in its section 7.
//!
//! Every case is an A4 page, 15 mm margins, `body { font: 14px/20px Inter; margin: 0 }`, `p { margin: 0 }`. The numbers
//! are Chrome m153's (`scripts/reference/render.mjs` settings, Inter from `fixtures/fonts` through `@font-face` so that
//! `pdftotext` sees the same font metrics as ours): the top of each word's box in pt (`pdftotext -bbox`, `yMin`; ours is
//! within 0.75 pt = 1 px of Chrome's) and the rows of the magenta (`#f0f`) border of the box under test at 96 dpi
//! (within 1 px).
//!
//! The tests are `#[ignore]`d until the step of the plan that turns them green; `cargo test -- --ignored` shows what is
//! still red. `b11_*` is a guard that holds before and after.

use std::process::Command;
use std::sync::{Mutex, Once};

use htmltopdf::{RenderOptions, Renderer};

const RED16: &[u8] = include_bytes!("data/red16.png");
const RED60: &[u8] = include_bytes!("data/red60.png");

const HEAD: &str = "@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}";
/// Tolerance of a word's `yMin` in pt (1 CSS px).
const TOL_PT: f32 = 0.75;
/// Tolerance of a box edge in raster rows at 96 dpi.
const TOL_ROWS: i32 = 1;

struct Case {
    name: &'static str,
    body: &'static str,
    /// `(word, yMin in pt)` on page 1, Chrome's numbers.
    words: &'static [(&'static str, f32)],
    /// First and last+1 raster row of the magenta border on page 1, Chrome's numbers.
    box_rows: Option<(i32, i32)>,
}

// --- harness ---------------------------------------------------------------------------------------------------

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Held while a document is rendered and its warnings are drained, so that one test's warnings are not another's.
static RENDER: Mutex<()> = Mutex::new(());
static INIT: Once = Once::new();

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.level() <= log::Level::Warn
            && (r.target().starts_with("fulgur") || r.target().starts_with("htmltopdf"))
        {
            LOG.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// The PDF and the warnings of rendering `body` on the page of this file.
fn render(body: &str) -> (Vec<u8>, Vec<String>) {
    INIT.call_once(|| {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    let _guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
    LOG.lock().unwrap().clear();
    let pdf = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .image("red16.png", RED16.to_vec())
        .image("red60.png", RED60.to_vec())
        .strict_fonts(true)
        .strict_images(true)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><meta charset=utf-8><style>{HEAD}</style></head><body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render");
    let warnings = std::mem::take(&mut *LOG.lock().unwrap());
    (pdf, warnings)
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-ibb-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

/// One printed word: its 1-based page, the top of its box in pt from the page top and its text.
#[derive(Clone, Debug)]
struct Word {
    page: usize,
    y_min: f32,
    text: String,
}

fn printed_words(pdf: &[u8]) -> Vec<Word> {
    let path = temp(pdf);
    let out = Command::new("pdftotext")
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

/// First and last+1 row of the magenta (`#f0f`) pixels on page 1 at 96 dpi.
fn magenta_rows(pdf: &[u8]) -> Option<(i32, i32)> {
    let path = temp(pdf);
    let out = Command::new("pdftoppm")
        .args(["-r", "96", "-f", "1", "-l", "1"])
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    let data = out.stdout;
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
    let rgb = &data[at..at + w * h * 3];
    let mut rows: Option<(i32, i32)> = None;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            if rgb[i] > 200 && rgb[i + 1] < 80 && rgb[i + 2] > 200 {
                let r = rows.get_or_insert((y as i32, y as i32 + 1));
                r.1 = y as i32 + 1;
                break;
            }
        }
    }
    rows
}

/// Every `Case` of `cases` against Chrome's numbers; one message lists all the differences.
#[track_caller]
fn check(cases: &[Case]) {
    let mut bad = Vec::new();
    for c in cases {
        let (pdf, warnings) = render(c.body);
        let words = printed_words(&pdf);
        for (text, chrome) in c.words {
            match words.iter().find(|w| w.text == *text) {
                None => bad.push(format!("{}: `{text}` is not printed", c.name)),
                Some(w) if w.page != 1 => bad.push(format!(
                    "{}: `{text}` is on page {}, Chrome has it on page 1",
                    c.name, w.page
                )),
                Some(w) if (w.y_min - chrome).abs() > TOL_PT => bad.push(format!(
                    "{}: `{text}` at {:.1} pt, Chrome {chrome:.1} pt",
                    c.name, w.y_min
                )),
                _ => {}
            }
        }
        if let Some((y0, y1)) = c.box_rows {
            match magenta_rows(&pdf) {
                None => bad.push(format!("{}: the box is not painted", c.name)),
                Some((a, b)) if (a - y0).abs() > TOL_ROWS || (b - y1).abs() > TOL_ROWS => {
                    bad.push(format!("{}: box rows {a}..{b}, Chrome {y0}..{y1}", c.name))
                }
                _ => {}
            }
        }
        if !warnings.is_empty() {
            bad.push(format!("{}: warnings {warnings:?}", c.name));
        }
    }
    assert!(
        bad.is_empty(),
        "differences from Chrome m153:\n{}",
        bad.join("\n")
    );
}

fn y_of(words: &[Word], text: &str) -> f32 {
    words
        .iter()
        .find(|w| w.text == text)
        .unwrap_or_else(|| panic!("`{text}` is not printed"))
        .y_min
}

const B1: &[Case] = &[
    Case {
        name: "b01_stamp",
        body: r##"<p>BEFORE</p><div><span style="display:inline-block;border:3px solid #f0f;width:140px;height:60px;padding-top:12px;text-align:center">ZAPLACENO</span></div><p>AFTER</p>"##,
        words: &[("BEFORE", 43.8), ("ZAPLACENO", 70.1), ("AFTER", 117.3)],
        box_rows: Some((77, 155)),
    },
    Case {
        name: "b01b_stamp_2lines",
        body: r##"<p>BEFORE</p><div><span style="display:inline-block;border:3px solid #f0f;width:140px;height:60px;padding-top:12px;text-align:center">ZAPLACENO<br>4.11.2026</span></div><p>AFTER</p>"##,
        words: &[("ZAPLACENO", 70.1), ("4.11.2026", 85.1), ("AFTER", 117.3)],
        box_rows: Some((77, 155)),
    },
    Case {
        name: "b01c_stamp_in_p_text",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-block;border:3px solid #f0f;width:140px;height:60px;padding-top:12px;text-align:center">ZAPLACENO</span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 70.1),
            ("ZAPLACENO", 70.1),
            ("konec", 70.1),
            ("AFTER", 117.3),
        ],
        box_rows: Some((77, 155)),
    },
];

const B2: &[Case] = &[
    Case {
        name: "b02_qr_caption",
        body: r##"<p>BEFORE</p><p>Zaplatte <span style="display:inline-block;border:3px solid #f0f;width:112px;height:112px;border-color:#f0f">QR kod</span> nebo prevodem na ucet</p><p>AFTER</p>"##,
        words: &[
            ("Zaplatte", 61.1),
            ("QR", 61.1),
            ("nebo", 61.1),
            ("AFTER", 147.3),
        ],
        box_rows: Some((77, 195)),
    },
    Case {
        name: "b02b_qr_caption_bottom",
        body: r##"<p>BEFORE</p><p>Zaplatte <span style="display:inline-block;border:3px solid #f0f;width:112px;height:112px;border-color:#f0f;padding-top:80px;height:32px">QR kod</span> nebo</p><p>AFTER</p>"##,
        words: &[
            ("Zaplatte", 121.1),
            ("QR", 121.1),
            ("nebo", 121.1),
            ("AFTER", 147.3),
        ],
        box_rows: Some((77, 195)),
    },
    Case {
        name: "x_ctx_p",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:112px;height:112px">QRTXT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("QRTXT", 61.1), ("Y", 61.1), ("AFTER", 147.3)],
        box_rows: Some((77, 195)),
    },
];

const B3: &[Case] = &[Case {
    name: "b03_f43_shape",
    body: r##"<p>BEFORE</p><p>Zaplatte QR kodem <span style="display:inline-block;border:3px solid #f0f;padding:16px"><span style="display:block;font-size:12px;line-height:16px;margin-bottom:4px">PLATBA</span><span style="display:block;width:112px;height:112px;background:#cde"></span><span style="display:block;font-size:12px;line-height:16px">AMOUNT</span></span> nebo prevodem</p><p>AFTER</p>"##,
    words: &[
        ("Zaplatte", 169.8),
        ("PLATBA", 72.3),
        ("AMOUNT", 171.3),
        ("nebo", 169.8),
        ("AFTER", 198.3),
    ],
    box_rows: Some((77, 263)),
}];

const B4: &[Case] = &[
    Case {
        name: "b04_badges",
        body: r##"<p>BEFORE</p><p>Stav: <span style="display:inline-block;border:3px solid #f0f;border-radius:9999px;padding:2px 12px;font-size:12px;line-height:16px;font-weight:600;background:#dfd">Zaplaceno</span> <span style="display:inline-block;border:3px solid #f0f;border-radius:9999px;padding:2px 12px;font-size:12px;line-height:16px;font-weight:600;background:#dfd;margin-left:4px">Zaloha</span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav:", 60.3),
            ("Zaplaceno", 61.8),
            ("Zaloha", 61.8),
            ("konec", 60.3),
            ("AFTER", 78.3),
        ],
        box_rows: Some((77, 103)),
    },
    Case {
        name: "b04b_badge_alone",
        body: r##"<p>BEFORE</p><p><span style="display:inline-block;border:3px solid #f0f;border-radius:9999px;padding:2px 12px;font-size:12px;line-height:16px;font-weight:600;background:#dfd">Zaplaceno</span></p><p>AFTER</p>"##,
        words: &[("Zaplaceno", 61.8), ("AFTER", 78.3)],
        box_rows: Some((77, 103)),
    },
];

const B8: &[Case] = &[
    Case {
        name: "b08_nested_last_child_ib",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:200px"><span style="display:block">NEST1</span><span style="display:block"><span style="display:inline-block;border:3px solid #f0f;width:60px;height:30px;border-color:#00f">INNER</span></span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 78.3),
            ("NEST1", 61.1),
            ("INNER", 78.3),
            ("Y", 78.3),
            ("AFTER", 105.3),
        ],
        box_rows: Some((77, 139)),
    },
    Case {
        name: "b08b_nested_ib_in_ib",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;"><span style="display:inline-block;width:60px;height:30px;border:2px solid #00f">INNER</span></span> Y</p><p>AFTER</p>"##,
        words: &[("X", 62.6), ("INNER", 62.6), ("Y", 62.6), ("AFTER", 88.8)],
        box_rows: Some((77, 117)),
    },
    Case {
        name: "b08c_nested_fixed",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:200px;height:100px"><span style="display:block">NEST1</span><span style="display:block"><span style="display:inline-block;width:60px;height:30px;border:2px solid #00f">INNER</span></span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 77.6),
            ("NEST1", 61.1),
            ("INNER", 77.6),
            ("Y", 77.6),
            ("AFTER", 138.3),
        ],
        box_rows: Some((77, 183)),
    },
    Case {
        name: "x_para_in_ib",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px"><span style="display:block;padding-bottom:8px">PARA1<br>PARA2</span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 76.1),
            ("PARA1", 61.1),
            ("PARA2", 76.1),
            ("Y", 76.1),
            ("AFTER", 99.3),
        ],
        box_rows: Some((77, 131)),
    },
    Case {
        name: "x_ib_block_mix",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px"><span style="display:block">AA</span>BB<span style="display:block">CC</span>DD</span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 106.1),
            ("AA", 61.1),
            ("BB", 76.1),
            ("CC", 91.1),
            ("DD", 106.1),
            ("Y", 106.1),
            ("AFTER", 123.3),
        ],
        box_rows: Some((77, 163)),
    },
    Case {
        name: "x_ib_ul_li",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px"><span style="display:block">FIRST</span><span style="display:block;height:30px">TALL</span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 76.1),
            ("FIRST", 61.1),
            ("TALL", 76.1),
            ("Y", 76.1),
            ("AFTER", 100.8),
        ],
        box_rows: Some((77, 133)),
    },
    Case {
        name: "x_ib_empty_last_block",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px"><span style="display:block">FIRST</span><span style="display:block;height:30px"></span></span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("FIRST", 61.1), ("Y", 61.1), ("AFTER", 100.8)],
        box_rows: Some((77, 133)),
    },
];

const B12: &[Case] = &[
    Case {
        name: "b12_margin_top",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;margin-top:10px;width:100px;height:60px">MT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 68.6), ("MT", 68.6), ("Y", 68.6), ("AFTER", 115.8)],
        box_rows: Some((87, 153)),
    },
    Case {
        name: "b12b_margin_bottom",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;margin-bottom:10px;width:100px;height:60px">MB</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("MB", 61.1), ("Y", 61.1), ("AFTER", 115.8)],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b12c_margin_both_auto_h",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;margin:7px 3px;width:100px">MTB</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 66.3), ("MTB", 66.3), ("Y", 66.3), ("AFTER", 88.8)],
        box_rows: Some((84, 110)),
    },
    Case {
        name: "b12d_last_child_margin_bottom",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px"><span style="display:block;margin-bottom:12px">LCM</span></span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("LCM", 61.1), ("Y", 61.1), ("AFTER", 87.3)],
        box_rows: Some((77, 115)),
    },
];

const B13: &[Case] = &[
    Case {
        name: "b13_float_last",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px"><span style="display:block">LINE1</span><span style="display:block">LINE2</span><span style="float:right">FLT</span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 76.1),
            ("LINE1", 61.1),
            ("LINE2", 76.1),
            ("FLT", 91.1),
            ("Y", 76.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b13b_abs_last",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;position:relative"><span style="display:block">LINE1</span><span style="display:block">LINE2</span><span style="position:absolute;right:0;top:0">ABS</span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 76.1),
            ("LINE1", 61.1),
            ("LINE2", 76.1),
            ("ABS", 61.1),
            ("Y", 76.1),
            ("AFTER", 93.3),
        ],
        box_rows: Some((77, 123)),
    },
    Case {
        name: "b13c_float_last_fixed",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;height:100px"><span style="display:block">LINE1</span><span style="float:right">FLT</span></span> Y</p><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("LINE1", 61.1),
            ("FLT", 76.1),
            ("Y", 61.1),
            ("AFTER", 138.3),
        ],
        box_rows: Some((77, 183)),
    },
];

const FIXED: &[Case] = &[
    Case {
        name: "f_fixed60",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;height:60px">TOPTXT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("TOPTXT", 61.1), ("Y", 61.1), ("AFTER", 108.3)],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "f_fixed112",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;height:112px">TOPTXT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("TOPTXT", 61.1), ("Y", 61.1), ("AFTER", 147.3)],
        box_rows: Some((77, 195)),
    },
    Case {
        name: "f_fixed300",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;height:300px">TOPTXT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("TOPTXT", 61.1), ("Y", 61.1), ("AFTER", 288.3)],
        box_rows: Some((77, 383)),
    },
    Case {
        name: "f_fixed600",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:150px;height:600px">TOPTXT</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("TOPTXT", 61.1), ("Y", 61.1), ("AFTER", 513.3)],
        box_rows: Some((77, 683)),
    },
    Case {
        name: "x_ib_ib_only_p",
        body: r##"<p>BEFORE</p><p><span style="display:inline-block;border:3px solid #f0f;width:100px;height:40px">ONLY</span></p><p>AFTER</p>"##,
        words: &[("ONLY", 61.1), ("AFTER", 93.3)],
        box_rows: Some((77, 123)),
    },
    Case {
        name: "x_lh_box_1",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;line-height:1">LH1</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 58.8), ("LH1", 58.8), ("Y", 58.8), ("AFTER", 108.3)],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "x_lh_box_30",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;line-height:30px">LH30</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 64.8), ("LH30", 64.8), ("Y", 64.8), ("AFTER", 108.3)],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "x_fs_box_big",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;font-size:30px;line-height:36px">BIG</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 71.6), ("BIG", 60.0), ("Y", 71.6), ("AFTER", 108.3)],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "x_pad_bottom",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;padding-bottom:20px;box-sizing:border-box">PADB</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 61.1), ("PADB", 61.1), ("Y", 61.1), ("AFTER", 103.8)],
        box_rows: Some((77, 137)),
    },
    Case {
        name: "x_ib_absent_text_strut_p",
        body: r##"<p>BEFORE</p><p style="font-size:30px;line-height:36px">X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:40px">TALLP</span> Y</p><p>AFTER</p>"##,
        words: &[("X", 60.0), ("TALLP", 60.0), ("Y", 60.0), ("AFTER", 93.3)],
        box_rows: Some((77, 123)),
    },
];

const CTX: &[Case] = &[
    Case {
        name: "x_ctx_grid",
        body: r##"<p>BEFORE</p><div style="display:grid;grid-template-columns:1fr 1fr;gap:16px"><div>X <span style="display:inline-block;border:3px solid #f0f;width:112px;height:112px">QRTXT</span> Y</div><div>R</div></div><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("QRTXT", 61.1),
            ("Y", 61.1),
            ("R", 58.8),
            ("AFTER", 147.3),
        ],
        box_rows: Some((77, 195)),
    },
    Case {
        name: "x_ctx_cell",
        body: r##"<p>BEFORE</p><table style="border-collapse:collapse;width:100%"><tr><td>X <span style="display:inline-block;border:3px solid #f0f;width:112px;height:112px">QRTXT</span> Y</td><td>R</td></tr></table><p>AFTER</p>"##,
        words: &[
            ("X", 61.8),
            ("QRTXT", 61.8),
            ("Y", 61.8),
            ("R", 96.3),
            ("AFTER", 148.8),
        ],
        box_rows: Some((78, 196)),
    },
];

const B5: &[Case] = &[
    Case {
        name: "b05_ovf_visible",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow:visible">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("OVFTEXT", 61.1),
            ("tail", 61.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b05_ovf_hidden",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow:hidden">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 97.1),
            ("OVFTEXT", 61.1),
            ("tail", 97.1),
            ("AFTER", 112.1),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b05_ovf_clip",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow:clip">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("OVFTEXT", 61.1),
            ("tail", 61.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b05_ovf_auto",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow:auto">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 97.1),
            ("OVFTEXT", 61.1),
            ("tail", 97.1),
            ("AFTER", 112.1),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b05_ovf_scroll",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow:scroll">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 97.1),
            ("OVFTEXT", 61.1),
            ("tail", 97.1),
            ("AFTER", 112.1),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b05_ovf_hidden_auto_h",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;overflow:hidden">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 67.1),
            ("OVFTEXT", 61.1),
            ("tail", 67.1),
            ("AFTER", 82.1),
        ],
        box_rows: Some((77, 103)),
    },
    Case {
        name: "b05_ovf_clip_auto_h",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;overflow:clip">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("OVFTEXT", 61.1),
            ("tail", 61.1),
            ("AFTER", 78.3),
        ],
        box_rows: Some((77, 103)),
    },
    Case {
        name: "b05_ovf_clip_x",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:100px;height:60px;overflow-x:clip">OVFTEXT</span> tail</p><p>AFTER</p>"##,
        words: &[
            ("X", 61.1),
            ("OVFTEXT", 61.1),
            ("tail", 61.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
];

const B6: &[Case] = &[
    Case {
        name: "b06_empty48",
        body: r##"<p>BEFORE</p><p><span style="display:inline-block;border:3px solid #f0f;width:48px;height:48px"></span></p><p>AFTER</p>"##,
        words: &[("AFTER", 103.1)],
        box_rows: Some((77, 131)),
    },
    Case {
        name: "b06b_empty48_text",
        body: r##"<p>BEFORE</p><p>X <span style="display:inline-block;border:3px solid #f0f;width:48px;height:48px"></span> Y</p><p>AFTER</p>"##,
        words: &[("X", 88.1), ("Y", 88.1), ("AFTER", 103.1)],
        box_rows: Some((77, 131)),
    },
];

const IMG: &[Case] = &[
    Case {
        name: "x_img_in_ib",
        body: r##"<p>BEFORE</p><p>Klik <span style="display:inline-block;border:3px solid #f0f;"><img src="red60.png" width="60" height="60"></span> end</p><p>AFTER</p>"##,
        words: &[("Klik", 94.8), ("end", 94.8), ("AFTER", 112.1)],
        box_rows: Some((77, 148)),
    },
    Case {
        name: "x_img_in_ib_text",
        body: r##"<p>BEFORE</p><p>Klik <span style="display:inline-block;border:3px solid #f0f;"><img src="red60.png" width="60" height="60"> IMGTXT</span> end</p><p>AFTER</p>"##,
        words: &[
            ("Klik", 94.8),
            ("IMGTXT", 94.8),
            ("end", 94.8),
            ("AFTER", 112.1),
        ],
        box_rows: Some((77, 148)),
    },
    Case {
        name: "x_img_in_ib_block",
        body: r##"<p>BEFORE</p><p>Klik <span style="display:inline-block;border:3px solid #f0f;"><span style="display:block"><img src="red60.png" width="60" height="60"></span></span> end</p><p>AFTER</p>"##,
        words: &[("Klik", 94.8), ("end", 94.8), ("AFTER", 112.1)],
        box_rows: Some((77, 148)),
    },
    Case {
        name: "x_img_in_button",
        body: r##"<p>BEFORE</p><p>Klik <button style="font:14px/20px Inter;border:3px solid #f0f;padding:0"><img src="red16.png" width="16" height="16"> Pay</button> end</p><p>AFTER</p>"##,
        words: &[
            ("Klik", 61.8),
            ("Pay", 61.8),
            ("end", 61.8),
            ("AFTER", 79.1),
        ],
        box_rows: Some((77, 104)),
    },
];

const B7: &[Case] = &[
    Case {
        name: "b07_iflex_badge",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-flex;border:3px solid #f0f;padding:2px 8px;font-size:12px;line-height:16px;align-items:center">Zaplaceno</span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 60.3),
            ("Zaplaceno", 61.8),
            ("konec", 60.3),
            ("AFTER", 78.3),
        ],
        box_rows: Some((77, 103)),
    },
    Case {
        name: "b07c_iflex_items_center",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-flex;border:3px solid #f0f;align-items:center"><span style="height:40px;width:40px;background:#fdd">ITEMA</span><span style="height:20px;width:40px;background:#dfd">ITEMB</span></span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 61.1),
            ("ITEMA", 61.1),
            ("ITEMB", 68.6),
            ("konec", 61.1),
            ("AFTER", 93.3),
        ],
        box_rows: Some((77, 123)),
    },
    Case {
        name: "b07e_iflex_col",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-flex;border:3px solid #f0f;flex-direction:column"><span>COL1</span><span>COL2</span></span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 61.1),
            ("COL1", 61.1),
            ("COL2", 76.1),
            ("konec", 61.1),
            ("AFTER", 93.3),
        ],
        box_rows: Some((77, 123)),
    },
    Case {
        name: "b07f_igrid",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-grid;border:3px solid #f0f;grid-template-columns:50px 50px"><span style="height:40px">GRIDA</span><span>GRIDB</span><span>GRIDC</span></span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 61.1),
            ("GRIDA", 61.1),
            ("GRIDB", 61.1),
            ("GRIDC", 91.1),
            ("konec", 61.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
    Case {
        name: "b07g_itable",
        body: r##"<p>BEFORE</p><p>Stav <span style="display:inline-table;border:3px solid #f0f;"><span style="display:table-row"><span style="display:table-cell;height:40px">TROW1</span></span><span style="display:table-row"><span style="display:table-cell">TROW2</span></span></span> konec</p><p>AFTER</p>"##,
        words: &[
            ("Stav", 61.1),
            ("TROW1", 61.1),
            ("TROW2", 91.1),
            ("konec", 61.1),
            ("AFTER", 108.3),
        ],
        box_rows: Some((77, 143)),
    },
];

/// `(with text, filler px, pages, [(word, page, yMin)])`, Chrome's numbers.
type Row = (bool, u32, usize, &'static [(&'static str, usize, f32)]);
const B10: &[Row] = &[
    (
        true,
        700,
        1,
        &[
            ("QRTEXT", 1, 570.3),
            ("ROWEND", 1, 570.3),
            ("AFTERROW", 1, 655.8),
            ("LASTWORD", 1, 670.8),
        ],
    ),
    (
        true,
        712,
        1,
        &[
            ("QRTEXT", 1, 579.3),
            ("ROWEND", 1, 579.3),
            ("AFTERROW", 1, 664.8),
            ("LASTWORD", 1, 679.8),
        ],
    ),
    (
        true,
        724,
        1,
        &[
            ("QRTEXT", 1, 588.3),
            ("ROWEND", 1, 588.3),
            ("AFTERROW", 1, 673.8),
            ("LASTWORD", 1, 688.8),
        ],
    ),
    (
        true,
        736,
        1,
        &[
            ("QRTEXT", 1, 597.3),
            ("ROWEND", 1, 597.3),
            ("AFTERROW", 1, 682.8),
            ("LASTWORD", 1, 697.8),
        ],
    ),
    (
        true,
        748,
        1,
        &[
            ("QRTEXT", 1, 606.3),
            ("ROWEND", 1, 606.3),
            ("AFTERROW", 1, 691.8),
            ("LASTWORD", 1, 706.8),
        ],
    ),
    (
        true,
        760,
        1,
        &[
            ("QRTEXT", 1, 615.3),
            ("ROWEND", 1, 615.3),
            ("AFTERROW", 1, 700.8),
            ("LASTWORD", 1, 715.8),
        ],
    ),
    (
        true,
        772,
        1,
        &[
            ("QRTEXT", 1, 624.3),
            ("ROWEND", 1, 624.3),
            ("AFTERROW", 1, 709.8),
            ("LASTWORD", 1, 724.8),
        ],
    ),
    (
        true,
        784,
        1,
        &[
            ("QRTEXT", 1, 633.3),
            ("ROWEND", 1, 633.3),
            ("AFTERROW", 1, 718.8),
            ("LASTWORD", 1, 733.8),
        ],
    ),
    (
        true,
        796,
        1,
        &[
            ("QRTEXT", 1, 642.3),
            ("ROWEND", 1, 642.3),
            ("AFTERROW", 1, 727.8),
            ("LASTWORD", 1, 742.8),
        ],
    ),
    (
        true,
        808,
        1,
        &[
            ("QRTEXT", 1, 651.3),
            ("ROWEND", 1, 651.3),
            ("AFTERROW", 1, 736.8),
            ("LASTWORD", 1, 751.8),
        ],
    ),
    (
        true,
        820,
        1,
        &[
            ("QRTEXT", 1, 660.3),
            ("ROWEND", 1, 660.3),
            ("AFTERROW", 1, 745.8),
            ("LASTWORD", 1, 760.8),
        ],
    ),
    (
        true,
        832,
        1,
        &[
            ("QRTEXT", 1, 669.3),
            ("ROWEND", 1, 669.3),
            ("AFTERROW", 1, 754.8),
            ("LASTWORD", 1, 769.8),
        ],
    ),
    (
        true,
        844,
        1,
        &[
            ("QRTEXT", 1, 678.3),
            ("ROWEND", 1, 678.3),
            ("AFTERROW", 1, 763.8),
            ("LASTWORD", 1, 778.8),
        ],
    ),
    (
        true,
        856,
        2,
        &[
            ("QRTEXT", 1, 687.3),
            ("ROWEND", 1, 687.3),
            ("AFTERROW", 1, 772.8),
            ("LASTWORD", 2, 43.8),
        ],
    ),
    (
        true,
        868,
        2,
        &[
            ("QRTEXT", 1, 696.3),
            ("ROWEND", 1, 696.3),
            ("AFTERROW", 1, 781.8),
            ("LASTWORD", 2, 43.8),
        ],
    ),
    (
        true,
        880,
        2,
        &[
            ("QRTEXT", 1, 705.3),
            ("ROWEND", 1, 705.3),
            ("AFTERROW", 2, 43.8),
            ("LASTWORD", 2, 58.8),
        ],
    ),
    (
        true,
        892,
        2,
        &[
            ("QRTEXT", 1, 714.3),
            ("ROWEND", 1, 714.3),
            ("AFTERROW", 2, 43.8),
            ("LASTWORD", 2, 58.8),
        ],
    ),
    (
        true,
        904,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        916,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        928,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        940,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        952,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        964,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        976,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        988,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        true,
        1000,
        2,
        &[
            ("QRTEXT", 2, 45.3),
            ("ROWEND", 2, 45.3),
            ("AFTERROW", 2, 130.8),
            ("LASTWORD", 2, 145.8),
        ],
    ),
    (
        false,
        700,
        1,
        &[
            ("ROWEND", 1, 644.6),
            ("AFTERROW", 1, 659.6),
            ("LASTWORD", 1, 674.6),
        ],
    ),
    (
        false,
        712,
        1,
        &[
            ("ROWEND", 1, 653.6),
            ("AFTERROW", 1, 668.6),
            ("LASTWORD", 1, 683.6),
        ],
    ),
    (
        false,
        724,
        1,
        &[
            ("ROWEND", 1, 662.6),
            ("AFTERROW", 1, 677.6),
            ("LASTWORD", 1, 692.6),
        ],
    ),
    (
        false,
        736,
        1,
        &[
            ("ROWEND", 1, 671.6),
            ("AFTERROW", 1, 686.6),
            ("LASTWORD", 1, 701.6),
        ],
    ),
    (
        false,
        748,
        1,
        &[
            ("ROWEND", 1, 680.6),
            ("AFTERROW", 1, 695.6),
            ("LASTWORD", 1, 710.6),
        ],
    ),
    (
        false,
        760,
        1,
        &[
            ("ROWEND", 1, 689.6),
            ("AFTERROW", 1, 704.6),
            ("LASTWORD", 1, 719.6),
        ],
    ),
    (
        false,
        772,
        1,
        &[
            ("ROWEND", 1, 698.6),
            ("AFTERROW", 1, 713.6),
            ("LASTWORD", 1, 728.6),
        ],
    ),
    (
        false,
        784,
        1,
        &[
            ("ROWEND", 1, 707.6),
            ("AFTERROW", 1, 722.6),
            ("LASTWORD", 1, 737.6),
        ],
    ),
    (
        false,
        796,
        1,
        &[
            ("ROWEND", 1, 716.6),
            ("AFTERROW", 1, 731.6),
            ("LASTWORD", 1, 746.6),
        ],
    ),
    (
        false,
        808,
        1,
        &[
            ("ROWEND", 1, 725.6),
            ("AFTERROW", 1, 740.6),
            ("LASTWORD", 1, 755.6),
        ],
    ),
    (
        false,
        820,
        1,
        &[
            ("ROWEND", 1, 734.6),
            ("AFTERROW", 1, 749.6),
            ("LASTWORD", 1, 764.6),
        ],
    ),
    (
        false,
        832,
        1,
        &[
            ("ROWEND", 1, 743.6),
            ("AFTERROW", 1, 758.6),
            ("LASTWORD", 1, 773.6),
        ],
    ),
    (
        false,
        844,
        1,
        &[
            ("ROWEND", 1, 752.6),
            ("AFTERROW", 1, 767.6),
            ("LASTWORD", 1, 782.6),
        ],
    ),
    (
        false,
        856,
        2,
        &[
            ("ROWEND", 1, 761.6),
            ("AFTERROW", 1, 776.6),
            ("LASTWORD", 2, 43.8),
        ],
    ),
    (
        false,
        868,
        2,
        &[
            ("ROWEND", 1, 770.6),
            ("AFTERROW", 1, 785.6),
            ("LASTWORD", 2, 43.8),
        ],
    ),
    (
        false,
        880,
        2,
        &[
            ("ROWEND", 1, 779.6),
            ("AFTERROW", 2, 43.8),
            ("LASTWORD", 2, 58.8),
        ],
    ),
    (
        false,
        892,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        904,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        916,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        928,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        940,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        952,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        964,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        976,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        988,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
    (
        false,
        1000,
        2,
        &[
            ("ROWEND", 2, 119.6),
            ("AFTERROW", 2, 134.6),
            ("LASTWORD", 2, 149.6),
        ],
    ),
];
// --- tests -----------------------------------------------------------------------------------------------------------
//
// Step 3 of the plan (Blitz reports the baseline, fulgur stops moving the box); step 4 (F183b: the strut covers atomic
// inlines); step 5 (first baseline of inline-flex, inline-grid, inline-table).

#[test]
fn b1_stamp_alone_in_a_block() {
    // The 140 x 60 stamp (12 px padding on top, 3 px border) alone in a block: Chrome's box top is at line top, its
    // text baseline is the box's baseline, the next block follows the box. Ours painted the box 36 pt low.
    check(B1);
}

#[test]
fn b2_qr_caption_box_in_a_sentence_is_level_with_its_text() {
    check(B2);
}

#[test]
fn b3_fixture_43_shape_label_qr_amount() {
    check(B3);
}

#[test]
fn b4_badges_on_a_text_line() {
    check(B4);
}

#[test]
fn b5_overflow_decides_the_baseline() {
    // Chrome m153: `overflow: hidden`, `auto` and `scroll` put the bottom margin edge on the baseline (the text sits on
    // the bottom of the box), `visible` and `clip` the last line box's baseline. The plan assumed `clip` is "not visible";
    // Chrome does not treat it so (plan section 7).
    check(B5);
}

#[test]
fn b6_empty_box_keeps_the_strut_descent_below_it() {
    // A box without in-flow content has its bottom margin edge on the baseline and the strut's descent under it:
    // the next block follows 4-5 px below the box (48 + 2 x 3 px border + the strut's descent).
    check(B6);
}

#[test]
fn images_inside_an_atomic_inline_keep_the_strut() {
    check(IMG);
}

#[test]
fn b7_inline_flex_grid_table_use_the_first_baseline() {
    // css-align-3 section 9.1: `baseline-source: auto` is the last baseline for `inline-block` and the first for every
    // other box. `inline-table` made of `display: table-row/-cell` boxes is covered; a real `<table>` with the default
    // `vertical-align: middle` cells is not (Chrome puts the baseline on the cell's bottom, plan section 7: filed).
    check(B7);
}

#[test]
fn b8_nested_boxes_the_last_block_child_holds_an_inline_block() {
    // The baseline of the outer box is that of the last line box in its normal flow, found through the last block
    // child that has one; an empty last block child is skipped.
    check(B8);
}

#[test]
fn b9_fixed_box_with_more_lines_than_fit_prints_every_line_once() {
    // F183c-b: 40 lines of 20 px in a 300 px box. Chrome prints all 40 lines (they overflow the box), the line box
    // reaches down to the last of them and AFTER follows at 646.1 pt. Ours seated the overflow above the page top:
    // L01 to L22 were on no page.
    let lines = (1..=40)
        .map(|i| format!("L{i:02}"))
        .collect::<Vec<_>>()
        .join("<br>");
    let body = format!(
        "<p><span style=\"display:inline-block;width:150px;height:300px;border:3px solid #f0f\">{lines}</span></p><p>AFTER</p>"
    );
    let (pdf, warnings) = render(&body);
    let words = printed_words(&pdf);
    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    let mut bad = Vec::new();
    for i in 1..=40 {
        let t = format!("L{i:02}");
        let hits: Vec<_> = words.iter().filter(|w| w.text == t).collect();
        if hits.len() != 1 {
            bad.push(format!("`{t}` is printed {} times", hits.len()));
        } else if hits[0].page != 1
            || (hits[0].y_min - (46.1 + 15.0 * (i - 1) as f32)).abs() > TOL_PT
        {
            bad.push(format!(
                "`{t}` at {:.1} pt on page {}, Chrome {:.1} pt on page 1",
                hits[0].y_min,
                hits[0].page,
                46.1 + 15.0 * (i - 1) as f32
            ));
        }
    }
    if (y_of(&words, "AFTER") - 646.1).abs() > TOL_PT {
        bad.push(format!(
            "AFTER at {:.1} pt, Chrome 646.1 pt",
            y_of(&words, "AFTER")
        ));
    }
    assert!(
        bad.is_empty(),
        "differences from Chrome m153:\n{}",
        bad.join("\n")
    );
    // 14 lines (they fit): the first line is at the top of the box, AFTER follows the box.
    let lines = (1..=14)
        .map(|i| format!("L{i:02}"))
        .collect::<Vec<_>>()
        .join("<br>");
    check(&[Case {
        name: "b9b_14_lines_in_300px",
        body: Box::leak(
            format!("<p><span style=\"display:inline-block;width:150px;height:300px;border:3px solid #f0f\">{lines}</span></p><p>AFTER</p>")
                .into_boxed_str(),
        ),
        words: &[("L01", 46.1), ("L14", 241.1), ("AFTER", 273.3)],
        box_rows: Some((57, 363)),
    }]);
}

#[test]
fn b10_box_in_a_plain_block_at_the_rooms_edge() {
    // The `pg_t_*` / `pg_e_*` probes of F217's validator, with a plain block instead of a flex row: a 112 px box with
    // text (`t`) or empty (`e`) in a sentence, N px of filler above. Pages, the place of every word and no warning
    // ("content extends N pt past the bottom page edge" for the `t` probes with N 856 to 892 on main).
    let mut bad = Vec::new();
    for (with_text, n, pages, words) in B10 {
        let inner = if *with_text { "QRTEXT" } else { "" };
        let body = format!(
            "<div style=\"height:{n}px;background:#ffd\"></div><p>Zaplatte <span style=\"display:inline-block;width:112px;height:112px;border:2px solid #999;background:#eef\">{inner}</span> nebo ROWEND</p><p>AFTERROW text text</p><p>LASTWORD</p>"
        );
        let name = format!("pg_{}_{n}", if *with_text { "t" } else { "e" });
        let (pdf, warnings) = render(&body);
        let got = printed_words(&pdf);
        if !warnings.is_empty() {
            bad.push(format!("{name}: warnings {warnings:?}"));
        }
        let got_pages = got.iter().map(|w| w.page).max().unwrap_or(0);
        if got_pages != *pages {
            bad.push(format!("{name}: {got_pages} pages, Chrome {pages}"));
        }
        if got.len() != if *with_text { 8 } else { 7 } {
            bad.push(format!(
                "{name}: {} words, Chrome has {}",
                got.len(),
                if *with_text { 8 } else { 7 }
            ));
        }
        for (text, page, y) in *words {
            match got.iter().find(|w| w.text == *text) {
                None => bad.push(format!("{name}: `{text}` is not printed")),
                Some(w) if w.page != *page || (w.y_min - y).abs() > TOL_PT => bad.push(format!(
                    "{name}: `{text}` at {:.1} pt on page {}, Chrome {y:.1} pt on page {page}",
                    w.y_min, w.page
                )),
                _ => {}
            }
        }
    }
    assert!(
        bad.is_empty(),
        "differences from Chrome m153:\n{}",
        bad.join("\n")
    );
}

#[test]
fn b11_two_top_aligned_boxes_keep_their_text_level() {
    // Guard (F183i, a non-goal of F183c): `vertical-align: top` is not implemented; such a box stays aligned by its last
    // line, as it is painted today. Two of them, and one of them next to a baseline box, keep the first line of their
    // text level, which is also where Chrome has it for these shapes (it puts the tops level, and the text is at the
    // top). Holds before and after F183c.
    let css_box = "display:inline-block;border:3px solid #f0f;width:100px";
    let body = format!(
        "<p>X <span style=\"{css_box};vertical-align:top;height:60px\">TOPA</span> <span style=\"{css_box};vertical-align:top;height:40px\">TOPB</span> Y</p><p>AFTER</p>"
    );
    let (pdf, _) = render(&body);
    let words = printed_words(&pdf);
    assert!(
        (y_of(&words, "TOPA") - y_of(&words, "TOPB")).abs() <= 0.3,
        "TOPA at {:.1} pt, TOPB at {:.1} pt",
        y_of(&words, "TOPA"),
        y_of(&words, "TOPB")
    );
    let body = format!(
        "<p>X <span style=\"{css_box};vertical-align:top;height:60px\">TOPA</span> <span style=\"{css_box};height:40px\">BASEB</span> Y</p><p>AFTER</p>"
    );
    let (pdf, _) = render(&body);
    let words = printed_words(&pdf);
    assert!(
        (y_of(&words, "TOPA") - y_of(&words, "BASEB")).abs() <= 0.3,
        "TOPA at {:.1} pt, BASEB at {:.1} pt",
        y_of(&words, "TOPA"),
        y_of(&words, "BASEB")
    );
}

#[test]
fn b12_margins_on_the_box() {
    // `margin-top` moves the box and the line's text down by the margin; `margin-bottom` adds to the line below the box;
    // the margin of the last child inside the box is part of the box and not of the baseline.
    check(B12);
}

#[test]
fn b13_a_float_or_absolutely_positioned_last_child_is_skipped() {
    check(B13);
}

#[test]
fn fixed_boxes_with_text_at_the_top_sit_on_the_text_baseline() {
    // Boxes of 60, 112, 300 and 600 px with one line of text at the top, and the other fixed shapes of the sweep: line
    // height 1 and 30 px, a 30 px font, padding under the text, the box alone in its paragraph.
    check(FIXED);
}

#[test]
fn boxes_in_a_grid_item_and_a_table_cell() {
    // The measuring pass of a grid item or a cell and the final pass must agree on the box (plan section 6, Taffy's
    // cache); flex items are F217's.
    check(CTX);
}

#[test]
fn a_600px_box_moved_to_page_two_by_f213d_is_printed_whole() {
    // Was `page_bottom_warning.rs` "F183c, a known residual loss": the 600 px box (a short block child) that does not fit
    // the 120 px of room moves to page 2 whole; ours painted it hanging 90.9 pt below the edge of page 2, so the net
    // warned. Chrome m153: BOXTOP at 44.6 pt and AFTER at 495.3 pt, both on page 2; no warning.
    let body = "<div style=\"height:889px\"></div><p><span style=\"display:inline-block;width:150px;height:600px;border:1px solid #000\"><span style=\"display:block\">BOXTOP</span></span></p><p>AFTER</p>";
    let (pdf, warnings) = render(body);
    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    let words = printed_words(&pdf);
    for (text, chrome) in [("BOXTOP", 44.6_f32), ("AFTER", 495.3)] {
        let w = words
            .iter()
            .find(|w| w.text == text)
            .unwrap_or_else(|| panic!("`{text}` is not printed"));
        assert_eq!(
            w.page, 2,
            "`{text}` is on page {}, Chrome has it on page 2",
            w.page
        );
        assert!(
            (w.y_min - chrome).abs() <= TOL_PT,
            "`{text}` at {:.1} pt, Chrome {chrome:.1} pt",
            w.y_min
        );
    }
}

#[test]
fn fixture_48_paid_stamp_and_qr_sit_where_chrome_puts_them() {
    // Fixture 48: a grid header with the 140 x 60 "ZAPLACENO" stamp under the title, 15 item rows and a payment
    // sentence with a 112 px QR box, ending 10 pt above the content bottom. Chrome m153 (the committed reference):
    // ZAPLACENO 100.1 pt, the sentence "Zaplaťte" 675.3 pt level with the caption "QR" of the box (676.8 pt),
    // "Děkujeme" 778.0 pt, one page. Ours before the fix: 109.6, 746.3 (the box 71 pt low, over the thank-you line and
    // 30 pt into the bottom margin), 747.8, 776.3, no warning.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let html =
        std::fs::read_to_string(root.join("fixtures/invoices/48-paid-stamp-header/index.html"))
            .unwrap();
    INIT.call_once(|| {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    let _guard = RENDER.lock().unwrap_or_else(|e| e.into_inner());
    LOG.lock().unwrap().clear();
    let mut builder = Renderer::builder();
    // Every Inter face, in a fixed order (as `scripts/render-fixtures.sh` registers them).
    for face in ["Bold", "ExtraBold", "Medium", "Regular", "SemiBold"] {
        builder = builder
            .font_file(root.join(format!("fixtures/fonts/Inter-{face}.ttf")))
            .unwrap();
    }
    let pdf = builder
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .unwrap()
        .strict_fonts(true)
        .build()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .expect("render");
    let warnings = std::mem::take(&mut *LOG.lock().unwrap());
    drop(_guard);
    let words = printed_words(&pdf);
    let mut bad = Vec::new();
    if words.iter().any(|w| w.page != 1) {
        bad.push("a word is not on page 1".to_string());
    }
    let at = |text: &str, n: usize| {
        words
            .iter()
            .filter(|w| w.text == text)
            .nth(n)
            .map(|w| w.y_min)
    };
    for (text, n, chrome) in [
        ("ZAPLACENO", 0, 100.1_f32),
        ("Zaplaťte", 0, 675.3),
        ("QR", 0, 675.3),
        ("QR", 1, 676.8),
        ("Děkujeme", 0, 778.0),
    ] {
        match at(text, n) {
            None => bad.push(format!("`{text}` #{n} is not printed")),
            Some(y) if (y - chrome).abs() > TOL_PT => {
                bad.push(format!("`{text}` #{n} at {y:.1} pt, Chrome {chrome:.1} pt"))
            }
            _ => {}
        }
    }
    if !warnings.is_empty() {
        bad.push(format!("warnings: {warnings:?}"));
    }
    assert!(
        bad.is_empty(),
        "differences from Chrome m153:\n{}",
        bad.join("\n")
    );
}
