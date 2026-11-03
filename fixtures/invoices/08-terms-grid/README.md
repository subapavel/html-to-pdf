# 08 — terms grid

Two-page Czech invoice whose "Obchodní podmínky" card is a `grid gap-4 rounded-lg border p-4` with a heading and one
paragraph taller than a page, followed by a `flex items-start gap-4` footer (icon + short paragraph).

Exercises:
- a paragraph that is itself a grid item and is taller than a page splits at line boxes (F30b): page 1 is filled to the
  content bottom, the card continues on page 2, no line is lost or printed twice (before F30b: 3 pages, the paragraph cut after clause 21)
- the card's border and padding close below the last line on page 2 (a one-column grid is block flow for fragmentation); the card is sliced at the break like Chrome's (F96, css-break-3 §5.3 / §5.4): open bottom edge on page 1, open top edge on page 2. The compare numbers (8.2 / 12.1 %) did not move: the frame lines are light grey, below the metric's 32/255 tolerance, and the PNG shows the difference
- a flex row (icon + paragraph) after the split card starts below it
- the same header, party grid, items table and total as 06 above the card
