# 10 — stacked items (table parts with a non-table `display`)

Invoice with the "stacked on mobile" template idiom (`block md:table-cell`, `flex justify-between`) as it prints on A4,
where the `md:` variant does not match: items are `tr.block` cards or `td.block` cells with a `td.flex justify-between`
line (quantity and price), and the totals are `td.flex` rows. 12 items, then the totals.

Exercises:
- a table part whose computed `display` is not a table display is wrapped in an anonymous table row/cell
  (CSS 2.1 §17.2.1) and its content is laid out: before F50 the items and totals were not painted at all and the rows
  collapsed
- consecutive `td.block` / `td.flex` cells share one anonymous cell, so they stack
- `tr.block` is a block box inside an anonymous cell
- the page break between the cards, with the header repeated
- the same header and party grid as 06 around the table

Only classes present in `fixtures/tailwind/tailwind.css` are used (`block`, `flex`, `justify-between`, `py-2`, `pt-2`, `pb-3`,
`border-b`, `border-t`, `text-xs`, `text-lg`, `tabular-nums`, ...).
