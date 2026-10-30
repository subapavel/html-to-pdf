//! Renders every fixture under fixtures/invoices with the bundled Tailwind CSS and Inter fonts.
//! Asserts: output is a PDF, render is byte-deterministic, text survives into the PDF.
//! Pixel comparison against Chrome is scripts/compare.py (needs poppler), not a cargo test.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use htmltopdf::{RenderOptions, Renderer};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn renderer() -> Renderer {
    renderer_with(true)
}

/// A fixture is rendered with the compiled Tailwind sheet only when its `index.html` links it (like
/// `scripts/render-fixtures.sh`; 27 is plain CSS and must not get the preflight).
fn uses_tailwind(html: &str) -> bool {
    html.contains("tailwind.css")
}

fn renderer_with(tailwind: bool) -> Renderer {
    let root = root();
    // Fixtures are under our control, so every font-family they mention must resolve: strict.
    let mut b = Renderer::builder()
        .strict_fonts(true)
        // Every `<img>` of a fixture must name a bundled image that decodes (F32a).
        .strict_images(true);
    if tailwind {
        b = b
            .css_file(root.join("fixtures/tailwind/tailwind.css"))
            .expect("tailwind.css");
    }
    let fonts = std::fs::read_dir(root.join("fixtures/fonts"))
        .expect("fixtures/fonts — run scripts/fetch-fonts.sh")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
        .collect::<Vec<_>>();
    // read_dir order is not deterministic; registration order must be.
    let mut fonts = fonts;
    fonts.sort();
    assert!(
        !fonts.is_empty(),
        "no fonts in fixtures/fonts — run scripts/fetch-fonts.sh"
    );
    for f in fonts {
        b = b.font_file(f).unwrap();
    }
    // The images next to each fixture's `index.html`, by file name (as `scripts/render-fixtures.sh`
    // registers them).
    let mut images = BTreeMap::new();
    for dir in fixtures() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "png" || x == "jpg"))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&f).unwrap();
            if let Some(other) = images.insert(name.clone(), bytes.clone()) {
                assert_eq!(other, bytes, "two fixtures have a different `{name}`");
            }
        }
    }
    for (name, bytes) in images {
        b = b.image(name, bytes);
    }
    b.build()
}

fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/invoices"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("index.html").is_file())
        .collect();
    v.sort();
    assert!(v.len() >= 5, "expected the five invoice fixtures");
    v
}

#[test]
fn every_fixture_renders_to_a_pdf_deterministically() {
    let (with, without) = (renderer_with(true), renderer_with(false));
    for dir in fixtures() {
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let r = if uses_tailwind(&html) {
            &with
        } else {
            &without
        };
        let opts = RenderOptions {
            lang: Some("cs".into()),
            ..Default::default()
        };
        let a = r
            .render(&html, &opts)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert!(a.starts_with(b"%PDF-"), "{}: not a PDF", dir.display());
        let b = r.render(&html, &opts).unwrap();
        assert_eq!(a, b, "{}: output is not deterministic", dir.display());
    }
}

/// Extracts the text of a PDF with poppler's `pdftotext` and collapses all whitespace runs to a
/// single space. Fails loudly when `pdftotext` is not installed: a silent skip would hide data loss.
fn pdf_text(pdf: &[u8], tag: &str) -> String {
    let path =
        std::env::temp_dir().join(format!("htmltopdf-test-{}-{tag}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "pdftotext failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn mono_identifiers_survive_into_pdf_text() {
    // Guards F3: text set in `font-mono` (`"JetBrains Mono", ui-monospace, monospace`) must not vanish
    // when the named face is not supplied by the caller.
    let expected: &[(&str, &[&str])] = &[
        (
            "01-basic",
            &[
                "123456789/0300",
                "CZ65 0300 0000 0001 2345 6789",
                "20260001",
                "IBAN: CZ65 0300 0000 0001 2345 6789",
            ],
        ),
        ("02-long", &["20260002"]),
        ("03-logo-qr", &["20260003", "48 400,00 CZK"]),
        ("04-bilingual-vat", &["CEKOCZPP", "20260004"]),
    ];
    let r = renderer();
    for (name, needles) in expected {
        let html = std::fs::read_to_string(
            root()
                .join("fixtures/invoices")
                .join(name)
                .join("index.html"),
        )
        .unwrap();
        let opts = RenderOptions {
            lang: Some("cs".into()),
            ..Default::default()
        };
        let pdf = r
            .render(&html, &opts)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = pdf_text(&pdf, name);
        for needle in *needles {
            assert!(
                text.contains(needle),
                "{name}: `{needle}` missing from PDF text"
            );
        }
    }
}

/// F111: fixture 19 has text and `<br>` directly in `<body>`; its tail (payment details, terms) was lost on main.
#[test]
fn plain_text_invoice_prints_every_line_on_both_pages() {
    let name = "19-plain-text-invoice";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, name);
    for needle in [
        "FAKTURA – DAŇOVÝ DOKLAD č. 2026-0019",
        "CELKEM K ÚHRADĚ: 83 178,00 Kč",
        "IBAN: CZ65 0300 0000 0001 2345 6789",
        "Variabilní symbol: 20260019",
        "15. Doba archivace dokladu je 10 let podle zákona o DPH.",
        "www.novavlna.example",
    ] {
        assert!(
            text.contains(needle),
            "{name}: `{needle}` missing from PDF text"
        );
    }
    let pages = std::process::Command::new("pdfinfo")
        .arg({
            let path =
                std::env::temp_dir().join(format!("htmltopdf-test-{}-19p.pdf", std::process::id()));
            std::fs::write(&path, &pdf).unwrap();
            path
        })
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let info = String::from_utf8_lossy(&pages.stdout);
    assert!(info.contains("Pages:           2"), "{info}");
}

/// F36: fixture 20 writes `<tfoot>` before `<tbody>` (HTML 4.01 order); the totals belong below the last item.
#[test]
fn tfoot_first_invoice_prints_the_totals_below_the_last_item() {
    let name = "20-tfoot-first";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-20.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    let xml = String::from_utf8_lossy(&out.stdout);
    let y_min = |word: &str| -> f32 {
        let line = xml
            .lines()
            .find(|l| l.contains(&format!(">{word}</word>")))
            .unwrap_or_else(|| panic!("`{word}` not found in the PDF text"));
        let start = line.find("yMin=\"").unwrap() + "yMin=\"".len();
        line[start..start + line[start..].find('"').unwrap()]
            .parse()
            .unwrap()
    };
    let (last_item, total) = (y_min("Cestovné"), y_min("úhradě"));
    assert!(
        total > last_item,
        "`Celkem k úhradě` (y {total}) is not below the last item (y {last_item})"
    );
    assert!(
        y_min("Základ") > last_item,
        "`Základ daně` is not below the last item"
    );
}

/// F150 / F153: fixture 21 puts its address blocks, the rows of its item grid, its notes and some list items inside
/// `display: contents` wrappers. On main's binary the items grid, the notes, the first terms and the supplier's lines
/// were missing from the PDF (7.4 % against Chrome); every word must be printed, in one page.
#[test]
fn display_contents_invoice_prints_everything_the_wrappers_hold() {
    let name = "21-display-contents";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "21");
    for needle in [
        "DODAVATEL",
        "IČO 12345678",
        "ODBĚRATEL",
        "IČO 87654321",
        "Servisní paušál",
        "Cestovné",
        "46 340,00 Kč",
        "POZNÁMKY",
        "Splatnost faktury je 2. 11. 2026",
        "Fakturu jsme vystavili",
        "Cena zahrnuje DPH",
        "Reklamace uplatněte",
        "Dodávka služeb",
        "20260021",
    ] {
        assert!(
            text.contains(needle),
            "{name}: `{needle}` missing from the PDF text: {text}"
        );
    }
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-21p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           1"),
        "{name}: one page like Chrome"
    );
}

/// F163: fixture 23 has `::before` / `::after` boxes on a `<table>` and on a `display: table` box. Main's binary panics
/// on it (`EnginePanic`); with only the crash fixed the note under the items and the label and closing cell of the VAT
/// recap are missing. Every generated text must be printed, in one page.
#[test]
fn table_pseudo_invoice_prints_the_generated_content() {
    let name = "23-table-pseudo";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "23");
    for needle in [
        "Cestovné",
        "Celkem bez DPH",
        "Ceny jsou uvedeny v korunách českých bez DPH",
        "Rekapitulace DPH",
        "Základ 21 %",
        "9 731,40 Kč",
        "56 071,40 Kč",
        "Uhrazeno",
        "20260023",
    ] {
        assert!(
            text.contains(needle),
            "{name}: `{needle}` missing from the PDF text: {text}"
        );
    }
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-23p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           1"),
        "{name}: one page like Chrome"
    );
}

/// F154: fixture 22 writes its labels as `<b>Splatnost: </b>2. 11. 2026`. On main's binary the space at the inner edge of
/// each element was deleted and the label was glued to its value (`Splatnost:4. 11. 2026`; 4.0 % against Chrome).
#[test]
fn inline_spacing_invoice_keeps_its_label_spaces() {
    let name = "22-inline-spacing";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "22");
    for needle in [
        "Datum vystavení: 21. 10. 2026",
        "Splatnost: 4. 11. 2026",
        "Variabilní symbol: 20260022 (shodný",
        "IČO: 12345678",
        "DIČ: CZ87654321",
        "Číslo objednávky: Příkladná integrace – smluvní servis (dle smlouvy o dílo)",
        "Celkem bez DPH: 35 500,00 Kč",
        "Celkem k úhradě: 42 955,00 Kč",
        "portálu zákazníka nebo ji najdete",
        "Poznámka: dodání do 14 dnů od potvrzení objednávky. Platba převodem na účet níže.",
        "IBAN: CZ65",
    ] {
        assert!(
            text.contains(needle),
            "{name}: `{needle}` missing from the PDF text: {text}"
        );
    }
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-22p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           1"),
        "{name}: one page like Chrome"
    );
}

#[test]
fn every_invoice_fixture_passes_the_font_check() {
    // Guards against false positives of the static `font-family` check on the real Tailwind output.
    let (with, without) = (renderer_with(true), renderer_with(false));
    for dir in fixtures() {
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let r = if uses_tailwind(&html) {
            &with
        } else {
            &without
        };
        if let Err(e) = r.render(&html, &RenderOptions::default()) {
            panic!("{}: {e}", dir.display());
        }
    }
}

/// F161 / F165: fixture 24's terms are a two-column container with two `column-span: all` headings, over two pages,
/// followed by a `display: contents` wrapper. On main's binary (`182ffb73`) 86 of its 948 words were lost (the clauses
/// after the first spanner that crossed the page break), silently.
#[test]
fn columns_span_invoice_prints_every_clause() {
    let name = "24-columns-span";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "24");
    let chrome = pdf_text(
        &std::fs::read(dir.join("reference/chrome.pdf")).unwrap(),
        "24c",
    );
    // Every clause heading and both spanners, in source order.
    let mut needles: Vec<String> = Vec::new();
    for (i, part) in html.split("<h3").skip(1).enumerate() {
        let heading = part
            .split_once('>')
            .unwrap()
            .1
            .split_once("</h3>")
            .unwrap()
            .0;
        if i == 6 {
            needles.push("Reklamační řád".into());
        }
        if i == 23 {
            needles.push("Ochrana osobních údajů".into());
        }
        needles.push(heading.to_string());
        // The last two words of the clause's paragraph.
        let para = part.split("<p").nth(1).unwrap().split_once('>').unwrap().1;
        let para = para.split_once("</p>").unwrap().0;
        let words: Vec<&str> = para.split_whitespace().collect();
        let tail = words[words.len().saturating_sub(2)..].join(" ");
        assert!(
            text.contains(&tail),
            "{name}: the end of clause `{heading}` (`{tail}`) is missing: {text}"
        );
    }
    assert_eq!(needles.len(), 29, "{name}: 27 clauses and 2 spanners");
    let mut at = 0;
    for n in &needles {
        let pos = text[at..]
            .find(n.as_str())
            .unwrap_or_else(|| panic!("{name}: `{n}` missing or out of order: {text}"));
        at += pos + n.len();
    }
    assert!(
        text.contains("Fakturu vystavila Jana Nováková"),
        "{name}: the signature in the contents wrapper is missing"
    );
    assert_eq!(
        text.split(' ').count(),
        chrome.split(' ').count(),
        "{name}: as many words as Chrome's reference"
    );
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-24p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           2"),
        "{name}: two pages like Chrome"
    );
}

/// F174: fixture 25's payment schedule (56 rows, a `thead`) sits in a two-column terms block and crosses a column and
/// a page break. Chrome repeats the header at the top of each of its 3 column pieces.
#[test]
fn terms_table_columns_invoice_prints_every_instalment() {
    let name = "25-terms-table-columns";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "25");
    for i in 1..=56 {
        let needle = format!("Splátka {i} ");
        assert_eq!(
            text.matches(&needle).count(),
            1,
            "{name}: `{needle}` must be printed once: {text}"
        );
    }
    assert_eq!(
        text.matches("Částka").count(),
        3,
        "{name}: the header once per column piece, as in Chrome"
    );
    for part in html.split("<p class=\"text-gray-600\">").skip(1) {
        let para = part.split_once("</p>").unwrap().0;
        let words: Vec<&str> = para.split_whitespace().collect();
        let tail = words[words.len().saturating_sub(2)..].join(" ");
        assert!(text.contains(&tail), "{name}: `{tail}` missing: {text}");
    }
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-25p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           2"),
        "{name}: two pages like Chrome"
    );
}

/// F198: fixture 30's terms are one bordered card, taller than a page in a single column, in a two-column block. It is
/// cut over both columns and both pages like Chrome's; every clause is printed once, in order, and the signature
/// below the card comes after the last clause.
#[test]
fn terms_card_columns_invoice_prints_every_clause() {
    let name = "30-terms-card-columns";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "30");
    let chrome = pdf_text(
        &std::fs::read(dir.join("reference/chrome.pdf")).unwrap(),
        "30c",
    );
    let mut needles: Vec<String> = Vec::new();
    for part in html.split("<h3").skip(1) {
        let heading = part
            .split_once('>')
            .unwrap()
            .1
            .split_once("</h3>")
            .unwrap()
            .0;
        needles.push(heading.to_string());
        // The last two words of the clause's paragraph.
        let para = part.split("<p").nth(1).unwrap().split_once('>').unwrap().1;
        let para = para.split_once("</p>").unwrap().0;
        let words: Vec<&str> = para.split_whitespace().collect();
        let tail = words[words.len().saturating_sub(2)..].join(" ");
        assert_eq!(
            text.matches(&tail).count(),
            chrome.matches(&tail).count(),
            "{name}: the end of clause `{heading}` (`{tail}`) must be printed as often as Chrome prints it: {text}"
        );
        assert!(
            text.contains(&tail),
            "{name}: the end of clause `{heading}` (`{tail}`) is missing: {text}"
        );
    }
    assert_eq!(needles.len(), 20, "{name}: 20 clauses");
    needles.push("Fakturu vystavila Jana Nováková".into());
    let mut at = 0;
    for n in &needles {
        let pos = text[at..]
            .find(n.as_str())
            .unwrap_or_else(|| panic!("{name}: `{n}` missing or out of order: {text}"));
        at += pos + n.len();
    }
    assert_eq!(
        text.split(' ').count(),
        chrome.split(' ').count(),
        "{name}: as many words as Chrome's reference"
    );
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-30p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           2"),
        "{name}: two pages like Chrome"
    );
}

/// `s` without its tags (`<svg>…</svg>`, `<img …>`): the text a reader sees.
fn without_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// F198b: fixture 31's terms are one `overflow-hidden rounded-lg border` card with a grey header strip, taller than a
/// page in a single column, in a two-column block, with inline icons (`<svg>`) and an inline logo (`<img>`) in the
/// clauses. The card is cut over both columns and both pages like Chrome's: every clause is printed once, in order, and
/// the signature below the card comes after the last clause.
#[test]
fn terms_clip_card_columns_invoice_prints_every_clause() {
    let name = "31-terms-clip-card-columns";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = pdf_text(&pdf, "31");
    let chrome = pdf_text(
        &std::fs::read(dir.join("reference/chrome.pdf")).unwrap(),
        "31c",
    );
    let mut needles: Vec<String> = vec!["OBCHODNÍ PODMÍNKY".into()];
    // The first `<h3` is the card's header strip; the 20 after it are the clauses.
    for part in html.split("<h3").skip(2) {
        let heading = without_tags(
            part.split_once('>')
                .unwrap()
                .1
                .split_once("</h3>")
                .unwrap()
                .0,
        );
        needles.push(heading.clone());
        // The last two words of the clause's paragraph.
        let para = part.split("<p ").nth(1).unwrap().split_once('>').unwrap().1;
        let para = without_tags(para.split_once("</p>").unwrap().0);
        let words: Vec<&str> = para.split_whitespace().collect();
        let tail = words[words.len().saturating_sub(2)..].join(" ");
        assert_eq!(
            text.matches(&tail).count(),
            chrome.matches(&tail).count(),
            "{name}: the end of clause `{heading}` (`{tail}`) must be printed as often as Chrome prints it: {text}"
        );
        assert!(
            text.contains(&tail),
            "{name}: the end of clause `{heading}` (`{tail}`) is missing: {text}"
        );
    }
    assert_eq!(needles.len(), 21, "{name}: the strip and 20 clauses");
    needles.push("Fakturu vystavila Jana Nováková".into());
    let mut at = 0;
    for n in &needles {
        let pos = text[at..]
            .find(n.as_str())
            .unwrap_or_else(|| panic!("{name}: `{n}` missing or out of order: {text}"));
        at += pos + n.len();
    }
    assert_eq!(
        text.split(' ').count(),
        chrome.split(' ').count(),
        "{name}: as many words as Chrome's reference"
    );
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-31p.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let info = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let _ = std::fs::remove_file(&path);
    assert!(
        String::from_utf8_lossy(&info.stdout).contains("Pages:           2"),
        "{name}: two pages like Chrome"
    );
}

/// F66f: fixture 33 has `pre-line` / `<br>`-terminated address blocks and a note card ending in `<br>`. A forced break at
/// the end of a block makes no line of its own, so what follows sits where Chrome puts it. On main's binary the items
/// table was 15 pt too low and "PLATEBNÍ" 30 pt (556.6 pt, Chrome 526.8 pt). Chrome's y (m141, `reference/chrome.pdf`).
#[test]
fn address_trailing_break_invoice_is_laid_out_like_chrome() {
    let name = "33-address-trailing-break";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-33.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    let xml = String::from_utf8_lossy(&out.stdout);
    assert_eq!(xml.matches("<page ").count(), 1, "one page like Chrome");
    let y_min = |word: &str| -> f32 {
        let line = xml
            .lines()
            .find(|l| l.contains(&format!(">{word}</word>")))
            .unwrap_or_else(|| panic!("`{word}` not found in the PDF text"));
        let start = line.find("yMin=\"").unwrap() + "yMin=\"".len();
        line[start..start + line[start..].find('"').unwrap()]
            .parse()
            .unwrap()
    };
    for (word, chrome) in [("Celkem", 325.9), ("Poznámka", 376.8), ("PLATEBNÍ", 526.8)] {
        let y = y_min(word);
        assert!(
            (y - chrome).abs() <= 1.0,
            "`{word}` at {y:.1} pt, Chrome puts it at {chrome} pt"
        );
    }
}

/// F30f-l: fixture 34's page 2 is a 51-line block of terms (20 px lines, 1020 px) that overflows its page by half a line.
/// Chrome puts 49 lines on page 2 and the last two clauses (49., 50.) and the signature on page 3 (`reference/chrome.pdf`,
/// m141). On main's binary page 2 held all 51 lines, the last at 786-805 pt below the content box (799.4 pt), and only the
/// signature was on page 3.
#[test]
fn terms_page_widow_invoice_leaves_two_clauses_for_the_next_page() {
    let name = "34-terms-page-widow";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-34.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    let xml = String::from_utf8_lossy(&out.stdout);
    assert_eq!(xml.matches("<page ").count(), 3, "three pages like Chrome");
    // (page, yMin, yMax) of each word that is a clause number ("48.") or one of the heading / signature words.
    let mut page = 0;
    let mut found: Vec<(usize, f32, f32, String)> = Vec::new();
    for line in xml.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            let attr = |name: &str| -> f32 {
                let start = attrs.find(name).unwrap() + name.len();
                attrs[start..start + attrs[start..].find('"').unwrap()]
                    .parse()
                    .unwrap()
            };
            found.push((page, attr("yMin=\""), attr("yMax=\""), word.to_string()));
        }
    }
    let find = |word: &str| -> (usize, f32, f32) {
        let hit = found
            .iter()
            .find(|w| w.3 == word)
            .unwrap_or_else(|| panic!("`{word}` not found in the PDF text"));
        (hit.0, hit.1, hit.2)
    };
    assert_eq!(find("Obchodní").0, 2, "the terms start on page 2");
    assert_eq!(find("48.").0, 2, "clause 48 is the last line of page 2");
    assert_eq!(find("49.").0, 3, "clauses 49 and 50 move to page 3");
    assert_eq!(find("50.").0, 3);
    assert_eq!(find("Převzal:").0, 3);
    for (p, _, y_max, w) in &found {
        if *p == 2 {
            assert!(
                *y_max <= 799.4,
                "`{w}` on page 2 ends at {y_max:.1} pt, below the content box (799.4 pt)"
            );
        }
    }
    // Chrome: 49. at 44.5 pt, the signature at 91.8 pt.
    for (word, chrome) in [("49.", 44.5), ("Převzal:", 91.8)] {
        let y = find(word).1;
        assert!(
            (y - chrome).abs() <= 1.5,
            "`{word}` at {y:.1} pt, Chrome puts it at {chrome} pt"
        );
    }
}

/// F66i: fixture 35 has generated content in inline elements nested in other inline elements: " Kč" after each amount
/// (`td > span.price > span.amount`), a status dot (`span.badge > span.dot::before`, a 6 px inline-block), `*` after a date in
/// `<strong>`, Czech quotes around a note in a plain `<span>`, " →" after a link text in `<a>`. On main's binary none of them
/// was printed and "Uhrazeno" stood 8.5 pt too far left (the dot and its margin). Chrome's x and y (m141, `reference/chrome.pdf`), within a point.
#[test]
fn nested_generated_content_invoice_prints_every_generated_string() {
    let name = "35-nested-generated-content";
    let html = std::fs::read_to_string(
        root()
            .join("fixtures/invoices")
            .join(name)
            .join("index.html"),
    )
    .unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-35.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    let xml = String::from_utf8_lossy(&out.stdout);
    assert_eq!(xml.matches("<page ").count(), 1, "one page like Chrome");
    // (word, xMin, yMin) in reading order.
    let attr = |attrs: &str, name: &str| -> f32 {
        let start = attrs.find(name).unwrap() + name.len();
        attrs[start..start + attrs[start..].find('"').unwrap()]
            .parse()
            .unwrap()
    };
    let mut words: Vec<(String, f32, f32)> = Vec::new();
    for line in xml.lines() {
        if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((attrs, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            words.push((
                word.to_string(),
                attr(attrs, "xMin=\""),
                attr(attrs, "yMin=\""),
            ));
        }
    }
    let nth = |word: &str, n: usize| -> (f32, f32) {
        let hit = words
            .iter()
            .filter(|w| w.0 == word)
            .nth(n)
            .unwrap_or_else(|| panic!("`{word}` #{n} not found in the PDF text"));
        (hit.1, hit.2)
    };
    let expected: [(&str, usize, f32, f32); 10] = [
        ("Uhrazeno", 0, 78.9, 189.3),
        ("2026*", 0, 214.0, 207.3),
        ("„převodem", 0, 123.8, 225.3),
        ("dnů“", 0, 277.1, 225.3),
        ("→", 0, 207.2, 243.3),
        ("Kč", 0, 540.9, 307.1),
        ("Kč", 1, 540.9, 334.8),
        ("Kč", 2, 540.9, 362.6),
        ("Kč", 3, 540.9, 390.3),
        ("Kč", 4, 536.4, 412.9),
    ];
    for (word, n, x, y) in expected {
        let (gx, gy) = nth(word, n);
        // The amounts sit in the right-aligned column of an auto-layout table, whose columns are 1.2 pt (CELKEM) to 8 pt
        // (MNOŽSTVÍ) off Chrome's on main too: the currency is checked against its number below, its x within 2 pt.
        let tol_x = if word == "Kč" { 2.0 } else { 1.0 };
        assert!(
            (gx - x).abs() <= tol_x && (gy - y).abs() <= 1.0,
            "`{word}` #{n} at ({gx:.1}, {gy:.1}), Chrome puts it at ({x}, {y})"
        );
    }
    assert_eq!(
        words.iter().filter(|w| w.0 == "Kč").count(),
        5,
        "five amounts"
    );
}

/// F30c: fixture 36 has the terms as one body-level `<p>` below a heading, about 210 pt above the page-1 content bottom. The
/// paragraph (32 lines of 19.5 px) fits a page but not the room, so Chrome starts it below the heading and breaks it between
/// line boxes: ten lines on page 1, the rest and the signature on page 2 (`reference/chrome.pdf`, m141). On main's binary the
/// heading stood alone at the bottom of page 1 and the whole paragraph moved to page 2 ("50." at 468.8 pt, Chrome 278.5 pt).
#[test]
fn terms_after_items_invoice_starts_the_terms_below_their_heading() {
    let name = "36-terms-after-items";
    let dir = root().join("fixtures/invoices").join(name);
    let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
    let pdf = renderer()
        .render(
            &html,
            &RenderOptions {
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let path = std::env::temp_dir().join(format!("htmltopdf-test-{}-36.pdf", std::process::id()));
    std::fs::write(&path, &pdf).unwrap();
    // (page, yMin, word) in reading order.
    let words_of = |pdf: &Path| -> Vec<(usize, f32, String)> {
        let out = std::process::Command::new("pdftotext")
            .arg("-bbox")
            .arg(pdf)
            .arg("-")
            .output()
            .expect(
                "pdftotext (poppler-utils) is required for this test and was not found in PATH",
            );
        let xml = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut page = 0;
        let mut found = Vec::new();
        for line in xml.lines() {
            if line.contains("<page ") {
                page += 1;
            } else if let Some(rest) = line.trim().strip_prefix("<word ")
                && let Some((attrs, word)) = rest.split_once('>')
                && let Some(word) = word.strip_suffix("</word>")
            {
                let start = attrs.find("yMin=\"").unwrap() + 6;
                let y = attrs[start..start + attrs[start..].find('"').unwrap()]
                    .parse()
                    .unwrap();
                found.push((page, y, word.to_string()));
            }
        }
        found
    };
    let ours = words_of(&path);
    let chrome = words_of(&dir.join("reference/chrome.pdf"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        ours.iter().map(|w| w.0).max(),
        Some(2),
        "two pages like Chrome"
    );
    let find = |words: &[(usize, f32, String)], word: &str| -> usize {
        words
            .iter()
            .position(|w| w.2 == word)
            .unwrap_or_else(|| panic!("`{word}` not found in the PDF text"))
    };
    assert_eq!(ours[find(&ours, "Obchodní")].0, 1);
    assert_eq!(
        ours[find(&ours, "1.")].0,
        1,
        "the terms start on page 1, below the heading"
    );
    assert_eq!(ours[find(&ours, "Převzal:")].0, 2);
    // The first word of page 2 is Chrome's, give or take one line (about 14 words).
    let first_on_2 = |words: &[(usize, f32, String)]| words.iter().position(|w| w.0 == 2).unwrap();
    let (o, c) = (first_on_2(&ours), first_on_2(&chrome));
    assert!(
        o.abs_diff(c) <= 14,
        "page 2 starts at word {o} (`{}`), Chrome at word {c} (`{}`)",
        ours[o].2,
        chrome[c].2
    );
    for word in ["50.", "Převzal:"] {
        let (y, cy) = (ours[find(&ours, word)].1, chrome[find(&chrome, word)].1);
        assert!(
            (y - cy).abs() <= 3.0,
            "`{word}` at {y:.1} pt, Chrome puts it at {cy:.1} pt"
        );
    }
}
