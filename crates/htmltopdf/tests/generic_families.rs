//! Generic `font-family` keywords map to bundled faces (F3b). With host fonts off the engine resolves
//! `monospace`, `serif`, `system-ui` … to nothing and drops the text; `htmltopdf` rewrites each keyword
//! to the bundled family it is mapped to: `monospace`/`ui-monospace` → the built-in mono face, every
//! other generic → the first bundled font, unless overridden with `RendererBuilder::generic_family`.

use htmltopdf::{GenericFamily, RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

const DOC: &str = r#"<!doctype html><html><head><style>
    .ui { font-family: ui-monospace }
    .sys { font: 12px/1.5 system-ui }
</style></head><body>
    <p style="font-family: monospace">GENMONO1</p>
    <p style='font-family: "Nope", sans-serif'>GENSANS2</p>
    <p style="font: italic 12px serif">GENSERIF3</p>
    <p class="ui">GENUI4</p>
    <p class="sys">GENSYS5</p>
</body></html>"#;

const MARKERS: [&str; 5] = ["GENMONO1", "GENSANS2", "GENSERIF3", "GENUI4", "GENSYS5"];

#[test]
fn generic_only_lists_render_and_pass_the_strict_check() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = r.render(DOC, &RenderOptions::default()).expect("render");
    let text = pdftotext(&pdf);
    for m in MARKERS {
        assert!(text.contains(m), "`{m}` was dropped; PDF text: {text:?}");
    }
    // `monospace` went to the built-in face, the others to Inter.
    assert!(pdf.windows(13).any(|w| w == b"JetBrainsMono"));
    assert!(pdf.windows(5).any(|w| w == b"Inter"));
}

#[test]
fn generics_in_bundled_css_and_custom_properties_are_mapped() {
    let r = Renderer::builder()
        .css(":root { --mono: ui-monospace, monospace } .m { font-family: var(--mono) } .s { font-family: serif }")
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = r
        .render(
            r#"<p class="m">BUNDLEDMONO</p><p class="s">BUNDLEDSERIF</p>"#,
            &RenderOptions::default(),
        )
        .expect("render");
    let text = pdftotext(&pdf);
    assert!(text.contains("BUNDLEDMONO"), "{text:?}");
    assert!(text.contains("BUNDLEDSERIF"), "{text:?}");
}

#[test]
fn generic_family_override_wins() {
    // Map `monospace` to Inter: the built-in face is then not used at all.
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .generic_family(GenericFamily::Monospace, "Inter")
        .strict_fonts(true)
        .build();
    let pdf = r
        .render(
            r#"<p style="font-family: monospace">OVERRIDE</p>"#,
            &RenderOptions::default(),
        )
        .expect("render");
    assert!(pdftotext(&pdf).contains("OVERRIDE"));
    assert!(!pdf.windows(13).any(|w| w == b"JetBrainsMono"));
}

#[test]
fn quoted_keyword_is_a_family_name_not_a_generic() {
    // `'monospace'` names a family called monospace (CSS Fonts 4 §4.2), which is not bundled.
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let res = r.render(
        r#"<p style="font-family: 'monospace'">x</p>"#,
        &RenderOptions::default(),
    );
    assert!(
        matches!(res, Err(htmltopdf::Error::UnresolvedFontFamilies(_))),
        "a quoted keyword must not be mapped"
    );
}

/// The PDF's text via poppler's `pdftotext`, whitespace collapsed. Fails loudly when it is not
/// installed: a silent skip would hide dropped text.
fn pdftotext(pdf: &[u8]) -> String {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("htmltopdf-generic-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext")
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
