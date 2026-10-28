# 27 — inline logo and inline QR code in a hand-written invoice (no Tailwind)

One-page Czech invoice written as plain HTML + CSS, the way a template without Tailwind emits it: there is no preflight, so
`<img>` keeps its default `display: inline`. The renderer is given no stylesheet for this fixture (`scripts/render-fixtures.sh`
and `fixtures.rs` pass `fixtures/tailwind/tailwind.css` only to fixtures whose `<link>` names it).

- The header is `<img src="logo.png" height="48"> <span>Příkladná integrace s.r.o.</span>` with a 2 px rule under it: an inline
  image on a line with text, the line's strut (CSS 2.1 §10.8.1) leaves 4 px under the image's bottom edge, so the rule sits 4 px
  lower than the image's bottom edge plus the padding (Chrome: rule rows 121..123).
- The payment cell holds the QR code (`<img src="qr.png" width="123" height="123">`) alone on its line: the cell is 123 + 4 + the
  padding tall (Chrome: QR rows 454..566, bottom border row 583).
- A small logo inline in a paragraph (`.note img`, 12 px high) must not stretch its line.
- Around them: two `inline-block` columns with `vertical-align: top`, a `border-collapse` items table, totals, a `dl` of
  `inline-block` terms.

Main's binary (Parley 0.6) counted an inline box only as ascent: every line holding an image was as tall as the image, the rule and
the cell border sat 4 px high and everything below moved up (compare 7.6 %). Plan: `docs/plans/2026-10-27-f183-inline-img-strut.md`.
The tests that pin the shapes with Chrome's numbers are `crates/htmltopdf/tests/inline_img_strut.rs` (S1-S12, guards G1-G5, and
this fixture). `logo.png` is fixture 03's; `qr.png` is the QR code of fixture 03's SVG rasterised at 3 px per module.
