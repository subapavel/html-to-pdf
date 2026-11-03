# 41 — a heading with `break-after: avoid` before the terms: it moves to page 2 with its paragraph

Two-page Czech invoice. Page 1 is the header, party grid and an items table of seventeen rows, "Celkem k úhradě", then
`<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>` and ONE body-level paragraph `<p class="text-xs text-gray-600
leading-relaxed">` of clauses 1-40 (about 24 lines of 19.5 px, the initial `orphans: 2; widows: 2`), then `<p class="mt-6 text-sm">Převzal: …</p>`.
The inline style sheet holds `@media print { h2, h3 { break-after: avoid; } }`, the usual line of a print sheet (the two `h2` of the party
grid are at the page top and the rule does nothing there).

The items table is sized (seventeen rows and a little extra margin on the header and the table) so that the heading block (20 px line + 8 px margin) ends
22 pt (29 px, 1.5 lines of the terms) above the bottom of the content box: the paragraph cannot start on page 1 (two lines are
needed), and the heading would be left alone at the bottom of the page. The room is 9.75 px from the one-line and the two-line boundary,
clear of the 1 px difference between our page capacity and Chrome's (F30f-c). The plan said nine rows, as in 38; nine rows leave the
page half empty, so this one has seventeen.

Exercises:
- css-break-3 §3.1, §4.4 Rule 1: `break-after: avoid` forbids the break between the heading and the paragraph. The break before the
  heading (between the total and the heading, §4.1) is allowed and heading + paragraph fit a page, so the pair moves together; no
  rule is relaxed. Chrome: page 1 ends with the total ("Kč" at 719.7 pt), page 2 starts with "Obchodní podmínky" at 43.8 pt, "40." at 402.3 pt and
  "Převzal:" at 448.8 pt, 2 pages.
- The twin without the `@media print` line leaves the heading alone at the bottom of page 1 and starts page 2 with clause 1, in Chrome and in ours
  ("Obchodní" at 757.8 pt). That is why a heading needs the declaration: Chrome's UA sheet has no `h2 { break-after: avoid }`.
- The declaration comes from a `<style>` block inside `@media print`; the fork's reader (`column_css`) sees it as the author's rule.

**On the binary before the change (F215, F87c):** `avoid` was read as `auto`, so ours left the heading at the bottom of page 1 ("Obchodní" at 757.6 pt,
page 2 starting with "1.") and the terms on page 2 sat 21 pt higher than Chrome's: 3.9 / 13.8 % (worst page 13.8 %).

The test is `terms_heading_keep_invoice_moves_the_heading_with_its_paragraph` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`keep_with_next.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`. Plan: `docs/plans/2026-11-02-f215-keep-with-next.md`.
