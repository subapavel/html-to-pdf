//! Gradients with a colour-interpolation method render (F8). The engine's Stylo build does not parse
//! `in <colorspace>` in gradients (pref `layout.css.gradient-color-interpolation-method.enabled` is off), so
//! the whole `background-image` was dropped — every Tailwind v4 `bg-gradient-*`, whose
//! `--tw-gradient-position` is `to right in oklab`. Each case is a red-to-red gradient written with an
//! interpolation method and must paint red; `color-mix(in …)`, where `in` is required, must keep working.

use htmltopdf::{RenderOptions, Renderer};

const RED: &str = "rgb(255 0 0)";

fn cases() -> Vec<(&'static str, String)> {
    vec![
        ("plain", format!("linear-gradient(to right, {RED}, {RED})")),
        (
            "direction then method",
            format!("linear-gradient(to right in oklab, {RED}, {RED})"),
        ),
        (
            "method then direction",
            format!("linear-gradient(in srgb to right, {RED}, {RED})"),
        ),
        (
            "method only",
            format!("linear-gradient(in oklab, {RED}, {RED})"),
        ),
        (
            "hue method",
            format!("linear-gradient(90deg in hsl longer hue, {RED}, {RED})"),
        ),
        (
            "repeating",
            format!("repeating-linear-gradient(in oklab, {RED} 0 10px, {RED} 10px 20px)"),
        ),
        (
            "radial",
            format!("radial-gradient(circle in oklab, {RED}, {RED})"),
        ),
    ]
}

#[test]
fn gradients_with_an_interpolation_method_paint_in_bundled_css() {
    for (name, gradient) in cases() {
        let px = centre_pixel(
            &format!(".g {{ background-image: {gradient} }}"),
            r#"<div class="g"></div>"#,
        );
        assert!(is_red(px), "{name} `{gradient}` painted as {px:?}");
    }
}

#[test]
fn gradients_with_an_interpolation_method_paint_in_the_document() {
    for (name, gradient) in cases() {
        let px = centre_pixel(
            "",
            &format!(
                r#"<style>.g {{ background-image: {gradient} }}</style><div class="g"></div>"#
            ),
        );
        assert!(is_red(px), "<style> {name} `{gradient}` painted as {px:?}");
        let px = centre_pixel(
            "",
            &format!(r#"<div style="background-image: {gradient}"></div>"#),
        );
        assert!(
            is_red(px),
            "style=\"\" {name} `{gradient}` painted as {px:?}"
        );
    }
}

#[test]
fn tailwind_gradient_through_custom_properties_paints() {
    // The shape Tailwind v4 emits for `bg-gradient-to-r from-… to-…`.
    let css = format!(
        ".g {{ --tw-gradient-position: to right in oklab; --tw-gradient-from: {RED}; --tw-gradient-to: {RED};
               --tw-gradient-stops: var(--tw-gradient-position), var(--tw-gradient-from), var(--tw-gradient-to);
               background-image: linear-gradient(var(--tw-gradient-stops)); }}"
    );
    let px = centre_pixel(&css, r#"<div class="g"></div>"#);
    assert!(is_red(px), "Tailwind-style gradient painted as {px:?}");
}

#[test]
fn color_mix_keeps_its_interpolation_space() {
    let css = format!(".g {{ background-color: color-mix(in srgb, {RED} 50%, {RED}) }}");
    let px = centre_pixel(&css, r#"<div class="g"></div>"#);
    assert!(is_red(px), "color-mix painted as {px:?}");
}

fn is_red([r, g, b]: [u8; 3]) -> bool {
    r >= 251 && g <= 4 && b <= 4
}

/// Renders a 200×200 px `div` (first child of `body`) with `css` bundled and returns the pixel at its
/// centre: 48 dpi, the default 15 mm page margin is 28 device pixels.
fn centre_pixel(css: &str, body: &str) -> [u8; 3] {
    let pdf = Renderer::builder()
        .css(format!(
            "body {{ margin: 0 }} div {{ width: 200px; height: 200px }} {css}"
        ))
        .build()
        .render(
            &format!("<!doctype html><html><head></head><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    let (w, rgb) = rasterise(&pdf);
    let i = (78 * w + 78) * 3;
    [rgb[i], rgb[i + 1], rgb[i + 2]]
}

/// First page as (width, raw RGB bytes) via poppler's `pdftoppm`. Fails loudly when it is not
/// installed: a silent skip would hide missing gradients.
fn rasterise(pdf: &[u8]) -> (usize, Vec<u8>) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("htmltopdf-gradient-{}-{n}", std::process::id()));
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
