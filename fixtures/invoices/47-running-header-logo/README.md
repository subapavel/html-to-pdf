# 47 — a letterhead in the margin: logo and name top left, a certification mark top right (`position: running()` with `<img>`)

The two-page invoice of 17 (42 item rows, "Celkem k úhradě"), with the footer replaced by a header that is written once, at the top of
`<body>`, and printed in the top margin of **every** page:

```html
<style>
  .hlavicka { position: running(hlavicka); }
  .znacka { position: running(znacka); }
  @page { size: A4; margin: 15mm; @top-left { content: element(hlavicka); } @top-right { content: element(znacka); } }
</style>
…
<header class="hlavicka flex items-center gap-4 text-sm">
  <img src="logo.png" alt="" style="height:32px"> <span class="font-semibold">Příkladná integrace s.r.o.</span>
</header>
<div class="znacka"><img src="znacka.svg" alt=""></div>
```

This is the GCPM way of putting a letterhead on every page. `logo.png` (fixture 03's, 240 × 72, no `width` / `height` in the
markup: the natural ratio gives 106.67 × 32 px) is a bundled PNG; `znacka.svg` (96 × 32 px, a rounded rectangle with the text
"ISO 9001" in Inter) is a bundled SVG, bundled like the PNGs (`scripts/render-fixtures.sh` and the fixture tests also register
`*.svg` now). Both sit inside the 56.69 px top band (F139) and are centred in it (F103).

Exercises:
- an `<img>` in a running element (F107; CSS GCPM 3 §2.2: the element is the content of the margin box, so its images are ordinary
  content). **On main's binary neither image was painted** (the margin-box documents were converted without the bundle, and an
  `<img>` without `width` / `height` was laid out 0 wide in the measure stage and the render stage; an SVG `<img>` was never
  drawn): the band held the name only, and `@top-left` was measured without the logo (css-page-3 §5.3.2: the boxes of an edge are
  sized from the max-content width of their contents).
- a Tailwind flex row with an image and text in a margin box, measured and then laid out in the rectangle the measure made.
- SVG text in a margin box drawn with the bundled font (F152), on both pages, from one parse per page.

**Reference.** Chrome does not implement `position: running()` or `element()` and prints the header in the flow. The probes of
2026-11-04 (Chromium m153):

| Probe | What | Result |
|---|---|---|
| P1 | `@top-left { content: url(logo.png) " name" }`, `@top-right { content: url(znacka.svg) }` | Chrome paints `url()` in margin boxes, but at the image's natural size: the 240 × 72 px logo is taller than the band and overlaps the heading. Not usable as it is. |
| P1b | the same boxes with the images as `background` (`no-repeat left center / auto 32px` and `center / 96px 32px`) and `padding-left` for the logo's room, the name as `content` | **Used.** Logo, name and mark are drawn at the sizes of `index.html`: mark x 642–738, y 12–44 on both pages (the arithmetic gives 641.0–737.0, 12.35–44.35). |
| P2 | a `position: fixed; top: -15mm` header (Chrome repeats fixed boxes on every printed page) | Nothing is painted above the content area on any page. Not usable. |
| P3 | the header as a normal block in a one-page document | Pins the sizes (logo 106.67 × 32 px, gap 16 px, name 14 px semibold, mark 96 × 32 px); not a reference of its own. |

`reference.html` is therefore the same invoice with the header deleted from the body and P1b's two margin boxes;
`scripts/reference/render.mjs` renders `reference.html` instead of `index.html` when a fixture has one. What the comparison shows:
where the body text sits, the page count (2), that the logo, the name and the mark are in the top band of both pages at the same
place, and that nothing stands above the first heading. It cannot show the flex layout (the name's x comes from the logo's width
and the gap by hand in `padding-left: 122.67px`), that the images are *elements* of a running element, or the SVG's own text
(Chrome draws it with whatever Inter it finds, ours with the bundled one). The pixel tests that pin the shape without Tailwind
and compare against a document with the header deleted are in `crates/htmltopdf/tests/running_images.rs`.

Every class is in the compiled `fixtures/tailwind/tailwind.css` (no rebuild).
