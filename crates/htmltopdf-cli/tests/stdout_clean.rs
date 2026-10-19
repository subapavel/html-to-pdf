//! The library must never write to fd 1 (fulgur's `CLAUDE.md`: library crates do not touch stdout). The CLI's
//! stdout stays empty so a caller can pipe it, and shaping on rayon workers (F99a) adds no "Initialising
//! FontContext" line to it.
//!
//! `htmltopdf render` has no `-o -` mode (it writes a file called `-`), so this checks the stream itself rather
//! than comparing `-o -` output with a file.

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
