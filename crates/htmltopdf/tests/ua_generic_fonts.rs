//! The user-agent stylesheet's `monospace` (`pre`, `code`, `kbd`, `samp`, `tt`, `xmp`, `listing`) must resolve to a
//! bundled face (F70). The UA sheet is compiled into the engine and never passes through the F3b keyword rewrite, so
//! with host fonts off the generic resolved to nothing and the element's text vanished without a message.

use htmltopdf::{GenericFamily, RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

const DOC: &str = "<!doctype html><html><body>\
    <p>Prose in the default family.</p>\
    <pre>UAPRE1\n  second line</pre>\
    <code>UACODE2</code> <kbd>UAKBD3</kbd> <samp>UASAMP4</samp> <tt>UATT5</tt>\
    <xmp>UAXMP6</xmp>\
    <listing>UALISTING7</listing>\
    <p>Částka <code>UACODE8</code> Kč</p>\
    </body></html>";

const TOKENS: [&str; 8] = [
    "UAPRE1",
    "UACODE2",
    "UAKBD3",
    "UASAMP4",
    "UATT5",
    "UAXMP6",
    "UALISTING7",
    "UACODE8",
];

fn assert_all_present(pdf: &[u8]) {
    let text = pdftotext(pdf);
    for t in TOKENS {
        assert!(text.contains(t), "`{t}` was dropped; PDF text: {text:?}");
    }
    assert!(
        text.contains("Částka UACODE8 Kč"),
        "the text around the inline <code> was damaged: {text:?}"
    );
}

fn has_jetbrains(pdf: &[u8]) -> bool {
    pdf.windows(13).any(|w| w == b"JetBrainsMono")
}

#[test]
fn ua_monospace_elements_render_in_the_builtin_mono_face() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = r.render(DOC, &RenderOptions::default()).expect("render");
    assert_all_present(&pdf);
    assert!(
        has_jetbrains(&pdf),
        "UA monospace must use the built-in mono face"
    );
}

#[test]
fn ua_monospace_follows_the_generic_family_override() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .generic_family(GenericFamily::Monospace, "Inter")
        .strict_fonts(true)
        .build();
    let pdf = r.render(DOC, &RenderOptions::default()).expect("render");
    assert_all_present(&pdf);
    assert!(
        !has_jetbrains(&pdf),
        "monospace was mapped to Inter, the built-in face must not be used"
    );
}

#[test]
fn ua_monospace_renders_with_no_caller_font_at_all() {
    let r = Renderer::builder().strict_fonts(true).build();
    let pdf = r.render(DOC, &RenderOptions::default()).expect("render");
    assert_all_present(&pdf);
    assert!(has_jetbrains(&pdf));
}

/// Blitz's UA sheet spells its monospace `-moz-fixed` (Stylo's alias of the keyword), and an author may too: the
/// engine draws it with the mapped face, so the static check must not call it unresolved.
#[test]
fn moz_fixed_is_monospace_for_the_static_check_too() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = r
        .render(
            r#"<p style="font-family:-moz-fixed">UAMOZ13</p><p style="font-family:'Nope', -MOZ-FIXED">UAMOZ14</p>"#,
            &RenderOptions::default(),
        )
        .expect("`-moz-fixed` is a generic family");
    let text = pdftotext(&pdf);
    assert!(
        text.contains("UAMOZ13") && text.contains("UAMOZ14"),
        "{text:?}"
    );
    assert!(has_jetbrains(&pdf));
}

/// A6, a probe: controls inherit the document family and have no family of their own in the UA sheet, so a
/// button label and the options of a `<select>` are drawn. The text of `<textarea>` and `<input>` is not
/// painted by the engine at all, with any font (F76), so it is not asserted here.
#[test]
fn button_and_select_text_is_drawn() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = r
        .render(
            "<button>UABUTTON9</button> <select><option>UASELECT11</option></select>",
            &RenderOptions::default(),
        )
        .expect("render");
    let text = pdftotext(&pdf);
    for t in ["UABUTTON9", "UASELECT11"] {
        assert!(text.contains(t), "`{t}` was dropped; PDF text: {text:?}");
    }
}

/// The PDF's text via poppler's `pdftotext`, whitespace collapsed. Fails loudly when it is not
/// installed: a silent skip would hide dropped text.
fn pdftotext(pdf: &[u8]) -> String {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-ua-{}-{n}.pdf", std::process::id()));
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
