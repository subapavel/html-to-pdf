# 06 — long notes

Two-page Czech invoice whose "Obchodní podmínky" box (`rounded-lg border p-4`) holds one paragraph
taller than a page, followed by a bulleted `<ul>`.

Exercises:
- a paragraph inside a block that is taller than a page splits at line boxes (F30): page 1 is filled
  to the content bottom, the box continues on page 2, no line is lost or printed twice
- the box's border and padding follow the text on the page it ends on
- `<ul>` with `list-style: disc inside` (the compiled Tailwind has no `list-*` utilities; outside disc markers are not painted yet, so the bullets are inside) and one bullet per item
- a small invoice header, party grid, items table and total above the box, as in the other fixtures
