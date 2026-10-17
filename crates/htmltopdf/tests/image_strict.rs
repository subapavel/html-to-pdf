//! `strict_images` (F32a): an `<img>` that cannot be drawn is reported instead of silently missing.
//!
//! Nothing is ever fetched, so an `<img src>` that names no bundled image is a logo that will not be
//! there; a bundled image that does not decode is not drawn either. Default: one `log::warn!` each, the
//! render succeeds. Strict: `Error::Images`, listing every problem in document order.

use htmltopdf::{Error, ImageProblem, ImageProblemKind, RenderOptions, Renderer};

/// 1 x 1 red PNG.
const DOT: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn renderer(strict: bool) -> Renderer {
    Renderer::builder()
        .strict_images(strict)
        .image("ok.png", DOT.to_vec())
        // The file ends inside the pixel data.
        .image("bad.png", DOT[..DOT.len() / 2].to_vec())
        .image("garbage.png", b"not an image at all".to_vec())
        // Decodes leniently: embedded although damaged, so not a problem.
        .image("cut.jpg", JPEG[..JPEG.len() * 3 / 4].to_vec())
        .build()
}

fn render(r: &Renderer, body: &str) -> htmltopdf::Result<Vec<u8>> {
    r.render(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        &RenderOptions::default(),
    )
}

fn problem(src: &str, kind: ImageProblemKind) -> ImageProblem {
    ImageProblem {
        src: src.into(),
        kind,
    }
}

/// A 64 x 64 JPEG.
const JPEG: &[u8] = include_bytes!("data/gradient64.jpg");

const BROKEN: &str = r#"
    <img src="ok.png">
    <img src="nope.png">
    <img src="./ok.png">
    <img src="">
    <img src="bad.png">
    <img src='x&amp;y.png'>
    <img src=garbage.png>
    <img src>"#;

#[test]
fn strict_images_rejects_unregistered_empty_and_undecodable_src() {
    let err = render(&renderer(true), BROKEN).expect_err("strict mode refuses the document");
    let Error::Images(found) = err else {
        panic!("expected Error::Images, got {err}");
    };
    // Document order; `ok.png` and `./ok.png` are fine; the entity in `x&amp;y.png` is decoded.
    let kinds: Vec<_> = found
        .iter()
        .map(|p| {
            (
                p.src.as_str(),
                matches!(p.kind, ImageProblemKind::Unregistered),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("nope.png", true),
            ("", true),
            ("bad.png", false),
            ("x&y.png", true),
            ("garbage.png", false),
            ("", true),
        ],
        "{found:?}"
    );
    assert_eq!(
        found[0],
        problem("nope.png", ImageProblemKind::Unregistered)
    );
    let ImageProblemKind::Undecodable(why) = &found[2].kind else {
        unreachable!()
    };
    assert!(!why.is_empty(), "the decoder's reason is reported");
    // The message names the images.
    let text = Error::Images(found).to_string();
    assert!(
        text.contains("nope.png") && text.contains("bad.png"),
        "{text}"
    );
}

#[test]
fn strict_images_ignores_srcless_commented_and_unreferenced_images() {
    let body = r#"
        <img alt="no src at all">
        <!-- <img src="nope.png"> -->
        <textarea><img src="nope.png"></textarea>
        <script>var s = '<img src="nope.png">';</script>
        <title><img src="nope.png"></title>
        <style>/* <img src="nope.png"> */</style>
        <template><img src="nope.png"><img src="bad.png"></template>
        <noscript><img src="nope.png"></noscript>
        <img src="ok.png">
        <p>bad.png and garbage.png are registered but nothing refers to them</p>"#;
    render(&renderer(true), body).expect("nothing in the document is a broken image");
}

#[test]
fn non_strict_renders_of_the_same_inputs_succeed() {
    for body in [BROKEN, "<img src=nope.png>"] {
        let pdf = render(&renderer(false), body).expect("warns, does not fail");
        assert!(pdf.starts_with(b"%PDF-"));
    }
}

#[test]
fn strict_images_is_independent_of_strict_fonts() {
    // Strict fonts alone does not turn a broken image into an error, and vice versa.
    let r = Renderer::builder().strict_fonts(true).build();
    render(&r, "<img src=nope.png>").expect("strict_fonts does not check images");
}

#[test]
fn a_damaged_jpeg_that_decodes_is_not_a_strict_problem() {
    // Truncated JPEGs are drawn as Chrome draws them; strict mode reports only what is refused.
    render(&renderer(true), r#"<img src="cut.jpg"><img src="ok.png">"#)
        .expect("a truncated JPEG is embedded, not an error");
}
