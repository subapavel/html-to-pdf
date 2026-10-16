# 09 — fixed row height

Three-page Czech invoice with the "uniform row height" template idiom: every `td` of the items table has the Tailwind
`h-12` class (48 px under preflight's `border-box`), 35 rows, four of them with a description that wraps to 3 lines
(more than 48 px with `py-2`), then the total.

Exercises:
- a cell's length `height` is a minimum for its row (CSS 2.1 §17.5.3), not a clamp: the wrapped rows grow to their
  content and the following rows move down (before F48 they overlapped the next row and the table was 2 pages instead
  of Chrome's 3)
- the rows split across pages (rows 1-15, 16-33, 34-35) exactly where Chrome splits them, with the header repeated
- the same header, party grid and total as 06 around the table

Only classes present in `fixtures/tailwind/tailwind.css` are used (`h-12`, `py-2`, `pr-2`, `px-2`, `pl-2`, `w-8`, `w-16`,
`w-24`, `w-28`, `tabular-nums`, `border-b`, ...).
