# F14 — `calc()` cell width mixing a percentage and a length panics

Date: 2026-10-15. Finding: F14 in `2026-10-15-m0-findings.md`. Status: **done** — carried on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` (`91c8a03`).

## 1. Problem

A first-row cell with `width: calc(25% + 20px)` reached `_ => unreachable!()` in blitz-dom
`layout/table.rs::collect_table_cells`: Stylo folds `calc(50%)` and `calc(20px)` to plain values, so only a mixed
expression arrives as a taffy calc value, and `CompactLength::tag()` of a calc value is the low byte of a pointer,
not a valid tag. `htmltopdf` returned `Error::EnginePanic` for valid CSS.

## 2. Spec

CSS Values 4, *Computed Value* of math expressions (<https://drafts.csswg.org/css-values-4/#calc-computed-value>):
"math expressions mixing both percentages and non-zero lengths for widths and heights on table columns, table
column groups, table rows, table row groups, and table cells in both auto and fixed layout tables MUST be treated as
if 'auto' had been specified." CSSWG #94 (MAY → MUST, PR #4389), #3482 (`% + 0px` behaves as `%`). WPT
`css/css-values/calc-width-table-{auto,fixed}-1.html`. Chrome, Firefox and WebKit treat such cell widths as `auto`.
Upstream Blitz PR #653 resolves the calc as a track instead, which does not conform; we do not take it.

## 3. Fix

- `table.rs`: test `into_raw().is_calc()` before matching the tag; a calc cell width makes an `auto` column.
- `damage.rs::flush_styles_to_layout`: a table cell's calc `width` becomes `auto` in its taffy style too; otherwise
  the cell still contributed `25% + 20px` of the table width as its own size (column 187 px instead of 133 px).

## 4. Acceptance

- `crates/htmltopdf/tests/table_columns.rs`: `calc(25% + 20px)` + two auto cells in a 400 px table → equal
  columns (second starts at 133 px); a later row's `200px` still sets that column. Both failed before (panic).
- Probes against Chromium 141 (`calc(25% + 20px)`, `calc(100px + 10%)` next to `calc(50%)`, `calc(50% + 0px)`,
  `table-layout: fixed`): column edges identical.
- Fixtures: no change (none uses calc cell widths). 01 4.4, 02 10.3/11.8/9.6, 03 8.2, 04 5.7, 05 21.6 %.

## 5. Not covered

`<col>`/`<colgroup>` widths are ignored by Blitz altogether (F15), so the same rule for columns has nothing to apply
to yet. Row heights with mixed calc are not touched.
