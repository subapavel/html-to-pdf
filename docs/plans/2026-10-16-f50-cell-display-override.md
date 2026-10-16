# F50 — A table part whose `display` is not a table display loses its content

Date: 2026-10-16. Finding: F50 in `2026-10-15-m0-findings.md`. Status: **landed**. Fix goes on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` (anonymous table objects) plus a small change on `subapavel/fulgur` `htmltopdf/srgb-colours`
(convert walk, row backgrounds), both carried via `[patch.crates-io]`.

## 1. Problem

`<td style="display:block">` (Tailwind `block` / `flex` on a `td`/`th`) is painted nowhere and its row collapses:
`tb_blk_noh` puts `N` at 47 pt where Chrome has 128, and all five `A` lines are missing (`/tmp/claude-0/vrt-f48/s/`).
Chrome (`tb_blk_noh.chrome.pdf`) shows the row almost unchanged: the content keeps the block's own 4 px padding, at the
same x as `N`, in a row exactly as tall as the block (108 px). `cell_flex.chrome.pdf` shows that a
`td { display:flex; height:30px }` is no longer a cell: its `height` is a real height (30 px row under preflight
`border-box`) and the content overflows over `N`. So F48's "height is a minimum" rule must not apply there — and it
does not, because it is keyed on `DisplayInside::TableCell`.

Root cause (from reading the forks):

1. Blitz `layout/table.rs:295-305` (`collect_table_cells`) skips every table descendant whose `display` is
   `Flow`/`FlowRoot`/`Flex`/`Grid` ("Probably a table caption: ignore") and removes its construct damage; `:306-309`
   does the same for a nested `display: table` and for columns; `:177-179` returns for text nodes, so the text of a
   `td { display: contents }` is lost although `:191-203` recurses into it.
2. A skipped node is no `TableItem`, so `build_table_context` (`:93-97`) leaves it out of the table root's
   `layout_children`; `resolve_layout_children` (`document.rs:1065-1111`) only constructs layout children, so the node
   is never constructed or laid out and its `final_layout` stays zero.
3. fulgur `convert/table.rs:260-290` recurses through zero-size nodes and skips zero-size leaves: nothing is painted.
   The fragmenter walks the table's `layout_children` (`pagination_layout.rs:2110`), so the row is only as tall as its
   remaining cells.
4. The same `match` runs at row-group and table level, so `tr.block`, `tr.flex`, `tbody.block`, `thead.flex` lose the
   whole row or body (derived; P5–P6 confirm). This is the print shape of the "stacked on mobile" template
   (`block md:table-cell`): `md:` does not match on A4 (≈ 680 px < 768 px).
5. `table { display: block }` is not a table root: its `tbody`/`tr` reach the `_` arm of `construct.rs:371-373`, are
   laid out as Taffy `Display::DEFAULT` = Flex (stylo_taffy `convert.rs:168-171`), and their whitespace text nodes become
   layout children, which `layout/mod.rs:142-150` reports with `println!` (stdout). Unverified (P8). Out of scope: F50b.

## 2. Spec

- CSS 2.1 §17.2.1 (wording from memory, unverified), anonymous table objects: (1) whitespace-only text between table
  boxes is treated as `display: none`; (2) missing wrappers — a run of consecutive non-`table-cell` children of a
  `table-row` gets one anonymous `table-cell`; a run of non-row children of a row group or table gets one anonymous
  `table-row` (then rule 2 applies inside it); (3) missing parents (cell or row outside a row or table): F50b.
  css-tables-3 §2.2 "fixup" restates the three phases.
- css-display-3 §2.4/§2.5: a box's table role comes from its computed `display`, not its tag; HTML §15.3.8's
  `td { display: table-cell }` is overridden by author CSS; `display: contents` children take part in the parent's
  fixup. CSS 2.1 §9.7: `float` or `position: absolute` blockifies a `td`, so it is wrapped too.
- CSS 2.1 §9.2.1.1: anonymous boxes inherit from the enclosing non-anonymous box; their `vertical-align` is the initial
  `baseline` (our F13 code treats it as `top`).
- CSS 2.1 §17.5.1: row and row-group backgrounds are painted behind their cells, anonymous ones included.

## 3. Design

Blitz builds the box tree, so the fixup belongs there. One rule at three levels inside `collect_table_cells`:
**partition the children (with `display: contents` flattened) into table parts and wrap each run of misparented
children.** Nodes are created only for runs, so documents without one keep their node ids and stay byte-identical.

```rust
enum TablePart { Cell, Row, RowGroup, Column, Caption, Hidden, Whitespace, Other }
enum Level { Table, RowGroup, Row }
enum RowSlot { Cell(usize), AnonymousCell(Range<usize>) }
enum Slot { Keep(usize), AnonymousRow(Vec<RowSlot>) }
fn table_part_of(node: &Node) -> TablePart;                        // computed display; text: Whitespace / Other
fn plan_row(parts: &[TablePart], range: Range<usize>) -> Vec<RowSlot>;
fn plan_table_children(level: Level, parts: &[TablePart]) -> Vec<Slot>; // pure, unit-tested
fn flatten_contents(doc: &BaseDocument, ids: &[usize], out: &mut Vec<usize>);
pub(crate) fn create_anonymous_block(doc: &mut BaseDocument, style_parent: usize) -> usize; // from construct.rs:521-555
```

- Row level: each `AnonymousCell` run gets one anonymous block (children = the run, `ServoAnonymousBox` style
  inheriting from the row, `parent` = the row — F12's `in_group`, `pagination_layout.rs:3198`, needs it), pushed through
  the existing cell path (`:233-294` extracted as `push_cell_item`; width `auto`, colspan 1). `Whitespace` and `Hidden`
  never open a run but stay inside an open one; a run's trailing ones are dropped (same as the spec under collapsing
  `white-space`).
- Row-group and table level: an `AnonymousRow` does `row += 1; col = 0`, pushes no `Row` item (no `height`), and its
  slots go through the row-level path, so a run of bare cells becomes a row (`display: table` divs). `Caption` stays
  skipped (fulgur's `CaptionRestructurePass` moves `<caption>` out before Blitz), as does `Column`. Run members keep
  their construct damage; the anonymous cell is in the table's `layout_children`, so it is constructed and laid out
  like any cell.
- A row or row group with an anonymous slot gets `layout_children = Some(cells / rows incl. anonymous, DOM order)`; all
  others are reset to `None`. fulgur's convert step walks layout children first (`convert/table.rs:260-262`), so it
  reaches the anonymous cells without a change.
- fulgur (table level + backgrounds only): `convert_table` (`convert/table.rs:58-71`) walks the table's DOM children to
  tag the header group, so a table-level anonymous cell is never reached.
  `blitz_adapter::anonymous_table_wrapper(doc, table_id, child_id) -> Option<usize>` (a child whose `layout_parent` is
  an `AnonymousBlock` whose `parent` is the table) maps the run to its wrapper, visited once with `convert_node`.
  `row_cells` and the group loop in `collect_row_backgrounds` (`:131-183`) walk layout children first and count an
  anonymous block with a box as a cell, so a striped `tr` stays striped behind a `td.flex`.
- Unchanged: F13 alignment (anonymous cells are not `TableCell`, `baseline` behaves like `top`), F48 (keyed on display),
  F5 widths (`auto`).

## 4. Acceptance

New `crates/htmltopdf/tests/table_fixup.rs` (F48's CSS and helpers: `render_with`, `words` + `x_min`, `word_at`,
`assert_paginated`, `page_count`). Red before the fix unless marked.

1. `tb_blk_noh` shape: all five `A` lines; `N` 81 pt (108 px) below `A0000`, ±1 (Chrome 128 − 47).
2. `td { display:flex; height:30px }` with two `<p>`: `B0000` on `A0000`'s line, to its right; `N` 28.5 pt below
   `A0000` (content-box 38 px; preflight border-box 22.5 pt, P3); the content overflows as in Chrome.
3. `<td>C1</td><td class=block>X</td><td class=block>Y</td><td>C4</td>`: `X` and `Y` share an x; `Y` 21 pt below `X`
   (one anonymous cell); `C4` right of `X`.
4. One row each for `grid`, `inline-block`, `list-item`, `contents`, nested `display:table`: every word present. Two
   `inline` cells: `IA IB` on one line.
5. `tr.block` and `tr.flex` between two normal rows: their words present, vertically between the neighbours
   (horizontal placement is F50b).
6. `tbody.block`, and the card shape `tr.block` + `td.block`: every word, DOM order, top to bottom.
7. A `display:table` div with two bare `table-cell` divs then a `table-row`: the two cells on one line, the row below
   (red or guard, P7).
8. A 120-row table with a normal `thead` and a `td.block` description per row: paginates, header repeats.
9. `tr { background: #eee }` with a `td.flex`: a pixel inside the flex cell is `#eee`.
10. Guards (green before and after): `td.hidden` leaves no gap; whitespace between normal cells creates no cell; F48's
    and F13's suites.

Blitz unit tests: `plan_table_children` at three levels, mixed runs, whitespace/hidden trimming, caption and column
skipped. fulgur: a unit test in `convert/table.rs` and a `render_smoke.rs` case. Fixtures 01–09 byte-identical (no
fixture puts a non-table `display` on a table part; grep). New fixture **`10-stacked-items`**: 06's header and
parties, ≈ 12 items as `tr.block` / `td.block` with a `td.flex justify-between` label/value line, totals with a
`td.flex`; classes already in the compiled `tailwind.css`. Today the items are missing; expected: Chrome's page count and
item order. Commit the Chrome reference; threshold = worst page + 1 point.

## 5. Steps

1. Ours: red tests 1–10, fixture 10 + reference. Chrome probes in `out/scratch/f50/`, recorded in §7: P1 `tb_blk_noh`;
   P2 test 3; P3 `cell_flex` content- and border-box; P4 test 4 incl. the `inline` space; P5 `tr.block` with real cells
   (expected: a nested anonymous table); P6 `tbody.block`, and a normal `thead` with `tbody.block`; P7 bare cells in a
   `display:table` div, and a cell after a row; P8 `table.block` (ours: layout and stdout); P9 test 9; P10 `td.block` as
   the first cell of a repeated `thead` row.
2. Blitz, output-neutral: extract `create_anonymous_block` and `push_cell_item`; fixtures `cmp` identical.
3. Blitz: `table_part_of`, `plan_table_children`, `flatten_contents` + unit tests (unused yet).
4. Blitz: row level + row `layout_children`; tests 1–4 and 8 green.
5. Blitz: row-group and table level; tests 5, 7, 6 (except `tbody.block`).
6. fulgur: `anonymous_table_wrapper` in `convert_table`, background coverage, lib and smoke tests; tests 6 and 9;
   `cargo test -p fulgur`, `fulgur-vrt` goldens untouched (a moved golden is inspected against Chrome and noted).
7. Ours: pin both forks; full `cargo test --release`; fixtures 01–09 identical, 10 at its threshold; 2000-row table
   byte-identical and timed, plus the same with `td.block`; `docs/upstream.md` (both rows), findings (F50 done, F50b
   filed); `vrt-validator`. `pdf-reviewer` not required.

## 6. Risks and non-goals

- Anonymous nodes are created on every construct of the table and not freed; Blitz's anonymous blocks do the same, we
  construct once, `incremental` is off.
- An anonymous block's computed `display` is `inline` (as for construct's wrappers): fulgur checks that read `display`
  (atomic inline, list item) must not treat it as inline-level; test 4 and fixture 10 guard this.
- The fulgur change touches F19 code; rebase conflicts with F30b's row-background path are likely.
- Non-goals: F50b (missing-parent fixup, `construct.rs` `_` arm: `table.block`, `tr.block` cells side by side, cells
  outside tables, the stdout `println!`); `white-space: pre` in a run; a second `<caption>`; `inline-table`; `rowspan`
  (F15); real `baseline` (F26); collapsed borders around anonymous cells (Blitz #386).

## 7. As landed

- Forks. `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`: `77a8593` (one commit: `create_anonymous_block` and `push_cell_item`
  extracted, then `table_part_of`, `plan_row`, `plan_table_children`, `flatten_contents`, `collect_table_children`,
  `collect_row`, `collect_row_group`, `push_row_slots`; 11 new unit tests, `cargo test -p blitz-dom` 26 passed). The
  extraction was checked output-neutral on its own (fixtures 01-09 `cmp` identical) before the rest went in.
  `subapavel/fulgur` `htmltopdf/srgb-colours`: `e81629c` (`anonymous_table_wrapper`, `convert_table`, `row_cells` /
  `cell_box` / group loop in `collect_row_backgrounds`; 3 lib tests, 1 `render_smoke` case). Pinned in `Cargo.lock`.
- Tests. `crates/htmltopdf/tests/table_fixup.rs`, 14 tests (the plan's 1-10). Red before the fix: 12 of 14 (`A0000` ..
  `A0004` not printed and `N` 47 pt below; flex cell `B0000` missing; `X`, `Y` missing; `R1b`-`R5b` missing, `IA`
  missing; `U1` missing for `tr.block`, `tr.flex`, `tbody.block`; `Title` missing for the card; `K3` 14 pt *above* `K1`
  and `K2` on the same row as `K1`; 120 `Desc` words missing; the pixel inside the flex cell white instead of `#eee`),
  green before and after: `td.hidden` leaves no gap, whitespace between cells creates no cell. After the Blitz fix alone
  12 of 14 were green; the row-background test (9) needed the fulgur change. Whole `cargo test --release --locked` green
  (23 binaries; `fixed_height_cells` 19, `cell_vertical_align` 14, `nested_paragraph_split` 35 + 1 ignored). fulgur with
  our Blitz: lib 1989 (was 1985), `render_smoke` 237, `fulgur-vrt` all green with its goldens untouched;
  `examples_determinism` only `committed_svg_matches_rendered` (environment). `cargo fmt`, `clippy -D warnings` green in
  ours and fulgur; Blitz fmt clean, clippy has the same 4 warnings before and after.
- Chrome 141 vs ours (`out/scratch/f50/`, pt, y = baseline of the word; Chrome / ours after, before in brackets):
  - P1 `tb_blk_noh`: `N` 81 pt below `A0000` in both (before 47 pt from the top, five `A` lines missing); `A0000` at the
    same x as `N`.
  - P2 `C1 | X Y | C4`: `X` and `Y` one 21 pt line apart, same x (ours 215.8, Chrome 223.4: the auto column widths differ);
    `N` 21 below `Y`; `C4` x 382 vs 354.
  - P3 flex cell with `height:30px`: content-box `N` 28.5 pt below `A0000` (Chrome 28.5), border-box 22.5 (Chrome 22.5);
    `B0000` on the line of `A0000`, x 80.0 vs 79.7.
  - P4 `grid`, `inline-block`, `list-item`, `contents`, nested `display:table`: all words printed, same rows. Offsets
    from Chrome: `inline-block` 6.6 pt lower (it is 6.6 pt lower in an ordinary cell too, `p4c`), nested table keeps the
    `td` padding (Chrome does not), columns narrower. Two `inline` cells on one line, `IB` right of `IA`; no horizontal
    padding on inline boxes in ours, so without a space the words merge (`IAIB`).
  - P5 `tr.block` with real cells: Chrome puts `U1 U2` side by side (nested anonymous table), ours stacks them (F50b); with
    `tr.flex` ours matches (`U2` x 64.3 vs 63.8). `T2` 21 pt below `U1` for `tr.flex` (Chrome 21); `tr.block`: 42 (stacked), Chrome 21.
  - P6 `tbody.block`: every word, `T2` 122.3 vs 122.5, `AFTER` 140.3 vs 140.5; with a normal `thead` (`H1`, `H2`) `U1`..`U3`
    match, `H2` at x 309.5 vs 385.2 (column widths); `tbody.block` with background, border and padding matches
    (`p6c.cmp.png`: the wrapper paints the block's own frame).
  - P7 bare `table-cell` K1, K2 then a row with K3: `K1`, `K2` on one line, `K3` 15 pt below, as Chrome (before: `K3`
    above). A cell after a row: `K2` below `K1`, same x (before: the same line).
  - P8 `table.block` (and `p8w`, pretty-printed): unchanged, wrong layout (all cells on one line) and 2028 bytes of `ERROR:
    Tried to lay out text node individually` on stdout from `layout/mod.rs:149`: F50b.
  - P9 `tr { background:#e6e6e6 }` with a `td.flex`: pixel in the flex cell `(230, 230, 230)` as Chrome (before white).
  - P10 normal `thead` with a `th.block` first cell, 120 rows: header `HD` on all 4 pages, 4 pages (before: `HD` missing
    everywhere); the rows match Chrome's (`r60` 584.3 vs 584.5, `r120` 374.3 vs 374.5).
- Fixtures. 01-09: PDFs byte-identical to the main (`db81570`) build (also `cmp` against the F48 head binary); the
  `compare.py` numbers are unchanged. New fixture 10-stacked-items: Chrome 2 pages; ours before 1 page (5.1 / 1.2 %, the items
  and totals missing), after 2 pages, 5.9 / 1.5 %, item 10 / 11 split at the same place, header repeated; threshold 0.069
  (worst page + 1 point). The residual is text drift (a card 0.5 px shorter) and the `<tr>` rules (Blitz #386, F22).
- Timing, 2000-row table (5 renders, release): plain 187-266 ms before 201-249 ms, output byte-identical; the same table with
  `<td style="display:block">` in the last column: before 202-237 ms (the cell was dropped), after 281-339 ms (2000 anonymous
  cells and 2000 more words are now laid out and painted).
- stdout: fixtures 01, 03-10 print nothing; 02 prints 216 bytes of CSS parse errors, the same with the F48 head binary.
- Deviations from the design: (1) `Level` has two variants (`Table`, `RowGroup`) and a row is planned by `plan_row`
  directly, instead of a third `Level::Row`; (2) Blitz is one commit, not three; (3) a `display: contents` element now
  takes part in the fixup: the old `Contents` arm of `collect_table_cells` was dead code (`display: contents` has outside
  `none`, so it was skipped as hidden), so its content was lost; (4) a table-part `display` that is out of place (a `tr` in
  a `tr`, a `tbody` in a `tbody`) is wrapped like any other box where the old code recursed into it; (5) a fulgur
  pagination test changed: a `<p>` that is a non-cell child of a `display: table` div used to be "placed as one unit", it is
  now in an anonymous cell and splits between lines like a real cell's paragraph (new test
  `a_paragraph_that_is_a_non_cell_child_of_a_table_box_splits_in_its_anonymous_cell`); (6) the `tbody.block` words were
  already printed with the Blitz fix alone (the block `tbody` is reached through its zero-size parent); the fulgur wrapper is
  needed for its own background and border.
- Not verified: the F50b cases above; `vrt-validator` (run by the coordinator); the fulgur lib tests were not run
  red-before in isolation (the anonymous-cell background is covered end to end by `table_fixup` test 9, red before the fulgur
  change); CSS 2.1 §17.2.1 wording is from memory (not re-read), as in §2.
