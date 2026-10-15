//! `@property` initial values in bundled CSS are applied (F2): the engine ignores registrations, so
//! `htmltopdf` emulates them. Asserted on rasterised pixels, with pure sRGB colours so the check does not
//! depend on colour-space conversion.

use htmltopdf::{RenderOptions, Renderer};

/// Renders `body` with `css` bundled and counts (red, blue) pixels on the first page.
fn red_blue(css: &str, body: &str, tag: &str) -> (usize, usize) {
    let pdf = Renderer::builder()
        .css(css)
        .build()
        .render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("render");
    let ppm = rasterise(&pdf, tag);
    let mut red = 0;
    let mut blue = 0;
    for px in ppm.chunks_exact(3) {
        let (r, g, b) = (px[0], px[1], px[2]);
        if r > 200 && g < 60 && b < 60 {
            red += 1;
        }
        if b > 200 && r < 60 && g < 60 {
            blue += 1;
        }
    }
    (red, blue)
}

/// First page as raw RGB bytes (binary PPM body) via poppler's `pdftoppm`. Fails loudly when it is
/// not installed: a silent skip would hide missing borders.
fn rasterise(pdf: &[u8], tag: &str) -> Vec<u8> {
    let dir = std::env::temp_dir();
    let base = dir.join(format!("htmltopdf-prop-{}-{tag}", std::process::id()));
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
    // Header: "P6\n<w> <h>\n<max>\n" — three whitespace-terminated fields after the magic.
    let mut fields = 0;
    let mut i = 2;
    while fields < 3 {
        while ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        while !ppm[i].is_ascii_whitespace() {
            i += 1;
        }
        fields += 1;
    }
    assert!(ppm.starts_with(b"P6"), "not a binary PPM");
    ppm[i + 1..].to_vec()
}

const BORDER: &str = r#"
    @property --bs { syntax: "*"; inherits: false; initial-value: solid; }
    @layer utilities {
        .b { border-style: var(--bs); border-width: 6px; border-color: rgb(255 0 0); width: 200px; height: 40px; }
    }
"#;

#[test]
fn registered_initial_value_applies() {
    // Tailwind's `.border`: no border at all without the default.
    let (red, _) = red_blue(BORDER, r#"<div class="b"></div>"#, "border");
    assert!(red > 50, "border missing: {red} red pixels");
}

#[test]
fn author_declarations_in_a_layer_or_unlayered_beat_the_default() {
    let layered = format!("{BORDER} @layer utilities {{ .none {{ --bs: none; }} }}");
    let (red, _) = red_blue(&layered, r#"<div class="b none"></div>"#, "layered");
    assert_eq!(red, 0, "layered `--bs: none` lost to the default");

    let unlayered = format!("{BORDER} .none {{ --bs: none; }}");
    let (red, _) = red_blue(&unlayered, r#"<div class="b none"></div>"#, "unlayered");
    assert_eq!(red, 0, "unlayered `--bs: none` lost to the default");
}

#[test]
fn non_inherited_property_without_initial_value_does_not_leak_to_children() {
    // Tailwind's `--tw-leading`: a child must use its own fallback, not the parent's value.
    let css = r#"
        @property --c { syntax: "*"; inherits: false; }
        .p { --c: rgb(0 0 255); }
        .k { background: var(--c, rgb(255 0 0)); width: 200px; height: 40px; }
    "#;
    let (red, blue) = red_blue(css, r#"<div class="p"><div class="k"></div></div>"#, "leak");
    assert_eq!(blue, 0, "child inherited a non-inherited property");
    assert!(red > 50, "child did not use its fallback: {red} red pixels");
}

#[test]
fn length_zero_substitutes_as_px() {
    // Tailwind's `--tw-ring-offset-width`: `calc(6px + 0)` would be invalid and drop the ring.
    let css = r#"
        @property --w { syntax: "<length>"; inherits: false; initial-value: 0; }
        .r { box-shadow: 0 0 0 calc(6px + var(--w)) rgb(0 0 255); width: 200px; height: 40px; margin: 20px; }
    "#;
    let (_, blue) = red_blue(css, r#"<div class="r"></div>"#, "length");
    assert!(blue > 50, "ring missing: {blue} blue pixels");
}

#[test]
fn inherited_initial_value_reaches_descendants() {
    let css = r#"
        @property --ink { syntax: "<color>"; inherits: true; initial-value: rgb(255 0 0); }
        .k { background: var(--ink); width: 200px; height: 40px; }
    "#;
    let (red, _) = red_blue(css, r#"<div><div class="k"></div></div>"#, "inherited");
    assert!(
        red > 50,
        "inherited initial value missing: {red} red pixels"
    );
}

#[test]
fn invalid_rules_are_not_applied() {
    // No `inherits` descriptor → the rule is invalid and registers nothing.
    let css = r#"
        @property --bs { syntax: "*"; initial-value: solid; }
        .b { border-style: var(--bs); border-width: 6px; border-color: rgb(255 0 0); width: 200px; height: 40px; }
    "#;
    let (red, _) = red_blue(css, r#"<div class="b"></div>"#, "invalid");
    assert_eq!(red, 0);
}
