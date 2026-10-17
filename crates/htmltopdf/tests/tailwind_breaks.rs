//! Tailwind break utilities reach the break reader (F87). Tailwind v4 puts every utility in `@layer utilities{...}`, and
//! fulgur's own reader for the properties Stylo does not compute (`break-*`, `column-fill`, `page`) skipped every
//! at-rule block, so `class="break-inside-avoid"` and `class="break-before-page"` did nothing while the same
//! declaration in a plain `<style>` or a `style` attribute worked.
//!
//! Spec: CSS Cascade 5 §6.1 (sort order: element-attached styles beat layers, which beat specificity and order) and §6.4
//! (cascade layers: a later layer beats an earlier one, unlayered styles beat every layer, `@layer a { @layer b }` ranks
//! `b` below the rest of `a`). Media Queries 4 §2.2 and Conditional 3 §5 for `@media print`.
//!
//! Which element shows `break-inside: avoid`: a body-level `<p>` does not. A body-level paragraph that does not fit
//! below the cursor moves to the next page whether or not it is `avoid` (the pre-advance in the fork's line path, F30c),
//! so the property cannot be seen on it. A multi-column container taller than the room under it is the box where the
//! two differ (F30d): `auto` splits it over the pages, `avoid` moves it whole to the next page, and Chrome does the same
//! (probes in the F87 plan §7).
//!
//! `TAILWIND_EXTRA` is Tailwind v4's own shape for utilities the committed `fixtures/tailwind/tailwind.css` does not
//! contain (`break-before-page`, `break-after-page`, `break-inside-auto`, `print:break-before-page`, and the `!` modifier `break-inside-auto!`). It is written inline
//! because rebuilding the sheet needs npm (precedent: `white_space.rs`). Asserted with `pdftotext -bbox`.

use htmltopdf::{RenderOptions, Renderer};

const TAILWIND_EXTRA: &str = r"@layer utilities{.break-before-page{break-before:page}.break-after-page{break-after:page}.break-inside-auto{break-inside:auto}.break-inside-auto\!{break-inside:auto!important}.break-before-auto\!{break-before:auto!important}@media print{.print\:break-before-page{break-before:page}.print\:break-inside-auto\!{break-inside:auto!important}}}";

fn inter() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/Inter-Regular.ttf");
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{} missing — run scripts/fetch-fonts.sh", path.display()))
}

/// `<head>` style shared by all documents: A4 with 15 mm margins (content box 1009.1 px), 20 px lines.
const HEAD: &str = "@page{size:A4;margin:15mm}body{font-family:Inter;line-height:20px;margin:0}";

/// 960 px of nothing: 49 px are left on page 1.
const SPACER: &str = r#"<div style="height:960px"></div>"#;

fn render(head_style: &str, body: &str) -> Vec<u8> {
    render_with_head(&format!("<style>{head_style}</style>"), body)
}

/// `head_html` is placed in `<head>` after the page style, so a test can add `<style media="...">`.
fn render_with_head(head_html: &str, body: &str) -> Vec<u8> {
    let tailwind = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tailwind/tailwind.css");
    Renderer::builder()
        .font_bytes(inter())
        .unwrap()
        .strict_fonts(true)
        .css_file(tailwind)
        .unwrap()
        .css(TAILWIND_EXTRA)
        .build()
        .render(
            &format!(
                "<!doctype html><html><head><style>{HEAD}</style>{head_html}</head>\
                 <body>{body}</body></html>"
            ),
            &RenderOptions::default(),
        )
        .expect("render")
}

/// Every word of the PDF with its 1-based page.
fn printed_words(pdf: &[u8]) -> Vec<(usize, String)> {
    let path = temp(pdf);
    let out = std::process::Command::new("pdftotext")
        .arg("-bbox")
        .arg(&path)
        .arg("-")
        .output()
        .expect("pdftotext (poppler-utils) is required for this test and was not found in PATH");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "pdftotext failed");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut page = 0;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.contains("<page ") {
            page += 1;
        } else if let Some(rest) = line.trim().strip_prefix("<word ")
            && let Some((_, word)) = rest.split_once('>')
            && let Some(word) = word.strip_suffix("</word>")
        {
            found.push((page, word.to_string()));
        }
    }
    found
}

/// The page (1-based) `word` is printed on.
fn page_of(pdf: &[u8], word: &str) -> usize {
    printed_words(pdf)
        .iter()
        .find(|(_, w)| w == word)
        .unwrap_or_else(|| panic!("{word} is not printed"))
        .0
}

/// Words `word00000 … word<n - 1>`.
fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("word{i:05}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A two-column box of 100 words after the spacer: 49 px are left on page 1 and the box is about 80 px tall. `auto`
/// splits it over the two pages (12 words on page 1), `avoid` moves it whole to page 2.
fn box_after_spacer(class: &str, style: &str) -> String {
    format!(
        r#"{SPACER}<div class="{class}" style="columns:2;{style}">{}</div>"#,
        words(100)
    )
}

fn temp(pdf: &[u8]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("htmltopdf-twb-{}-{n}.pdf", std::process::id()));
    std::fs::write(&path, pdf).unwrap();
    path
}

// -- controls: the property itself works --------------------------------------------------------

#[test]
fn control_a_two_column_box_splits_without_avoid_and_moves_whole_with_it() {
    let auto = render("", &box_after_spacer("", ""));
    assert_eq!(page_of(&auto, "word00000"), 1, "auto: splits in place");
    assert_eq!(page_of(&auto, "word00099"), 2);
    let inline = render("", &box_after_spacer("", "break-inside:avoid"));
    assert_eq!(page_of(&inline, "word00000"), 2, "inline avoid: whole");
    assert_eq!(page_of(&inline, "word00099"), 2);
    let sheet = render(".k{break-inside:avoid}", &box_after_spacer("k", ""));
    assert_eq!(page_of(&sheet, "word00000"), 2, "unlayered rule: whole");
}

// -- the finding --------------------------------------------------------------------------------

/// `class="break-inside-avoid"` (the bundled sheet's utility) keeps the box together.
#[test]
fn break_inside_avoid_class_keeps_a_box_together() {
    let pdf = render("", &box_after_spacer("break-inside-avoid", ""));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    assert_eq!(page_of(&pdf, "word00099"), 2);
}

#[test]
fn break_before_page_class_starts_a_page() {
    let pdf = render("", r#"<p>alpha</p><p class="break-before-page">bravo</p>"#);
    assert_eq!(page_of(&pdf, "alpha"), 1);
    assert_eq!(page_of(&pdf, "bravo"), 2);
}

#[test]
fn break_after_page_class_starts_the_next_page() {
    let pdf = render("", r#"<p class="break-after-page">alpha</p><p>bravo</p>"#);
    assert_eq!(page_of(&pdf, "alpha"), 1);
    assert_eq!(page_of(&pdf, "bravo"), 2);
}

/// `print:break-before-page` compiles to `@media print{...}` nested in the layer. The media type reaches the reader
/// as written in the text it receives (the renderer swaps `print` and `screen` for Blitz), and the escaped class name
/// `print\:break-before-page` is the class token `print:break-before-page`.
#[test]
fn print_variant_class_starts_a_page() {
    let pdf = render(
        "",
        r#"<p>alpha</p><p class="print:break-before-page">bravo</p>"#,
    );
    assert_eq!(page_of(&pdf, "alpha"), 1);
    assert_eq!(page_of(&pdf, "bravo"), 2);
}

// -- cascade: Cascade 5 §6.1, §6.4 --------------------------------------------------------------

/// Case A: an unlayered rule of lower specificity beats the layered utility. Green before and after; a descent that
/// folded the layered rule after it (source order alone) would turn it red.
#[test]
fn unlayered_rule_beats_the_utility_layer() {
    let pdf = render(
        "div{break-inside:auto}",
        &box_after_spacer("break-inside-avoid", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

/// The `style` attribute beats every layer.
#[test]
fn inline_style_beats_the_utility_layer() {
    let pdf = render(
        "",
        &box_after_spacer("break-inside-avoid", "break-inside:auto"),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
    let pdf = render(
        "",
        &box_after_spacer("break-inside-avoid", "break-inside:avoid"),
    );
    assert_eq!(page_of(&pdf, "word00000"), 2);
}

/// Case B: a `base` layer declared after the bundled sheet (a `<style>` in the body) is still below `utilities`.
#[test]
fn a_later_source_in_an_earlier_layer_loses_to_the_utility_layer() {
    let body = format!(
        "{SPACER}<style>@layer base{{div{{break-inside:auto}}}}</style>\
         <div class=\"break-inside-avoid\" style=\"columns:2\">{}</div>",
        words(100)
    );
    let pdf = render("", &body);
    assert_eq!(page_of(&pdf, "word00000"), 2);
}

/// Case C: a layer declared after `utilities` beats it.
#[test]
fn a_layer_declared_after_utilities_beats_it() {
    let pdf = render(
        "@layer utilities, overrides;@layer overrides{.t{break-inside:auto}}",
        &box_after_spacer("break-inside-avoid t", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

/// Case D: a nested layer ranks below the rest of its parent.
#[test]
fn a_nested_layer_ranks_below_its_parent() {
    let pdf = render(
        "@layer a{.k{break-inside:auto} @layer b{.k{break-inside:avoid}}}",
        &box_after_spacer("k", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

// -- media: Conditional 3 §5, Media Queries 4 §2 ------------------------------------------------

/// The reader and Stylo read the same `@media` blocks: the author's `screen` rules do not apply to a printed
/// document, the `print` ones do. The renderer swaps the two for Blitz, whose device is a screen.
#[test]
fn media_screen_does_not_break_and_media_print_does() {
    let body = r#"<p>alpha</p><p class="x">bravo</p>"#;
    let screen = render("@media screen{.x{break-before:page}}", body);
    assert_eq!(page_of(&screen, "bravo"), 1, "@media screen is not printed");
    let print = render("@media print{.x{break-before:page}}", body);
    assert_eq!(page_of(&print, "bravo"), 2, "@media print applies");
}

// -- importance (Cascade 5 §6.1, §6.4.4) --------------------------------------------------------

/// Tailwind's `!` modifier compiles to `!important`: `break-inside-auto!` beats the plain utility `break-inside-avoid`
/// although both are in the same layer and the plain one comes later in the class list.
#[test]
fn important_modifier_beats_the_plain_utility() {
    let pdf = render(
        "",
        &box_after_spacer("break-inside-avoid break-inside-auto!", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

#[test]
fn important_modifier_in_a_print_variant_beats_the_plain_utility() {
    let pdf = render(
        "",
        &box_after_spacer("break-inside-avoid print:break-inside-auto!", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

#[test]
fn important_modifier_cancels_a_forced_break() {
    let pdf = render(
        "",
        r#"<p>alpha</p><p class="break-before-page break-before-auto!">bravo</p>"#,
    );
    assert_eq!(page_of(&pdf, "bravo"), 1);
}

/// An important rule beats the `style` attribute's normal declaration, and the attribute's own `!important` beats
/// an important rule.
#[test]
fn important_rule_beats_the_style_attribute_and_loses_to_its_important() {
    let head = ".k{break-inside:avoid!important}";
    let pdf = render(head, &box_after_spacer("k", "break-inside:auto"));
    assert_eq!(page_of(&pdf, "word00000"), 2);
    let pdf = render(head, &box_after_spacer("k", "break-inside:auto!important"));
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

// -- specificity (Cascade 5 §6.1) ---------------------------------------------------------------

/// Inside one layer the more specific selector wins whatever the order: `div.k` beats `.k`.
#[test]
fn specificity_orders_rules_inside_a_layer() {
    let pdf = render(
        "@layer utilities{div.k{break-inside:auto}.k{break-inside:avoid}}",
        &box_after_spacer("k", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

/// The same for unlayered rules, which the reader folded in source order before.
#[test]
fn specificity_orders_unlayered_rules() {
    let pdf = render(
        "div.k{break-inside:auto}.k{break-inside:avoid}",
        &box_after_spacer("k", ""),
    );
    assert_eq!(page_of(&pdf, "word00000"), 1);
}

// -- syntax -------------------------------------------------------------------------------------

/// Whitespace in a dotted layer name makes the at-rule invalid (Cascade 5 §6.4.2).
#[test]
fn whitespace_in_a_layer_name_drops_the_block() {
    let body = r#"<p>alpha</p><p class="x">bravo</p>"#;
    let pdf = render("@layer a .b{.x{break-before:page}}", body);
    assert_eq!(page_of(&pdf, "bravo"), 1);
    let pdf = render("@layer a.b{.x{break-before:page}}", body);
    assert_eq!(page_of(&pdf, "bravo"), 2);
}

// -- break values and harvesting (F87 review) --------------------------------------------------

/// A later `avoid` / `avoid-page` / `avoid-column` / `column` value overrides an earlier `page` (css-break-3 §3.1),
/// before and after; Chrome: no break.
#[test]
fn a_later_avoid_value_cancels_an_earlier_forced_break() {
    for v in ["avoid", "avoid-page", "avoid-column", "column"] {
        let head = format!("@layer u{{.a{{break-before:page}}.b{{break-before:{v}}}}}");
        let pdf = render(&head, r#"<p>alpha</p><p class="a b">bravo</p>"#);
        assert_eq!(page_of(&pdf, "bravo"), 1, "break-before:{v}");
        let head = format!("@layer u{{.a{{break-after:page}}.b{{break-after:{v}}}}}");
        let pdf = render(&head, r#"<p class="a b">alpha</p><p>bravo</p>"#);
        assert_eq!(page_of(&pdf, "bravo"), 1, "break-after:{v}");
    }
}

/// `always` is a value of `page-break-before`, not of `break-before`: the latter is ignored (Chrome too).
#[test]
fn always_breaks_only_as_a_legacy_page_break_value() {
    let body = r#"<p>alpha</p><p class="x">bravo</p>"#;
    let pdf = render("@layer u{.x{break-before:always}}", body);
    assert_eq!(page_of(&pdf, "bravo"), 1);
    let pdf = render("@layer u{.x{page-break-before:always}}", body);
    assert_eq!(page_of(&pdf, "bravo"), 2);
    let pdf = render("@layer u{.x{page-break-before:page}}", body);
    assert_eq!(page_of(&pdf, "bravo"), 1);
}

/// The `media` attribute of a `<style>` is a media query list for print, not a substring test.
#[test]
fn style_media_attribute_is_a_media_query_list_for_print() {
    let body = r#"<p>alpha</p><p class="x">bravo</p>"#;
    let rule = ".x{break-before:page}";
    for (media, breaks) in [
        ("print", true),
        ("not screen", true),
        ("print and (min-width:1px)", true),
        ("not print", false),
        ("screen", false),
        ("only screen", false),
    ] {
        let pdf = render_with_head(&format!(r#"<style media="{media}">{rule}</style>"#), body);
        let want = if breaks { 2 } else { 1 };
        assert_eq!(page_of(&pdf, "bravo"), want, "media={media:?}");
    }
}

/// A `<style>` inside `<noscript>` or `<template>` is read like the engine reads it. There is no scripting, so
/// `<noscript>` content applies; Blitz also applies a `<style>` in an inert `<template>` (a browser never does), and the
/// break reader follows the engine's cascade so that one stylesheet does not apply half its declarations. The
/// fork's `column_reader_agrees_with_stylo_on_template_and_noscript_styles` pins the agreement.
#[test]
fn styles_inside_template_and_noscript_follow_the_engine() {
    for wrap in ["template", "noscript"] {
        for css in [".x{break-before:page}", "@layer u{.x{break-before:page}}"] {
            let body = format!(
                r#"<{wrap}><style>{css}</style></{wrap}><p>alpha</p><p class="x">bravo</p>"#
            );
            let pdf = render("", &body);
            assert_eq!(page_of(&pdf, "bravo"), 2, "<{wrap}> {css}");
        }
    }
}
