# 14 — totals, payment details and signature kept together

Czech invoice with an items table of eight rows of two to three lines (fixed `w-20` / `w-32` / `w-28` number columns, so
the column widths agree with Chrome's), then one `<div class="break-inside-avoid mt-6">` holding the totals table (Základ
daně, DPH 21 %, Celkem k úhradě), the payment details (account, IBAN, variable symbol, due date) and the signature block
(Vystavil, razítko a podpis): about 300 px together. The table ends about 170 px above the page-1 content bottom, so the
wrapper does not fit the rest of page 1 and fits a page.

Exercises:
- `break-inside: avoid` on a block wrapper with several children moves the wrapper whole to the top of page 2 (F83b,
  css-break-3 §4.5: a break before the box is preferred to one inside it; Chrome does the same). **Before F83b (main) the
  fragmenter entered the wrapper because its first row fit, so page 1 ended with the totals table under the items and the
  payment details and signature started page 2.** Compare numbers: main 7.8 / 1.9 %, F83b 7.5 / 1.6 %
- the margin above the moved wrapper (`mt-6`) is truncated at the page top (§5.2): the totals start at the content top

The wrapper is the Tailwind class `break-inside-avoid` (F87 made it reach the break table). The tests that pin the
shapes are in `crates/htmltopdf/tests/avoid_wrappers.rs`.

Not exercised: `tr { break-inside: avoid }` rows (they have no box of their own in the fragmenter, and Chrome keeps
them whole too; a row whose cell is entered, F83d), avoid on flex / grid items (F83d). Chrome paints a rule above the
total (`border-t-2` on the `<tr>`); ours does not (collapsed `<tr>` borders, Blitz #386).
