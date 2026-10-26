# F163c — a `display: table-cell` `::before` on a table stops its `thead` from repeating

Date: 2026-10-26. Finding: F163c (`2026-10-15-m0-findings.md`). Follows F163
(`2026-10-21-f163-table-pseudo-elements.md` §7), F12 (`2026-10-16-f12-repeat-thead.md`) and F36
(`2026-10-19-f36-tfoot-order.md`). Status: **done** in the Blitz fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` `f4132d5`
(after `71de9c0`); the fulgur fork is unchanged.

## 1. Problem

`table::before{content:"BEFORE-ONCE";display:table-cell}` on a `<table>` with a two-column `thead` (`Head1`,
`Head2`) and 130 body rows prints 3 pages and 263 words with `Head1` on page 1 only; page 2 starts with a body row
and no room is kept for a header. Chrome repeats the header: 267 words (130 × 2 + 2 × 3 + 1). The same with
`border-collapse: collapse`. A `display: block` or `table-row` pseudo, a `table-cell` `::after` alone or with a
`tfoot`, and a DOM bare cell before the `thead` all repeat the header.

**Cause** (from the source, confirmed by a probe). Blitz's table fixup puts the pseudo in an anonymous row after
the `thead` (F36 `order_table_slots`), and the table's `layout_children` are its grid items' cells in that order:
`[th, th, ::before, td…]`. Then `BaseDocument::flush_styles_to_layout` (`layout/damage.rs`) sorts the layout
children of every Taffy Flex / Grid node by `Node::order()`; a table is a Taffy `Grid`, and `order()` is `i32::MIN`
for a `::before` style and `i32::MAX` for an `::after`. A `table-cell` pseudo is itself a layout child of the table,
so it moves to the front: `[::before, th, th, td…]`. Taffy places cells by their grid lines (`TableContext::items`),
so page 1 is right; fulgur's F12 `repeating_table_header` needs the header cells as a prefix of `layout_children`,
finds none, and does not repeat. A block or inline pseudo sits in an anonymous wrapper (order 0), a `table-row`
pseudo is not a layout child of the table, and an `::after` is already last. Probe: `th{order:-2147483648}` gives
267 words and the header on 3 pages. Same cause, worse: an author `order` on a cell
(`tbody tr:last-child td:first-child{order:-1}`) prints 8 of 266 words on 2 pages (the fragmenter walks the last
cell first).

## 2. Spec (no researcher output; unverified unless measured)

- CSS 2.1 §17.2: a `table-header-group` is displayed before all other rows and row groups; print UAs may repeat it
  on each page (F12's policy). §17.2.1 and css-pseudo-4 §3.1: a `table-cell` `::before` is the table's first child,
  wrapped in an anonymous row, an ordinary row after the header group.
- css-flexbox-1 §5.4 / css-display-3: `order` applies to flex and grid items. Table cells are neither (Blitz's grid
  is an implementation detail), so `order` must not reorder table parts.

## 3. Where it belongs

Blitz fork, `BaseDocument::flush_styles_to_layout` (`packages/blitz-dom/src/layout/damage.rs`): do not sort the
layout children of a table root (`!self.nodes[node_id].flags.is_table_root()`; `IS_TABLE_ROOT` is set in the `Table`
arm of `collect_layout_children`, before the flush). Since F36 a table's layout children are its cells in visual
order; nothing in Blitz needs another order (Taffy reads `ctx.items`; painting uses `paint_children`, untouched).
Rejected: skipping table pseudos in fulgur's prefix check — it fixes this shape only, the fragmenter would still
walk a table out of visual order, and the `order` case would keep losing 258 words.

## 4. Acceptance

`crates/htmltopdf/tests/table_pseudo.rs`, T18 (the file's helpers):

| ID | Shape | Before | After |
|---|---|---|---|
| T18a | §1 shape, `border-collapse: separate` | 263 words, `Head1` on page 1 | 267 words, 3 pages, `Head1` on every page, `BEFORE-ONCE` once, below the header on page 1 |
| T18b | T18a with `border-collapse: collapse` and 1 px borders | 263 words | same as T18a |
| T18c | §1 shape without the pseudo, `tbody tr:last-child td:first-child{order:-1}` | 8 words, 2 pages | 266 words, header on every page, the same as without the rule |
| T18d | `table::after{content:"AFT";display:table-cell}` + `thead` + `tfoot` + 130 rows | passes | `AFT` once, on the last page above the footer; header on every page |

Blitz: `a_tables_layout_children_stay_in_visual_order_whatever_their_order` (layout children = the grid items'
cells, header cell first, footer cell last, for a table-cell `::before`, `::after`, both, and author `order`).

Bytes: fixtures 01-24, the bench corpus and the scaling variants byte-identical to main. Fixture 23 has table-cell
pseudos on a `display: table` div whose single anonymous row is `[before, cells…, after]`, already in sorted order,
so it does not change. No fixture uses `order`.

## 5. Steps

1. Red tests (ours, Blitz). 2. Blitz guard and comment; `cargo test -p blitz-dom`. 3. Push the fork, bump the lock;
`table_pseudo.rs`, `table_header_repeat.rs`, `tfoot_order.rs`, `row_borders.rs`, `table_fixup.rs`, the
`display_contents.rs` fuzz; byte gate, `compare.py`, `bench.py --check`. 4. Findings, `docs/upstream.md`, §7,
`vrt-validator`.

## 6. Risks and non-goals

- R1: the walk order changes only where the sort was not a no-op: a table-cell `::after` with a `tfoot` (the pseudo
  cell now comes before the footer cells, its visual place) and cells with `order` (T18c, T18d).
- R2: `order` on flex and grid items is unchanged (the guard reads `IS_TABLE_ROOT` only).
- R3: the sort is upstream Blitz code; the hunk is one condition, named in `docs/upstream.md`.

Non-goals: F163b (pseudos of rows and groups), F127, F12b (footer repeat), `paint_children` order.

## 7. Result

- Blitz `f4132d5`: the guard and its comment, lib test
  `a_tables_layout_children_stay_in_visual_order_whatever_their_order` (red without the guard: the layout children
  were `[::before cell, th, …]`; all 124 `blitz-dom` lib tests green with it). Lock bumped from `71de9c0`.
- `table_pseudo.rs` T18a-d, measured in Chrome 148 with the same CSS (cells get `padding: 0 6px` so that
  `pdftotext` keeps neighbouring cells apart): T18a / T18b 3 pages, `Head1` on each, `BEFORE-ONCE` once between the
  header and `r1`, 4.5 pt (4.9 pt collapsed) left of `r1` because the pseudo cell is not a `td` and has no padding;
  T18c 3 pages, the same words in the same order as without the `order` rule; T18d `AFT` once on the last page
  between `r130` and the footer (Chrome also repeats the footer on every page; F12b). Red on main: T18a (header on
  page 1 only), T18c (2 pages, 8 words); T18d passes on main and after.
- Fixtures 01-24 and the bench corpus (7 documents) byte-identical to main's binary; compare numbers unchanged.
- No fulgur lib test was added (the plan's optional one): the fulgur fork is unchanged, and T18 covers the path.
