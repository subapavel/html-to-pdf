# F174 — a table inside a multicol container loses most of its rows

Date: 2026-10-26. Finding: F174 (`2026-10-15-m0-findings.md`). Status: planned. Fork `subapavel/fulgur`
`htmltopdf/srgb-colours` after `49e16712`; Blitz fork (`f4132d5`) unchanged. No `css-spec-researcher` output: §2 is
from memory and unverified.

## 1. Problem

A4, Inter, `td,th{padding:0 6px}`, `line-height:20px`, `table{border-spacing:0}`. On main `e7ee896` a
`<div style="columns:2">` with a 130-row `<table>` (with a `<thead>`) prints 107 of 269 words: the table is a
`FlowItem::Block` of the F30d flow (`column_flow`, `multicol_layout.rs`), `plan_multicol_pages` places it whole, and
`record_multicol_row` warns "a `<table>` 2620px tall … overflows the page"; rows r53-r130 are drawn below page 1.
Chrome: page 1 `Head` + r1-r49 / `Head` + r50-r98, page 2 `Head` + r99-r114 / `Head` + r115-r130, then `tail`.

| Shape | Chrome | Ours |
|---|---|---|
| `a_thead` | 2 pages, 269 words | 2 pages, 107 words |
| `b_plain` (no `thead`) | 2 pages, 261 words | 2 pages, 107 words |
| `d_para_then_table` (30 one-word `<p>`, then the table) | 2 pages, 291 words | 3 pages, 137 words |
| `c_short` (20 rows, fits the page) | r1-r10 in column 1, r11-r20 in column 2 | all 20 rows in column 1 |

Tables in columns are also as wide as the column where Chrome shrinks them to fit (F174b, §6).

**How fulgur cuts a table over pages today.** `fragment_block_subtree` walks the table's `layout_children` (its cells,
in visual order since F36 / F163c); the cells of a row share `location.y`, so the walker cuts before the first cell
that crosses the page bottom. F12's `HeaderRepeat` / `reserve_header_band` reserve the band of a repeated header and
`paint_repeated_table_headers` paints the copy. That walker cannot run per column (it is keyed by page and cursor,
writes geometry as it walks, knows no column x or budget, and balancing calls the fill up to 33 times as a trial).
This plan reuses its policy (`repeating_table_header`, the ¼-page rule, "a row that fits the page but not page minus
header gets no band", no orphan header) and its render path; the cut itself becomes atoms of the F30d planner.

## 2. Spec (unverified)

css-break-3 §2 (column boxes are fragmentainers), §4.1 (between sibling rows and row groups are class A break points),
§4.4 / §5 (monolithic content that does not fit overflows); css-multicol-1 §7.1 / §8.2 (only the last fragment is
balanced; columns continue on the next page); CSS 2.1 §17.2 (print UAs may repeat header rows; css-tables-3
fragmentation repeats header groups at the top of each fragment; F12 takes Chrome's ≤ ¼ page policy); §17.5.1 (row
backgrounds), §17.6.2 (collapsed borders); css-break-3 §5.4 (`box-decoration-break`); CSS 2.1 §17.5.2 (an `auto` table
is shrink-to-fit, F174b).

## 3. Where it belongs

The fulgur fork (Blitz and Taffy do not lay out multicol); no upstream PR. `multicol_layout.rs` is unchanged
(`FlowItem` stays), so the layout pass of fixtures 12 and 24 is untouched.

`pagination_layout.rs`, pure part:

- `struct RowBand { cells: Range<u32>, top: f32, height: f32 }`: a run of cells (indices into `layout_children`) whose
  rows are joined by `rowspan`; `top` from `final_layout`; `height` = next band's top − this top (first band from 0,
  last to the table's height), so the bands add up to the table's height.
- `pub(crate) struct McTable { bands: Vec<RowBand>, head: Option<McHead> }`, `struct McHead { group: usize, bands: u32,
  height: f32 }` (from `repeating_table_header`; `height` is F12's band).
- `fn mc_table(doc, node, column_styles, fresh) -> Option<McTable>`: `Some` only for `display` inside `table`, cells in
  non-decreasing y, ≥ 2 bands, `!table_subtree_is_wrapped` (F12's helper), not `break-inside: avoid` while it fits
  `fresh`. The header repeats for `<table>` only, as in F12.
- `plan_multicol_pages(…, last_inset, tables: &BTreeMap<usize, McTable>)`: a `Block` whose node is in `tables` expands
  into one `McAtom::Row { table, band, h }` per band; the header bands and the first body band are **one** atom (no
  orphan header). Existing callers and tests pass an empty map.
- `fill_multicol_columns`: when a `Row` atom opens a column and the previous atom is a row of the same table, `used`
  starts at `head.height` if `head.height + h <= budget` (F12's rule: a row that does not fit with the band gets the
  column without it).
- `McPiece::Rows { table, first, end, y, head: bool }`; consecutive bands merge into one piece like `Lines`.
- `flow_first_unit(flow, tables)` returns a table's first atom (header + first row), not its whole height (F28 would
  otherwise push a container that starts with a big table to the next page).

Wiring: `multicol_container` builds `McContainer.tables`; F161's "box taller than a page with a spanner keeps the old
path" skips tables whose bands all fit a page; `record_multicol_row` gets a `Rows` arm (one table fragment per piece at
the column's x and the piece's y; each cell through `record_descendant` with `parent_page_y = row_top + y + band −
bands[first].top`; with `head`: the source fragment, written lazily from the table's first fragment, then a repeat
fragment of `head.group` at the top of the piece, `is_repeat` set); a band taller than a page warns naming "a table
row" (placed alone, overflows, as today).

`render.rs`: `draw_table_v2` paints row backgrounds only for the table's first fragment on a page (else twice:
translucent ones darken, bytes duplicate); new `PaginationGeometry.table_pieces: bool`, when set `table_box_size` uses
the fragment's height (else every piece paints the frame at its full height down the columns).

`table_borders.rs`: `page_rows` returns one `(rows, x_off)` per column slot, keyed by x quantised to 1/64 px in a
`BTreeMap` (today the first cell's `x_off` applies to every row of the page: column 2's lines at column 1's x);
`copy_here` / `first_body_row` per slot.

`convert/mod.rs`: `convert_paged_multicol_slices` already skips non-`Lines` pieces. Ours: `Cargo.lock`, tests, fixture
25, `compare.py`.

## 4. Acceptance

`crates/htmltopdf/tests/multicol_tables.rs` (helpers as in `multicol.rs`; `p{margin:0}` because child margins in
columns are F30d-b):

| Test | Shape | Asserts | Main |
|---|---|---|---|
| A | `a_thead` | 2 pages, every `rN`/`vN` once, Chrome's split per column (page 1 r1-r49 / r50-r98, page 2 r99-r114 / r115-r130), `Head1` at the top of each of the 4 column pieces, nothing below the content box | 107 words |
| B | `b_plain` | every row once, Chrome's split | red |
| C | `c_short` | 1 page, r1-r10 column 1, r11-r20 then `after` column 2, `tail` below | red |
| D | `d_para_then_table` | Chrome's pages, words, split | red |
| E | A + `border-collapse:collapse; td,th{border:1px solid #000}` | words as A; column 2's line between its cells drawn at column 2's x; the frame does not run on below column 1's last row on page 2 | red |
| F | A + odd rows `rgba(37,99,235,.3)` | r1, r51, r101 colour equals a one-column render ±2 per channel (painted once) | red |
| G | guards: (1) table taller than a page + `column-span:all`; (2) `break-inside:avoid` table that fits a column; (3) one row taller than a page; (4) table inside a `<div>` in columns; (5) container in a bordered `<div>` then `<p>` | (1) every word; (2) whole in one column; (3) alone, warned, no loop; (4) warning kept, words ≥ main (F174c); (5) border encloses every row, `<p>` below | — |

Fork tests: planner unit tests with tables (rows tile, band reserved at continuation columns, header + first row never
ends a column, a row that does not fit with the band gets none, last page balanced, empty map = today's output);
`mc_table` (`rowspan` bands, out-of-order cells); a lib test (column 2's cells at column 2's x, `thead` 1 source + 3
repeat fragments); `table_borders` one slot byte-identical; `render_smoke` (every cell once; tagged: as many `/TH` as a
one-column render, copies as `/Artifact`).

Fixture `fixtures/invoices/25-terms-table-columns/` (the fixture 24 recipe): "Faktura 2026-0025", header, parties, a
short items table, the total, then `<section class="text-xs" style="columns:2;column-gap:24px">` with clauses 1-4, a
"Splátkový kalendář" heading and a `<table class="w-full">` (`thead` Splátka · Splatnost · Částka, 36 rows, 1 px bottom
border, odd rows `#f9fafb`), clauses 5-8; tuned in Chrome so the table crosses a column break and the page break in
exactly 2 pages. `fixtures.rs`: Splátka 1-36 once each, `Splatnost` as often as Chrome's column pieces, the last word of
every clause, 2 pages. Threshold: worst page + 1.

Fixtures 01-24, the bench corpus and the fork's VRT goldens stay byte-identical.

## 5. Steps

0. Measure (½ day): Chrome numbers for A-F; probes for a collapsed border at a column cut, `border-spacing` at the top
   of a piece, a separate-model `table{border:2px solid}` at a cut. Stop and re-plan if Chrome does not repeat the
   `thead` per column piece.
1. Red tests and fixture 25 with its Chrome reference (½ day).
2. Fork, pure (1 day): `RowBand`, `McTable`, `mc_table`, `McAtom::Row`, `McPiece::Rows`, the header band in the fill,
   the `tables` parameter, `flow_first_unit`, unit tests; nothing calls it yet.
3. Fork, wiring (1 day): `multicol_container` builds `tables`; the spanner-gate exemption; `Rows` arm with the header
   repeat; the row warning; lib tests. A, B, D green on words.
4. Fork, render (1 day): first, as its own commit gated by `cmp` on fixtures 01-24 and the goldens, the output-neutral
   `table_borders` slot refactor; then row backgrounds once per page, `table_pieces`, the per-slot header copy, smoke
   tests; then the single-page gate in both call sites (`wants(…) || (!mc.tables.is_empty() && some group has n ≥ 2)`).
   C, E, F green.
5. Ours (1 day): lock bump, tests, `cmp` against main, `bench.py --check`, compare fixture 25 and its PNGs, threshold;
   findings (F174 done, F174b, F174c), `docs/upstream.md`; `vrt-validator`, `pdf-reviewer` (render path, tagged
   copies).

## 6. Risks and non-goals

- Fixtures 12 and 24 must stay byte-identical: neither has a table in its multicol section; without tables the atoms,
  fill and `flow_first_unit` are unchanged and the single-page gate does not fire. Their items tables are collapsed and
  painted by the refactored `table_borders`, hence the refactor's own `cmp` gate.
- Determinism: `tables` and the border slots are `BTreeMap`s with quantised integer keys; bands come from
  `final_layout`, the numbers `record_descendant` uses.
- A container that fits its page and is now cut becomes shorter than its Taffy height, so later content moves up; a
  parent's frame could still read the Taffy height (G(5)).
- A row taller than a column is placed alone and overflows with a warning; splitting inside a cell is a non-goal.
- Non-goals: F174b (a table in columns is laid out at the column width, not shrink-to-fit: `child_inputs` gives every
  child the known width `col_w`); F174c (a table inside a wrapper in the columns stays one block, the warning stays);
  footer repeat (F12b); caption kept with its table; child margins in columns (F30d-b); exact slice drawing of a
  separate-model table border at a cut (each piece is a closed box).
