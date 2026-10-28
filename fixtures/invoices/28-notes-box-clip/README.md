# 28 — a note longer than the box the template reserves for it (`h-28 overflow-hidden`)

One-page Czech invoice (the shape of 01: header, parties, four items, totals) with a note box as a **direct child of `<body>`**:
`<div class="h-28 overflow-hidden rounded-lg border border-gray-300 p-4 text-sm">Poznámka: …</div>`. The template reserves
112 px for the note; an ERP pasted in about 28 lines of direct text. The box is inside the page, but its hidden lines would run
past the page bottom.

- Chrome: one page, the first lines of the note and the 5th cut by the box edge, then the rounded border. 184 words in the text
  layer (`reference/chrome.pdf`): nothing of the hidden lines.
- Main's binary: the body-level line path split the box's lines at the page bottom, so the box started page 2 and showed all of
  the note (2 pages, 491 words), and the clip, cut at each page's own fragment, hid nothing.
- With F30f-b: the box fits the room, so it is placed whole at 112 px; the clip hides the lines below it, and the writer leaves
  them out of the PDF (ADR-0009): one page, 184 words, as Chrome.

The test is `crates/htmltopdf/tests/clip_boxes.rs` (`fixture_28_*`). Plan: `docs/plans/2026-10-28-f30f-b-clip-boxes.md`.
