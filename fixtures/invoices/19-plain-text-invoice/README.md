# 19 — a plain-text invoice: text and `<br>` directly in `<body>`, 2 pages

A Czech "faktura – daňový doklad" written the way e-mail and ERP templates write it: **no block element at all**, only text, `<br>`
and `<b>` headings straight in `<body>` (85 lines of 20 px: header dates, supplier, customer, 18 item lines, VAT summary, the total,
IBAN / VS payment details, 15 numbered terms). About 1.7 pages: page 1 ends with the total, page 2 starts with `PLATEBNÍ ÚDAJE`
(Chrome: the same).

Exercises (F111, CSS 2.1 §9.4.2 and §13.3.3, css-break-3 §4.1):
- `<body>` whose in-flow children are all inline-level is itself the inline formatting root; the fragmenter splits it between its
  line boxes like any paragraph taller than a page. **On main's binary this printed 1 page with the first 50 lines and no warning**:
  the payment details and the terms were lost.
- `style="line-height:20px"` on `<body>` keeps the lines on the 20 px grid (no F81 drift), so the page break is the same as Chrome's.
- `@page { size: A4; margin: 15mm }`.

The remaining diff (3.1 / 2.8 %) is the glyph-edge drift every fixture has. Every class is in the compiled
`fixtures/tailwind/tailwind.css` (`font-sans`, `text-[10pt]`, `text-gray-900`), no rebuild. The tests that pin the behaviour without
Tailwind are `crates/htmltopdf/tests/body_inline_split.rs` and `page_bottom_warning.rs`.
