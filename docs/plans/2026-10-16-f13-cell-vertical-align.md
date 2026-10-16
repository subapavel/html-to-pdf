# F13 — Table cell content ignores `vertical-align` (cells are top-aligned)

Date: 2026-10-16. Finding: F13 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4`, carried via `[patch.crates-io]`. No fulgur change.

## 1. Problem

A table cell's content always sits at the top of the cell. In `fixtures/invoices/02-long`, rows whose description
wraps to two lines have their `#`, `Množ.`, `Cena/h`, `Celkem` cells on the first line; Chrome centres them.

Root cause (`blitz-dom` `layout/table.rs`): the table is a Taffy grid of cells; Taffy stretches each cell to the row
height (`taffy-0.9.2` `compute/grid/alignment.rs:173-193`); the cell's block/inline layout puts content at the top of
the stretched box. Nothing reads `vertical-align`. Blitz's UA sheet sets it correctly (`assets/default.css:505-547`).

| Fixture | Cells shorter than their row | Expected |
|---|---|---|
| 01, 03 | none | byte-identical |
| 02 | `#` and number cells in rows whose description wraps | numbers centred |
| 04, 05 | number cells, but `align-top` | byte-identical |

## 2. Spec

- CSS 2.1 §17.5.3: cell `vertical-align` is `baseline`, `top`, `bottom` or `middle`; other values act as baseline.
- HTML §15.3.8: `thead, tbody, tfoot, table > tr { vertical-align: middle }`, `tr, td, th { vertical-align: inherit }`.
- CSS Box Alignment 3 / CSS Tables 3: the cell box keeps its size, only its content moves. WebKit / legacy Blink:
  "intrinsic padding". Tailwind v4 `align-*` → `vertical-align`.

## 3. Design (Blitz fork)

Offset in `final_layout` (fulgur's fragmenter, F12 and F19 read Blitz geometry); the cell box stays row-height.

| File | Change |
|---|---|
| `node/node.rs` | `pub table_cell_offset: f32` (CSS px), the offset applied by the cell's latest `PerformLayout`. |
| `layout/mod.rs` `compute_child_layout` | Closure body moved unchanged into `compute_node_layout(tree, node_id, inputs)`; closure dispatches aligned table cells to `table::compute_table_cell_layout`. |
| `layout/table.rs` | `is_aligned_table_cell` (PerformLayout, `display: table-cell`, layout parent is a table root); `cell_align_factor` (`middle` 0.5, `bottom` 1, else 0); `compute_table_cell_layout`: measure natural height (`ComputeSize`, through the cache), run `compute_node_layout`, `offset = max(0, h − natural) × factor`, shift non-absolute `layout_children` by it, store it in `table_cell_offset`. |
| `layout/table.rs` `TableTreeWrapper::set_unrounded_layout` | For cells, add `table_cell_offset` to `layout.padding.top` (moves fulgur's text in inline-root cells, which have no child boxes). |

Inside the cached closure so a cache hit does not shift twice; measure before laying out because Taffy's block
`ComputeSize` rewrites child positions; `padding.top` only, so unmoved cells are byte-identical.

## 4. Acceptance

`crates/htmltopdf/tests/cell_vertical_align.rs` (`pdftotext -bbox`): cell A `Řa<br>Řb<br>Řc`, cell B `Bé`.

1. `middle_is_default`: B within ±1 pt of `Řb`.
2. `align_top_keeps_top`: B with `Řa`.
3. `align_bottom`: B with `Řc` (also `tr.align-bottom`).
4. `baseline_acts_as_top` (`align-baseline`, `align-sub`).
5. `single_line_row_does_not_move`.
6. `block_children_cell` (divs).
7. `row_background_fills_row` (F19 cell rect still row-height).
8. `paginated_and_repeated` (120 two-line rows + thead: centred on page 2, header on every page).
9. `fixture_02_row_2_centred`: `6 h` centred between `Konzultace` and `XSLT`.

Confirm 1–4, 6 against Chrome 141 before fixing expectations. Compare: 02 p1 should drop; 01, 03, 04, 05
byte-identical.

## 5. Steps

1. Ours: tests (red where stated), Chrome check.
2. Fork: output-neutral refactor (`compute_node_layout`, field always 0).
3. Fork: alignment + `set_unrounded_layout` bump; tests green.
4. Ours: lock bump, compare, threshold, docs (upstream, findings, new F26 baseline, F27 `valign`), `vrt-validator`.

## 6. Risks and non-goals

- Cache hazard (a `ComputeSize` miss after `PerformLayout` would drop the shift); render twice with one renderer.
- One extra measurement per aligned cell (normally a cache hit); watch the 2000-row table (≈300 ms).
- `padding.top` grows visibly (affects `background-clip: content-box` on shifted cells).
- Not done: real `baseline` (F26), `valign` attribute (F27), `rowspan` (F15), cells with explicit `height`
  (unstretched), absolutely positioned children at static position.

## 7. As landed

- Fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`: `bb18d4c` (output-neutral: `compute_node_layout` extracted,
  `Node::table_cell_offset`), `dda274d` (`middle` / `bottom` alignment, `padding.top` bump in
  `set_unrounded_layout`).
- Tests: `cell_vertical_align.rs` 11 tests; 8 red before (default middle, explicit middle, bottom, inherited from `tr`,
  block children, row background still fills the row, paginated + repeated header, fixture 02 row 2), 3 green before
  and after (top, baseline-like values, single-line row unchanged). Chromium 141, 14 renders of the test HTML: every
  expectation matches (3 lines at 15 pt pitch: default on line 2, bottom on line 3, top / baseline / sub / super /
  text-top on line 1).
- Deviations: only cells with a definite (stretched) height are aligned; offsets < 0.001 px count as 0; a stale offset
  is reset on any other `PerformLayout`; absolutely positioned children and the cell's reported first baseline are
  not shifted.
- Compare: 02 10.3 / 11.5 / 9.9 → 10.2 / 11.4 / 9.8 % (threshold 0.125 → 0.124); 01, 03, 04, 05 byte-identical;
  2000-row table unchanged (≈280 ms, byte-identical — single-line rows do not move); deterministic.
- After `vrt-validator` (PR #15), fork `6904781` + `0f30f6f`: aligned cells are laid out at their natural height (as
  if `height`/`min-height` were auto), so a `height:100%` child is auto and centred like Chrome instead of pushed out
  of the cell (M1); a cell's length `height` becomes a row minimum (`min_size.height`), so Tailwind `h-12` cells are
  stretched and aligned with their siblings (M3, within ≈1 pt of Chrome). 3 more tests (14 total). Fixtures
  unchanged vs `dda274d`; 2000-row table byte-identical, ≈250 ms; deterministic.
- Known, not fixed: `background-clip: content-box` on a shifted cell shrinks to a strip (the `padding.top` bump); a
  row taller than a page loses its short cells (pre-existing truncation of oversized rows); `vertical-align: top`
  cells with a `height:100%` child still fill the cell (Chrome: auto).
