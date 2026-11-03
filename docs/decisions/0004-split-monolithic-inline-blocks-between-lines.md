# ADR-0004: Split a tall `inline-block` between its line boxes instead of slicing it at the page edge

Date: 2026-10-18 · Status: accepted · First use: F30f (`docs/plans/2026-10-18-f30f-inline-block-overflow-wrapper.md`) · Amended 2026-10-18 by F30e (`inline-table`, below) and 2026-11-03 by F213d (a box that fits a page, below)

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
boxes, with the inner block container's `orphans` and `widows` (initially 2; read since F30c-o), each line in exactly one
fragment (css-break-3 §4.4, the F78 rule), exactly like a block. We do not slice a line in two.

An `inline-block` that shares its line, and `inline-flex` / `inline-grid` / `inline-table`, are not split (F30f-b);
the warning names the barrier.

**Amended 2026-10-18 by F30e: `inline-table` is covered like `inline-block`.** An `inline-table` that is the only thing
on its carrier's single line is split between its line boxes in the same way (orphans and widows 2, each line in
one fragment). Chrome m141 slices it exactly as it slices an `inline-block` (F30e plan §7, A4, 15 mm, 120 lines of
20 px, width 100 %: 3 pages 51 / 51 / 19 with the straddling line printed on both pages; with a bordered frame around
it, the table moves whole to page 2 and the frame stays empty on page 1, 4 pages; after an `<h1>` the same). Its
inside is a table box whose real or anonymous cells are block containers, so the line splitter reaches the lines
through them. What stays whole is now: an `inline-block` or `inline-table` that shares its line, and
`inline-flex` / `inline-grid` (F30f-b); the warning names them ("an inline-block or inline-table that shares its
line").

**Amended 2026-11-03 by F213d: an alone box that fits a fresh page is monolithic; the split applies to boxes taller than a page.**
The reason for splitting an alone box between its lines was that a box taller than a page, kept whole, loses what is
below the page edge, and Chrome's slice prints a line twice. That reason does not apply to a box that fits a fresh
page: moving it whole to the next page, through the Class A break in front of its paragraph, prints every line once at
Chrome's position (css-break-3 §4.4: monolithic content that does not fit the room is pushed to the next fragmentainer; only
content taller than the fragmentainer is sliced). Measured with Chrome m153 (F213d plan §7): an `inline-block` or
`inline-table` alone on its line, auto height with 15 block children, a fixed or `min-height` box, with 120 px of room, is on
page 2 as a whole; ours (before) split an auto box between its children and left a fixed box with a short child on page 1,
painted below the edge and printed nowhere. So an `inline-block` / `inline-table` alone on its line is not entered when
the carrier fits a fresh page as its wrappers hold it (its height at most the page less the block-axis padding and border of its ancestors, with no slack: a box in the last pixel of a page does not fit, and a wrapper would place it whole below the page edge) and its content ends inside it
(`content_ends_inside`, the walk of `clip_hides_nothing`, where the margin below the last child is not content: Blitz's auto height omits it); the split between its lines stays for a box taller than a page and
for one whose content overflows it visibly (the overflow would stay behind). The box taller than a page with a short child
(a fixed 1200 px box with one line) is not split either way and is lost, with a warning (F213d-b).

## Consequences

- Every word is printed once, at the fragmenter's position. Chrome prints a line twice (cut in half) at each page
  edge, which is not what a reader or `pdftotext` wants either.
- At most one line per page is placed differently from Chrome, and where the box is not first on its page we start it
  where it is (below the heading, like a paragraph), while Chrome starts it on the next page: A3 has 12 pages here, 13
  in Chrome. A box that starts a page has Chrome's page count (A1, A2: 12; A2c: 20).
- It is the same mechanism as for clipping wrappers, so there is one place that decides line ranges.
- (F30e) An `inline-table` alone on its line has Chrome's page count where it starts a page (3), and one page fewer
  than Chrome where something precedes it on its page (3 against 4 for a bordered frame or an `<h1>` before it).
- The frame of a split box (border, radius) is closed on every page it touches (F96), where Chrome slices it.
- If a product ever needs Chrome's exact slicing (a monolithic box cut at the page edge, a line printed on both pages),
  it is a new decision with its own renderer work; nothing here prevents it.
