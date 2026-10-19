//! Page flags of `htmltopdf render` (F6, `@page` half). With no page flag the document's `@page` decides the page
//! size and margins (css-page-3); a flag the caller passes wins over the document for that property only, like a print
//! dialog choice. `--landscape` alone means A4 landscape, as it always did.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Case {
    dir: PathBuf,
}

impl Case {
    fn new(name: &str, css: &str) -> Case {
        let dir =
            std::env::temp_dir().join(format!("htmltopdf-flags-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("in.html"),
            format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><style>{css} body {{ margin: 0 }} p {{ margin: 0 }}\
                 </style></head><body><p>Ahoj svete</p></body></html>"
            ),
        )
        .unwrap();
        Case { dir }
    }

    /// Renders with `flags`; returns the exit status success and the PDF path.
    fn render(&self, flags: &[&str]) -> (bool, PathBuf, String) {
        let out = self.dir.join("out.pdf");
        let _ = std::fs::remove_file(&out);
        let o = Command::new(env!("CARGO_BIN_EXE_htmltopdf"))
            .arg("render")
            .arg(self.dir.join("in.html"))
            .arg("-o")
            .arg(&out)
            .args(flags)
            .output()
            .expect("run htmltopdf");
        (
            o.status.success(),
            out,
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    }

    fn ok(&self, flags: &[&str]) -> PathBuf {
        let (ok, out, err) = self.render(flags);
        assert!(ok, "htmltopdf {flags:?} failed: {err}");
        out
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn media_box(pdf: &Path) -> (f32, f32) {
    let o = Command::new("pdfinfo")
        .arg("-box")
        .arg(pdf)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test");
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    let n: Vec<f32> = text
        .lines()
        .find_map(|l| l.strip_prefix("MediaBox:"))
        .expect("no MediaBox")
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    (n[2] - n[0], n[3] - n[1])
}

fn first_word_x(pdf: &Path) -> f32 {
    let o = Command::new("pdftotext")
        .arg("-bbox")
        .arg(pdf)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test");
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .find_map(|l| l.trim().strip_prefix("<word xMin=\""))
        .and_then(|r| r.split('"').next())
        .and_then(|v| v.parse().ok())
        .expect("no word in the PDF")
}

fn assert_size(pdf: &Path, want: (f32, f32)) {
    let (w, h) = media_box(pdf);
    assert!(
        (w - want.0).abs() <= 0.1 && (h - want.1).abs() <= 0.1,
        "MediaBox {w} x {h} pt, want {} x {}",
        want.0,
        want.1
    );
}

#[test]
fn no_flag_the_document_decides_the_page() {
    let c = Case::new("a5", "@page { size: A5; margin: 10mm }");
    let pdf = c.ok(&[]);
    assert_size(&pdf, (419.53, 595.28));
    assert!((first_word_x(&pdf) - 28.35).abs() <= 0.1);
}

#[test]
fn no_flag_and_no_page_rule_is_a4_with_15mm() {
    let c = Case::new("plain", "");
    let pdf = c.ok(&[]);
    assert_size(&pdf, (595.28, 841.89));
    assert!((first_word_x(&pdf) - 42.52).abs() <= 0.1);
}

#[test]
fn page_size_flag_beats_the_document() {
    let c = Case::new("size", "@page { size: A5 }");
    assert_size(&c.ok(&["--page-size", "A4"]), (595.28, 841.89));
    assert_size(&c.ok(&["--page-size", "a3"]), (841.89, 1190.55));
    assert_size(&c.ok(&["--page-size", "letter"]), (612.0, 792.0));
    assert_size(&c.ok(&["--page-size", "100x200"]), (283.46, 566.93));
}

#[test]
fn landscape_alone_is_a4_landscape() {
    let c = Case::new("land", "@page { size: A5 }");
    assert_size(&c.ok(&["--landscape"]), (841.89, 595.28));
}

#[test]
fn landscape_turns_the_page_size_flag() {
    let c = Case::new("land2", "");
    assert_size(
        &c.ok(&["--page-size", "A5", "--landscape"]),
        (595.28, 419.53),
    );
}

#[test]
fn margin_flag_beats_the_document() {
    let c = Case::new("margin", "@page { size: A4; margin: 15mm }");
    assert!(first_word_x(&c.ok(&["--margin-mm", "0"])) < 1.0);
    assert!((first_word_x(&c.ok(&["--margin-mm", "25.4"])) - 72.0).abs() <= 0.1);
    // The flag leaves the size to the document.
    assert_size(&c.ok(&["--margin-mm", "0"]), (595.28, 841.89));
}

#[test]
fn a_bad_page_size_is_an_error() {
    let c = Case::new("bad", "");
    let (ok, _, err) = c.render(&["--page-size", "Foo"]);
    assert!(!ok, "--page-size Foo must fail");
    assert!(err.contains("page size"), "{err}");
}
