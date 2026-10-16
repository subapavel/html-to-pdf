# F19 — Row and row-group backgrounds

Date: 2026-10-16. Finding: F19 in `2026-10-15-m0-findings.md`. Status: in progress. Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (fulgur 0.40.0 + F1/F11/F12), carried via `[patch.crates-io]`. No Blitz change.

## 1. Problem

Backgrounds on `<tr>`, `<thead>`, `<tbody>` and `<tfoot>` are never painted. In `fixtures/invoices/03-logo-qr`
(line 84) the total row `<tr class="bg-gray-900 text-white font-bold">` "Celkem k úhradě 48 400,00 Kč" prints white
text on a white page: `pdftotext` finds it but the amount due cannot be seen (`out/compare/03-logo-qr-p1.png`).

Blitz builds the table grid from cells only (`blitz-dom` `layout/table.rs` `build_table_context`). Row groups get no
box; rows are grid items (`TableItemKind::Row`) but start at `line(0)` (Taffy: auto), so a row covers only the last
column, and rounding walks cells only, so a row's `final_layout` stays zero. fulgur's `convert/table.rs`
`collect_table_cells` converts only cells; no row or row-group style ever reaches `Drawables`.

| Fixture | Backgrounds (F19) | Borders (not F19) |
|---|---|---|
| 01 | none | `tr` `border-b`, totals `tr` `border-t-2` |
| 02 | none | `tr` `border-b` (head, 60 rows), totals `tr` `border-t-2` |
| 03 | head `tr.bg-gray-100`, body row 2 `bg-gray-50`, `tfoot` row 3 `bg-gray-900` | none (on the cells) |
| 04 | none | head `tr` `border-b-2`, `tbody.divide-y`, VAT head `border-b`, Σ `border-t` |
| 05 | `thead.bg-gray-50`, `tr.even:bg-gray-50/60` | `tbody.divide-y` |

## 2. Spec

- CSS 2.1 §17.5.1 (table layers): table, column groups, columns, row groups, rows, cells, bottom to top; a row-group
  or row background shows where the cells above it are transparent; with `border-collapse: separate` the
  `border-spacing` area always shows the table's background.
- CSS Tables 3 (drawing table backgrounds): a row / row-group background is painted only inside the cells it covers,
  positioned against the row's (group's) own area. *(Section number unverified.)*
- css-backgrounds-3 §3.7–3.9; HTML §15.3.8 (`thead`/`tbody`/`tfoot`/`tr` display values).

## 3. Design (fork, painted from the table)

Rejected: real row geometry in Blitz (row groups are not grid items; adding rows to `layout_children` would make
fulgur's convert, the fragmenter and F12's prefix check treat rows as cells); a CSS polyfill
(`tr, td { background: inherit }`: wrong positioning area, no stacking, engine behaviour in our layer).

| File (fork) | Change |
|---|---|
| `drawables.rs` | `RowBackground { source, group: Option<NodeId>, style: BlockStyle, area: Size, cells: Vec<RowBackgroundCell { cell, dx, dy }> }`; `Drawables::table_row_backgrounds: BTreeMap<NodeId /* table */, Vec<RowBackground>>` (groups in DOM order, then rows). |
| `convert/table.rs` `convert_table` | `collect_row_backgrounds(doc, table, assets)`: children classified by computed `display`; keep elements with a background colour or layers and `visibility: visible`; strip borders / padding / radii / shadows; cells = `display: table-cell` children with non-zero layout; `area` = union of their border boxes; group area = union over its rows. px → pt via `.as_px().in_pt()`. |
| `render.rs` `paint_table_row_backgrounds(...)` | Per layer, per cell, per fragment of the cell on this page: colour-only → `draw_background` on the cell rect; image/gradient → clip to the cell rect and draw at `(cell − dx/dy, area)`. |
| `render.rs` `draw_table_v2`, `draw_under_clip_table` | Call it after the outer frame (inside the table's opacity group / clip), before any cell. |
| `render.rs` `paint_repeated_table_headers` (F12) | Paint the header group's layers on the copy canvas (inside the `/Artifact`). |

A spanning cell takes its DOM parent row's background (Blitz ignores `rowspan`, F15).

## 4. Acceptance

`crates/htmltopdf/tests/row_backgrounds.rs` (pdftoppm, `@page { margin: 0 }`, 48 dpi):

1. `row_paints_its_cells_only`: red row with a green cell, blue row with one cell → red / green / blue, missing
   cell white.
2. `row_group_background_and_order`: green `tbody`, a red row inside → plain body row green, red row red, head white.
3. `backgrounds_on_every_page`: 120 rows, blue `thead`, green `tbody` → blue header band and green body on every page
   (also the F12 repeat).
4. `fixture_03_total_is_dark`: 03's total row dark behind `úhradě` and the white text visible.
5. `border_spacing_gap_is_not_painted` — `#[ignore]`, F21 (Blitz ignores `border-spacing`).

Fork: convert unit tests (order, cells/dx/dy, colspan, hidden rows, display-classified groups); render_smoke (row,
tbody, overflow-hidden wrapper, `truncate` cell, gradient row, tagged repeated header fill inside the artifact);
goldens byte-identical unless an example uses row backgrounds.

Compare: 03 ≈ 8.2 → 5.5 % (estimate), lower its threshold; 05 PDF changes, number within ±0.1 (tints under the 32/255
tolerance); 01, 02, 04 byte-identical.

## 5. Steps

1. Ours, red tests (Chrome check of "missing cell stays bare").
2. Fork convert + unit tests (output-neutral).
3. Fork render + smoke tests + golden check.
4. Fork F12 repeat path paints header-group layers.
5. Ours: lock bump, compare, thresholds, docs (upstream, findings, new F21 `border-spacing`, F22 row borders),
   `vrt-validator`.

## 6. Risks and non-goals

- Relies on the table node being dispatched before its cells on every page it spans (test 3 guards it).
- Abutting per-cell fills may show hairline seams; merge contiguous colour rects of a row if visible.
- Not done: row/group `opacity`, `visibility: collapse`, `empty-cells: hide`, `<col>` backgrounds, `display: table`
  divs (as F12), `background-attachment`, row borders (F22 / Blitz #386), `border-spacing` layout (F21).
