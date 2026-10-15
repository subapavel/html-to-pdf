//! Gradients with a colour-interpolation method (F8).
//!
//! The engine's Stylo build parses `<color-interpolation-method>` in gradients only when the static pref
//! `layout.css.gradient-color-interpolation-method.enabled` is set, and `stylo_static_prefs` 0.8.0 does not
//! list it. `linear-gradient(to right in oklab, …)` is then invalid and the whole `background-image` is
//! dropped. Tailwind v4 writes every gradient that way (`--tw-gradient-position: to right in oklab`).
//! Until the pref is enabled upstream (see `docs/upstream.md`) we remove the method:
//! `in <colorspace> [<hue-method> hue]` at the top level of a gradient function's first argument, and at
//! the top level of a custom-property value (where Tailwind keeps it). The gradient then uses its default
//! method — oklab for any stop that is not a legacy sRGB colour (css-images-4 §3.1), which covers
//! Tailwind's `oklch()` palette. `in` elsewhere (`color-mix(in oklab, …)`, where it is required) stays.

use std::borrow::Cow;
use std::ops::Range;

use crate::css_text::{
    ident_end, is_ident_start, is_unquoted_url, skip_comment, skip_escape, skip_string,
    skip_unquoted_url,
};

const GRADIENTS: [&str; 6] = [
    "linear-gradient",
    "radial-gradient",
    "conic-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "repeating-conic-gradient",
];

const COLOR_SPACES: [&str; 15] = [
    "srgb",
    "srgb-linear",
    "display-p3",
    "a98-rgb",
    "prophoto-rgb",
    "rec2020",
    "lab",
    "oklab",
    "xyz",
    "xyz-d50",
    "xyz-d65",
    "hsl",
    "hwb",
    "lch",
    "oklch",
];

const HUE_METHODS: [&str; 4] = ["shorter", "longer", "increasing", "decreasing"];

/// `css` (a stylesheet or a declaration list) without colour-interpolation methods in gradients.
pub(crate) fn strip_gradient_interpolation(css: &str) -> Cow<'_, str> {
    let cuts = interpolation_spans(css);
    if cuts.is_empty() {
        return Cow::Borrowed(css);
    }
    let mut out = String::with_capacity(css.len());
    let mut last = 0;
    for r in cuts {
        out.push_str(&css[last..r.start]);
        last = r.end;
    }
    out.push_str(&css[last..]);
    Cow::Owned(out)
}

enum Frame {
    /// Inside a gradient function; `open` is the byte after its `(`.
    Gradient {
        open: usize,
        first_arg: bool,
    },
    Other,
}

/// Byte ranges to delete, in order.
fn interpolation_spans(css: &str) -> Vec<Range<usize>> {
    let b = css.as_bytes();
    let mut out = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // Inside the value of a custom property (`--x: …`), at parenthesis depth 0: where the value starts.
    let mut custom_value: Option<usize> = None;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => i = skip_comment(css, i),
            b'"' | b'\'' => i = skip_string(b, i),
            b'\\' => i = skip_escape(css, i),
            b'u' | b'U' if is_unquoted_url(b, i) => i = skip_unquoted_url(b, i),
            b'(' => {
                stack.push(Frame::Other);
                i += 1;
            }
            b')' => {
                stack.pop();
                i += 1;
            }
            b',' => {
                if let Some(Frame::Gradient { first_arg, .. }) = stack.last_mut() {
                    *first_arg = false;
                }
                i += 1;
            }
            b';' | b'{' | b'}' if stack.is_empty() => {
                custom_value = None;
                i += 1;
            }
            c if is_ident_start(c) => {
                let end = ident_end(b, i);
                let word = &css[i..end];
                if b.get(end) == Some(&b'(') {
                    stack.push(if GRADIENTS.iter().any(|g| g.eq_ignore_ascii_case(word)) {
                        Frame::Gradient {
                            open: end + 1,
                            first_arg: true,
                        }
                    } else {
                        Frame::Other
                    });
                    i = end + 1;
                    continue;
                }
                if stack.is_empty() && word.starts_with("--") {
                    let colon = skip_ws(b, end);
                    custom_value = (b.get(colon) == Some(&b':')).then_some(colon + 1);
                } else if word.eq_ignore_ascii_case("in") {
                    let gradient_open = match stack.last() {
                        Some(Frame::Gradient {
                            open,
                            first_arg: true,
                        }) => Some(*open),
                        _ => None,
                    };
                    // In a custom property only after a direction, angle or shape (`to right in oklab`):
                    // a value that is just `in srgb` may feed `color-mix(var(--m), …)`, where it is valid.
                    let in_position = stack.is_empty()
                        && custom_value.is_some_and(|start| !css[start..i].trim().is_empty());
                    if (gradient_open.is_some() || in_position)
                        && let Some(method_end) = method_end(css, end)
                    {
                        out.push(cut(b, i, method_end, gradient_open));
                        i = method_end;
                        continue;
                    }
                }
                i = end;
            }
            _ => i += 1,
        }
    }
    out
}

/// End of `<colorspace> [<hue-method> hue]` following the `in` that ends at `i`, if that is what follows.
fn method_end(css: &str, i: usize) -> Option<usize> {
    let (space, end) = next_ident(css, i)?;
    if !COLOR_SPACES.iter().any(|s| s.eq_ignore_ascii_case(space)) {
        return None;
    }
    if let Some((hue_method, after)) = next_ident(css, end)
        && HUE_METHODS
            .iter()
            .any(|h| h.eq_ignore_ascii_case(hue_method))
        && let Some((hue, after_hue)) = next_ident(css, after)
        && hue.eq_ignore_ascii_case("hue")
    {
        return Some(after_hue);
    }
    Some(end)
}

/// The range to delete for a method spanning `in_start..method_end`, with the whitespace before it. When
/// the method opens a gradient's first argument, the whitespace after it goes instead, and when it is the
/// whole first argument, the comma after it too.
fn cut(b: &[u8], in_start: usize, method_end: usize, gradient_open: Option<usize>) -> Range<usize> {
    let mut start = in_start;
    while start > 0 && b[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    if let Some(open) = gradient_open
        && start <= open
    {
        let after = skip_ws(b, method_end);
        if b.get(after) == Some(&b',') {
            return open..skip_ws(b, after + 1);
        }
        return open..after;
    }
    start..method_end
}

fn next_ident(css: &str, i: usize) -> Option<(&str, usize)> {
    let b = css.as_bytes();
    let start = skip_ws_and_comments(css, i);
    if start == i || start >= b.len() || !is_ident_start(b[start]) {
        return None;
    }
    let end = ident_end(b, start);
    Some((&css[start..end], end))
}

fn skip_ws_and_comments(css: &str, mut i: usize) -> usize {
    let b = css.as_bytes();
    loop {
        let next = skip_ws(b, i);
        if b.get(next) == Some(&b'/') && b.get(next + 1) == Some(&b'*') {
            i = skip_comment(css, next);
        } else {
            return next;
        }
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(css: &str) -> String {
        strip_gradient_interpolation(css).into_owned()
    }

    #[test]
    fn removes_the_method_from_gradients() {
        assert_eq!(
            s("a{background:linear-gradient(to right in oklab, red, blue)}"),
            "a{background:linear-gradient(to right, red, blue)}"
        );
        assert_eq!(
            s("a{background:linear-gradient(in srgb to right,red,blue)}"),
            "a{background:linear-gradient(to right,red,blue)}"
        );
        assert_eq!(
            s("a{background:LINEAR-GRADIENT( in OKLAB , red, blue)}"),
            "a{background:LINEAR-GRADIENT(red, blue)}"
        );
        assert_eq!(
            s("a{background:conic-gradient(from 0deg in hsl longer hue, red, blue)}"),
            "a{background:conic-gradient(from 0deg, red, blue)}"
        );
        assert_eq!(
            s(
                "a{background:radial-gradient(circle in oklch, red, blue), repeating-linear-gradient(in lab, red 0 1px, blue 1px 2px)}"
            ),
            "a{background:radial-gradient(circle, red, blue), repeating-linear-gradient(red 0 1px, blue 1px 2px)}"
        );
    }

    #[test]
    fn removes_the_method_from_custom_properties() {
        assert_eq!(
            s(".a{--tw-gradient-position:to right in oklab;color:red}"),
            ".a{--tw-gradient-position:to right;color:red}"
        );
        assert_eq!(
            s(".a{--p:45deg /* c */ in /* d */ oklab}"),
            ".a{--p:45deg /* c */}"
        );
    }

    #[test]
    fn comments_may_separate_the_method_tokens() {
        assert_eq!(
            s("a{b:linear-gradient(to right in/**/oklab,red,blue)}"),
            "a{b:linear-gradient(to right,red,blue)}"
        );
    }

    #[test]
    fn leaves_other_uses_of_in_alone() {
        for css in [
            "a{color:color-mix(in oklab, red, blue)}",
            "a{--c:color-mix(in oklab, red 30%, transparent)}",
            "a{background:linear-gradient(to right, color-mix(in oklab, red, blue), blue)}",
            "a{background:linear-gradient(to right, red in oklab, blue)}",
            "a{content:'linear-gradient(in oklab, red, blue)'}",
            "/* linear-gradient(in oklab, red, blue) */",
            "a{background:url(x/linear-gradient(in-oklab.png)}",
            "a{background:linear-gradient(in foo, red, blue)}",
            "@supports (color:color-mix(in lab, red, red)){a{b:c}}",
            "a{margin:0 in}",
            // Only the method: may feed `color-mix(var(--m), …)`.
            ".a{--m: in srgb} .b{background:color-mix(var(--m), red, blue)}",
        ] {
            assert_eq!(strip_gradient_interpolation(css), css, "{css}");
        }
    }
}
