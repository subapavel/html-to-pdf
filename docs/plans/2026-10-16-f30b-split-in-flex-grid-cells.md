# F30b — A paragraph taller than a page that is itself a flex item, grid item or table cell loses its tail

Date: 2026-10-16. Finding: F30b (follow-up of `2026-10-16-f30-nested-paragraph-split.md`). Status: **done** (§7). Fix
goes on `subapavel/fulgur` `htmltopdf/srgb-colours`, carried via `[patch.crates-io]`.

## 1. Problem

When an inline root (`<p>`, or a `div` whose content is text) is itself a flex item, a grid item or a table cell and
is taller than a page, only the lines that fit its first page are printed. The rest is drawn below the page box, no
page is added, and F30's `warn_unprinted_inline_lines` logs it. Probes at main `7269c2c` (`out/scratch/f30b/`,
`gen.py`, 300 lines of 20 px): `flex-col > p`, `flex-col > div.box` with direct text and `grid > p` print 53 of 300
lines (Chrome: all, 7 pages). `<td>Item</td><td>{text}</td>` and a single direct-text `<td>` print 53 lines on 1 page
(Chrome: 6 pages). The same paragraph one level deeper (`flex-col > .box > p`, `grid > .box > p`, a `flex` row or
`grid-cols-2` with a short item beside `.box > p`) already paginates like Chrome.

Root cause (fork `crates/fulgur/src/pagination_layout.rs`, from reading):

1. `line_split_candidate` (`:3312`) refuses the child when the parent is a flex/grid container (`:3327`), the parent
   is `display: table` (`:3332`) or the child is a table cell (`:3338`), so `nested_line_split` (`:2410`) returns
   `None`.
2. The recursion gate (`:2400`) is false for an inline root (`would_split_block_subtree` skips its zero-height text
   children, `:1530`); the child takes the leaf path (`:2810`): one full-height fragment, warning at `:2815`.
3. Why the deeper arrangement works: the body-level gate (`:930-949`) and the nested gate see the item's oversized
   `<p>` grandchild (`:1539`), so `fragment_block_subtree(item)` runs, where the `<p>` has a block parent and F30
   splits it. Items in the same row are co-split by `RowState` (`:1873`, `:2148-2213`).
4. For flex/grid parents F30's box closing is off: `child_tail` (`:2557`) and `closing` (`:2914`) are gated on
   `!allow_same_row_rebase`, so a single-column flex/grid card with `border p-4` around a split item would lose its
   bottom padding and border.
5. `table_row_mate_can_split` (`:3380`) asks `leading_block_size`, which counts a direct-text cell whole (`:1646`), so
   a short cell before such a cell pushes the row to a fresh page (a `<td><p>` row starts in place).
6. Render: `paint_table_row_backgrounds` (`render.rs:3135-3143`) reads raw cell fragments, which for a split inline
   root hold the line span only; cell frames go through `frame_fragment` (`render.rs:2893`).

## 2. Spec

- css-break-3 §4.1: breaks between line boxes inside a block container are class B; §3.3 `orphans`/`widows` (initial
  2), relaxed per §4.4 when no break avoids a violation; a single line box is monolithic.
- css-flexbox-1 §4 / css-grid-2 §6: flex and grid items establish an independent formatting context (a block
  container), so their lines are class-B break points. css-flexbox-1 §12: flex containers break between items, between
  flex lines and inside items; in the §12.1 sample algorithm a single-line column flex container lays items out like
  block flow, and items of a row fragment in parallel. css-grid-2 "Fragmenting Grid Layout": breaks between rows and
  inside items, items of a row in parallel. Section numbers and wording unverified (planned without web access).
- CSS 2.1 §17.4/§17.5: a table cell is a block container; css-tables-3 (draft) fragments a row's cells in parallel
  (unverified).
- Chrome (reference, verified from the probe PDFs): `flexcol_p` puts 50 lines on every page and "Footer" alone on
  page 7 — block flow. In `td_direct` the middle-aligned "Item" prints beside Line0000 at the top of page 1, so Chrome
  does not centre cells of a fragmented row (that belongs to F35).

## 3. Design (fulgur fork, `pagination_layout.rs` + `render.rs`)

Principle: **an inline-root item is split exactly where the same paragraph wrapped in a block item is split today.**
When to split does not change: only paragraphs that fit no page whole, or are first on their strip and overflow it.
Shorter ones still move whole (F30c). Fixtures 01–07 log no unprinted-lines warning, so they stay byte-identical.

- `line_split_candidate`: allow flex/grid parents, and table cells of a `display: table` parent. Still excluded:
  other children of table parents (F30e), atomic-inline parents (`inline-flex`/`inline-grid`, F30f), multicol,
  orthogonal flows, clipping, transformed, floating.
- `fn items_stack_vertically(doc, children) -> bool` (new, pure): no two in-flow children with a height overlap in y
  (column flex, one-column grid, single item). `fragment_block_subtree` uses
  `!allow_same_row_rebase || items_stack_vertically(..)` for `child_tail` (`:2557`) and `closing` (`:2914`): a
  single-column flex/grid box is block flow for fragmentation (§12.1), so its bottom padding and border close below a
  split item as F30 does for blocks. Multi-item rows keep today's behaviour.
- `first_break_h` (`:2444`): a line-split item that is not `first_in_row` gets `0.0`, like the recursion branch (F28:
  only the first item of a row moves the row; parity with block items).
- `table_row_mate_can_split` → `row_mate_can_split`: still table-only; also accepts a mate whose
  `line_split_candidate(..)` has `leading() <= page_h`, so a direct-text cell starts its row in place like `<td><p>`.
- `leading_block_size` picks up flex items and cells through `line_split_candidate` (`:1638`) unchanged.
- `render.rs` `paint_table_row_backgrounds`: each cell rect from `frame_fragment(cell_geom, frag)`, so a `<tr>`
  background covers a split direct-text cell's padding on every page.
- `warn_unprinted_inline_lines`: message names what stays unsplit (clipping, multicol, transformed, atomic-inline and
  `display: table` wrappers) instead of "plain block flow".

## 4. Acceptance

`crates/htmltopdf/tests/nested_paragraph_split.rs`, same CSS and helpers, `L = lines(1..=300)` unless stated; every
line once, in order, none below the content box (`assert_paginated`):

1. `<div style="display:flex;flex-direction:column"><p>{L}</p><p>AFTER</p></div>`: 7 pages (Chrome `flexcol_p`: 7),
   AFTER below Line0300.
2. `flex-col > div.box` with direct text: same page count as `direct_text_in_a_bordered_div`.
3. `<div style="display:grid;gap:16px"><h3>T</h3><p>{L}</p><p>AFTER</p></div>`.
4. Parity: flex-col and grid wrappers give the same lines per page as `<div><p>{L}</p></div>`.
5. `FRAME_CSS` box with `display:flex;flex-direction:column;gap:16px` around `<h3>` + `<p>` (140..=160 lines): the
   frame ends at the content bottom on the pages it leaves; the last frame stays inside the content box.
6. `display:flex;align-items:flex-start;gap:16px`: a 20 px square, `<p>{L}</p>`, `<p>AFTER</p>`; page count equals
   the same row with `<div><p>` as the second item.
7. Un-ignore `direct_text_in_a_table_cell`.
8. Port `cells_after_a_split_cell_sit_at_the_top_of_the_row` to direct-text cells (120 lines, `vertical-align: top`):
   `right` beside Line0001 on page 1; AFTER and page count `(3, 3)`; the `x`-first variant needs `row_mate_can_split`.
9. Port `a_paragraph_in_a_cell_repeats_the_table_header_on_every_page` to direct text: 4 pages, header once per page,
   ≥ 45 lines on page 1.
10. `tr{background:#ffe9a8}`, `td{padding:16px}`, direct-text cell: on page 2 the fill starts at the content top.
11. `#[ignore = "F35"]`: `<td>Item</td><td>{L}</td>`, default `middle`: Item on page 1 beside Line0001, as Chrome.

Fork unit tests: invert `paragraphs_in_flex_items_and_table_cells_are_not_split` (`:9131`; flex, grid and cell items
each ≥ 2 fragments); exclusion guard (`overflow:hidden` parent, `inline-flex` parent, non-cell child of a
`display:table` div stay whole); `items_stack_vertically` (column flex, one-column grid true; flex row, 2-column grid
false); last fragment of a padded flex-col container includes its bottom padding; `row_mate_can_split` accepts a
direct-text mate; move `inline_lines_below_the_page_are_reported_for_unsplit_paragraphs` to an `overflow:hidden`
parent; one `render_smoke` test (flex-col `<p>` + direct-text `<td>`).

Fixtures: 01–07 byte-identical; fork example goldens and `fulgur-vrt` byte-identical. New fixture **`08-terms-grid`**
(pixels move in a realistic invoice): 06's header, parties, items table and total, then
`<section class="grid gap-4 rounded-lg border border-gray-200 p-4 text-sm">` with an
`<h2 class="text-xs font-semibold uppercase tracking-wider text-gray-500">` and one
`<p class="leading-relaxed text-gray-600">` of Czech terms ≈ 1.4 pages, then 05's `flex items-start gap-4` footer
(svg `size-5 shrink-0` + short `<p>`). Every class is in the compiled `tailwind.css` (`flex-col`, `grid-cols-1` are
not, hence `grid gap-4`; Tailwind is not rebuilt). Today 1 page with the tail lost; expected 2/2 like Chrome. Commit
the Chrome reference and a `compare.py` threshold of the worst page plus 1 point, as for 06.

## 5. Steps

1. Ours: red tests 1–11, fixture 08 + reference. Chrome probes, recorded in §7: P1 stretched bordered short card
   beside a tall `<p>` in a `flex` row mid-page (does its frame continue on page 2?); P2 a later `grid-cols-2` row,
   label + tall `<p>`, mid-page (does the row start in place?); P3 `items-start` icon + `<p>` row starting 30 px above
   the page bottom; P4 the fixture-08 card; P5 a `flex-col` holding direct text (anonymous flex item); P6 `grid1_p`.
2. Fork: `items_stack_vertically` + tail/closing switch with its test (changes output only for padded single-column
   flex/grid boxes around a deeper split).
3. Fork: `line_split_candidate`, `first_break_h` gating, `row_mate_can_split`, unit tests.
4. Fork: row-background `frame_fragment`, warning text, `render_smoke`, clippy, fmt, goldens.
5. Ours: lock bump, tests, compare (01–07 identical, 08 at its threshold), `docs/upstream.md`, findings (F30b done,
   add F30h), `vrt-validator`. `pdf-reviewer` not required (no writer, font or metadata change).

## 6. Risks and non-goals

- Multi-item rows (new **F30h**): a stretched sibling of a split item keeps one page-tall fragment (background/border
  to the page edge on page 1, missing after), as for block items today (tables: F35); an item that is not first in
  its row and whose first two lines don't fit overflows by up to one line (F28: only the first item moves the row); a
  later row whose first item is a short stretched leaf still moves to a fresh page.
- Cells: a direct-text cell that is not the tallest in its row keeps its F13 offset inside `padding.top`, reserved on
  every strip; middle/bottom-aligned short cells lose their text — both F35.
- An anonymous flex item (direct text in a flex container) is covered only if Blitz gives it a layout child with
  inline layout; if P5 shows otherwise, record it under F30h.
- Rebase conflicts in `fragment_block_subtree` next to F12, F28, F30 code.
- Non-goals: F30c–F30g, `box-decoration-break`, `column-reverse` / out-of-DOM-order grid placement, `row-span-*`
  items, reading `orphans`/`widows`.

## 7. As landed

- Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `1cc10f7` (`items_stack_vertically` + `is_flow_child_with_height`;
  `child_tail` / `closing` switch, `closes_like_block`; 2 unit tests), `4209f06` (`line_split_candidate` admits flex /
  grid items and cells of a `display: table` box; `first_break_h` is 0 for a line-split item that is not first in its
  row; `table_row_mate_can_split` → `row_mate_can_split` accepts a direct-text mate; row state restart on a cut; 6 new
  unit tests, 1 inverted, 1 moved to an `overflow:hidden` parent), `ce302a6` (`paint_table_row_backgrounds` through
  `frame_fragment`, warning text, 1 `render_smoke` test). Fork: 1982 lib (1975 before), 236 render_smoke, every other
  test binary green (2211 tests over lib + 40 integration binaries, 0 failed), fulgur-vrt green, `examples_determinism`
  only `committed_svg_matches_rendered` fails (environment, as before), example goldens and VRT goldens byte-identical,
  clippy and fmt clean.
- Ours: `nested_paragraph_split.rs` 30 pass + 1 ignored (`a_short_cell_beside_a_split_cell_stays_on_the_first_page_when_centred`,
  F35: fails with "Item not printed"); 10 of the 11 new tests were red before (the 11th is the ignored one). Whole
  workspace `cargo test --release --locked`: 193 passed, 0 failed, 2 ignored.
- Fixtures 01–07 byte-identical (`cmp` of `out/<name>/htmltopdf.pdf` before and after). Fixture `08-terms-grid`: Chrome
  reference generated, 2 / 2 pages like Chrome, 13.2 / 12.1 %, threshold 0.142 (before the fix: 1 page, tail lost).
- Probes (120 or 300 lines of 20 px, Chrome vs ours before → after; `out/scratch/f30b/`):
  - `flexcol_p`, `grid1_p` (300 lines): Chrome 7 pages, 50 lines each; ours 2 pages with 53 lines → 7 pages, 50 per page, Footer on 7.
  - `flexcol`, `grid1` (header, bordered `.box > p`, footer) and `flexrow`, `grid2`: already like Chrome, unchanged (7, 7, 6, 6 pages).
  - `flexcol_direct` (`.box` with direct text): 7 pages like Chrome, but 48 + 2 lines on pages 6 and 7 against Chrome's 50
    (the F30 box reserves its padding on every strip, `box-decoration-break` is a non-goal).
  - `td_direct1` (single direct-text cell): 53 lines on 1 page → 6 pages like Chrome. `td_direct` (`<td>Item</td><td>{L}</td>`):
    6 pages like Chrome, but `Item` is not printed (F35, test 11 stays ignored); the warning now names it.
  - **P1** (stretched bordered short card beside a tall `<p>` in a `flex` row, 400 px down): Chrome starts the row in
    place (30 / 50 / 40 lines) and continues the card's frame on pages 2 and 3. Ours: same line positions and 3 pages
    (before: 33 lines on 1 page, 2 pages), but the card's frame is one page-tall fragment (to the page edge on page 1,
    missing on 2–3) — F30h.
  - **P2** (`grid-cols-2`, row 2 = bordered label + tall `<p>`, row 1 500 px high): Chrome starts row 2 in place
    (24 lines on page 1, 3 pages). Ours moves the row to page 2 (the first item, a stretched leaf, does not fit
    beside nothing; 50 / 50 / 20 lines, 4 pages) — F30h. Before: 53 lines on page 2 and the rest lost.
  - **P3** (`items-start` icon + `<p>` row 30 px above the page bottom): Chrome moves the row to page 2 (0 / 50 / 50 / 20);
    ours the same, 4 pages. The row moves because `leading_block_size` of the container takes the tallest item's two lines.
  - **P4** (fixture-08 card): 2 pages like Chrome, see above.
  - **P5** (`flex-col` holding direct text): already like Chrome before the change (3 pages, 50 / 50 / 20): Blitz makes
    the flex container itself the inline root, so no anonymous flex item exists and the plan's assumption is moot.
  - **P6** (`grid > p` + Footer): 53 lines on 1 page → 7 pages like Chrome at 300 lines (3 pages at 120 lines, like Chrome).
- Deviations from the plan: (1) a cut of a first-in-row line-split item restarts the flex / grid `RowState` on the
  fresh page (the same update the recursion branch always did), otherwise its row mates stayed on the old page; found
  by a unit test, not in the plan. (2) `first_break_h` gating by `first_in_row` is only visible in a later row of a
  grid / flex box: a first row is still moved as a whole by the parent's `leading_block_size` (it takes the tallest
  item's two lines), which is also what Chrome does (P3). (3) The unit test for the row-background fix is the
  htmltopdf test (pixel check on page 2) and the smoke test; there is no fork unit test of `paint_table_row_backgrounds`.
  (4) Test 11 is named `a_short_cell_beside_a_split_cell_stays_on_the_first_page_when_centred`.
- Not verified: `vrt-validator` (run by the caller); row backgrounds with gradients / images on a split direct-text
  cell (colour fill only is tested); `column-reverse` and out-of-DOM-order grid placement (non-goals); the
  `examples_determinism` committed-SVG test (fails before and after for environment reasons).
