# 29 — a bordered card holding its text directly, across the page break

One-page-and-a-bit Czech invoice (the header, party grid and items table of 16), "Celkem k úhradě", then a card that is **not** a clip
box and has no paragraph inside it:

```html
<div class="rounded-lg border border-gray-300 p-4 mb-6 text-sm"><span class="font-semibold">1.</span> Tyto obchodní podmínky … 30 articles …</div>
```

and a "Platební údaje" section. The text is a direct child of the box, which has a border and padding and is taller than the room on
page 1: the shape of an ERP template that pours a note into a bordered `<div>` (F30f-d). It is the commoner and more visible
sibling of 16, which has `overflow-hidden` and a `<p>`.

Exercises:
- `box-decoration-break: slice` (css-break-3 §5.4, the initial value) for a body-level inline root with insets: page 1 has the
  card's top border and radius and its side borders down to the page bottom, no bottom border, no line in the bottom margin; page 2
  has the side borders from the content box top, **no top border and no padding at the top, the first line at the content box top**,
  the last line inside the frame, the bottom border with its radius and the payment block below with no overlap.
- 2 pages like Chrome's, 497 / 505 words.

**On main's binary:** the card moved whole to page 2 (84 words on page 1), its first line was `inset_top` lower than the line fit
assumed, the last line of page 2 was painted 5 pt below the content box, and the frame closed above it; 3 pages.

The tests are in `crates/htmltopdf/tests/padded_paragraph_split.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`.
Plan: `docs/plans/2026-10-29-f30f-d-padded-paragraph-split.md`.
