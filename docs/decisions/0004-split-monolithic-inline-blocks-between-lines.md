# ADR-0004: Split a tall `inline-block` between its line boxes instead of slicing it at the page edge

Date: 2026-10-18 · Status: accepted · First use: F30f (`docs/plans/2026-10-18-f30f-inline-block-overflow-wrapper.md`)

## Context

css-break-3 §4.1 lets a UA treat "inline-block and inline-table boxes (and other inline-level display types that
establish an independent formatting context)" as monolithic, and says that monolithic content taller than the
fragmentainer may be sliced at an arbitrary point in paged media instead of being lost. Chrome does that. Measured
with headless Chromium m141 (F30f plan §7, A4, 15 mm margins, 4000 words in the box):

- a long paragraph in `display:inline-block` (width 100 % or auto, centred or not, also direct text in the box)
  prints every word on 12 pages, but the line that straddles each page edge is printed on both pages, half on each
  (42 words appear twice; `pdftotext` sees them twice);
- when something precedes the box on its page (a heading, a bordered frame around it), Chrome does not start the box
  there: it moves the whole box to the next page and slices it from its top (13 pages instead of 12, the heading
  alone on page 1, an empty frame on page 1);
- the same holds for an `inline-block` that shares its line with text, and for `inline-flex`.

A clipping box (`overflow` hidden, clip, auto or scroll) is different: Chrome fragments it between lines like any
block (no slicing, every word once, 12 pages), probably because Blink does not treat scroll containers as monolithic when
printing (from memory, the Blink source was not read). Those wrappers are not covered by this decision.

## Decision

An `inline-block` that is the only thing on its carrier's single line (`alone_on_its_line`) is split between its line
boxes, with orphans and widows 2, each line in exactly one fragment (css-break-3 §4.4, the F78 rule), exactly like a
block. We do not slice a line in two.

An `inline-block` that shares its line, and `inline-flex` / `inline-grid` / `inline-table`, are not split (F30f-b);
the warning names the barrier.

## Consequences

- Every word is printed once, at the fragmenter's position. Chrome prints a line twice (cut in half) at each page
  edge, which is not what a reader or `pdftotext` wants either.
- At most one line per page is placed differently from Chrome, and where the box is not first on its page we start it
  where it is (below the heading, like a paragraph), while Chrome starts it on the next page: A3 has 12 pages here, 13
  in Chrome. A box that starts a page has Chrome's page count (A1, A2: 12; A2c: 20).
- It is the same mechanism as for clipping wrappers, so there is one place that decides line ranges.
- The frame of a split box (border, radius) is closed on every page it touches (F96), where Chrome slices it.
- If a product ever needs Chrome's exact slicing (a monolithic box cut at the page edge, a line printed on both pages),
  it is a new decision with its own renderer work; nothing here prevents it.
