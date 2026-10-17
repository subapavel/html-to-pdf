# 13 — terms kept together

Czech invoice whose "Obchodní podmínky" are one body-level `<p class="text-xs" style="break-inside:avoid">` of 37 clauses
(1167 words, 74 lines of 16 px), about 1.3 pages long, below the 06 header, party grid, items table and total.

Exercises:
- `break-inside: avoid` on a paragraph that does not fit the rest of page 1 moves it whole to the top of page 2 (css-break-3
  §4.4, Chrome does the same)
- the paragraph is taller than a page, so `avoid` is relaxed and it breaks between its line boxes (F83): page 2 is filled to
  the content bottom, page 3 holds the last 9 lines, every line is printed once. **Before F83 (main) 9 of the 1167 words were
  printed on no page**: the strip slicer cut the paragraph at the page height and the line straddling the cut (the first line
  of page 3) was dropped (1158 of 1167 words; with F83: 1167 of 1167)
- `text-xs` has 16 px line boxes, which tile exactly (F78)

The property is set inline because the compiled Tailwind `break-inside-avoid` is never read by fulgur (F87: the utility sits in
`@layer utilities`, which the break / column reader skips); with the class, main and the branch render the same pages and the
fixture would show nothing. Switch it back to the class when F87 is fixed.

Not exercised (F83a, F83b): `page-break-inside: avoid` (not aliased), an avoid box with several block children
(the recursion gate ignores `break-inside`).
