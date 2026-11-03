# 42 — a bordered card of clauses, with a nested box, split between its children (F96)

Two-page Czech invoice. Page 1 is the header, party grid, an items table of two rows and "Celkem k úhradě" (as in 06). Then
`<section class="rounded-lg border border-gray-300 bg-gray-50 p-4 mb-6 text-sm">` holds an `<h2>`, clauses 1 to 8 (each a
`<p class="mb-2 break-inside-avoid">` of 2 to 3 lines), a nested `<div class="rounded-md border border-gray-200 bg-white p-4
mb-2">` with its own `<h3>` and clauses 9 to 14, and clauses 15 to 21. After the card come 16's "Platební údaje" list and the
signature line. Clause 9 is the last one on page 1, clause 10 heads page 2: the page break falls between two clauses of the
nested box, so **both boxes are split between whole children** (no line is cut). Every class is in the compiled
`fixtures/tailwind/tailwind.css`.

Exercises:
- css-break-3 §5.4 `box-decoration-break: slice` (the initial value): at a break the box has no border and no padding. The top
  border and the rounded top corners are on the first fragment only (page 1), the bottom border and the rounded bottom corners
  on the last only (page 2). The corners at a cut are square. The grey and the white backgrounds belong to the unbroken box and
  are sliced with it.
- css-break-3 §5.3: a box that breaks fills the fragmentainer. The side borders and backgrounds of page 1 run to the content
  bottom (row 1065 of 1065.8 at 96 dpi) although clause 9 ends about 65 px higher (its last line is at 998 px); page 2's start at the content top (row 57), where
  clause 10 begins (43.8 pt).
- what follows a box split between children starts below the bottom padding, border and margin of its last child: Chrome puts
  the signature line ("jednatelka") at 693.3 pt on page 2.

**Before F96** (main's binary): both frames were closed on both pages (bottom border and radius under clause 9 and the nested
box, top border and radius over clause 10), the backgrounds stopped at the last clause, and the payment block stood 43.7 pt too
high (649.6 pt) because a box split only between whole children ended its last frame at its last child. Compare: 5.7 / 12.1 %.

The test is `terms_card_clauses_invoice_slices_both_cards` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`block_box_slice.rs`. Plan: `docs/plans/2026-11-02-f96-block-box-decoration-slice.md`.
