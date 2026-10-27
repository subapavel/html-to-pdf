//! Media queries for the `print` medium (F7).
//!
//! The engine (Blitz 0.2.4) builds its Stylo `Device` with `MediaType::screen()` and fulgur 0.40.0 offers
//! no way to replace it, so `@media print` rules never apply and `@media screen` rules do — Tailwind's
//! `print:` variants are lost. Until that is fixed upstream (see `docs/upstream.md`), we rewrite the media
//! types in the CSS we hand over: every `print` becomes `screen` and every `screen` becomes `print`.
//! Because the device is always `screen`, a query evaluated after the swap gives exactly the answer it
//! would give on a print device, including `not`, `only` and comma-separated lists. Media *features*
//! (`(min-width: …)`) and every other media type are left alone.
//!
//! Covered: `@media` and `@import` preludes in bundled stylesheets and in the document's `<style>`
//! elements. Not covered: the `media` attribute of `<style>`/`<link>` (the engine ignores it on
//! `<style>` and applies the sheet regardless) and `@custom-media`.

use std::borrow::Cow;
use std::ops::Range;

use crate::css_text::{
    ident_end, is_ident_start, is_unquoted_url, skip_comment, skip_escape, skip_string,
    skip_unquoted_url,
};

/// `css` with the `print` and `screen` media types swapped in every `@media` / `@import` prelude.
pub(crate) fn print_media(css: &str) -> Cow<'_, str> {
    swap(css, &media_type_spans(css, 0))
}

/// `html` with [`print_media`] applied to the contents of every `<style>` element. The renderer applies it together
/// with its other rewrites in one pass (`Renderer::prepare`); this is the single rewrite, for tests.
#[cfg(test)]
pub(crate) fn print_media_in_html(html: &str) -> Cow<'_, str> {
    crate::fonts::HtmlSites::walk(html).rewrite(html, |css, site| {
        if site.attribute {
            Cow::Borrowed(css)
        } else {
            print_media(css)
        }
    })
}

fn swap<'a>(text: &'a str, spans: &[Range<usize>]) -> Cow<'a, str> {
    if spans.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + spans.len());
    let mut last = 0;
    for r in spans {
        out.push_str(&text[last..r.start]);
        out.push_str(if text[r.clone()].eq_ignore_ascii_case("print") {
            "screen"
        } else {
            "print"
        });
        last = r.end;
    }
    out.push_str(&text[last..]);
    Cow::Owned(out)
}

/// Byte ranges (shifted by `offset`) of the `print` / `screen` media types in `css`. Only identifiers at
/// parenthesis depth 0 of an `@media` or `@import` prelude are media types: features, `url(…)`,
/// `layer(…)` and `supports(…)` are all inside parentheses, and `and`/`not`/`only`/`or` are not swapped.
fn media_type_spans(css: &str, offset: usize) -> Vec<Range<usize>> {
    let b = css.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => i = skip_comment(css, i),
            b'"' | b'\'' => i = skip_string(b, i),
            b'\\' => i = skip_escape(css, i),
            b'u' | b'U' if is_unquoted_url(b, i) => i = skip_unquoted_url(b, i),
            b'@' => {
                let end = ident_end(b, i + 1);
                let name = &css[i + 1..end];
                i = end;
                if name.eq_ignore_ascii_case("media") || name.eq_ignore_ascii_case("import") {
                    i = scan_prelude(css, i, offset, &mut out);
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Walks a prelude from `i` up to (not including) its `{` or `;`, collecting media types.
fn scan_prelude(css: &str, mut i: usize, offset: usize, out: &mut Vec<Range<usize>>) -> usize {
    let b = css.as_bytes();
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'{' | b';' | b'}' if depth == 0 => return i,
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => i = skip_comment(css, i),
            b'"' | b'\'' => i = skip_string(b, i),
            b'\\' => i = skip_escape(css, i),
            b'u' | b'U' if is_unquoted_url(b, i) => i = skip_unquoted_url(b, i),
            c if is_ident_start(c) => {
                let end = ident_end(b, i);
                let word = &css[i..end];
                if depth == 0
                    && (word.eq_ignore_ascii_case("print") || word.eq_ignore_ascii_case("screen"))
                {
                    out.push(offset + i..offset + end);
                }
                i = end;
            }
            _ => i += 1,
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(css: &str) -> String {
        print_media(css).into_owned()
    }

    #[test]
    fn swaps_media_types_in_media_and_import_preludes() {
        assert_eq!(p("@media print{a{b:c}}"), "@media screen{a{b:c}}");
        assert_eq!(p("@media screen { a {} }"), "@media print { a {} }");
        assert_eq!(p("@MEDIA not PRINT {}"), "@MEDIA not screen {}");
        assert_eq!(
            p("@media only screen and (min-width: 1px), print {}"),
            "@media only print and (min-width: 1px), screen {}"
        );
        assert_eq!(
            p(r#"@import url("print.css") print;"#),
            r#"@import url("print.css") screen;"#
        );
        assert_eq!(
            p("@import 'screen.css' screen;"),
            "@import 'screen.css' print;"
        );
        assert_eq!(
            p("@layer x{@media print{a{b:c}}}"),
            "@layer x{@media screen{a{b:c}}}"
        );
    }

    #[test]
    fn leaves_everything_else_alone() {
        for css in [
            ".print { color: red } .screen{}",
            "@media (min-width: 640px) { .print {} }",
            "@media all and (hover: hover) {}",
            "@import url(print.css) layer(print) supports(display: grid);",
            "/* @media print {} */ a { content: '@media print' }",
            "@media printer, screens, -print {}",
            "@supports (x: print) {}",
            "@page { @top-center { content: 'print' } }",
            "@media \\é print-x {}",
        ] {
            assert_eq!(print_media(css), css, "{css}");
        }
    }

    #[test]
    fn escapes_urls_and_broken_strings_do_not_derail_the_scan() {
        // A backslash before a multi-byte character must not split it (this used to panic).
        assert_eq!(p("@media \\é print {}"), "@media \\é screen {}");
        assert_eq!(
            p("@media \\é{} @media print {}"),
            "@media \\é{} @media screen {}"
        );
        // `/*` inside an unquoted url is not a comment.
        assert_eq!(
            p("a { b: url(a/*b.png) } @media print {}"),
            "a { b: url(a/*b.png) } @media screen {}"
        );
        assert_eq!(
            p("a { b: url( \"a/*b.png\" ) } @media print {}"),
            "a { b: url( \"a/*b.png\" ) } @media screen {}"
        );
        // An unterminated string ends at the newline.
        assert_eq!(
            p("a { b: 'x\n} @media print {}"),
            "a { b: 'x\n} @media screen {}"
        );
    }

    #[test]
    fn rewrites_only_style_elements_in_html() {
        let html = r#"<p class="print">@media print</p><style>@media print{.a{}}</style><!-- <style>@media screen{}</style> --><STYLE media="all">@media screen{}</STYLE>"#;
        assert_eq!(
            print_media_in_html(html),
            r#"<p class="print">@media print</p><style>@media screen{.a{}}</style><!-- <style>@media screen{}</style> --><STYLE media="all">@media print{}</STYLE>"#
        );
    }
}
