# 45 — an invoice on pre-printed letterhead paper: the first block does not fit page 1, which starts 70 mm down, and moves whole to page 2

Two-page Czech invoice for paper that already carries the sender's logo and address: `@page { size: A4; margin: 15mm }` and
`body { margin-top: 70mm }` (in the fixture's own `<style>`; every Tailwind class is in the committed `tailwind.css`, there is no
`mt-[…]`). The first and only block of the body is `<main class="inline-block w-full break-inside-avoid">` holding the header
(as in 13), the party grid, an items table of fourteen rows (`w-full border-collapse text-sm`, `border-b border-gray-200` rows,
`py-2` cells, `text-right tabular-nums` amounts) and the totals (`flex justify-end`, `w-72`). After it `<p class="mt-6 text-xs
text-gray-500">Děkujeme za spolupráci.</p>`.

`main` is 843 px tall. Page 1 has 1009.1 px of content box minus 264.6 px (70 mm) = 744.5 px of room: the box does not fit page 1 and
does fit a fresh page. The table is sized (fourteen rows) for that: between 744.6 and 1009 px.

Exercises:
- css-break-3 §4.1 / §4.4 and CSS 2.1 §13.3.3: an `inline-block` is monolithic (no break point inside it), so when it does not fit the room
  below the page's offset the break in front of it is taken, and the whole box goes to page 2. §5.2: the 70 mm at the top of page 1 is
  kept (the page is empty, not collapsed); on page 2 the box starts at the content top. Chrome (m153): page 1 empty, "Faktura" at
  43.3 pt, "Děkujeme" at 693.3 pt, 2 pages.
- The break in front of the box is a break between the *first* box of the body and nothing: `cursor_y` is 0 there whatever the offset,
  which is why the fragmenter did not take it (F213e, plan `docs/plans/2026-11-03-f213e-first-block-below-top-margin.md`).

**Why `inline-block`.** A plain `<main class="break-inside-avoid">` is not kept together by Chrome in this position: an avoid box with
nothing before it has no break point in front of it, so Chrome ignores the avoid and splits it in place (page 1 holds the header and
13 rows, 207 words; measured with the same HTML minus the two classes; the same for `flex`, `grid`, a table and nested blocks).
Only a monolithic box is moved. Ours already split such a box like Chrome (guards in `crates/htmltopdf/tests/body_offset.rs`).
A blank first page is Chrome's output here and is better than the invoice printed over the letterhead.

**On the binary before the change (F213e, main efdd02a):** the fragmenter entered the box (its first row fits) and split it like the
plain box: page 1 has the header, the parties and 13 rows ("Faktura" at 241.8 pt), page 2 the 14th row, the totals and the thank-you line. 2 pages; pixel diff 4.3 / 5.5 % (worst page 5.5 %).

The test is `letterhead_invoice_moves_whole_to_page_two` in `crates/htmltopdf/tests/fixtures.rs`.

**After F213e:** page 1 is empty and the whole invoice is on page 2 with the thank-you line, like Chrome: 0.0 / 9.2 % (worst page 9.2 %, threshold 10.2 %).
Page 1 went from 4.3 % to 0; page 2 from 5.5 % to 9.2 %, because the invoice that used to be split is now complete on page 2 and ours paints its box 5.8 pt
(7.7 px) lower than Chrome: it seats an inline box bottom-on-baseline and Blitz aligns the box's last line there (F183c, not part of F213e; "Faktura" at 49.1 pt,
Chrome 43.3 pt, while "Děkujeme" is at 693.1 pt, Chrome 693.3 pt). `align-top` does not change ours, and `overflow-hidden` (which gives Chrome a
bottom-edge baseline) makes the box a clip box that ours then cuts differently, so neither is used.
