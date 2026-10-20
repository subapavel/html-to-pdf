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
    let root = root();
    // Fixtures are under our control, so every font-family they mention must resolve: strict.
    let mut b = Renderer::builder()
        .strict_fonts(true)
        // Every `<img>` of a fixture must name a bundled image that decodes (F32a).
        .strict_images(true)
        .css_file(root.join("fixtures/tailwind/tailwind.css"))
        .expect("tailwind.css");
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
    let r = renderer();
    for dir in fixtures() {
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
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

#[test]
fn every_invoice_fixture_passes_the_font_check() {
    // Guards against false positives of the static `font-family` check on the real Tailwind output.
    let r = renderer();
    for dir in fixtures() {
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        if let Err(e) = r.render(&html, &RenderOptions::default()) {
            panic!("{}: {e}", dir.display());
        }
    }
}
