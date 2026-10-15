# 02 — long (multi-page)

60 line items → 3 pages. This is the fixture that separates "renders HTML" from "makes invoices".

Exercises:
- table fragmentation across pages
- `<thead>` repeated on every page (`display: table-header-group`)
- `break-inside: avoid` on rows and on the totals block (totals must never split from the last row)
- `@page` margin boxes: `@bottom-right` "Strana X / Y" with `counter(pages)`, `@bottom-left` running text
- non-breaking spaces (U+00A0) as thousands separators in `tabular-nums` cells

Regenerate: `python3 fixtures/invoices/02-long/generate.py` (seeded, deterministic).
