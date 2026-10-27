# 26 — "Zaplaceno" stamp on an invoice whose margin is body's, not the page's

One-page Czech invoice written the way a browser expects: `@page { size: A4; margin: 0 }` and `body { margin: 15mm }`. The stamp
`<div style="position:absolute; top:20mm; right:20mm; border:4px solid #16a34a; …">Zaplaceno</div>` is a direct child of
`<body>` with no positioned ancestor, so its containing block is the initial containing block (the page area, here the whole
page), not body: the stamp's right edge is 20 mm from the right edge of the paper and its top edge 20 mm from the top, whatever
body's margin is.

Exercises:
- CSS 2.1 §10.1 (4) / §10.3.7 / §10.6.4: an absolutely positioned box whose nearest positioned ancestor is none is placed against
  the initial containing block; body's margin, border and padding do not move it (F187, also F92)
- the same box in the old model: x was short by body's x offset (15 mm = 57 px), so the stamp sat 57 px too far left, 35 mm from the
  right edge of the paper instead of 20 mm (the page-1 y was right, the offset on y only shows on pages 2+)
- a normal-flow invoice around it (`grid`, a `<table>`, totals), so the rest of the page must not move

The position is an inline `style` because the compiled `fixtures/tailwind/tailwind.css` has no `absolute` / `top-*` / `right-*`
utilities and is not rebuilt (the rebuild needs npm). Unrotated: `transform` is not under test. The tests that pin the shapes
(A1-A11, with Chrome's numbers) are in `crates/htmltopdf/tests/abspos_body.rs`. Compare numbers are in `scripts/compare.py`.
