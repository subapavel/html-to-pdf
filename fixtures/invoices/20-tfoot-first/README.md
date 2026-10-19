# 20 — `<tfoot>` written before `<tbody>`: totals under the last item, 1 page

A one-page Czech invoice whose items table is written in the HTML 4.01 order (§11.2.3 required it, older ERP templates still emit it):
`<thead>`, then `<tfoot>` (Základ daně, DPH 21 %, the dark "Celkem k úhradě" row), then `<tbody>` with 8 items. Supplier, customer and
dates above, payment details below.

Exercises (F36, CSS 2.1 §17.2, css-display-3 §2.4):
- `table-footer-group` is "always displayed after all other rows and row groups"; `table-header-group` before them. The role comes from the
  computed `display`, not from where the element is written. Chrome prints the totals under "Cestovné – Trutnov, Praha", the last item.
  **On main's binary the totals were printed straight under the column headings, above the items** (12.4 % diff, 1 page; with the fix 8.9 %), and the 8 items
  moved down by the height of the footer.
- `bg-gray-900 text-white` on the footer's last `<tr>` (row background, F19) must follow the row to the bottom.
- Column widths come from the first row in visual order (the header), as in Chrome's fixed layout (probes fx1-fx4 of the plan), while the footer's
  rows use `colspan="4"` as in fixture 03.

Every class is in the compiled `fixtures/tailwind/tailwind.css` (the set of fixture 03 plus `text-2xl`), no rebuild. The tests that pin the behaviour
without Tailwind are `crates/htmltopdf/tests/tfoot_order.rs` and `fixtures.rs` (`tfoot_first_invoice_prints_the_totals_below_the_last_item`).

Compare: 8.9 % (1 page, Chrome 1). The rest is the text drift and rows about 0.9 px taller than Chrome's (collapsed `<tr>` borders, Blitz #386 / F22), 7 px at the last item.
