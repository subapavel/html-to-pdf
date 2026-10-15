//! `font-family` handling: with `strict_fonts(true)` lists nothing can satisfy are an error
//! (the text would silently vanish); by default they only warn.

use htmltopdf::{Error, RenderOptions, Renderer};

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

fn render(r: &Renderer, html: &str) -> htmltopdf::Result<Vec<u8>> {
    r.render(html, &RenderOptions::default())
}

fn unresolved(r: Result<Vec<u8>, Error>) -> String {
    match r {
        Err(Error::UnresolvedFontFamilies(list)) => list
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(" | "),
        Err(e) => panic!("unexpected error: {e}"),
        Ok(_) => panic!("expected UnresolvedFontFamilies, but the render succeeded"),
    }
}

#[test]
fn unknown_family_is_an_error_naming_it() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let msg = unresolved(render(&r, "<p style=\"font-family:'Nope'\">x</p>"));
    assert!(msg.contains("'Nope'"), "{msg}");
    assert!(msg.contains("style attribute"), "{msg}");
}

#[test]
fn a_registered_family_later_in_the_list_renders() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    render(&r, "<p style=\"font-family:'Nope', Inter\">x</p>").expect("render");
}

#[test]
fn font_shorthand_is_checked() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let msg = unresolved(render(&r, "<p style='font: 12px/1.2 \"Nope\"'>x</p>"));
    assert!(msg.contains("Nope"), "{msg}");
}

#[test]
fn style_elements_and_bundled_css_are_checked() {
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let msg = unresolved(render(
        &r,
        "<style>p{font-family:'FromStyle'}</style><p>x</p>",
    ));
    assert!(
        msg.contains("FromStyle") && msg.contains("<style> element"),
        "{msg}"
    );

    let r = Renderer::builder()
        .css("p{font-family:'FromBundle'}")
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let msg = unresolved(render(&r, "<p>x</p>"));
    assert!(
        msg.contains("FromBundle") && msg.contains("bundled stylesheet"),
        "{msg}"
    );
}

#[test]
fn strict_fonts_false_only_warns() {
    let r = Renderer::builder()
        .strict_fonts(false)
        .font_bytes(inter())
        .unwrap()
        .build();
    render(&r, "<p style=\"font-family:'Nope'\">x</p>").expect("render");
}

#[test]
fn builtin_mono_is_always_available_by_name() {
    let r = Renderer::builder().strict_fonts(true).build();
    render(&r, "<p style=\"font-family:'JetBrains Mono'\">x</p>").expect("render");
    render(&r, "<p style=\"font-family:'jetbrains mono'\">x</p>").expect("case-insensitive");
}

#[test]
fn text_without_any_font_family_uses_the_first_bundled_font() {
    // No font-family anywhere: the engine alone would drop the text. The builder's `html` prelude
    // maps it to the first bundled face (Inter here), so the run is laid out and the font embedded.
    let r = Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .build();
    let pdf = render(&r, "<p>Bez písma</p>").expect("render");
    assert!(
        pdf.windows(5).any(|w| w == b"Inter"),
        "Inter was not embedded, text probably dropped"
    );
}

#[test]
fn text_without_any_font_family_and_no_bundled_font_uses_builtin_mono() {
    let r = Renderer::builder().strict_fonts(true).build();
    let pdf = render(&r, "<p>Bez písma</p>").expect("render");
    assert!(
        pdf.windows(13).any(|w| w == b"JetBrainsMono"),
        "built-in mono was not embedded"
    );
}
