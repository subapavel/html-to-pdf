# ADR-0007: SVG text is shaped with the bundled fonts only

Date: 2026-10-26 · Status: accepted · First use: F152 (`docs/plans/2026-10-26-f152-svg-text-bundled-fonts.md`)

## Context

usvg shapes an SVG's `<text>` with the `fontdb::Database` in its `Options`. Blitz built that database for every inline
`<svg>` with `load_system_fonts()`: the first inline SVG scanned the host's font directories and its text was drawn with
whatever the host had installed (FreeSerif, FreeSans, FreeMono, even a host copy of Inter instead of the bundled one),
so the PDF depended on the machine — against AGENTS.md goal 3 and the "fonts are bundled bytes" invariant. An SVG used
as an image (`<img src>`, `background-image`, `list-style-image`) was parsed with `usvg::Options::default()`, whose
database is empty, so its text was silently not drawn. css-fonts-4 §5 never drops text for want of a family: generic
families map to faces the UA chooses and an unmatched list falls back to a UA face. Our UA fonts are the bundle
(ADR-0002).

## Decision

- Every SVG parse that draws (inline, as an image, in a margin box) gets one font database built from the bundled faces
  only, in bundle order, built lazily once per render. No SVG parse ever calls `load_system_fonts()`; Blitz receives
  the whole `usvg::Options` from fulgur (`DocumentConfig::svg_options`).
- Generic families (`serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`) and keywords such as `system-ui` or
  `ui-monospace` resolve through the same generic-family map HTML text uses. Family names match ASCII
  case-insensitively.
- A family list that matches nothing is drawn with the `serif` mapping and a warning naming the SVG and the list. With
  `strict_fonts(true)` the document is refused (`Error::UnresolvedFontFamilies`, `Origin::Svg`), exactly as an HTML
  list that matches nothing is: the same switch reports the same problem, whatever element it is written on. A list
  whose later entry is bundled is not a problem.
- A family name spelled like a generic keyword but quoted (`'monospace'`) is a family name, not the generic (CSS
  Fonts 4 §4.2); a CSS-wide keyword (`initial`, `unset`, …) selects the default family.
- CSS inside an inline `<svg>` (its `<style>` elements, the `style=""` of the `<svg>` and its descendants) is left to
  the SVG resolver: our HTML generic-family rewrite and the strict-fonts scan of the HTML skip it, so a list is
  reported once, as SVG text.
- Validation-only parses (the bundled-image check before the engine runs) keep an empty database and a silent font
  resolver: they never draw text.

## Consequences

- Text that rendered in a host face now renders in a bundled one; text in an SVG used as an image is now drawn.
- Text in SVGs is part of the strict-fonts contract; a template that names an unbundled family in an SVG fails in
  strict mode where it passed before.
- SVG text in a page margin box is not checked by strict mode (warned only), like HTML text in a margin box.
- A `<style>` inside an inline `<svg>` that targets HTML elements is no longer checked by the strict scan nor has its
  `ui-*` generics mapped (the six generics the engine parses still resolve, F70).
- A document without SVG pays nothing; one with SVG pays one database build per render (name tables of the bundled
  faces).
- Not covered: inline SVG text does not inherit `font-family` / `font-size` from the HTML (F152c; Blitz parses the
  SVG's markup alone); usvg's `println!` for a COLRv1 face used in SVG text (F152b; we bundle none); `@font-face`
  inside an SVG.
