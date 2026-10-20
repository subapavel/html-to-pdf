//! The library must never write to fd 1 (fulgur's `CLAUDE.md`: library crates do not touch stdout). The CLI's
//! stdout stays empty so a caller can pipe it, and shaping on rayon workers (F99a) adds no "Initialising
//! FontContext" line to it.
//!
//! `htmltopdf render` has no `-o -` mode (it writes a file called `-`), so this checks the stream itself rather
//! than comparing `-o -` output with a file.
//!
//! F100: html5ever parse errors and the other `println!`s in Blitz go to `log`. The second and third tests render a
//! document that used to print them: no doctype (html5ever reports "Unexpected token"), margin boxes (fulgur parses
//! each margin box from a wrapper without a doctype) and an inline `<svg>` that usvg rejects.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn render_writes_nothing_to_stdout() {
    let root = root();
    let font = root.join("fixtures/fonts/Inter-Regular.ttf");
    assert!(font.is_file(), "run scripts/fetch-fonts.sh");
    let dir = std::env::temp_dir().join(format!("htmltopdf-stdout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let html = dir.join("in.html");
    let body: String = (0..300)
        .map(|i| format!("<p>Odstavec číslo {i} <b>tučně</b> a <code>kód {i}</code></p>"))
        .collect();
    std::fs::write(&html, format!("<!doctype html><body>{body}</body>")).unwrap();
    let out = dir.join("out.pdf");

    let o = Command::new(env!("CARGO_BIN_EXE_htmltopdf"))
        .args(["render"])
        .arg(&html)
        .arg("-o")
        .arg(&out)
        .arg("--font")
        .arg(&font)
        .output()
        .expect("run htmltopdf");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(
        o.stdout.is_empty(),
        "stdout must stay empty, got: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF-"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Three pages, no doctype, a counter in the page footer and an SVG with a zero size (usvg: `InvalidSize`).
fn write_no_doctype_document(dir: &Path) -> PathBuf {
    let paras: String = (0..140)
        .map(|i| format!("<p>Odstavec číslo {i} s textem, který zabírá místo na stránce.</p>"))
        .collect();
    let html = format!(
        "<html><head><style>@page {{ @bottom-right {{ content: \"Strana \" counter(page) \" / \" counter(pages) }} }}\
         </style></head><body>{paras}<svg width=\"0\" height=\"0\"><rect width=\"1\" height=\"1\"/></svg></body></html>"
    );
    let path = dir.join("in.html");
    std::fs::write(&path, html).unwrap();
    path
}

fn render_no_doctype(tag: &str, rust_log: Option<&str>) -> (std::process::Output, Vec<u8>) {
    let root = root();
    let font = root.join("fixtures/fonts/Inter-Regular.ttf");
    assert!(font.is_file(), "run scripts/fetch-fonts.sh");
    let dir = std::env::temp_dir().join(format!("htmltopdf-stdout-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let html = write_no_doctype_document(&dir);
    let out = dir.join("out.pdf");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_htmltopdf"));
    cmd.arg("render")
        .arg(&html)
        .arg("-o")
        .arg(&out)
        .arg("--font")
        .arg(&font);
    cmd.env_remove("RUST_LOG");
    if let Some(l) = rust_log {
        cmd.env("RUST_LOG", l);
    }
    let o = cmd.output().expect("run htmltopdf");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let pdf = std::fs::read(&out).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (o, pdf)
}

#[test]
fn no_doctype_and_margin_boxes_write_nothing_to_stdout() {
    let (o, pdf) = render_no_doctype("quiet", None);
    assert!(
        o.stdout.is_empty(),
        "stdout must stay empty, got: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(pdf.starts_with(b"%PDF-"));
    // Parse errors are diagnostics (HTML LS 13.2.2), not warnings: not on stderr at the default level.
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!err.contains("Unexpected token"), "{err}");
    // The SVG is dropped, which is content the author wrote: that one is a warning.
    assert!(err.contains("SVG"), "{err}");
}

#[test]
fn parse_errors_are_visible_at_debug() {
    let (o, _) = render_no_doctype("debug", Some("blitz_html=debug"));
    assert!(
        o.stdout.is_empty(),
        "stdout must stay empty, got: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("Unexpected token"), "{err}");
}
