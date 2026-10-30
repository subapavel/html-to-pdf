# 33 — address blocks and a note card that end in a forced break, on one page

One-page Czech invoice (fixture 32's header, items table and payment block) whose blocks end in a forced break:

- the supplier's address is a `<div style="white-space: pre-line">` of three lines with indentation, ending in a newline
  before `</div>` (the shape of an ERP template that pours a text column into a block);
- the customer's address is three lines each terminated by `<br>`, the last one too;
- the note card (`rounded-lg border bg-gray-50 p-4 mb-6`, text directly in it, five notes) ends in `<br>`.

`whitespace-pre-line` is not in the compiled `tailwind.css`, so the style is inline (as fixture 22's `.note`).

Exercises:
- CSS 2.1 §9.4.2 / CSS Text 3: a forced break ends its line and creates no line of its own when nothing follows it in the
  block. Chrome: the party grid is three lines, the items table starts right below it, "Celkem k úhradě" at 325.9 pt, the
  note heading at 376.8 pt, "PLATEBNÍ ÚDAJE" at 526.8 pt.
- 1 page like Chrome's.

**On main's binary:** Parley's empty last line after the final newline / `<br>` made the party grid one line (15 pt) too
tall and the card one line too tall: "Celkem" at 340.7 pt, the note at 391.6 pt, "PLATEBNÍ" at 556.6 pt (Chrome 325.9 / 376.8 / 526.8), the whole page below the addresses shifted. 7.8 % on main against 3.8 % now; the rest is the text drift every fixture has.

The tests are in `crates/htmltopdf/tests/trailing_break.rs` and `fixtures.rs`
(`address_trailing_break_invoice_is_laid_out_like_chrome`). Plan: `docs/plans/2026-10-29-f66f-trailing-forced-break.md`.
