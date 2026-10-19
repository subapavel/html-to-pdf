//! A table cell's explicit width includes its padding under `box-sizing: border-box` (F5). Blitz added the
//! padding again when building the column tracks, so every padded fixed-width Tailwind column (preflight sets
//! `border-box` everywhere) came out too wide, the auto column too narrow, and 02 paginated to 4 pages.
//! A `calc()` width mixing a percentage and a length panicked (F14).

use htmltopdf::{RenderOptions, Renderer};

/// x of the left edge of the second cell's red background, in CSS px from the table's left edge.
fn second_column_start(box_sizing: &str, a: &str) -> f32 {
    first_red_x(
        &format!("td {{ box-sizing: {box_sizing} }} .a {{ width: 100px; {a} }}"),
        r#"<tr><td class="a"></td><td class="b"></td></tr>"#,
        20,
    )
}

/// Renders `rows` in a 400 px table whose cells have 20 px side padding and returns the x of the first red
/// pixel on the line `y` CSS px below the table's top, in CSS px from the table's left edge.
fn first_red_x(css: &str, rows: &str, y: usize) -> f32 {
    let css = format!(
        "@page {{ margin: 0 }} body {{ margin: 0 }} table {{ border-spacing: 0; width: 400px }}
         td {{ padding: 0 20px; height: 40px }} .b {{ background: rgb(255 0 0) }} {css}"
    );
    let pdf = Renderer::builder()
        .css(css)
        .build()
        .render(
            &format!("<!doctype html><html><body><table>{rows}</table></body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    // 96 dpi: one device pixel per CSS px; the document's `@page { margin: 0 }` applies, so the table starts at the
    // page corner (before F6 the renderer's 15 mm margin won over it).
    let (w, rgb) = rasterise(&pdf, 96);
    let red = (0..w)
        .find(|&x| {
            let i = (y * w + x) * 3;
            rgb[i] > 200 && rgb[i + 1] < 60 && rgb[i + 2] < 60
        })
        .expect("no red cell found");
    red as f32
}

#[test]
fn border_box_width_includes_the_padding() {
    let x = second_column_start("border-box", "border: 0 solid white; border-width: 0 5px");
    assert!(
        (x - 100.0).abs() <= 1.0,
        "second column starts at {x} px, want 100"
    );
}

#[test]
fn content_box_width_excludes_the_padding() {
    let x = second_column_start("content-box", "");
    assert!(
        (x - 140.0).abs() <= 1.0,
        "second column starts at {x} px, want 140"
    );
}

#[test]
fn content_box_width_excludes_padding_and_border() {
    let x = second_column_start("content-box", "border: 0 solid white; border-width: 0 5px");
    assert!(
        (x - 150.0).abs() <= 1.0,
        "second column starts at {x} px, want 150"
    );
}

#[test]
fn later_row_content_box_width_includes_padding_and_border() {
    // The first row leaves the column auto; the second row's width sets it.
    let x = first_red_x(
        "td { box-sizing: content-box } .a { width: 100px; border: 0 solid white; border-width: 0 5px }",
        r#"<tr><td></td><td class="b"></td></tr><tr><td class="a"></td><td class="b"></td></tr>"#,
        60,
    );
    assert!(
        (x - 150.0).abs() <= 1.0,
        "second column starts at {x} px, want 150"
    );
}

#[test]
fn border_box_width_never_shrinks_below_padding_and_border() {
    let x = second_column_start("border-box", "width: 10px");
    assert!(
        (x - 40.0).abs() <= 1.0,
        "second column starts at {x} px, want 40 (the padding)"
    );
}

#[test]
fn calc_width_mixing_percentage_and_length_is_auto() {
    // F14: this reached `unreachable!()` in Blitz. Chrome treats such a cell width as `auto`, so three
    // auto columns share the 400 px equally.
    let x = first_red_x(
        "",
        r#"<tr><td style="width: calc(25% + 20px)"></td><td class="b"></td><td></td></tr>"#,
        20,
    );
    assert!(
        (x - 133.0).abs() <= 1.0,
        "second column starts at {x} px, want 133"
    );
}

#[test]
fn later_row_width_applies_to_a_calc_column() {
    let x = first_red_x(
        "td { box-sizing: border-box }",
        r#"<tr><td style="width: calc(25% + 20px)"></td><td class="b"></td></tr>
           <tr><td style="width: 200px"></td><td class="b"></td></tr>"#,
        60,
    );
    assert!(
        (x - 200.0).abs() <= 1.0,
        "second column starts at {x} px, want 200"
    );
}

/// First page as (width, raw RGB bytes) via poppler's `pdftoppm`.
fn rasterise(pdf: &[u8], dpi: u32) -> (usize, Vec<u8>) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("htmltopdf-table-{}-{n}", std::process::id()));
    let pdf_path = base.with_extension("pdf");
    std::fs::write(&pdf_path, pdf).unwrap();
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-f", "1", "-l", "1", "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(out.status.success(), "pdftoppm failed");
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
    assert!(ppm.starts_with(b"P6"), "not a binary PPM");
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(
            std::str::from_utf8(&ppm[start..i])
                .unwrap()
                .parse::<usize>()
                .unwrap(),
        );
    }
    (fields[0], ppm[i + 1..].to_vec())
}
