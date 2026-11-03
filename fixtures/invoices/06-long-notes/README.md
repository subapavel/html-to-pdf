# 06 — long notes

Two-page Czech invoice whose "Obchodní podmínky" box (`rounded-lg border p-4`) holds one paragraph
taller than a page, followed by a bulleted `<ul>`.

Exercises:
- a paragraph inside a block that is taller than a page splits at line boxes (F30): page 1 is filled
  to the content bottom, the box continues on page 2, no line is lost or printed twice
- the box is sliced at the break (F96, css-break-3 §5.3 / §5.4, `box-decoration-break: slice`): page 1 has the top border and radius, no bottom border, the sides run to the content bottom; page 2 has no top border and radius, the sides start at the content top, the bottom border and radius follow the last line. Compare p1 10.7 -> 10.6 %, p2 11.8 % unchanged (dense text drift)
- `<ul>` with the default outside `list-style: disc` (set inline: the compiled Tailwind has no `list-*` utilities, preflight resets `ul` to `list-style: none`), one bullet per item, left of the text (F33)
- a small invoice header, party grid, items table and total above the box, as in the other fixtures
