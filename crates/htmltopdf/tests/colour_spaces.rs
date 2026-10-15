//! Colours given in a non-sRGB notation paint as their sRGB equivalent (F1). Tailwind v4's palette is
//! `oklch()` and its opacity modifiers are `color-mix(in oklab, …)`, so before F1 every Tailwind colour
//! came out as its raw L/C/H numbers read as R/G/B. Each case is sRGB red written another way, painted
//! as a background and as an inline-SVG `currentColor` fill, and must rasterise as red.

use htmltopdf::{RenderOptions, Renderer};

/// sRGB `rgb(255 0 0)` in each notation the engine has to convert.
const RED: &[(&str, &str)] = &[
    ("rgb", "rgb(255 0 0)"),
    ("oklch", "oklch(62.8% 0.2577 29.23)"),
    ("oklab", "oklab(62.8% 0.2249 0.1258)"),
    ("lab", "lab(54.29% 80.82 69.91)"),
    (
        "color-mix",
        "color-mix(in oklab, rgb(255 0 0) 40%, rgb(255 0 0))",
    ),
];

/// Max per-channel distance from pure red. Conversions round-trip through f32 and get clipped to
/// gamut, so allow a couple of levels.
const TOL: u8 = 4;

#[test]
fn background_colours_convert_to_srgb() {
    for (name, colour) in RED {
        let px = centre_pixel(&format!(
            r#"<div style="width:200px;height:200px;background:{colour}"></div>"#
        ));
        assert!(is_red(px), "background {name} `{colour}` painted as {px:?}");
    }
}

#[test]
fn inline_svg_current_colour_converts_to_srgb() {
    for (name, colour) in RED {
        let px = centre_pixel(&format!(
            r#"<svg style="color:{colour}" width="200" height="200" viewBox="0 0 10 10" fill="currentColor"><rect width="10" height="10"/></svg>"#
        ));
        assert!(
            is_red(px),
            "svg currentColor {name} `{colour}` painted as {px:?}"
        );
    }
}

fn is_red([r, g, b]: [u8; 3]) -> bool {
    r >= 255 - TOL && g <= TOL && b <= TOL
}

/// Renders `body` on a margin-less page and returns the pixel at the centre of its 200×200 px box
/// (first page, 48 dpi → the box covers 100×100 device pixels from the top-left corner).
fn centre_pixel(body: &str) -> [u8; 3] {
    let pdf = Renderer::builder()
        .css("@page { margin: 0 } body { margin: 0 }")
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    let (w, rgb) = rasterise(&pdf);
    let i = (50 * w + 50) * 3;
    [rgb[i], rgb[i + 1], rgb[i + 2]]
}

/// First page as (width, raw RGB bytes) via poppler's `pdftoppm`. Fails loudly when it is not
/// installed: a silent skip would hide wrong colours.
fn rasterise(pdf: &[u8]) -> (usize, Vec<u8>) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("htmltopdf-colour-{}-{n}", std::process::id()));
    let pdf_path = base.with_extension("pdf");
    std::fs::write(&pdf_path, pdf).unwrap();
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "48", "-f", "1", "-l", "1", "-singlefile"])
        .arg(&pdf_path)
        .arg(&base)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(
        out.status.success(),
        "pdftoppm failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ppm_path = base.with_extension("ppm");
    let ppm = std::fs::read(&ppm_path).unwrap();
    let _ = std::fs::remove_file(&pdf_path);
    let _ = std::fs::remove_file(&ppm_path);
    // Header: "P6\n<w> <h>\n<max>\n".
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
