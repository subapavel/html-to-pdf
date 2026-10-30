# 36 — terms as one body-level paragraph below a heading, starting where two lines fit

Two-page Czech invoice. Page 1 is the header, party grid and an items table of twelve rows (fixture 34's, with more rows), "Celkem k
úhradě", then `<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>` and one body-level `<p class="text-xs text-gray-600
leading-relaxed">` holding fifty numbered clauses as running text (about 450 words, 32 lines of 19.5 px: fits a page, not the
room left on page 1), then `<p class="mt-6 text-sm">Převzal: …</p>`. The heading starts 213 pt above the content bottom.

Exercises:
- css-break-3 §4.1 / §4.4: a break between line boxes is as good a break opportunity as one between blocks; a paragraph may start
  on a page when its first two lines (`orphans`) fit and two lines (`widows`) can be left for the next page. Chrome: the heading and
  the first ten lines (clauses 1-22) on page 1, "údržby" starts page 2, "50." at 278.5 pt, "Převzal:" at 325.1 pt, 2 pages.
- Chrome does not keep a heading with the lines below it (it leaves an `h2` alone at the page bottom when fewer than two lines fit,
  plan §8); the heading here has room for ten lines, so it is not what this fixture shows.

**On main's binary:** the pre-advance of the line path moved the whole paragraph to page 2 because it did not fit the room, leaving
the heading alone at the bottom of page 1 and a third of the page empty ("50." at 468.8 pt, Chrome 278.5 pt).

The test is `terms_after_items_invoice_starts_the_terms_below_their_heading` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes
are in `pre_advance_split.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`. Plan:
`docs/plans/2026-10-30-f30c-pre-advance-split.md`.
