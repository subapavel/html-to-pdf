//! CSS that ends inside an open block renders instead of crashing (F10). fulgur 0.40.0's `::marker`
//! pre-pass panics on a block that is opened at the very end of a stylesheet (`a{`). CSS Syntax 3 §5.4
//! closes every open block at the end of the input, so `htmltopdf` appends the missing closers itself;
//! the rules inside keep applying, as in a browser.

use htmltopdf::{RenderOptions, Renderer};

fn render(css: &str, html: &str) -> htmltopdf::Result<Vec<u8>> {
    Renderer::builder()
        .css(css)
        .build()
        .render(html, &RenderOptions::default())
}

#[test]
fn a_block_opened_at_the_end_of_a_style_element_renders() {
    for html in [
        "<style>a{</style><p>x</p>",
        "<style>p{color:red</style><p>x</p>",
        "<style>@media print{p{</style><p>x</p>",
        "<style>a::marker{color:red</style><p>x</p>",
        "<style>a{content:'x</style><p>x</p>",
        "<style>a{/* x</style><p>x</p>",
        // An escaped brace is not a block for CSS, but fulgur's pre-pass still sees a final `{`.
        "<style>a\\{</style><p>x</p>",
    ] {
        let pdf = render("", html).unwrap_or_else(|e| panic!("{html}: {e}"));
        assert!(pdf.starts_with(b"%PDF-"), "{html}");
    }
}

#[test]
fn a_block_opened_at_the_end_of_a_bundled_stylesheet_renders() {
    render("p{color:red}\na{", "<p>x</p>").expect("render");
}

#[test]
fn rules_in_an_unclosed_block_still_apply() {
    // Browsers apply `div { background: red` as if it were closed; so must we.
    let pdf = render(
        "body{margin:0} div{width:200px;height:200px}",
        "<style>div{background:rgb(255 0 0)</style><div></div>",
    )
    .expect("render");
    assert!(
        pdf.windows(7).any(|w| w == b"1 0 0 rg") || has_red_pixel(&pdf),
        "the unclosed rule was dropped"
    );
}

fn has_red_pixel(pdf: &[u8]) -> bool {
    let path = std::env::temp_dir().join(format!("htmltopdf-malformed-{}", std::process::id()));
    std::fs::write(path.with_extension("pdf"), pdf).unwrap();
    let out = std::process::Command::new("pdftoppm")
        .args(["-r", "24", "-f", "1", "-l", "1", "-singlefile"])
        .arg(path.with_extension("pdf"))
        .arg(&path)
        .output()
        .expect("pdftoppm (poppler-utils) is required for this test and was not found in PATH");
    assert!(out.status.success());
    let ppm = std::fs::read(path.with_extension("ppm")).unwrap();
    let _ = std::fs::remove_file(path.with_extension("pdf"));
    let _ = std::fs::remove_file(path.with_extension("ppm"));
    ppm.chunks_exact(3)
        .any(|p| p[0] > 240 && p[1] < 15 && p[2] < 15)
}

#[test]
fn an_engine_panic_is_an_error_and_the_renderer_stays_usable() {
    // fulgur's pre-pass also scans a `<style>` inside an HTML comment, which is not CSS and which
    // `htmltopdf` therefore does not touch: the engine still panics there. That must come back as an
    // error, not unwind into the caller, and the same renderer must keep working.
    let r = Renderer::builder().build();
    let res = r.render(
        "<!-- <style>a{</style> --><p>x</p>",
        &RenderOptions::default(),
    );
    assert!(
        matches!(res, Err(htmltopdf::Error::EnginePanic(_))),
        "expected EnginePanic, got {:?}",
        res.map(|p| p.len())
    );
    r.render("<p>after</p>", &RenderOptions::default())
        .expect("renderer unusable after a caught panic");
}
