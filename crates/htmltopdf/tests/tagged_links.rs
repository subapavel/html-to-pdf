//! A link that becomes several boxes must not break a tagged render (F150 review). krilla panicked with
//! "AnnotationIdentifier ... appears twice in the tag tree" when fulgur put the annotation of one link into the `/Link`
//! group of each paragraph the link touched. Reachable on main with `<a href><p>A</p><div>B</div></a>`, and made common
//! by `display: contents` (a wrapper `<a>` around flex items or grid cells), whose children are laid out as boxes of
//! their own.
//!
//! Every annotation needs exactly one parent in the structure tree (ISO 32000-1 §14.7.4.3, PDF/UA-1 7.18.5): each
//! `/Link` annotation with a `/StructParent` has exactly one `/OBJR` pointing at it, that key is in the `/ParentTree`,
//! and `qpdf --check` is clean. The repros:
//!
//! - `<a>` around a `<p>` and a `<div>` (main already panicked);
//! - `<a style="display:contents">` around two spans in a flex container, and in a two-column grid.

use htmltopdf::{RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render_tagged(body: &str) -> Vec<u8> {
    let html = format!("<!doctype html><html><body>{body}</body></html>");
    Renderer::builder()
        .font_bytes(inter())
        .expect("Inter")
        .strict_fonts(true)
        .build()
        .render(
            &html,
            &RenderOptions {
                tagged: true,
                lang: Some("cs".into()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("tagged render of {body:?}: {e}"))
}

fn tmp(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("htmltopdf-tl-{}-{n}-{name}", std::process::id()))
}

/// `qpdf --check` is clean; returns the file in QDF form (objects uncompressed, one key per line).
fn check_and_qdf(pdf: &[u8]) -> String {
    let (input, qdf) = (tmp("in.pdf"), tmp("qdf.pdf"));
    std::fs::write(&input, pdf).unwrap();
    let check = std::process::Command::new("qpdf")
        .arg("--check")
        .arg(&input)
        .output()
        .expect("qpdf is required for this test and was not found in PATH");
    assert!(
        check.status.success(),
        "qpdf --check: {}{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let status = std::process::Command::new("qpdf")
        .args(["--qdf", "--object-streams=disable"])
        .arg(&input)
        .arg(&qdf)
        .status()
        .expect("qpdf");
    assert!(status.success(), "qpdf --qdf failed");
    let text = String::from_utf8_lossy(&std::fs::read(&qdf).unwrap()).into_owned();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&qdf);
    text
}

/// Checks the links of a QDF file; returns the number of `/Link` annotations.
fn assert_each_annotation_has_one_parent(qdf: &str, what: &str) -> usize {
    let lines: Vec<&str> = qdf.lines().map(str::trim).collect();
    // OBJR targets: `/Obj N 0 R` in a dictionary that has `/Type /OBJR` within the next lines.
    let mut objr_targets: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("/Obj ")
            && lines[i + 1..(i + 4).min(lines.len())].contains(&"/Type /OBJR")
        {
            objr_targets.push(rest.split_whitespace().next().unwrap().parse().unwrap());
        }
    }
    // Annotations: objects with `/Subtype /Link`, and their `/StructParent`.
    let mut annots: Vec<(usize, Option<usize>)> = Vec::new();
    let mut current: Option<usize> = None;
    let mut subtype_link = false;
    let mut struct_parent: Option<usize> = None;
    for line in &lines {
        if let Some(num) = line.strip_suffix(" 0 obj")
            && let Ok(n) = num.parse::<usize>()
        {
            current = Some(n);
            subtype_link = false;
            struct_parent = None;
        } else if let Some(v) = line.strip_prefix("/StructParent ") {
            struct_parent = v.parse().ok();
        } else if *line == "/Subtype /Link" {
            subtype_link = true;
        } else if *line == "endobj"
            && let Some(n) = current.take()
            && subtype_link
        {
            annots.push((n, struct_parent));
        }
    }
    assert!(!annots.is_empty(), "{what}: no /Link annotation at all");
    // The keys of the ParentTree.
    let nums_at = lines
        .iter()
        .position(|l| *l == "/Nums [")
        .expect("ParentTree");
    let keys: Vec<usize> = lines[nums_at + 1..]
        .iter()
        .take_while(|l| **l != "]")
        .step_by(2)
        .map(|l| l.parse().unwrap())
        .collect();
    for (obj, parent) in &annots {
        let Some(key) = parent else {
            panic!("{what}: annotation {obj} has no /StructParent");
        };
        assert!(
            keys.contains(key),
            "{what}: key {key} not in the ParentTree"
        );
        let n = objr_targets.iter().filter(|t| *t == obj).count();
        assert_eq!(n, 1, "{what}: annotation {obj} has {n} OBJR, not 1");
    }
    let annot_ids: Vec<usize> = annots.iter().map(|a| a.0).collect();
    assert!(
        objr_targets.iter().all(|t| annot_ids.contains(t)),
        "{what}: an OBJR points at something that is not a link annotation"
    );
    annots.len()
}

#[test]
fn a_link_over_two_blocks_has_one_parent_per_annotation() {
    let pdf = render_tagged(r#"<a href="https://example.cz"><p>A</p><div>B</div></a>"#);
    let qdf = check_and_qdf(&pdf);
    assert_each_annotation_has_one_parent(&qdf, "a around p and div");
}

#[test]
fn a_contents_link_over_flex_items_has_one_parent_per_annotation() {
    let pdf = render_tagged(
        r#"<div style="display:flex"><a href="https://example.cz" style="display:contents"><span>a</span><span>b</span></a></div>"#,
    );
    let qdf = check_and_qdf(&pdf);
    assert_each_annotation_has_one_parent(&qdf, "contents link in flex");
}

#[test]
fn a_contents_link_over_grid_cells_has_one_parent_per_annotation() {
    let pdf = render_tagged(
        r#"<div style="display:grid;grid-template-columns:1fr 1fr"><a href="https://example.cz" style="display:contents"><span>a</span><span>b</span></a></div>"#,
    );
    let qdf = check_and_qdf(&pdf);
    assert_each_annotation_has_one_parent(&qdf, "contents link in grid");
}
