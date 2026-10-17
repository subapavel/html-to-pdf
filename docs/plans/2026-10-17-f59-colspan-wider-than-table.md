# F59 — A `colspan` wider than the table breaks the column widths

Date: 2026-10-17. Finding: F59 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` (`packages/blitz-dom/src/layout/table.rs`), carried via `[patch.crates-io]`; no fulgur
change.

## 1. Problem

Probe `/tmp/claude-0/vrt-f55/t/full99.html` (A4, `table{width:600px}`, rows `A1 B1 C1` / `<td colspan="99">FULLROW</td>` /
`A3 B3 C3`): ours (`f99n-1.png`) packs the three real columns into ≈ 100 px at the left edge while the full row is 600 px
wide (B1 at x = 72 pt); Chrome (`f99c-1.png`) gives three ≈ 200 px columns (B1 at 196 pt). `colspan` 4 or 5 is already
off by 2–29 pt, while Chrome's `full4.chrome.pdf` looks like an exact span. This is the "colspan wide enough for any
table" idiom of older invoice templates (section headers, notes, a totals line).

Root cause (Blitz fork, `layout/table.rs`):

1. `push_cell_item` (`:195-255`) places each cell at `line(col + 1) / span(colspan)` with the span as written
   (`:238-241`) and advances `col` by it (`:254`); nothing knows how many columns the table really has.
2. `build_table_context` sizes the explicit grid by the **last** row's `col` (`:85`, `column_sizes.resize(col, auto)`).
   Spans past it run into implicit tracks, `auto` because `grid_auto_columns` is emptied (`:57`). The same `resize` also
   drops first-row widths when the last row is shorter.
3. Taffy sizes 99 `auto` tracks, correctly for a grid: the spanning cell's contribution is spread over all 99 (taffy
   0.9.2 `track_sizing.rs:939` `distribute_item_space_to_base_size`), and `stretch_auto_tracks` (`:1332-1358`) splits
   the free 600 px equally, so each real column gets 1/99 of it.
4. First-row widths are pushed once per cell (`:222`) but read once per column (`:223-236`), so a first-row span shifts
   the widths after it (F15).
5. F55's `MAX_COLUMNS` clamp (`:198-200`) counts these raw columns, so in a row of 17+ `colspan=1000` cells the later
   cells share the last column (`many20x5.html`); Chrome (`many20x5.chrome.pdf`) shows 20 content-sized columns.

## 2. Spec

- HTML §4.9.12.1 "forming a table" (from memory, unverified): a cell's colspan grows x_width, so `colspan=99` makes 99
  columns; a column whose slots have no anchored cell is a **table model error**, with no rendering defined by HTML.
- CSS 2.1 §17.5.2.2 (automatic layout) is non-normative (wording unverified): auto layout is up to the UA, and Chrome is
  the reference. Chrome (TablesNG; Gecko and legacy WebKit likewise, unverified) gives a column in which no cell starts
  no width, and spans across it shrink (`full4`, `many20x5`). Open: whether css-tables-3 "computing column measures"
  defines this (`css-spec-researcher` not run).
- CSS 2.1 §17.5.2.1 (fixed layout, normative): "Any remaining columns equally divide the remaining horizontal table
  space" — a span-only column is a remaining column and gets width. Chrome agrees (page images, not measured): in
  `fix99` / `fixw99` (`table-layout:fixed; width:600px`, 100 / 200 px columns, a `colspan=99` row) the auto column C and
  the 96 span-only columns get ≈ 3 px each, and C1 overflows. css-tables-3: `fixed` applies only when the table width is
  not `auto` (unverified; P6).
- css-grid-1 §11.5 / §11.8: Taffy is right for a grid; the error is in Blitz's mapping of the table onto a grid.

## 3. Design

Blitz builds the table grid, so the fix belongs there (not Taffy — correct grid behaviour; not fulgur — it reads laid-out
boxes). **Rule:** in auto layout the grid tracks are the columns in which at least one cell starts, and a cell spans the
tracks whose columns lie in [start, start + colspan); in fixed layout every column up to the widest row is a track
(today's behaviour). Cells are placed after all rows are collected.

```rust
struct CellPlacement { item: usize, start: u32, span: u32 }            // one per cell item, raw columns
fn track_columns(placements: &[CellPlacement], fixed: bool) -> Vec<u32>; // ascending; auto: start columns, dedup; fixed: 0..widest row
fn grid_column(tracks: &[u32], start: u32, span: u32) -> (i16, u16);     // 1-based line; span = tracks in [start, start + span); MAX_COLUMNS fallback
```

- `GridBuilder.col` becomes a `u32` raw column (saturating add). `push_cell_item` no longer clamps or sets
  `grid_column`; it records a `CellPlacement`.
- First-row widths are keyed by the cell's start column (`BTreeMap<u32, Dimension>`, deterministic); later rows widen
  the column they start in, as today. For a first row without spans (every fixture) this is identical to today; the
  keying is needed because merging maps columns by index.
- `build_table_context`: tracks from `track_columns`; `grid_template_columns` is each track column's width or `auto`;
  each cell item gets its `grid_column(..)`; no implicit columns remain. Row items (`line(0)/line(-1)`) unchanged.
- `MAX_COLUMNS` limits tracks, not raw columns: the F55 fallback stays only past 16 384 real cells in a row (or a fixed
  table that wide), so 20 × `colspan=1000` gets 20 tracks in auto layout.
- `uses_fixed_algorithm` = `table-layout: fixed` and a non-`auto` table `width` if P6 shows Chrome uses auto layout for
  `fixed; width:auto`; otherwise `is_fixed`. F5's `is_fixed` (later rows do not widen columns) is unchanged.

## 4. Acceptance

Extend `crates/htmltopdf/tests/table_spans.rs` (its CSS, `render`, `words`, `x_of`). Each case is compared with an
exact-span twin (after the fix both produce the same grid, so the test does not depend on font metrics). Red before the
fix unless marked.

1. `table{width:600px}`, `A1 B1 C1` / `<td colspan=N>FULL</td>` / `A3 B3 C3`: `B1`, `C1`, `B3`, `C3` at the x of the
   `colspan=3` twin ±0.5 pt for N ∈ {4, 5, 99, 1000}, full row first, middle and last.
2. The same without a table width and with a 60-character `FULL` (shrink-to-fit: the span's content goes to the real
   columns only).
3. Header idiom: `<tr><th colspan=99>Hlavička</th></tr>` then two 3-cell rows, `width:100px` on row 2's first cell: equal
   to the `colspan=3` twin.
4. A column only spans cover: every row `<td colspan=2>X</td><td>Y</td>`; `Y` at the x of `<td>X</td><td>Y</td>` (if P3
   confirms Chrome merges it).
5. Two rows of twenty `colspan=1000` cells: `X0`..`X19` at the x of the `colspan=1` twin (today `X16`..`X19` share one
   x); the 40-cell F55 test also asserts `X39` right of `X38`.
6. Nested: a `colspan=99` table inside a `colspan=99` cell; both equal their exact twins.
7. Green before and after: fixed layout keeps span-only columns (`fixw99` shape: `TAIL` x > `C1` x + 200 pt); a table
   whose only row is one `colspan=99` cell; exact spans (fixture 03 shape); a 17 000-cell row renders (F55 fallback);
   the existing five tests.

Blitz unit tests for `track_columns` / `grid_column`: spans past the end, an inner column only spans cover, a spanning
first row, rows that all span (one track), fixed keeps every column, 20 × 1000, an empty table, the `MAX_COLUMNS`
fallback. Fixtures 01–10 byte-identical (`cmp`; only 03 uses `colspan`, with exact spans); `fulgur-vrt` goldens untouched.

New fixture **`11-full-width-rows`**: a one-page Czech invoice; items table `w-full border-collapse text-sm` with `th`
widths `w-20` / `w-28` / `w-32` and an auto description column; section rows
`<td colspan="99" class="bg-gray-100 px-3 py-2 font-semibold">Služby</td>`, a `colspan="99"` totals line
`text-right font-bold`, a notes row `text-xs text-gray-500 leading-relaxed`. Other classes only from `border
border-gray-300 border-t px-3 py-2 text-left tabular-nums bg-gray-50 bg-gray-900 text-white uppercase tracking-wider
text-gray-600 mt-6 mb-6` (all in the compiled `tailwind.css`; `italic`, `table-fixed`, `whitespace-nowrap`, `p-8`,
`text-center` are not). Today the description column collapses; expected: Chrome's column positions, 1/1 page. Commit
the Chrome reference; threshold = worst page after the fix + 1 point.

## 5. Steps

1. Ours: Chrome probes in `out/scratch/f59/` (Chrome PDF, ours before, `pdftotext -bbox`), recorded in §7: P1
   `full3/4/5/99/1000` (`full3/4/99` exist in `/tmp/claude-0/vrt-f55/t/`); P2 test 2; P3 an inner span-only column, also
   with `border-spacing:10px`; P4 a spanning first row with widths in row 2; P5 a table whose only row spans, rows that
   all span (5 and 3); P6 `fix99`, `fixw99`, `table-layout:fixed; width:auto`; P7 the fixture-11 shape with preflight;
   P8 `<colgroup>` of 5 `<col>`, 3 cells and `colspan=99` (record only); P9 nested; P10 `many17x5`, `many20x5`. Then red
   tests 1–7, fixture 11 + reference + README, compare numbers before.
2. Blitz, output-neutral: `col: u32`, `CellPlacement`, the width map, placement in `build_table_context` with every
   column as a track and the F55 clamp; fixtures 01–10 `cmp` identical, Blitz tests green.
3. Blitz: `track_columns`, `grid_column` + unit tests.
4. Blitz: auto layout uses start-column tracks; tests 1–6 green; then `uses_fixed_algorithm` per P6.
5. Ours: pin the fork; `cargo test --release`; fixtures 01–10 identical, 11 at its threshold; `cargo test -p fulgur` and
   `fulgur-vrt` against our Blitz; a 2000-row table byte-identical and timed; `docs/upstream.md`; findings (F59 done,
   F15 keeps "first-row colspan width division"); `vrt-validator`. `pdf-reviewer` not required.

## 6. Risks and non-goals

- Chrome may keep `border-spacing` around a merged column (P3); Blitz maps no `border-spacing`, so nothing to match yet.
- Chrome does not merge a column a `<col>` defines (from memory, P8); Blitz skips `<col>`, so we would merge it — rare in
  invoices, belongs to F15's `<col>` item.
- Extra width is still shared equally between auto tracks (`stretch_auto_tracks`), Chrome shares it by max-content:
  expect leftover column drift in fixture 11 (not this finding).
- Rebase conflicts with F50 (`push_cell_item`, `GridBuilder`) and F15 work; the width map touches F5's path, guarded by
  `table_columns.rs`.
- `grid.row` is still `u16` cast to `i16`: a table over 32 767 rows is unchecked (file a finding if it panics).
- Non-goals (F15): dividing a spanning first-row cell's width over its columns, `rowspan`, `<col>` widths, `min-width`,
  sharing extra width by max-content, fixed layout ignoring content.

## 7. As landed

- Blitz fork `htmltopdf/blitz-dom-0.2.4`: `ee5477b` (one commit; the plan's steps 2-4 were verified in that order but not
  committed separately: with every column as a track the 01-10 PDFs were `cmp`-identical to main, then the auto-layout
  switch went in). `GridBuilder.col` is a `u32`, `push_cell_item` records a `CellPlacement`, `build_table_context` places
  the cells from `track_columns` / `grid_column` once every row is known, first-row widths are a `BTreeMap` keyed by the
  start column, `uses_fixed_algorithm` = `table-layout: fixed` and a table `width` that is not `auto`. Our `Cargo.lock`
  moves only the blitz-dom source. No fulgur change.
- Tests: `crates/htmltopdf/tests/table_spans.rs` 17 tests, **8 red before the fix** (colspan 4 / 5 / 99 / 1000 at the
  three positions, shrink-to-fit with a 60-character cell, header idiom, inner span-only column, rows that all span,
  20 x `colspan=1000`, nested, and the 40-cell `X39` right of `X38`, e.g. `colspan=4: B1 at 163.3 pt, twin 195.5 pt`);
  9 green before and after. Blitz: 10 new unit tests in `layout/table.rs` (37 in `blitz-dom`). Ours `cargo test
  --release --locked --no-fail-fast`: 272 passed, 0 failed, 2 ignored. Against our Blitz: `fulgur --lib` 2006,
  `render_smoke` 240, `fulgur-vrt` goldens byte-identical.
- Fixtures 01-10: PDFs byte-identical to main (`cmp`). Fixture 11 added with a Chrome reference: 1 / 1 page, **8.1 % before
  -> 6.9 %** (threshold 7.9 %). Before, the description column collapsed to a few words per line and the header row
  stopped at 405 px; now the headers MNOŽ. / CENA / CELKEM start at x = 322.3 / 382.3 / 466.3 pt, Chrome 322.5 / 382.5 / 466.5
  (`pdftotext -bbox`). What is left is text drift, rows 1-2 px taller than Chrome's and the cell rules (Blitz #386).
- Probes (`out/scratch/f59/`, `xMin` in pt, Chrome / ours-before / ours-after; no preflight, `table{width:600px}`):
  P1 `colspan` 4 / 5 / 99 / 1000, B1 196 / 165, 145, 72, 69 / **196**, C1 344 / 282, 244, 97, 90 / **346** (the 2 pt is the
  extra width shared equally instead of by max-content, the same as for `colspan=3`); full row first and last the same.
  P2 shrink-to-fit with a 60-character cell, B1 165 / 68 / **165**. P3 inner span-only column: Chrome merges it
  (`Y` 270, equal to the twin), also with `border-spacing: 10px` (274); ours 339 / **271**; `border-spacing` itself is still
  not mapped (X 54 / 46). Mixed spans, B1 195 / 163 / 196. Header idiom c1 315 / 153 / 313. P4 spanning first row with widths
  below: 129 / 286, unchanged (it was right by coincidence of the old misaligned vector). P5 rows spanning 5, 3 and 4 + 1,
  R3b 269 / 390 / 271; a table whose only row is one cell renders. P6 `table-layout: fixed`: with `width:100%` Chrome divides
  equally (B1 302), with `width: auto` it is automatic (B1 205, TAIL 84 for a span row; ours 205 / 84 after the
  `uses_fixed_algorithm` rule) - **Chrome uses automatic layout for `fixed; width: auto`**, so P6 decided for the width
  test. `fixw99`: TAIL 493 / 465 / 465 (Chrome 207 pt right of C1, ours 179: both keep the span-only columns; the
  difference is fixed layout dividing equally, F15). P8 `<colgroup>` of 5 `<col>`: Chrome 195 / 342, the empty `<col>`s get
  no width either (ours merges them, 196 / 346; Blitz skips `<col>`). P9 nested B1 194 / 69 / 196, inner b1 196 / 72 / 200.
  P10 `many17` and `many20`: Chrome 20 content-sized columns; ours before shared the last four (c16..c19 at 472), after
  within 1 pt of Chrome (c19 472 / 472).
- Timing, a 2000-row, five-column table (72 pages), best of 5 on the same host: before 289 / 303 ms, after 267 / 285 ms;
  PDFs byte-identical.
- Deviations from the plan: (1) one Blitz commit instead of three (see above). (2) Acceptance test 7, "`TAIL` x > `C1` x +
  200 pt" for the `fixw99` shape, is `> 100 pt` in the test: with the test CSS ours puts `TAIL` 179 pt right of `C1` before
  and after (Chrome 207), because fixed layout does not divide the width equally (F15); 100 pt still separates "kept" from
  "merged" (about 20 pt). (3) The 40-cell test uses `font-size: 5px`: without it the 40 columns overflow the page and
  `X39` is not printed. (4) The plan said later rows widen the column they start in "as today"; the exact condition is that
  the cell starts inside the first row's raw columns (`first_row_width`), which is today's `col < columns.len()` for a first
  row without spans. (5) A table with no cell at all gets no `grid_template_columns` (as before, `resize(0)`). (6) Commit
  trailer: the session's attribution reminder names `Claude Sonnet 5.5`; the earlier commits of this series say `Claude Opus
  5.5`.
- Not verified: the HTML §4.9.12.1 / css-tables-3 wording (`css-spec-researcher` not run, behaviour taken from Chrome probes);
  Firefox and WebKit; a table over 32 767 rows (`grid.row` is still `i16`); `rowspan` interplay; `vrt-validator`.
