# 06 — long notes

Two-page Czech invoice whose "Obchodní podmínky" box (`rounded-lg border p-4`) holds one paragraph
taller than a page, followed by a bulleted `<ul>`.

Exercises:
- a paragraph inside a block that is taller than a page splits at line boxes (F30): page 1 is filled
  to the content bottom, the box continues on page 2, no line is lost or printed twice
- the box's border and padding follow the text on the page it ends on
- `<ul>` with the default outside `list-style: disc` (set inline: the compiled Tailwind has no `list-*` utilities, preflight resets `ul` to `list-style: none`), one bullet per item, left of the text (F33)
- a small invoice header, party grid, items table and total above the box, as in the other fixtures
