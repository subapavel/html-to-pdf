# 18 — an A5 receipt: `@page { size: A5; margin: 10mm }` with a footer string

A Czech "Zjednodušený daňový doklad" (simplified tax document, a cash receipt) on **A5**, one page: seller with IČO / DIČ, dates of
issue / DUZP, six item lines, a VAT summary for 12 % and 21 %, the total, "Zaplaceno platební kartou", and a footer printed by
`@bottom-center { content: "Děkujeme za nákup · www.example.cz" }`. A5 is what Czech shops print receipts and simplified tax
documents on, and it is the fixture that shows the page size coming from the document.

```css
@page {
  size: A5;
  margin: 10mm;
  @bottom-center { content: "Děkujeme za nákup · www.example.cz"; font-family: Inter, sans-serif; font-size: 8pt; color: #6b7280 }
}
```

Exercises (F6, `@page` half; css-page-3 "Page context" and the `size` property):
- the document's `@page { size }` decides the page: 419.53 x 595.28 pt (Chrome: 420 x 594.96 pt, both 560 x 794 px at 96 dpi).
  **On main's binary this printed on A4 with 15 mm margins** (the renderer forced both over the document).
- the document's `@page { margin: 10mm }` decides the content box: text starts 38 px from the left and top edge (Chrome: the same).
- a margin box on a non-A4 page: the footer sits in the bottom 38 px band of the A5 page.

The remaining diff (5.3 %) is the text drift every fixture has, the `border-b` rules Chrome paints on the item rows (`<tr>` borders,
Blitz #386 / F22) and the footer: Chrome centres it in the margin box, ours starts at the box's left edge and sits about 11 px higher
(centre margin box alignment, F110, and vertical centring, F103). The threshold in `scripts/compare.py` is the worst page + 1 point.

Every class is in the compiled `fixtures/tailwind/tailwind.css` (no inline styles, no rebuild). The tests that pin the sizes without
Tailwind are in `crates/htmltopdf/tests/page_from_css.rs`.
