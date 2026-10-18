# 13 — terms kept together

Czech invoice whose "Obchodní podmínky" are one body-level `<p class="text-xs text-gray-600 break-inside-avoid">` of 37 clauses
(1167 words, 74 lines of 16 px), about 1.3 pages long, below the 06 header, party grid, items table and total.

Exercises:
- `break-inside: avoid` on a paragraph that does not fit the rest of page 1 moves it whole to the top of page 2 (css-break-3
  §4.4, Chrome does the same)
- the paragraph is taller than a page, so `avoid` is relaxed and it breaks between its line boxes (F83): page 2 is filled to
  the content bottom, page 3 holds the last 9 lines, every line is printed once. **Before F83 (main) 9 of the 1167 words were
  printed on no page**: the strip slicer cut the paragraph at the page height and the line straddling the cut (the first line
  of page 3) was dropped (1158 of 1167 words; with F83: 1167 of 1167)
- `text-xs` has 16 px line boxes, which tile exactly (F78)

The property is the compiled Tailwind utility `break-inside-avoid`. Tailwind v4 puts it in `@layer utilities`, which fulgur's
break / column reader skipped before F87, so for a while this fixture set the property inline. Honest scope: with F83 in, an
`avoid` paragraph and an `auto` one are placed alike at body level (both move whole to the next page when they do not fit the
rest of the page, F30c), so this fixture renders the same PDF with the class, with the inline style and with neither on main
and on the F87 branch. It documents the invoice idiom; the tests that fail without F87 are in
`crates/htmltopdf/tests/tailwind_breaks.rs`.

Not exercised (F83a): `page-break-inside: avoid` (not aliased). An avoid box with several block children is
fixture 14 (F83b).
