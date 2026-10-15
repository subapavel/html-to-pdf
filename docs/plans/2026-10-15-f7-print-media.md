# F7 — Evaluate media queries for `print`

Date: 2026-10-15. Finding: F7 in `2026-10-15-m0-findings.md`.

## 1. Problem

Blitz 0.2.4 builds the Stylo `Device` with `MediaType::screen()` (`blitz-dom/src/document.rs::make_device`), and
fulgur 0.40.0 re-creates it on every `set_viewport`. Rules in `@media print` never apply, rules in `@media screen`
and `@media not print` do. Tailwind's `print:` variants are lost (05's `print:bg-slate-900` header).
Probe (before): `@media print` → not applied, `@media screen` → applied, `not print` → applied.

## 2. Spec

mediaqueries-4 §2.3 media types; §2.2 `not` / `only`; css-page-3 / HTML: a document rendered to paged output is
the `print` medium. Chrome's reference (`scripts/reference/render.mjs`) uses `emulateMedia({ media: "print" })`.

## 3. Where it belongs

Upstream: `make_device` should take the media type from `DocumentConfig`/`Viewport`, and fulgur should ask for
`print` (fulgur's Blitz PR #390 points that way). That needs forks of fulgur and Blitz, and creating forks is blocked
in agent sessions (postmortem on branch `f1-srgb-conversion`); fulgur exposes no DOM-pass hook
through which our crate could swap the device.

Local, in `crates/htmltopdf/src/media.rs` (same pattern as the F2 polyfill): because the device is *always*
`screen`, swapping the `print` and `screen` media types in every query gives exactly the result of evaluating the
original query on a print device — `not`, `only`, comma lists and features included. Applied to
- bundled stylesheets (`RendererBuilder::css`), and
- the contents of the document's `<style>` elements (`render`, `render_to_file`), located with the tag scanner
  that `fonts` already uses (`fonts::style_element_ranges`).

Only identifiers at parenthesis depth 0 in `@media` / `@import` preludes are swapped; comments, strings, `url()`,
`layer()`, `supports()` and feature names are untouched. Remove the module once the engine evaluates `print`.

## 4. Acceptance

- `crates/htmltopdf/tests/print_media.rs`: `print`, `screen`, `not print`, `not screen`, `only print and (…)`,
  `screen, PRINT`, a feature-only query — in bundled CSS and in a `<style>` element — paint as on a print device.
  Red before (screen semantics), green after. Unit tests in `media.rs` for the rewrite.
- 05-fancy: `print:bg-slate-900` applies — the header is dark, as in Chrome.
- Compare: 01–04 unchanged. 05 25.4 % → 29.8 %: content that was missing (the header background) is now painted,
  but (a) in F1's wrong colour and (b) inside the page margin, while Chrome prints the header full-bleed
  (`@page { margin: 0 }`, F6). Measured with F1's patches applied locally: 21.4 % (F1 alone) → 24.9 % (F1 + F7),
  the remainder being the header position. Threshold raised to 0.308 per the standing decision, comment in
  `compare.py`; it comes back down with F1 and F6.

## 5. Known gaps (not in scope)

- The `media` attribute of `<style>` is ignored by the engine: `<style media="screen">` applies in the PDF.
  Rewriting it into an `@media` wrapper breaks `@import`/`@charset`/`@layer` statements inside; logged in
  `docs/upstream.md`.
- `<link rel=stylesheet media=…>` (fulgur rewrites it into an `@import … media` inside a synthetic `<style>`
  after our rewrite runs) — still evaluated as screen.
- `@custom-media`, media types inside `@supports`/`@container` conditions — not used by Tailwind.
- `@page` (and its margin boxes) inside any `@media` block is ignored by fulgur 0.40.0's GCPM parser, before and
  after this change — a print-only `@page` must be written at top level. Found by `vrt-validator` on PR #3.

## 6. Validation

`vrt-validator` on PR #3: numbers as above, byte-identical re-renders, fonts embedded with ToUnicode. It found a
panic (a backslash before a multi-byte character in a prelude split the character), an unquoted `url(a/*b)` read
as a comment, and unterminated strings running past the newline; all three fixed with unit tests. The tag
scanner no longer lowercases the remaining document for every raw-text element.
