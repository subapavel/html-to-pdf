# F48 — A table cell with a `height` smaller than its content cuts or overlaps the content

Date: 2026-10-16. Finding: F48 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4`, carried via `[patch.crates-io]`; no fulgur change (§3).

## 1. Problem

A `<td>` with a length `height` smaller than its content (`td { height: 40px }`, Tailwind `h-12` under preflight's
`border-box`) gets a row only as tall as that `height` plus padding; the content overflows the row. When the content is
taller than a page the fragmenter never enters the cell: `z_tdh_one` (one cell, a 120-line `<p>`) prints 53 lines on
1 page with AFTER drawn at y = 80 pt over the content (Chrome: all 120 lines on 3 pages, AFTER at y = 347);
`y_tdh_small` (both cells `height:40px`) loses 94 words, 1 overlap, 4 lines below the content box (probes in
`/tmp/claude-0/vrt-f30b/p/`).

AFTER at 80 pt is 37.5 pt (50 px) below the 15 mm margin, so the whole table is 48 px (40 + 2 × 4 padding): a **layout**
bug, not only a pagination bug. The same mechanism makes a page-fitting cell overlap the next row on page 1 (5 lines in
a `height:40px` cell → a 48 px row) — derived from the code, confirmed in step 1.

Root cause (from reading the forks):

1. Blitz `layout/table.rs:236-254` (F13) turns a length `height` into `min_size.height` only on the **grid-item style**
   (`TableItem.style`, built at `:190`). The cell node's own style is rebuilt from Stylo at `layout/damage.rs:295`
   (`flush_styles_to_layout`) and keeps `size.height = 40px`.
2. The grid measures and lays out the cell through `BaseDocument::compute_child_layout` (`layout/mod.rs:102-113`): for
   an inline-root cell `compute_leaf_layout(.., &node.style, ..)` (`layout/inline.rs:27-33`), otherwise
   `compute_block_layout`. Both clamp the cell box to its style height, so the cell contributes 48 px to its row
   whatever its content.
3. `compute_table_cell_layout` (`table.rs:342-364`) lays content out with `height`/`min-height` swapped to auto, but
   only for `PerformLayout` of `middle`/`bottom` cells — too late to size the row.
4. fulgur reads `final_layout` only (`pagination_layout.rs` reads no style height). The table is 48 px, so
   `would_split_block_subtree` (`:1529-1543`) sees nothing oversized and places it whole; the `<p>` overflows its cell.
5. `td { height ≥ 1500px }` works because the row exceeds the page, so the cell is entered and F30b splits the `<p>`.
6. `tr { height }` works: the row item is a zero-content leaf (`table.rs:460-463`) that only sets a row minimum.
7. Same mechanism, not probed: `table { height: 40px }` (`build_table_context`, `table.rs:59`, keeps the fixed height on
   the grid container, so rows overflow the table box), and `max-height` on a cell (clamps both styles).

## 2. Spec

- CSS 2.1 §17.5.3 (from memory, unverified wording): a row's height is the maximum of the row's computed `height`, each
  cell's computed `height` and the minimum height the cells require; a cell box is at least "the minimum height
  required by the content"; a table's non-`auto` `height` "is treated as a minimum height". So a cell or table `height`
  smaller than the content must neither clip it nor let it overlap.
- CSS 2.1 §10.7: the effect of `min-height`/`max-height` on tables, cells, rows and row groups is **undefined**.
- css-sizing-3 §5.1 `box-sizing`: the `height` minimum is a content-box or border-box length. Content-box verified:
  Chrome makes a `height:48px; padding:4px 8px` cell's row 56 px (`cell_vertical_align.rs`
  `cell_height_is_a_row_minimum_and_aligns`, 31.5 pt). Border-box in Chrome unverified (P2).
- css-break-3 §4.1 (via F30/F30b): once the cell holds its content, its lines are class-B break points.

## 3. Design (Blitz fork)

One rule in one place: **on a table cell and on a table root, a length `height` is a minimum and `max-height` is
ignored**, applied to both styles the table code uses for the box, so the cell's own box is never clamped below its
content. Ignoring `max-height` is our call under §10.7 (content preservation wins); Chrome's behaviour is recorded from
P3.

- `pub(crate) fn height_as_table_minimum(style: &mut taffy::Style<Atom>)` (new, pure, `layout/table.rs`): today's
  `:236-254` logic moved unchanged (a plain length `height` merges into an `auto`/length `min-height` by max; `height`
  becomes `auto`; percentage and `calc()` heights untouched), plus `max_size.height = auto`.
- Callers: `collect_table_cells` (`:236`, grid-item style); `flush_styles_to_layout` (`damage.rs`, after `:295`, next
  to the cell calc-width rule `:298-302`) for `DisplayInside::TableCell` — the fix; `build_table_context` (`:59`) for the
  table's grid style.
- `compute_table_cell_layout` unchanged (still swaps `min-height` out to measure the natural height for
  `vertical-align`).
- `box-sizing`: no code; Taffy applies it to `min_size` in leaf, block and grid layout.
- fulgur: nothing. With the cell as tall as its content, `z_tdh_one` takes the auto-height `<td><p>` path (F30, F30b).
  If tests 4–6 still fail after step 3, the rest is in the fragmenter: record it in §7 and fix it in
  `fragment_block_subtree` in the same PR (same finding).

## 4. Acceptance

New `crates/htmltopdf/tests/fixed_height_cells.rs` (helpers `render_with`, `lines`, `assert_paginated`, `words` copied
from `nested_paragraph_split.rs`; CSS `body{margin:0} p{margin:0;line-height:20px} td{padding:4px;vertical-align:top}
table{width:100%;border-spacing:0}`). "Paginates" = every line once, in order, none below the content box, AFTER below
the last line, same page count as the same table without `height`.

1. `<td style="height:40px">` with 5 lines, then a row `NEXT`: NEXT is 81 pt (108 px) below line 1, not 36 pt. **Red.**
2. Test 1 with `box-sizing:border-box`: same 81 pt. **Red.**
3. One line, `height:60px; padding:4px`: content-box → 68 px row, border-box → 60 px (P2 confirms). Guard.
4. `z_tdh_one` (120 lines): paginates. **Red.**
5. `y_tdh_small` (A × 120, B × 80, both `height:40px`): every A and B line once, AFTER below A0119. **Red.**
6. `max-height:40px` cell: 5 lines as in test 1; 120 lines paginates. **Red.**
7. `<table style="height:40px">` with 5 one-line rows, then AFTER: AFTER below row 5. **Red, expected.**
8. Guards (green today): `min-height:40px` + 120 lines paginates; `tr{height:40px}` + 120 lines; `td{height:1500px}` +
   120 lines; default `middle` + `height:40px` + 120 lines gives the same line positions as `top`.

Blitz unit test for `height_as_table_minimum`: length → min; a larger `min-height` kept; `%` and `calc()` untouched;
`max-height` → auto. Must stay green: `cell_vertical_align.rs` (F13, incl. the 31.5 / 28.5 pt row tests),
`nested_paragraph_split.rs`, `table_header_repeat.rs`, `row_backgrounds.rs`, `consecutive_tables.rs`.

Fixtures 01–08 byte-identical (none puts `height`, `min-h`, `max-h` or `h-*` on a `table`, `tr` or `td`). New fixture
**`09-fixed-row-height`** (the uniform-row-height template idiom): 06's header and parties, an items table of ≈ 35 rows
with `class="h-12"` on every `td` (in the compiled `tailwind.css`; preflight → 48 px border-box), four descriptions
wrapping to 2–3 lines, then the total. Today the wrapped rows overlap the next row; expected: Chrome's page count and
row positions. Commit the Chrome reference and a `compare.py` threshold of the worst page plus 1 point.

## 5. Steps

1. Ours: red tests 1–8; Chrome probes recorded in §7 (P1 test 1; P2 test 3, fixes the border-box number; P3 test 6 —
   does Chrome ignore a cell's `max-height`?; P4 test 7; P5 a `min-height` cell taller than its content); P1 with the
   release CLI + `pdftotext -bbox` to confirm the page-1 overlap; fixture 09 + reference.
2. Blitz: extract `height_as_table_minimum` + unit test (output-neutral, `max-height` aside).
3. Blitz: call it from `flush_styles_to_layout` for cells and from `build_table_context`; clippy, fmt.
4. Ours: pin the Blitz commit; tests 1–8 green; fixtures 01–08 byte-identical (`cmp`), 09 at its threshold; a
   2000-row table's timing within noise of F13's ≈ 250 ms; `docs/upstream.md`, findings (F48 done); `vrt-validator`.
   `pdf-reviewer` not required (no writer, font or metadata change).

## 6. Risks and non-goals

- A percentage-height child of a `top` cell with a length `height` was measured against that height; it is now `auto`
  at measurement time and fills the stretched cell at layout (the F13 "known, not fixed" shape). Guard probe in §7.
- If P3 shows Chrome honours a cell's `max-height`, we still ignore it (undefined in the spec; Chrome is the reference,
  not the truth). Record the difference.
- Rebase: same `table.rs` lines as F13 and F14.
- Non-goals: `height`/`max-height` on `thead`/`tbody`/`tr` beyond today (row groups are not grid items); percentage and
  `calc()` cell or table heights; the legacy `<td height>` attribute (in scope only if Stylo maps it, checked in P1);
  changing `min-height` semantics (stays a row minimum, as F13 made it); `rowspan` (F15); siblings of a split cell (F35).

## 7. As landed

- Fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`: `117c57d` (`height_as_table_minimum` extracted from
  `collect_table_cells`, `max_size.height = auto` added, 4 unit tests), `ebb49fb` (called from `flush_styles_to_layout`
  for `DisplayInside::TableCell` and from `build_table_context`). No fulgur change: red tests 4-6 went green with the
  Blitz fix alone, so the §3 fallback was not needed.
- Tests: `fixed_height_cells.rs`, 11 tests; 6 red before (cell `height` smaller than content: NEXT 36 pt instead of
  81 pt below line 1; the same under `border-box`: 30 pt; `z_tdh_one`: 53 of 120 lines; `y_tdh_small`: 53 of 120 A lines;
  `max-height` cell: 36 pt; `<table style="height:40px">`: AFTER above the last row), 5 green before and after
  (`height` larger than the content, content-box 68 px / border-box 60 px; `min-height` + 120 lines; `tr { height }` +
  120 lines; `td { height: 1500px }` + 120 lines; `middle` equals `top` for a cell with a small `height`). Whole
  `cargo test --release` green (all htmltopdf test binaries, incl. `cell_vertical_align` 14, `nested_paragraph_split`
  35 + 1 ignored). Blitz `cargo test -p blitz-dom` 13 passed. fulgur with the patched Blitz: lib 1985, `render_smoke`
  236, `fulgur-vrt` all green with its goldens untouched.
- Chrome 141 probes (`out/scratch/f48/`, page-relative px from `Line0001` to the next row's text), ours before (a
  scratch build of the F30b branch) / after F48:
  - P1 `td height:40px`, 5 lines, then NEXT: Chrome 108, before 48, after 108.
  - P2 `border-box`: 5 lines + `height:40px` 108 / 40 / 108; one line + `height:60px`: content-box 68, border-box **60**
    (Chrome; before and after 68 / 60).
  - P3 `max-height:40px` with 5 lines: Chrome **ignores** it (108), before 48, after 108; `max-height:100px` with 10
    lines: Chrome 208, before 108, after 208. Same as our rule.
  - P4 `<table height:40px>` with 5 rows, AFTER: Chrome 136 (AFTER below the rows), before 36, after 136;
    `height:300px`: Chrome and ours 296 (extra height is shared between the rows, unchanged).
  - P5 `min-height:40px` cell with 5 lines: Chrome 108, ours 108. `min-height:200px` with one line: Chrome **28**
    (ignores it), ours 208 (F13 made `min-height` a row minimum; unchanged, a non-goal here).
- Fixture 09: Chrome 3 pages; ours before 2 pages, 6.4 / 6.7 / 0.6 % (rows 19, 27 overlapping the next row); after 3
  pages, 5.1 / 4.9 / 0.7 %, the same rows on every page as Chrome (1-15, 16-33, 34-35 + total); threshold 0.061. The
  residual is text drift and the `<tr>` row rules (F22).
- Fixtures 01-08: PDFs byte-identical to the build before (`cmp`); `compare.py` numbers unchanged.
- Timing, 2000-row table (`out/scratch/big2000.html`, 5 renders): before 240-305 ms, after 213-239 ms (noise), output
  byte-identical; the same table with `td { height: 30px }`: after 201-314 ms (before, same table: 227-294 ms).
- Guard probe for the §6 risk (`q1.html`: `height:100px` top cell with a `height:100%` child): identical before and
  after (the child is 100 px tall in both; ours starts 20 px lower than Chrome's, pre-existing, not part of F48).
- Deviations: none from the design. `max-height` on a cell is ignored (Chrome agrees, P3). Not done (non-goals):
  `min-height` on a cell stays a row minimum where Chrome ignores it (P5); the legacy `<td height="60">` attribute is **not mapped to a
  CSS height** by Blitz (probe `p7`: one line, `height="60"`, NEXT 68 px below in Chrome, 28 px in ours, before and after F48), a
  separate gap, left open.
