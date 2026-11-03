# 43 — a payment sentence with a tall `inline-block` (QR box) at the bottom of page 1: the whole line moves to page 2

Two-page Czech invoice. Page 1 is the header, the two-column party grid and an items table of sixteen rows (the first sixteen of
41), then "Celkem k úhradě 251 200,00 Kč". Then comes `<section class="mt-6 text-sm"><p>Zaplaťte QR kódem [box] nebo převodem na účet
123456789/0300.</p></section>` and `<p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
The box is `<span class="inline-block border border-gray-300 rounded-lg p-4">` holding three block spans: the label `QR Platba`
(`text-xs … uppercase`), the QR `<svg class="block w-28 h-28">` (the path of 03) and the amount `251 200,00 CZK` (`text-xs font-mono`).
The sentence is ONE line, about 530 px wide; the box is 186 px tall (2 + 32 padding and border, 20 + 112 + 20 of content).

The table is sized (sixteen rows) so that the paragraph starts about 100 px above the bottom of the content box in Chrome (727.2 pt for
the line with a 16 px stand-in for the box) and about 97 px in ours (729.8 pt). That room is above the box's first row (the
label, about 33 px with the padding) and below its height (186 px): a box that fits as a whole is not this case, and neither is one
whose first row does not fit. There is no `align-top`: ours seats every inline box bottom-on-baseline (F183c), so a top-aligned box would add
a misplaced `Zaplaťte` to the diff; the bare `vertical-align: top` shape is in `tall_inline_box_line.rs`.

Exercises:
- css-break-3 §4.1: there is no break point inside a line box, and an `inline-block` may be monolithic. The line holds a box that does not
  fit the room, so the break is the one before the paragraph (a Class A break between block boxes) and the whole line, text and box,
  goes to the next page. CSS 2.1 §13.3.3 says the same. Chrome: page 1 ends with "Kč" of the total, page 2 starts with the box
  (top at 42.6 pt), "Zaplaťte" and "převodem" at 153.4 pt (the baseline of the box is its last line, the amount, 157.3 pt), then "Děkujeme" at
  198.0 pt, 2 pages.

**On the binary before the change (F213, main 58f8992):** the fragmenter entered the box (its first row, the label, fits the 100 px),
left the line on page 1 and cut the box in front of the `<svg>`: the label stayed on page 1, the QR and the amount went to page 2. The line's baseline,
at the bottom of the box, is painted 25 pt below the page edge, so "Zaplaťte QR kódem" and "nebo převodem na účet 123456789/0300." are printed
on no page (the renderer warns "content extends 24.9 pt past the bottom page edge on page 1 and is not printed"). 2 pages like Chrome; pixel diff
3.8 / 1.3 % (worst page 3.8 %), low because the page is almost empty and the text that is missing is one line.

The test is `qr_inline_block_invoice_moves_the_whole_payment_line_to_page_two` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`tall_inline_box_line.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`. Plan:
`docs/plans/2026-11-03-f213-text-beside-tall-inline-block.md`.
