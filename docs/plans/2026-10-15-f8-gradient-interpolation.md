# F8 — Gradients with a colour-interpolation method

Date: 2026-10-15. Finding: F8 in `2026-10-15-m0-findings.md`.

## 1. Problem

`linear-gradient(to right in oklab, …)` paints nothing. Tailwind v4 emits `--tw-gradient-position: to right in oklab`
for every `bg-gradient-*` / `bg-linear-*`, so 05's header and totals box have no background.

Root cause: Stylo 0.8.0 parses `<color-interpolation-method>` in gradients only when the static pref
`layout.css.gradient-color-interpolation-method.enabled` is on (`stylo-0.8.0/values/specified/image.rs:37`), and
`stylo_static_prefs` 0.8.0 does not list it (→ false). The gradient is a parse error; through `var()` it is invalid
at computed-value time. Same mechanism as F2b (`@property`).

## 2. Spec

css-images-4 §3.1: `<color-interpolation-method> = in [<rectangular-color-space> | <polar-color-space>
<hue-interpolation-method>?]`, allowed before or after the direction / shape / angle in the first argument. Without
it, the default is oklab unless every stop is a legacy sRGB colour. css-color-4 §12.4 hue interpolation methods.

## 3. Where it belongs

Upstream: enable the pref (fork of `stylo_static_prefs` or a Blitz bump of Stylo). Blocked on fork creation; tracked
with F2b in `docs/upstream.md`.

Local, `crates/htmltopdf/src/gradient.rs` (same pattern as F7): remove `in <colorspace> [<hue> hue]` where it is an
interpolation method — top level of the first argument of `linear/radial/conic-gradient()` and their `repeating-`
forms, and top level of a custom-property value (Tailwind's `--tw-gradient-position`). A leading method also drops
the whitespace after it, and a method that is the whole first argument drops its comma. `color-mix(in …)` and
every other `in` stay. Applied to bundled CSS, `<style>` elements and `style=""` attributes without character
references (shared `fonts::rewrite_css_in_html`). The byte-level lexing helpers from `media.rs` moved to
`css_text.rs` so both rewrites share them.

What changes visually: none for Tailwind's oklch stops (the default is oklab, which is what Tailwind asked for).
`in srgb` / `in hsl longer hue` on purpose lose their effect — the gradient renders with the default method instead
of not at all.

## 4. Acceptance

- `crates/htmltopdf/tests/gradient_interpolation.rs`: red→red gradients written with `to right in oklab`,
  `in srgb to right`, `in oklab` alone, `90deg in hsl longer hue`, `repeating-…`, `radial-gradient(circle in oklch …)`
  paint red in bundled CSS, `<style>` and `style=""`; Tailwind's custom-property shape paints; `color-mix(in srgb …)`
  still works. Red before (white), green after. Unit tests in `gradient.rs`.
- 05: header and totals box show their gradients.
- Compare: 01–04 unchanged. 05 29.8 % → 32.3 %: the gradients are new content, painted in F1's wrong colours and with
  the header inside the page margin (F6) and the totals box lower than in Chrome (layout). With F1's patches applied
  locally 05 measures 21.5 % (24.9 % before F8). Threshold raised to 0.333 per the standing decision.

## 5. Limits

- In a custom property the method is removed only after a direction, angle or shape (`to right in oklab`): a value
  that is only `in srgb` may feed `color-mix(var(--m), …)`, where it is valid (found by `vrt-validator` on PR #5).
  So Tailwind's `bg-radial` / `bg-conic` (`--tw-gradient-position: in oklab` alone) still render nothing — as before.
- `style=""` values with a character reference are not rewritten (as for F3b).
- Gradients in `<link>`ed or `@import`ed sheets are not seen (as for F7, F3b).
