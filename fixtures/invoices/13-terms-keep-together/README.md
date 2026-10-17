# 13 — terms kept together

Czech invoice whose "Obchodní podmínky" are one body-level `<p class="break-inside-avoid text-xs">` (Tailwind
`break-inside-avoid`) of 37 clauses, about 1.3 pages long, below the 06 header, party grid, items table and total.

Exercises:
- `break-inside: avoid` on a paragraph that does not fit the rest of page 1 moves it whole to the top of page 2 (css-break-3
  §4.4, Chrome does the same)
- the paragraph is taller than a page, so `avoid` is relaxed and it breaks between its line boxes (F83): page 2 is filled to
  the content bottom, page 3 holds the rest, every line is printed once. Before F83 the strip slicer cut it at fixed pixel
  heights and the line that straddled each cut was printed on no page (up to 1 line per break, plus the last word)
- `text-xs` has 16 px line boxes, which tile exactly (F78)

Not exercised (F83a, F83b): `page-break-inside: avoid` (not aliased), an avoid box with several block children
(the recursion gate ignores `break-inside`).
