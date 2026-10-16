# F28 — Consecutive tables do not paginate

Date: 2026-10-16. Finding: F28 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (fulgur 0.40.0 + F1, F11, F12, F19), carried via `[patch.crates-io]`.

## 1. Problem

Sixty one-row tables in a row render on **one** page with ≈50 rows visible; the rest are drawn below the page.
Chrome: 2 pages, all 60. The same rows as `<p>`: 3 correct pages.

Root cause (`crates/fulgur/src/pagination_layout.rs`, from reading):

1. A body child that does not fit the rest of the page goes through the recursion gate (`:837-853` body level,
   `:2027-2035` in `fragment_block_subtree`); `would_split_block_subtree` returns true as soon as any child overflows
   (`:1374`), so the table is entered at the current cursor with `page_start_y = cursor_in` (`:1664`).
2. Its first cell sits exactly at `page_start_y` (Blitz: no top gap, F21). The overflow cut (`:2230`,
   `child_page_y > page_start_y + band && child_page_y + child_h > page_height_px`) only fires below the strip's top
   — meant to protect the first content of a fresh page, but compared against the parent's start, not the page top.
   A parent starting mid-page places its first row overflowing. Same guard upstream 0.40.0 `:2189`.
3. The cursor returns > page height; the next table is entered past the page bottom and is again "first in strip";
   everything stacks on page 0; `implied_page_count` stays 1; cells past the page box are clipped.

Not table-specific: any box whose first in-flow row starts at its top edge (flex rows, grid rows, `<div><div>`).

## 2. Spec

css-break-3 §4.1: class A break points between in-flow siblings (incl. rows); class C between a box's content edge
and its first child only with a non-zero gap — so the only break before a gapless table's first row is the class A
point before the table. Content may overflow a page only if no earlier break point exists (css-break-3 §4.4, CSS 2.1
§13.3.3). Tables are fragmentable between rows. Margins truncate at unforced breaks.

## 3. Design (fulgur fork)

- `leading_block_size(doc, node_id, depth) -> f32`: block-size from the node's top to its first usable break point
  (class C gap → first child's y; else max over the first row of `y + leading_block_size(member)`, recursing only
  where the gate would; leaf → own height; capped at the node's height).
- `fragment_pagination_root`: if `needs_recursion && cursor_y > 0 && cursor_y + leading > page_height_px`, advance
  the page first (class A break before the child), then recompute `needs_recursion`.
- `fragment_block_subtree`: move the overflow cut before the recursion gate; its test becomes
  `child_page_y + first_break_h > page_height_px` (`first_break_h = leading_block_size(child)` when the gate would
  recurse, else `child_h`); recompute after a cut; flex/grid: only the first cell of a row may be cut, then
  re-snapshot `RowState`.
- Push before recursing (not loosen the guard): loosening leaves an empty table fragment on the old page (ghost frame
  from `draw_table_v2`, wrong `fragments.first()` for anchors/counters).

## 4. Acceptance

`crates/htmltopdf/tests/consecutive_tables.rs`: every `RadekNN` once, in page order, no word below the content box;
cases (a) 60 one-row tables, (b) inside a `<div>`, (c) 30 × (`<p>` + table), (d) two-row table with room for one row
→ split, (e) no room → both rows on the next page, (f) thead table whose header doesn't fit → whole table moves,
header once per page. Fork unit tests (geometry: one fragment per cell, `y + h ≤ page_h`; moved table has no fragment
on the old page; flex/grid/nested paginate; class C gap still breaks inside; `leading_block_size`), one render_smoke
test. Fork goldens byte-identical (else inspect, name, re-bless). Fixtures 01–05 byte-identical; thresholds unchanged.

## 5. Steps

1. Probes (m60, m60nodiv, flex60, grid60, nest60, pad60, div60, p60, mixed) + red tests.
2. Fork: `leading_block_size` + unit tests.
3. Fork: body-level push + subtree cut before recursion; fork tests, clippy, fmt, goldens, smoke test.
4. Ours: lock bump, tests, compare + `cmp`, probe PNGs, docs, `vrt-validator`.

## 6. Risks and non-goals

F12: header source and repeat start on the new page; a header that fits while the first body row doesn't is still
orphaned (F12 step 5). F19 unaffected. Grid/flex co-split needs `RowState` re-snapshot. Real documents may gain a page
(content that hung into the margin now moves) — expected. Non-goals: border-spacing (F21), full-frame paint of split
tables, splitting nested inline roots.

## 7. As landed

- Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `594ede0` (`leading_block_size`, 6 unit tests), `558646b` (body-level
  push and the overflow cut moved above the recursion gate; 11 unit tests, 1 render_smoke test; four existing co-split
  tests rewritten — their spacers left the first inner block hanging below the page, i.e. the bug itself), `f18aaed`
  (re-blessed `examples/transform/index.pdf`: its last grid row hung clipped off page 1, now on page 2).
  Fork: 1924 lib, 226 render_smoke, all other test binaries, fulgur-vrt green; clippy/fmt clean; 21 other examples
  byte-identical.
- Probes (release CLI, before → after, Chrome 141): m60 / m60nodiv / flex60 / grid60 / nest60 1 page, 43 of 60 rows →
  2 pages 40/20, every row once (Chrome 40/20); pad60 1 page 34/60 → 2 pages 32/28 (Chrome 32/28); div60, p60, mixed
  byte-identical to before.
- `consecutive_tables.rs`: 9 tests, 8 red before (the "room for one row" split is a guard).
- Fixtures 01–05 byte-identical; thresholds unchanged.
- Deviations: the old post-gate strip-overflow cut is removed (the pre-gate cut does the same); `RowState` is
  re-snapshotted only when a recursive child is cut; `leading_block_size` has no strip parameter, so a nested box
  whose bottom padding / explicit height is what doesn't fit still overflows as before.
- Known: no keep-with-next, so a heading can stay on the page before its moved grid/table; a header that fits while
  the first body row does not is still orphaned (F12 step 5).
