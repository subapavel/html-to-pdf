# 44 — a stamp-and-signature box (`inline-block`, fixed height) alone on its last line at the bottom of page 1: the whole box moves to page 2

Two-page Czech invoice. Page 1 is the header, the two-column party grid and an items table of thirteen rows (the first thirteen of 41),
then "Celkem k úhradě 222 400,00 Kč" and the thank-you line "Děkujeme za spolupráci.". Last comes
`<p class="mt-8 text-right"><span class="podpis inline-block w-72 border border-gray-300 rounded-lg p-4 text-left">…</span></p>`:
a box 70 mm tall (`.podpis { height: 70mm }` in the fixture's `<style>`; Tailwind has no usable height class, and no rebuild is
needed) holding three block children: the caption `Razítko a podpis dodavatele` (`block text-xs … uppercase`), a 72 px `<svg class="block mt-2">`
stamp (two circles and a check mark, no SVG text) and the name `Ing. Jana Nováková, jednatelka` (`block … border-t`). The rest of the
box, about 118 px, is free space for a signature. The paragraph holds nothing but the box: it is alone on its line.

The table is sized (thirteen rows) so that the paragraph starts about 160 px above the bottom of the content box in Chrome and in ours
(the thank-you line is at 646 pt in both). That room is above the layout height of the box's content (about 147 px: the caption row fits and
so does the name, so the fragmenter does not cut the box between its children) and far below the box (266 px). Twelve rows leave 196 px and the
name is only just below the page edge; fourteen to sixteen leave 121 to 65 px and the box is cut (caption on page 1, name on page 2); seventeen
move the box whole. A box 52 mm tall (the plan's first guess) never loses a word at any row count: its free space is below the 15 mm page margin,
so the name is painted in the margin. 70 mm is the smallest round height that does.

The thank-you line comes before the box because ours paints a fixed-height box with in-flow content seated on the line's baseline (F183c): the
box and its text sit lower than Chrome's by about the free space below the name. Placed after the box, the next line would be overlapped.

Exercises:
- css-break-3 §4.1 and §4.4: there is no break point inside a line box, and an `inline-block` may be monolithic. The box does not fit the room and fits
  a page, so the break is the one before the paragraph (a Class A break between block boxes) and the whole box goes to the next page. CSS 2.1 §13.3.3
  says the same. Chrome (version of the committed references): page 1 ends with "Děkujeme" at 646 pt, page 2 starts with the box (caption "RAZÍTKO" at 55.8 pt,
  name at 140.5 pt), 2 pages.

**On the binary before the change (main 5c04d20):** the fragmenter enters the paragraph because the box's first row, the caption, fits the room. The box
is not cut (its content ends inside the room) and the carrier closes as one fragment on page 1. The box is painted seated on the baseline: its top part, the
border, the caption and the top of the stamp, is on page 1 without a bottom border; "Ing. Jana Nováková, jednatelka" is painted 30 pt below the page edge and is
printed on no page. The document has ONE page; Chrome has two. The renderer says nothing: the box's text is painted under a local canvas that the F111 bottom-edge
check does not follow. Pixel diff 3.5 % (page 1), page 2 absent (0.4 % against Chrome's page 2 as a white page), worst 3.5 %, page count 1 against 2.

The test is `signature_box_invoice_moves_the_whole_box_to_page_two` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`alone_inline_block_line.rs`, the warnings in `page_bottom_warning.rs` and `no_unpainted_content.rs`. Every class is in the compiled
`fixtures/tailwind/tailwind.css`. Plan: `docs/plans/2026-11-03-f213d-alone-inline-block-lost.md`.

**After F213d (main 5c04d20 + fork 62216d9):** the whole box is on page 2 (border, caption, stamp, name; "RAZÍTKO" at 147.1 pt, Chrome 55.8 pt; the name at 231.8 pt, Chrome 140.5 pt), page 1 ends with the thank-you
line ("Děkujeme" at 645.8 pt, Chrome 646.0) and a gap below it, 2 pages, no warning. The box sits 91.3 pt (121.7 px) lower than Chrome's: a fixed-height box with in-flow content hangs below its line by about its free
space (F183c, not part of F213d). 3.3 / 0.8 % (before 3.5 / 0.4 % with one page), threshold 4.3 %.
