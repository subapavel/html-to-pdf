# 48 — a "ZAPLACENO" stamp (`inline-block`, fixed size) under the title of a grid header, and a QR box (`inline-block`) in the payment sentence

One-page Czech invoice. The header is `<header class="grid grid-cols-2 gap-8 …">`: the left column is the supplier, the right column
has the title `<h1>Faktura 2026-0048</h1>`, a date line and, under them, `<p class="mt-2"><span class="razitko inline-block">…</span></p>`:
the stamp, 140 x 60 px with a 3 px `#cc0000` border, `padding-top: 12px` and `text-align: center`, holding two block spans, `ZAPLACENO`
(bold) and the date `4. 11. 2026`. `tailwind.css` has no `text-center` and no red border colour, so `.razitko` is written in the fixture's
`<style>` (no Tailwind rebuild); every other class is in the compiled sheet. Then an items table of fifteen rows, the total, the payment
sentence `Zaplaťte QR kódem [box] nebo převodem na účet 123456789/0300.` where the box is
`<span class="inline-block w-28 h-28 border border-gray-300 bg-gray-50 rounded-md text-xs">QR kód</span>` (112 px, one line of text at its
top, free space under it), and the thank-you line `Děkujeme za spolupráci.`

The header is a grid, not a flex row, on purpose: a fixed-size box in a flex row is F217. The rows are tuned (fifteen) so that the
thank-you line ends 13 px above the content bottom in Chrome (778.0 pt, content bottom 799.4 pt): whatever the box does wrong near the
page end is then visible in the bottom margin.

Exercises:
- CSS 2.1 §10.8.1: the baseline of an `inline-block` is the baseline of its last line box. The stamp's first line (`ZAPLACENO`) and the
  date are in the box; the baseline of the box is the date line's, so the box hangs below it and the line box is as tall as the
  box (CSS 2.1 §10.8: it reaches from the top of the highest box to the bottom of the lowest). The text of the sentence is on the
  baseline of the box's caption `QR kód`, near the top of the box, and the next block follows the box.
- Chrome m153 (the committed reference): `ZAPLACENO` at 100.1 pt, the sentence `Zaplaťte` at 675.3 pt level with the caption `QR` (second
  `QR`, 676.8 pt), `Děkujeme` at 778.0 pt, one page.

**On the binary before the change (main f550a8e):** ours seats an inline box bottom-on-baseline and fulgur then paints the box lower by the
free space under its last line (F183c). The stamp and its text are 9.5 pt below Chrome's (`ZAPLACENO` 109.6 pt); the QR box and the
sentence are 71 pt low (`Zaplaťte` 746.3 pt), the box (grey fill, 747.8 to 829.5 pt; Chrome 675.0 to 758.2 pt) hangs over the thank-you line (776.3 pt, 1.7 pt above Chrome's) and
30 pt into the bottom margin (the content bottom is 799.4 pt); the renderer says nothing. One page like Chrome. Pixel diff 4.25 %, threshold 5.3 %.

The test is `fixture_48_paid_stamp_and_qr_sit_where_chrome_puts_them` in `crates/htmltopdf/tests/inline_block_baseline.rs`
(`#[ignore]` until the fix). Plan: `docs/plans/2026-11-04-f183c-inline-block-baseline.md`.
