//! `colspan` values a browser accepts render instead of crashing (F55). Blitz passed the attribute to Taffy as a grid
//! span unchecked: `colspan="0"` panicked in Taffy's track sizing, `colspan="65535"` and a row whose spans add up to
//! more than Taffy's `i16` grid lines panicked in grid placement, so the whole document failed.
//!
//! HTML §4.9.11 parses `colspan` with the rules for parsing non-negative integers (leading whitespace skipped, the
//! leading digits taken, so `" 2"` and `"2.5"` are 2) and clamps it: 0 or an unparsable value is 1, above 1000 is
//! 1000. Column positions are asserted against the next row, so the numbers do not depend on font metrics.

use htmltopdf::{RenderOptions, Renderer};

const CSS: &str = "body { margin: 0 } td { padding: 4px } table { border-spacing: 0 }";

fn render(body: &str) -> htmltopdf::Result<Vec<u8>> {
    Renderer::builder().css(CSS).build().render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

/// `(xMin, text)` of every word, in extraction order.
fn words(pdf: &[u8]) -> Vec<(f32, String)> {
    let path = std::env::temp_dir().join(format!(
        "htmltopdf-spans-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("<word ")?;
            let (attrs, word) = rest.split_once('>')?;
            let x = attrs
                .split_whitespace()
                .find_map(|a| a.strip_prefix("xMin=\""))?
                .trim_end_matches('"')
                .parse()
                .ok()?;
            Some((x, word.strip_suffix("</word>")?.to_string()))
        })
        .collect()
}

static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn x_of(found: &[(f32, String)], text: &str) -> f32 {
    found
        .iter()
        .find(|(_, w)| w == text)
        .unwrap_or_else(|| panic!("{text} is not printed; printed words: {found:?}"))
        .0
}

/// A first row `<td colspan=…>A</td><td>B</td>` over a three-cell row `C D E`: `B` starts where the cell under it
/// in the second row starts, which tells how many columns `A` spans.
fn spanned_columns(colspan: &str) -> usize {
    let pdf = render(&format!(
        "<table><tr><td colspan=\"{colspan}\">A</td><td>B</td></tr>\
         <tr><td>Ccc</td><td>Ddd</td><td>Eee</td></tr></table>"
    ))
    .unwrap_or_else(|e| panic!("colspan={colspan:?}: {e}"));
    let found = words(&pdf);
    let b = x_of(&found, "B");
    let d = x_of(&found, "Ddd");
    let e = x_of(&found, "Eee");
    if (b - d).abs() < 0.5 {
        1
    } else if (b - e).abs() < 0.5 {
        2
    } else {
        panic!("colspan={colspan:?}: B at {b} is under neither D ({d}) nor E ({e})")
    }
}

#[test]
fn a_zero_or_unparsable_colspan_spans_one_column() {
    for value in ["0", "00", "-1", "abc", "", "+1"] {
        assert_eq!(spanned_columns(value), 1, "colspan={value:?}");
    }
}

#[test]
fn colspan_takes_the_leading_digits_after_whitespace() {
    for value in ["2", " 2", "\t2", "2.5", "2px", "02"] {
        assert_eq!(spanned_columns(value), 2, "colspan={value:?}");
    }
}

#[test]
fn a_huge_colspan_renders() {
    for value in ["1000", "1001", "65535", "65536", "99999999999999999999"] {
        let pdf = render(&format!(
            "<table><tr><td colspan=\"{value}\">A</td><td>B</td></tr><tr><td>C</td></tr></table>"
        ))
        .unwrap_or_else(|e| panic!("colspan={value:?}: {e}"));
        let found = words(&pdf);
        for w in ["A", "B", "C"] {
            x_of(&found, w);
        }
    }
}

#[test]
fn a_row_whose_spans_exceed_the_grid_line_limit_renders_every_cell() {
    // 40 × 1000 columns is beyond Taffy's i16 grid lines; every cell must still be printed.
    let cells: String = (0..40)
        .map(|i| format!("<td colspan=1000>X{i}</td>"))
        .collect();
    let pdf = render(&format!(
        "<table><tr>{cells}</tr><tr><td>Last</td></tr></table>"
    ))
    .expect("render");
    let found = words(&pdf);
    x_of(&found, "Last");
    x_of(&found, "X0");
}

#[test]
fn a_zero_or_huge_rowspan_renders() {
    for value in ["0", "65535", "70000", "-3"] {
        let pdf = render(&format!(
            "<table><tr><td rowspan=\"{value}\">A</td><td>B</td></tr><tr><td>C</td></tr></table>"
        ))
        .unwrap_or_else(|e| panic!("rowspan={value:?}: {e}"));
        let found = words(&pdf);
        for w in ["A", "B", "C"] {
            x_of(&found, w);
        }
    }
}
