# 12 — terms in two columns

Two-page Czech invoice whose "Obchodní podmínky" block is a `<section class="gap-8 text-xs" style="columns:2">` with 27
clauses, each an `<h3>` and a `<p>` (the columns are set inline: the compiled Tailwind has no `columns-2`, as for `ul`
in 06). The block is about 1.3 pages tall and starts below the items table and the total.

Exercises:
- a multi-column container taller than the room under the table is cut over pages (F30d): the clauses start on page 1
  directly below the total, both columns are filled to the bottom of page 1, continue in both columns of page 2, and the
  last page is balanced (about 7 lines apart at most), like Chrome. Before F30d the block printed no text at all
  (every column was one slice taller than a page and the renderer skipped it)
- a heading is a box of its own in the column flow (placed whole, may end a column), a paragraph is cut between its
  lines at the column bottom (F30d, `plan_multicol_pages`)
- the header, party grid, items table and total above the block are those of 06 and 08

Not exercised (F30d-b): space between the clauses (child margins and padding inside columns are ignored, so there is
none, in Chrome's rendering too: the fixture sets none), `column-rule`, `column-span: all`, `orphans` / `widows`.
