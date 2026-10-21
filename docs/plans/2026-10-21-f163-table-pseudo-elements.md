# F163: `::before` / `::after` on a table box panics Blitz or is silently not painted

Date: 2026-10-21. Finding: F163 (`2026-10-15-m0-findings.md:884-886`). This also closes the pseudo-element bullet of F30e-b (`:297`). That bullet says "a real `<table>` is not affected", which is wrong: `<table></table>` panics. Status: planned.

**Owner action: none.** Both forks already exist:

- Blitz fork: `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`.
- fulgur fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, locked at `945667d`.

Our crate gets tests, fixture 23 and the lock bump.

Line references:

- Blitz: the working tree of `/home/user/blitz`. The panic site `table.rs:1275` / `:1373` matches the investigation, but I had no git access to confirm the tree is `d04479a`. Step 0 checks it with `git -C /home/user/blitz rev-parse HEAD`.
- Taffy: read from the 0.9.2 registry copy. I assume our fork `7bbedd5` (F99d, track sizing only) does not touch the loop cited here; step 0 checks.

## 1. Problem

A table box (`<table>`, `display: table`, `inline-table`) with a `::before` or `::after` panics with "index out of bounds" when it has fewer rows than pseudo-elements. The library returns `EnginePanic` and the document is lost. With enough rows it does not panic, but the generated text is silently not painted. No committed fixture shows this; the grep in §4 finds no table pseudo anywhere we render. The repros are in `/tmp/claude-0/f163/repro/`:

- `min-before-no-rows.html`, `<style>table::before{content:"B"}</style><table></table>`: panic.
- `min-before-after-one-row.html`: panic.
- `min-div-table-after.html`, a `display:table` div with a bare cell and `::after`: panic.
- `ok-before-one-row.html`: no panic, but `B` is missing.
- `gen.py`, 500 seeds: 176 panic, all at this site; stripping the `<style>` removes every panic.

Mechanism, verified in source:

- **Pseudos are created but never placed.** `collect_layout_children` flushes the table's pseudos (`construct.rs:148`). The `Table` arm then builds the grid from the DOM `children` only (`build_table_context`, `table.rs:86`, `:121`). Afterwards it pushes `before` / `after` around the grid's cells into the table's `layout_children` (`construct.rs:341-347`).
- **Two counts disagree.**
  - `TableTreeWrapper::child_ids` / `get_child_id` index `ctx.items` (rows + cells, `table.rs:1270-1283`).
  - `child_count` returns `doc.child_count(table)` = `layout_children.len()` = pseudos + cells (`:1275-1277`, `layout/mod.rs:58-65`).
- **Why it panics.** Taffy's "position hidden and absolutely positioned children" loop runs over `0..child_count` and calls `get_grid_child_style(get_child_id(i))` (taffy `compute/grid/mod.rs:528-532`). So it indexes past `items` exactly when pseudos > rows (`table.rs:1373`).
- **Why the text is lost when it does not panic.** The pseudo box is a layout child that Taffy never lays out. fulgur's `convert_table` walks DOM `children` only (`fulgur convert/table.rs:63`), so nothing paints it.

Pseudos on rows, row groups and `display: contents` wrappers inside a table do not have the count mismatch. They are never flushed, because rows and groups are not constructed (`collect_row` clears their damage, `table.rs:1080`), so they are dropped silently. Pseudos on cells are ordinary cell content and work.

## 2. Spec

There is no `css-spec-researcher` output. Every item below is from memory and **unverified**.

- **CSS 2.1 §12.1 / css-pseudo-4 §3.1.** `::before` / `::after` behave as if they were real elements inserted as the first / last child of their originating element.
- **CSS 2.1 §17.2.1, rule 2 (missing wrappers).** A child of a table that is not a proper table child is wrapped, together with its consecutive non-row siblings, in one anonymous `table-row`. Inside that row, non-cells are wrapped in one anonymous `table-cell`. So an inline or block `::before` becomes an anonymous row/cell before the first row. A `table-cell` `::before` becomes a cell of an anonymous row. A `table-row` `::before` is a row. Rule 1 (whitespace) does not apply to generated content.
- **CSS 2.1 §17.2.** The first header group is displayed before all other rows; the first footer group after all of them. So a `::before` row follows `thead`, and an `::after` row precedes `tfoot` (F36).
- **CSS 2.1 §17.5.2.1.** Fixed layout takes column widths from the first row, which can now be the `::before` row (risk R4).
- **CSS 2.1 §9.7.** `position: absolute` / `float` blockify a pseudo.
- **css-tables-3 §3.3 "fixup".** Restates §17.2.1. Its treatment of out-of-flow children is not checked.
- **WPT:** `css/css-tables/` fixup tests and `css/CSS2/tables/` anonymous-box tests. Specific files were not checked.

## 3. Where it belongs

**Decision: Blitz fork (A) + (B), and a small fulgur fork change (C). One PR.**

Why not our crate:

- Rewriting the DOM changes what selectors match (`tr:first-child`, `:nth-child`); this was rejected for the same reason in F111 and F150.
- Dropping the pseudos loses author content.

(A) and (B) share one cause: the table's pseudo boxes are layout children but not grid items.

**(A) Crash fix, must land regardless (Blitz `layout/table.rs`).**

- `TableTreeWrapper::child_count` returns `self.ctx.items.len()`, so it agrees with `child_ids`.
- The Taffy loop then visits every item. For each one it does nothing, because no item is ever `display: none` or absolutely positioned:
  - hidden parts are skipped (`release_skipped`, `:992-1001`);
  - absolute or fixed rows and cells are blockified to `Other` and wrapped in a static anonymous cell (`:770-803`).
- So documents without table pseudos stay byte-identical.
- It is its own commit. If (B) or (C) fails review, (A) ships alone and (B) is filed as F163a.

**(B) Table pseudos as F50 slots (Blitz).**

- `table.rs`, new `fn table_children_with_pseudos(doc: &BaseDocument, table_id: usize, children: &[usize]) -> Cow<'_, [usize]>`:
  - It returns `[before] ++ children ++ [after]`.
  - It borrows `children` when the table has no pseudo, so tables without pseudos allocate nothing new.
  - `build_table_context` (`:121`) passes its result to `collect_table_children` at `Level::Table`.
  - `table_part_of` already classifies the pseudo node (an `AnonymousBlock` with styles, `:780`) by `display`. `plan_table_children` and `order_table_slots` (F36) then place it unchanged.
  - The anonymous cell's `parent` is the table (`create_anonymous_block`, `construct.rs:693`), which is what fulgur's `anonymous_table_wrapper` expects.
- `construct.rs:341-347`: the `Table` arm stops pushing `before` / `after`. They now reach `layout_children` only as an anonymous cell, a cell item or the content of a row.
- **Conditional on probe T13** (column widths with a `::before` row): a `GridBuilder` field `width_row: Option<u16>`, the first grid row that is not made only of a table pseudo's anonymous cell. `WidthEvent.first_row` compares against it.

**(C) fulgur (`convert/table.rs`, `blitz_adapter.rs`).**

- New `blitz_adapter::table_box_children(node: &Node) -> impl Iterator<Item = usize> + '_`, yielding `before`, `children`, `after`. Both of these use it instead of `table.children`:
  - `convert_table` (`:62-94`);
  - `collect_row_backgrounds` (`:279`), so a `display: table-row` pseudo keeps its background.
- A pseudo that is not a row, group or cell and has no anonymous wrapper (a `table-caption` pseudo) is skipped, not handed to `collect_table_cells`.
- `is_anonymous_table_cell` (`:1163-1177`): update the doc comment, which says pseudo boxes are pushed into the table's layout children. Keep the guard.
- `split_barrier` (`pagination_layout.rs:5099-5102`) needs no change: the wrapper is an anonymous cell of the table, and a `table-cell` pseudo is caught by its display.

**Out of this PR: F163b.** Pseudos of rows, row groups and contents wrappers in tables have a different cause (they are never flushed). Fixing them touches `collect_row` / `collect_row_group` / `flatten_contents` and fulgur's `row_cells` / `collect_table_cells`. It is filed with the step-0 evidence.

## 4. Acceptance

New `crates/htmltopdf/tests/table_pseudo.rs`. It uses the `display_contents.rs` helpers (`try_render`, `words`), bundled Inter and strict fonts. The Chrome constants come from step 0, rendered with the bundled Inter loaded through `@font-face` (the F154 method). Positions are offsets from the first word, tolerance 1.5 pt.

| ID | Shape | Main | Expected (Chrome to confirm in step 0) / assert |
|---|---|---|---|
| T1 | `<table></table>` + `::before "B"` | panic | `Ok`, `B` printed |
| T2 | `::before` + `::after`, one row `b` | panic | `B`, `b`, `A` top to bottom, same `x` (column 1); **geometry** |
| T3 | `display:table` div: bare cell `x` + `::after "A"` | panic | one anonymous row: `x A` on one line, `A` right of `x`; **geometry** |
| T4 | `::before` + one row (`ok-before-one-row`) | `B` lost | `B` above `b` |
| T5-T7 | pseudo `display: block` / `table-cell` / `table-row`, 2-column table | T5, T6 lost | own row above row 1, column 1 |
| T8 | pseudo `display: table-caption` | lost | `Ok`; caption `#[ignore = "F30e-b"]` |
| T9 | `tfoot, tbody, thead` written in that order + both pseudos | lost | `H B body A F` (F36); **geometry** |
| T10 | `inline-table` with `::before` in a `<p>` | measure | word order of the line |
| T11 | `border-collapse` + borders + `::before` (F22) | lost | `B` inside the table frame; row lines at Chrome's `y` |
| T12 | 60 rows, `thead`, both pseudos (F12) | measure | header on every page; `B` once on page 1, `A` once on the last page; page count = Chrome |
| T13 | auto and fixed layout, `td` widths in row 1, `::before` | measure | `x` of each column = the same table without the pseudo, if Chrome agrees (it decides the `width_row` rule) |
| T14 | `tr` / `tbody` / `td` / contents-wrapper `::before` | `td` fine, rest lost | `Ok`; `td` word guard; rest `#[ignore = "F163b"]` |
| T15 | `content: ""`, `content: counter(c)`, `position: absolute` pseudo | measure | `Ok`; table height vs Chrome recorded (an empty row adds border spacing) |
| T16 | every `gen.py` seed 0-499 (`<table>` tags) | 176 panic | 0 panics (one-off run, recorded in §7) |

**Blitz tests** (`table.rs` `mod tests`, `doc.resolve`):

- For every table root, `child_count == child_ids().count()`.
- T1 resolves without panicking.
- `layout_children` of T2 = `[anon(B), td, anon(A)]`, and no pseudo id is listed directly.
- T3 has one grid row with two cells.
- T9 slot order.
- Caption pseudo skipped; `table-cell` pseudo is a `Cell` item; `table-row` pseudo is a `Row` item.

**fulgur tests:**

- a lib test that `convert_table` reaches the pseudo's wrapper;
- a `render_smoke` case for T1-T3 (the codecov rule in fulgur's `CLAUDE.md`).

**Fuzz:** the F150 generator's `Gen::table` gets three additions:

- a class with a per-seed CSS rule (`.pN::before { content: "wNNNN"; display: … }`) passed with `FUZZ_CSS`;
- tables with 0 rows or with bare cells;
- `::after`.

The oracle stays wrapped against plain, plus the pseudo tokens present. Every 8th seed renders twice and the bytes must be identical. 64 seeds run in CI.

**Bytes:**

- The grep found table pseudos in none of fixtures 01-22, `bench/`, fulgur VRT or the fulgur examples. Fixtures 21 and 22 put pseudos on a `section` and a `p`, not on table parts.
- `tailwind.css` has `*,:before,:after` rules without `content`, so Stylo creates no pseudo box.
- Gate: fixtures 01-22, the bench corpus and the scaling variants are byte-identical to main, 1 and 4 threads. Pixel-diff change for them: 0.
- New `fixtures/invoices/23-table-pseudo/`, required by the "fixtures are the spec" invariant: a Czech invoice whose items table carries an `::after` note, plus a `display:table` recap with a `::before` label and bare cells. Main produces no PDF (`EnginePanic`). The threshold is the fix's number + 1 point.
- fulgur `cargo test -p fulgur`; the VRT goldens do not move (only "the same goldens fail" is compared, as in F154).

## 5. Steps

0. **Measure (½ day).**
   - Chrome T1-T15 with `/tmp/claude-0/-home-user/a456d4dd-daaf-5a66-be03-3af9ac054072/scratchpad/chrome-pdf.mjs`, plus main's output.
   - Confirm the Blitz head and the Taffy loop in the fork.
   - Write the red tests (ours, Blitz, fulgur). Draft fixture 23.
1. **(A) (½ day).** `child_count`. Green: no panic in T1-T16 and the Blitz count test. Byte gate on this commit alone.
2. **(B) (≤1 day).** `table_children_with_pseudos`, the construct arm, and `width_row` if T13 requires it. Green: the Blitz tests and `cargo test -p blitz-dom`.
3. **(C) (½ day).** `table_box_children`, both walks, the skip, the comment. Green: fulgur lib and `render_smoke`, through a local `[patch]` path.
4. **Wire up (1 day).**
   - Push the forks and bump the lock.
   - Green: `table_pseudo.rs`, the fuzz (64 seeds, and 2000 once in release), `table_fixup.rs`, `tfoot_order.rs`, `row_borders.rs`, `table_header_repeat.rs`, `table_wrapper_split.rs`.
   - Byte gate, `compare.py`, then look at `out/compare/23-*`. `bench.py --check`.
5. **Records (½ day).**
   - Fixture 23 references and threshold.
   - `docs/upstream.md` fork row.
   - Findings: F163 done, F30e-b bullet corrected and closed, F163b filed.
   - Run `vrt-validator`.

## 6. Risks and non-goals

**Risks**

- **R1, F36 order.** `order_table_slots` puts the `::before` row after the first header group and the `::after` row before the first footer group. T9 checks that Chrome agrees. If not, record it; do not special-case.
- **R2, F22.** The extra anonymous row is a `RowOwner { row: None, group: None }`, which is already handled for F50 rows. Grid line indices shift; fulgur reads them from Blitz, not from counting `<tr>`s. T11 checks the lines.
- **R3, F12.** A leading anonymous row could hide the header if it came first. The header is moved first (R1), so it does not. T12 checks it.
- **R4, column widths.** The `::before` row becomes the "first row" (`fold_column_width`, `:579-616`). In auto layout this would stop later explicit widths from widening columns 2 and up; in fixed layout it would set the columns. T13 decides between `width_row` and Chrome's behaviour.
- **R5, F127.** Unchanged: `flatten_contents` and the slot plan of DOM children are untouched.
- **R6, rebase cost.** `table.rs` and `construct.rs` carry F13, F22, F36, F50, F150 and F154.

**Non-goals**

- F163b (pseudos of rows, groups and contents wrappers in tables).
- `table-caption` pseudos (F30e-b).
- `table-header-group` pseudos act as body groups (`group_role_of` requires an element).
- Absolute, fixed or float table pseudos are wrapped in flow, as DOM children with the same style are today. File a finding if T15 differs from Chrome.
- `content: url()` / counters (Blitz flushes strings only, `construct.rs:474-485`).
- Tagging of anonymous-cell text (F30e-b).
- `text-transform` on generated text (F160).

## 7. Result

(to be filled)
