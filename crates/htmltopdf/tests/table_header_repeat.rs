//! A table that spans pages repeats its `<thead>` at the top of every continuation page (F12), as Chrome,
//! Prince and WeasyPrint do (CSS 2.1 §17.2). Asserted on each page's text (`pdftotext -f N -l N`).

use htmltopdf::{RenderOptions, Renderer};

fn render(body: &str) -> Vec<u8> {
    Renderer::builder()
        .css("table { border-collapse: collapse } td, th { padding: 4px }")
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render")
}

fn long_table(rows: usize) -> String {
    let rows: String = (1..=rows)
        .map(|i| format!("<tr><td>Řádek {i}</td><td>{i},00</td></tr>"))
        .collect();
    format!(
        "<table><thead><tr><th>Hlavička</th><th>Cena</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

#[test]
fn thead_repeats_on_every_page() {
    let pdf = render(&long_table(120));
    let pages = page_count(&pdf);
    assert!(
        pages >= 3,
        "expected a multi-page table, got {pages} page(s)"
    );
    for page in 1..=pages {
        let t = pdftotext(&pdf, page);
        assert!(
            t.contains("Hlavička Cena"),
            "page {page} has no header row: {t:?}"
        );
    }
}

#[test]
fn every_row_is_printed_once() {
    let pdf = render(&long_table(120));
    let all: String = (1..=page_count(&pdf))
        .map(|p| pdftotext(&pdf, p) + " ")
        .collect();
    for i in 1..=120 {
        let needle = format!("Řádek {i} ");
        assert_eq!(
            all.matches(&needle).count(),
            1,
            "row {i} printed a wrong number of times"
        );
    }
    assert_eq!(all.matches("Hlavička").count(), page_count(&pdf));
}

fn page_count(pdf: &[u8]) -> usize {
    let path = temp(pdf);
    let out = std::process::Command::new("pdfinfo")
        .arg(&path)
        .output()
        .expect("pdfinfo (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo printed no page count")
}

/// One page's text via poppler's `pdftotext`, whitespace collapsed.
fn pdftotext(pdf: &[u8], page: usize) -> String {
    let path = temp(pdf);
    let out = std::process::Command::new("pdftotext")
        .args(["-f", &page.to_string(), "-l", &page.to_string()])
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-thead-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}
