//! F178 at the command line: a bundled font the PDF writer cannot embed is refused by `--strict-fonts` (exit 1, the
//! message names the file and the face, stdout stays empty) and left out with a warning on stderr otherwise.
//! Plan: docs/plans/2026-10-27-f178-krilla-rejected-faces.md.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Inter-Bold with its `head` table renamed to `hexd` (directory kept sorted).
fn bold_without_head(root: &Path) -> Vec<u8> {
    let font = std::fs::read(root.join("fixtures/fonts/Inter-Bold.ttf"))
        .expect("run scripts/fetch-fonts.sh");
    let n = u16::from_be_bytes([font[4], font[5]]) as usize;
    let mut records: Vec<[u8; 16]> = (0..n)
        .map(|i| font[12 + 16 * i..28 + 16 * i].try_into().unwrap())
        .collect();
    for r in records.iter_mut().filter(|r| &r[..4] == b"head") {
        r[..4].copy_from_slice(b"hexd");
    }
    records.sort_by(|a, b| a[..4].cmp(&b[..4]));
    let mut out = font.clone();
    for (i, r) in records.iter().enumerate() {
        out[12 + 16 * i..28 + 16 * i].copy_from_slice(r);
    }
    out
}

fn run(strict: bool) -> (std::process::Output, PathBuf, PathBuf) {
    let root = root();
    let dir = std::env::temp_dir().join(format!(
        "htmltopdf-cli-fontrej-{}-{}",
        std::process::id(),
        strict
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("Bad-Bold.ttf");
    std::fs::write(&bad, bold_without_head(&root)).unwrap();
    let html = dir.join("in.html");
    std::fs::write(
        &html,
        "<!doctype html><body><p>Normální</p><p style=\"font-weight:700\">TUČNÝ ŘÁDEK</p></body>",
    )
    .unwrap();
    let out = dir.join("out.pdf");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_htmltopdf"));
    cmd.arg("render")
        .arg(&html)
        .arg("-o")
        .arg(&out)
        .arg("--font")
        .arg(root.join("fixtures/fonts/Inter-Regular.ttf"))
        .arg("--font")
        .arg(&bad);
    if strict {
        cmd.arg("--strict-fonts");
    }
    (cmd.output().expect("run htmltopdf"), bad, dir)
}

#[test]
fn strict_fonts_refuses_a_font_the_pdf_writer_cannot_embed() {
    let (o, bad, dir) = run(true);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(1), "{stderr}");
    assert!(
        o.stdout.is_empty(),
        "stdout must stay empty: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    for want in [&bad.display().to_string(), "face 0 (Inter Bold)", "head"] {
        assert!(stderr.contains(want), "`{want}` is missing from {stderr:?}");
    }
    assert!(
        !dir.join("out.pdf").exists(),
        "a refused render writes no PDF"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_strict_fonts_the_font_is_left_out_with_a_warning() {
    let (o, bad, dir) = run(false);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{stderr}");
    assert!(o.stdout.is_empty());
    for want in [
        &bad.display().to_string(),
        "face 0 (Inter Bold)",
        "left out",
    ] {
        assert!(stderr.contains(want), "`{want}` is missing from {stderr:?}");
    }
    assert!(
        std::fs::read(dir.join("out.pdf"))
            .unwrap()
            .starts_with(b"%PDF-")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
