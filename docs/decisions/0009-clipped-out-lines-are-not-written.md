# ADR-0009: A line box wholly outside the block-axis clip is not written to the PDF

Date: 2026-10-28 · Status: accepted · First use: F30f-b, clip part (`docs/plans/2026-10-28-f30f-b-clip-boxes.md`)

## Context

A box whose computed `overflow-y` is not `visible` (`hidden`, `clip`, `auto`, `scroll`; also `overflow-x: hidden`, whose
`visible` y computes to `auto`) shows only its padding box (css-overflow-3 §3). When its `height` or `max-height` is
below its content, the lines below the padding box are invisible. Before F30f-b our writer still wrote their glyphs and
link annotations: the clip path hid them, but `pdftotext`, copy and paste, search and screen readers read them. A
template that reserves a fixed box for a note (`h-28 overflow-hidden`) and an ERP that pastes in a longer one would
give an invoice a text layer holding words that are not on the page — an old amount, a variable symbol — read as data
by whatever matches or indexes the PDF.

No spec says anything about a PDF's text layer (CSS 2.1 §11.1.1 says content outside the clip "is clipped"). Chrome
(m141, A4, 15 mm, Inter, 20 px lines, 4000 words, F30f-b plan §7) writes none for such lines: `overflow:hidden;
height:300px` holds 105 words (15 lines of 7), not the 372 that fit the page; a 600 px box 211 (30 lines); `height:310px`
112 (16 lines, the 16th is cut by the box edge and written); a link in the hidden part has no annotation, one in the
visible part has.

## Decision

- A line box wholly outside the block-axis clip on its page is not written: no glyphs, no link rectangle, no inline
  box, no marked content. A line that shows at all is written, and the clip hides the rest (Chrome's numbers above).
  "Shows" means at least 0.5 pt of the line box is inside the band: line boxes are quantised to the pixel (F78), and
  their tops are sums of the heights above.
- The band is the padding box of a clipping box in page coordinates, intersected with the bands of the clipping boxes
  around it. Only a box whose block size the author constrains (a `height`, or a `max-height` other than `none`; a
  percentage counts when its basis is definite) has a band: an auto-height box is as tall as its content and hides no
  line, whatever its line boxes measure (Blitz's are as tall as the font when the line height is below it). Without this
  rule the first version dropped words that showed (plan §7, "Auto-height clip boxes"). A table, row group, row or cell
  never counts: its `height` is a minimum (CSS 2.1 §17.5.3) and the box grows with its content (plan §7, "Padded and
  bordered clip boxes").
  It exists for the block axis only (`overflow-y`); `overflow-x: clip` alone hides nothing here.
- The policy applies to a constrained clip box whose content is a *supported shape* (plan §7, "Supported shapes"): direct
  text, or exactly one in-flow child that starts at the box's content top (by the resolved layout, so a margin counts) and
  is a paragraph or a block wrapper around one, with no `break-inside: avoid` and no percentage vertical padding on the way.
  Any other constrained clip box (a heading, margin, float, image or earlier paragraph above the paragraph, a second
  paragraph) has no band and keeps main's output, byte for byte, until F30c lets the fragmenter place more than the
  first thing on a strip (finding F30f-f). Reason: the fragmenter moved such a paragraph whole and the band then dropped
  lines that Chrome shows on the page the box starts on; a band is only as good as the layout under it.
- Inside a transform, and inside an inline box that the carrier paints under a translate, the coordinates are not page
  coordinates: no band is known there and none is set, so the output stays as it was. A clip box inside a transformed box
  still writes what it hid before.
- The fix is a writer rule (fulgur `draw_shaped_lines` tests `Canvas::clip_band`, `draw_under_clip` sets it). The
  fragmenter's part of F30f-b (a clip box is fragmented by its own block size, nothing at or below the clip bottom gets
  a page) is in the plan, not here: it is layout, and the spec allows both ways for `height`.

## Consequences

- `pdftotext` of a fixed-height note box shows the lines you see. Link annotations of hidden text are gone.
- Pixels do not change: what is dropped was clipped away. The PDF's bytes change only for a document that has a clip
  box with lines wholly outside it; fixtures 01 to 27 have none.
- A paragraph clipped out completely leaves its structure element in a tagged PDF with no marked content, and a partly
  clipped one has a shorter run. `pdf-reviewer` checks that this is no defect; if it is, the converter drops the element
  at tag time.
- Not covered: the inline axis (a line wider than a `text-overflow` or `overflow-x` box keeps all its glyphs; Tailwind's
  `truncate` is not in the compiled CSS), tables with `overflow:hidden` (`draw_under_clip_table`, F51), images and boxes
  that are clipped out (their bytes are written and clipped), `overflow-clip-margin`.
- A box that is not a supported shape still writes its hidden lines (main's behaviour); the text layer of a fixed-height box
  with a heading above its text can hold words that are not on the page until F30f-f.
- A wrong band would drop text that is visible. The 0.5 pt rule, the transform exemption and the tests in the fulgur
  fork and in `crates/htmltopdf/tests/clip_boxes.rs` pin it.
