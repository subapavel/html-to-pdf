# 32 — a bordered note card holding its text directly, on one page

One-page Czech invoice (the header, party grid and items table of 29, one more row), "Celkem k úhradě", then a card that is **not** a
clip box, has a border, a background, `rounded-lg` and `p-4`, and has no paragraph inside it: a heading span, then four notes
separated by `<br>`, two of them wrapping, nine lines in all:

```html
<div class="rounded-lg border border-gray-300 bg-gray-50 p-4 mb-6 text-sm"><span class="font-semibold">Poznámka k faktuře</span><br>Fakturujeme dle … <br>…</div>
```

and the "Platební údaje" section below. The card fits its page. It is the fitting counterpart of 29 (F30f-d, a card that spans the
break): an ERP template that pours a note into a bordered `<div>` (F30f-h).

Exercises:
- CSS 2.1 §10.6.3 / §8.1: the card's box is as tall as its lines **plus the padding and border below them**, so "Platební údaje"
  starts below the card's bottom border, with the card's `mb-6` between (Chrome: heading at 608 px).
- 1 page like Chrome's.

**On main's binary:** the line path placed what follows at the end of the card's lines, 32 px (padding 16 + border 1, doubled
by the top and bottom insets) too high: the heading "PLATEBNÍ ÚDAJE" was printed over the card's bottom border and its last line,
and the bullets followed 28 px too high (5.0 % against 3.8 % now; the rest is the text drift every fixture has).

The tests are in `crates/htmltopdf/tests/padded_paragraph_fit.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`.
Plan: `docs/plans/2026-10-29-f30f-h-padded-paragraph-fit.md`.
