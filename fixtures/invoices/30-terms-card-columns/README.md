# 30 — terms in one bordered card, in two columns, across the column and the page break

A two-page Czech invoice (the header, parties, a short items table and the total of fixture 24) whose "Obchodní podmínky" are a
`<section class="terms text-xs">` with `columns: 2; column-gap: 24px` holding **one** card:

```html
<div class="card">  <!-- padding: 12px; border: 1px solid #d1d5db; border-radius: 6px; background: #f9fafb -->
  <h3>1. Předmět plnění</h3><p>…</p>  … 20 clauses …
</div>
```

and a signature `<p>` below the section. The card is taller than a page in a single column (about 2300 px at the column width), so
it starts in column 1 of page 1, goes on in column 2, then in both columns of page 2, where the two columns are balanced and the
card closes above the signature. The shape of an ERP template that wraps its terms in a framed `<div>` inside a multi-column block (F198).

Exercises (css-break-3 §4.1 / §4.4 breaks between line boxes inside a box that does not fit, §5.4 `box-decoration-break: slice`;
css-multicol-1 §7.1 / §8.2 columns that do not fit the page continue on the next, the last fragment is balanced):
- every clause is printed once, in order, the signature after the last one,
- the card's background and side borders are in both columns of both pages; the top border and the radius appear at its start (page 1,
  column 1) only, the bottom border and radius at its end (page 2, column 2) only; at a break edge the card has no border, no padding and no radius,
- the text is 17 px (border and padding) inside the frame, and starts at the top of every column that continues the card,
- 2 pages like Chrome's.

Not exercised: margins inside the columns (F30d-b: the clauses have none), `orphans` / `widows` (set to 1 in the fixture: fulgur does not implement
them in columns), a table inside the card (F174c), a card that fits a page (it stays whole in one column, F30d-c).

**On main's binary** the card is one box placed whole in column 1: its text runs from the top of page 1 to far below the page edge (nothing below the
content box is printed), column 2 is empty, and the signature follows on page 2. Numbers: README of the F198 plan §7.

Everything multicol is in the fixture's `<style>`; `fixtures/tailwind/tailwind.css` is not rebuilt. The tests that pin the behaviour without a fixture
are in `crates/htmltopdf/tests/multicol_boxes.rs`; `fixtures.rs` has `terms_card_columns_invoice_prints_every_clause`.
Plan: `docs/plans/2026-10-29-f198-multicol-padded-loss.md`.
