# F50 — A table part whose `display` is not a table display loses its content

Date: 2026-10-16. Finding: F50 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix goes on `subapavel/blitz`
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
