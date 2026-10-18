# 16 — terms in an `overflow-hidden rounded-lg border` card whose paragraph crosses the page break

Czech invoice of 3 item rows (the header, party grid and items table of 06), "Celkem k úhradě", then a card:

```html
<section class="overflow-hidden rounded-lg border border-gray-300 mb-6 text-sm">
  <h2 class="bg-gray-50 border-b border-gray-300 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-gray-500">Obchodní podmínky</h2>
  <p class="p-4">… 30 articles …</p>
</section>
```

and a "Platební údaje" section (účet, IBAN, splatnost) with a signature line. `overflow-hidden` is there for the usual reason: the
title bar's background must not poke out of the rounded corners. The paragraph is the **direct child** of the clip box, which
is the F30f shape; it starts on page 1 (articles 1-13) and ends on page 2 (14-30).

Exercises:
- a paragraph taller than the room below it in a clipping box whose content ends inside it: the box is fragmented between line
  boxes and the clip is applied to each fragment (F30f; css-break-3 §4.1, §4.4: only an `overflow: hidden` box with a definite
  block size may be treated as monolithic; css-overflow-3 §3: a fragmented box clips each fragment). **On main's binary the
  paragraph was placed whole: its last lines (the warning says 307 px below the content box) were not printed, the payment block
  moved to a third page.** Compare numbers: main's binary 3 pages, 11.4 / 25.0 / 0.9 %; F30f 2 pages like Chrome's, 16.6 / 20.8 %.

The remaining diff is dense text, not lost content: every word is on the same page as in Chrome. Ours closes the card's frame on
every page (top border and 16 px of padding again on page 2, `box-decoration-break: slice` is not implemented, F96), so page 2's
lines sit about 20 px lower than Chrome's; page 1 has the text drift every fixture has (rows about 3 px lower). The threshold
(`scripts/compare.py`) is the worst page + 1 point and goes down with F96.

Every class is in the compiled `fixtures/tailwind/tailwind.css` (no inline styles besides the list style 06 uses, no rebuild).
The tests that pin the shape without Tailwind are in `crates/htmltopdf/tests/wrapper_line_split.rs`.
