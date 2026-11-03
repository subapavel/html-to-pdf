# 40 — terms in one clipping card whose clause headings are full-bleed strips, in two columns (F198j)

The invoice of fixture 31 whose "Obchodní podmínky" card is Tailwind's card idiom with full-bleed headings:
`overflow-hidden rounded-lg border p-4`, and each of the 20 clause headings is `-mx-4 bg-gray-100 px-4 py-1.5 mb-2`, so that the
grey strip runs from the card's left border to its right one. The first heading also has `-mt-4`: it sits on the card's top
border and the rounded corners clip it. Clause 6's heading is nudged with `relative top-0.5` (2 px down). The card is in a
`columns: 2` section and is taller than a page in a single column (about 3000 px at the column width), so it starts in column 1
of page 1, goes on in column 2, then in both columns of page 2, and closes above the signature. There are no icons and no
logo (fixture 31 has them). Chrome (version of the committed references): 2 pages, 945 words.

The committed `fixtures/tailwind/tailwind.css` has none of `-mx-4`, `-mt-4`, `relative` and `top-0.5` and is not rebuilt for a
fixture: the rules are in the fixture's inline `<style>` and say which utility each stands for.

Exercises (css-break-3 §2 / §4.1 / §4.4 breaks between line boxes and between children, §5.4 `box-decoration-break: slice`;
css-break-3 §5.2 margins at an unforced break are truncated; css-position-3 §3 / CSS 2.1 §9.4.3 a relative offset moves the box,
not the boxes after it; CSS 2.1 §8.3.1 negative margins; css-overflow-3 §3 the clip is the padding box of each fragment;
css-multicol-1 §7.1 / §8.2):
- every clause heading and every clause is printed once, in order, the signature after the last clause (945 words like Chrome's),
- the strips run from border to border in both columns of both pages,
- the first strip is flush with the top border and clipped to the rounded corners in column 1 of page 1 only,
- the heading of clause 6 is 2 px lower than it would be, and nothing after it moves,
- text in both columns of both pages, 2 pages like Chrome's.

Not exercised: a line taller than a page (F198j's third shape: `multicol_boxes.rs`), the box's own margins in the columns (F198d).

**On main's binary** (fork `ffdf0c49`) the card is placed whole in column 1: its text runs below the page edge, column 2 and
most of page 2 are empty: 290 of 945 words are printed, 2 pages, compare 7.8 / 8.6 %, and a warning ("a <div> 2994px tall inside a
multi-column container is taller than a page"). The numbers after the fix are in `scripts/compare.py` and the plan.

The test is `terms_card_strips_columns_invoice_prints_every_clause` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`crates/htmltopdf/tests/multicol_boxes.rs`. Plan: `docs/plans/2026-11-02-f198j-multicol-shifted-children-and-tall-lines.md`.
