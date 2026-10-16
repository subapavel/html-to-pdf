# 08 — terms grid

Two-page Czech invoice whose "Obchodní podmínky" card is a `grid gap-4 rounded-lg border p-4` with a heading and one
paragraph taller than a page, followed by a `flex items-start gap-4` footer (icon + short paragraph).

Exercises:
- a paragraph that is itself a grid item and is taller than a page splits at line boxes (F30b): page 1 is filled to the
  content bottom, the card continues on page 2, no line is lost or printed twice (before F30b: 1 page, tail lost)
- the card's border and padding close below the last line on page 2 (a one-column grid is block flow for fragmentation)
- a flex row (icon + paragraph) after the split card starts below it
- the same header, party grid, items table and total as 06 above the card
