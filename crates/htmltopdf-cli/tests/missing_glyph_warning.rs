//! F101 at the command line: a character no bundled font has a glyph for is drawn as `.notdef`; `--strict-fonts`
//! refuses the document (exit 1, stderr names the code point, stdout and the output file stay empty), without the flag
//! it is a warning on stderr and the PDF is written. Plan: docs/plans/2026-11-04-f101-missing-glyph-warning.md.
//!
//! The tests marked `ignore` are red until the fulgur fork reports `.notdef`; run them with `--ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The test G1 document: seven CJK letters, an arrow and an emoji, none in Inter.
const HTML: &str = "<!doctype html><html><body>\
    <p>Odběratel: 株式会社サクラ, Ōsaka</p>\
    <table><tr><td>Cestovné – Trutnov ⇄ Berlin</td></tr></table>\
    <p>Děkujeme za nákup 🙂</p></body></html>";

fn run(strict: bool, tag: &str) -> (std::process::Output, PathBuf) {
    let root = root();
    let dir = std::env::temp_dir().join(format!(
        "htmltopdf-cli-missing-glyph-{}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let html = dir.join("in.html");
    std::fs::write(&html, HTML).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_htmltopdf"));
    cmd.arg("render")
        .arg(&html)
        .arg("-o")
        .arg(dir.join("out.pdf"))
        .arg("--font")
        .arg(root.join("fixtures/fonts/Inter-Regular.ttf"));
    if strict {
        cmd.arg("--strict-fonts");
    }
    (cmd.output().expect("run htmltopdf"), dir)
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn strict_fonts_refuses_a_character_with_no_glyph() {
    let (o, dir) = run(true, "strict");
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(1), "{stderr}");
    assert!(
        o.stdout.is_empty(),
        "stdout must stay empty: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    for want in ["U+21C4", "U+682A", "U+1F642", "Inter"] {
        assert!(stderr.contains(want), "`{want}` is missing from {stderr:?}");
    }
    assert!(
        !dir.join("out.pdf").exists(),
        "a refused render writes no PDF"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "F101: red until the fork reports .notdef"]
fn without_strict_fonts_the_character_is_a_warning_on_stderr() {
    let (o, dir) = run(false, "lenient-warns");
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{stderr}");
    for want in ["U+21C4", "U+682A", "U+1F642", "Inter"] {
        assert!(stderr.contains(want), "`{want}` is missing from {stderr:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Green today and after: the warning never stops a non-strict render.
#[test]
fn without_strict_fonts_the_pdf_is_written() {
    let (o, dir) = run(false, "lenient-writes");
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{stderr}");
    assert!(o.stdout.is_empty());
    assert!(
        std::fs::read(dir.join("out.pdf"))
            .unwrap()
            .starts_with(b"%PDF-")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
