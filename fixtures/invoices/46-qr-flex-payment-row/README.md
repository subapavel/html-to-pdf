# 46 — a fixed 112 px `inline-block` (QR code, stamp box) in the text of a `flex` payment row: the row is as tall as the box

One-page Czech invoice: header, the two-column party grid, three items, the total, then two rows in `flex gap-4 text-sm`:

- **Platba:** a `w-32 shrink-0` label and an item whose text is `Zaplaťte QR kódem [box] nebo převodem na účet 123456789/0300.`.
  The box is `inline-block w-28 h-28 border border-gray-300 rounded-lg p-4` (112 px square, `border-box` from the preflight) holding a
  64 px block `<svg>` QR code (a hand-written finder-pattern path, no text). The content is smaller than the box on purpose: at 78 px
  and above the auto height would be 112 px and the row is right without the fix. The paragraph after the row is
  `Variabilní symbol 2026046, konstantní symbol 308.`.
- **Razítko:** the same row shape with `Převzal [box] dne 3. 11. 2026.`, the box empty (`inline-block w-28 h-28 border border-gray-300
  rounded-lg`), then `Děkujeme za spolupráci.`.

Every class is in the compiled `fixtures/tailwind/tailwind.css`; there is no `<style>` except `@page`.

Exercises:
- CSS 2.1 §10.3.9 and §10.6.6: a non-replaced `inline-block` uses its specified `width` and `height`; §10.8.1: the margin box of an
  `inline-block` is what the line box holds. css-flexbox-1 §9.4 step 7: a flex item is laid out as an in-flow block-level box, nothing in
  its descendants is suspended. Chrome (version of the committed references): both rows 112 px tall, "Variabilní" at 469.1 pt and
  "Děkujeme" at 595.8 pt, 1 page.

**On the binary before the change (main c625feb, Blitz 3cea708f):** Taffy lays a flex item out in `SizingMode::ContentSize` ("the caller has
already applied this node's own size") and Blitz passed that mode on to the inline boxes in the item's lines, so each box was measured
without its own `width` / `height`. Row A's box was drawn at its content size (about 98 px, not 112) and the row ended at the text line:
"Variabilní" at 454.6 pt, over the bottom of the box. Row B's empty box collapsed to its 2 px border and "Děkujeme" was at 508.6 pt, 87 pt above
Chrome's. Pixel diff 3.0 % (threshold 3.8 %).

**After F217 (main c625feb + Blitz fork 5305fa41):** both boxes are 112 px square with nothing over them; "Variabilní" at 465.1 pt and
"Děkujeme" at 588.1 pt, each paragraph 3.75 pt above Chrome's (the line box of an `inline-block` lacks the strut's descent under the
baseline, F183b, also in a block on main). 2.8 %.

The test is `r7_fixture_46_payment_rows_end_below_their_boxes` in `crates/htmltopdf/tests/flex_item_inline_box.rs`; the bare shapes
are the `r*` and `g*` tests of that file. Plan: `docs/plans/2026-11-03-f217-fixed-inline-block-in-flex-item.md`.
