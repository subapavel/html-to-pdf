# F22: collapsed table borders (row and row-group rules, shared cell borders)

Date: 2026-10-19. Finding: F22 (`2026-10-15-m0-findings.md:209-213`). Status: planned. The work is split into two PRs. **F22a** (this PR) puts the collapsing border model into Blitz *layout*, on `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` after `310878c`. **F22b** (the next queue item) *paints* the resolved borders, on `subapavel/fulgur` `htmltopdf/srgb-colours` after `19c4782`. F22b's design is fixed here (§3.3) so both PRs share one contract. Line numbers are from those heads. I had no `css-spec-researcher` output.

## 1. Problem

Tailwind's preflight puts **every table in the collapsing border model**: `table{text-indent:0;border-color:inherit;border-collapse:collapse}` (`fixtures/tailwind/tailwind.css:2`). Blitz never reads `border-collapse` or `border-spacing`. No match in `packages/` or in fulgur's `src` outside tests. Stylo 0.8.0 does compute both for servo (`properties/longhands/inherited_table.mako.rs:7-16`, `:40-49`). As a result, every table is laid out and painted in the separated model, with `border-spacing: 0`.

That has two visible effects, and both are the "F22 drift":

- **Row and row-group borders are dropped.** Examples: `tr.border-b` (01, 02, 07, 09, 13, 16, 18), `tr.border-b-2` (04), `tr.border-t-2` (01, 02) and `tbody.divide-y` (04, 05).
  - In Tailwind v4.3.3 `divide-y` is `:where(.divide-y>:not(:last-child))`, which sets **`border-bottom-width: 1px`** and top `0` on every row but the last. It is a bottom border, not a top border.
  - Chrome collapses these borders into the cells, half on each side of the grid line, so **each ruled row is taller by the rule's width**. We are short by that amount on every rule.
  - 01: our totals block is about 3-4 px too high.
  - 04: the indigo `border-b-2` header rule is missing, and everything below the items table is about 4 px too high.
  - 02: Chrome ends page 1 after row 19; we fit row 20.
- **Shared cell borders are counted twice** (03 and 20, `td.border` with `border-collapse`). Each internal line is 2 px instead of 1 px, and each row is about 1 px taller than Chrome's. 20's pitch is 31.6 px against 30.7 px, which is 7 px over 11 rows (F36 §7). In `out/compare/03-logo-qr-p1.png` the diff traces every grid line of the table.

The rules alone barely count in the compare numbers. `scripts/compare.py:26` ignores channel differences of 32/255 or less. Tailwind gray-100 (#f3f4f6, Δ12) and gray-200 (#e5e7eb, Δ26) rules are invisible to it; gray-300 (Δ46), gray-900 and indigo-600 are not. **What moves the numbers is the geometry**: every line of text below a ruled table is shifted.

Unrelated, and not part of F22: the 01/06 totals table is placed too far right (F34).

## 2. Spec

All from memory, **unverified**.

- **CSS 2.1 §17.6.1 (separated):** "Rows, columns, row groups, and column groups cannot have borders (i.e., user agents must ignore the border properties for those elements)."
- **CSS 2.1 §17.6.2 (collapsing):**
  - Borders may be set on cells, rows, row groups, columns and column groups, and are "centered on the grid lines between the cells".
  - Row width is `0.5·b0 + padding + width + padding + b1 + … + 0.5·bn`.
  - The table's left and right border is half of the first row's first and last cell's collapsed border; a larger border in a later row "spills into the margin".
  - The table's top and bottom border is half of the **maximum** collapsed border along that edge.
  - "The width of the table includes half the table border … a table does not have padding."
- **CSS 2.1 §17.6.2.1 (conflict resolution), in order:**
  1. `hidden` wins and suppresses the border.
  2. `none` has the lowest priority.
  3. The wider border wins.
  4. At equal width, by style: `double` > `solid` > `dashed` > `dotted` > `ridge` > `outset` > `groove` > `inset`.
  5. At equal style, by origin: cell > row > row group > column > column group > table.
  6. Between two elements of the same type, the one further left (ltr) and further up wins.
- **CSS 2.1 §17.5.1 (table layers)** and **Appendix E:** collapsed borders are painted above all cell backgrounds.
- **CSS Tables 3 (collapsed-borders section):** a cell's used border on a side is half of the collapsed borders along that side; for a spanning cell it is the maximum. Fragmentation of collapsed borders is not specified, so Chrome is the reference (step 0).
- **css-backgrounds-3 §5:** `border-radius` does not apply to internal table boxes in the collapsing model.

## 3. Where it belongs

| Option | Decision |
|---|---|
| **A. Blitz resolves the collapsed borders and lays cells out with half widths; fulgur paints the resolved edges** | **Chosen, staged as F22a (layout) then F22b (paint).** Resolution needs the grid (rows, tracks, spans), which only Blitz's `build_table_context` has. Painting is fulgur's, because fulgur paints from its own `Drawables` and not through `blitz-paint`. |
| B. fulgur only: paint row borders at the union edges of their cells, as F19 does for backgrounds | Rejected as the first step. The drift stays, and the drift is the measured diff. Most fixture rules are below the compare tolerance anyway. The rules would also be drawn 1-7 px away from Chrome's. |
| C. Paint first (B), then geometry | Rejected order. Geometry is what lowers the numbers, and the resolver is the shared foundation that the paint step consumes. |
| D. Ours: rewrite CSS (`tr.border-b > td {…}`) or the DOM | Rejected. It edits author CSS, gets the conflict rules wrong, cannot express half widths, and hides an engine gap. |
| E. Taffy | No change needed. The half widths reach Taffy's grid as ordinary item borders. |

### 3.1 Blitz (F22a)

New module `packages/blitz-dom/src/layout/collapsed_borders.rs` (pure, no `BaseDocument`):

- `#[derive(Clone, Copy, Debug, PartialEq)] pub struct BorderSpec { pub width: f32 /* px */, pub style: BorderStyle, pub color: AbsoluteColor }`.
  - `color` is resolved against the source element's `currentColor`.
  - F22a carries the colour but does not use it; F22b does.
- `#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)] pub enum EdgeOrigin { Table, RowGroup, Row, Cell }`. Slots for columns are reserved for F22c.
- `pub(crate) fn winning_border(a: Candidate, b: Candidate) -> Candidate`, implementing §17.6.2.1 rules 1-6. `Candidate = (BorderSpec, EdgeOrigin, position)`, where `position` is (row, track) for rule 6.
- `pub struct CollapsedBorders`:
  - `rows: u32`, `tracks: u32`;
  - `horizontal: Vec<BorderSpec>`, laid out as `(rows+1) × tracks`;
  - `vertical: Vec<BorderSpec>`, laid out as `rows × (tracks+1)`;
  - `cells: Vec<CollapsedCell { node: usize, row: u32, track: u32, span: u32 }>`.
- Methods:
  - `pub fn cell_half_widths(&self, cell: usize) -> taffy::Rect<f32>`: the maximum over the segments of each side, divided by two.
  - `pub fn table_half_widths(&self) -> taffy::Rect<f32>`: by the §17.6.2 rules above.
- `pub(crate) fn resolve(rows: &[RowOwner], cells: &[CellInput], table: [BorderSpec; 4]) -> CollapsedBorders`. It is linear in the number of segments. It returns `Vec`s indexed by grid position, so there is no hash iteration.

`layout/table.rs`:

- `GridBuilder` (`:716`) gets `row_owners: Vec<RowOwner { row: Option<usize>, group: Option<usize> }>`, one per grid row.
  - `collect_row` (`:810`) pushes the row id.
  - `collect_row_group` (`:800`) passes the group id down.
  - Anonymous rows (`:789-793`) push `None`.
- `build_table_context` (`:64`) reads `clone_border_collapse()` from the table's own computed style. The property is inherited, so `display: table` divs and nested tables follow the CSS cascade.
  - When the value is `Collapse`, after the placements are known (`:110-118`):
    - call `resolve`;
    - set each cell item's `style.border` to its half widths;
    - set the table's `style.border` to the table halves and its `padding` to 0;
    - set row items' `border` and `padding` to 0 (`:818`);
    - recompute the explicit first-row column widths with the halves (`cell_border_box_width`, `:285`; this only matters under `content-box`);
    - store `collapsed: Option<Arc<CollapsedBorders>>` in `TableContext` (`:26`) behind `pub fn collapsed_borders(&self) -> Option<&CollapsedBorders>`.
  - `separate` tables are untouched and stay byte-identical.
- **Why there is also a Node field.** `flush_styles_to_layout` runs *after* construction (`document.rs:925` then `:932`) and rebuilds each node's Taffy style from Stylo (`damage.rs:295`). The cell lays itself out from that node style (`TableTreeWrapper::compute_child_layout`, `table.rs:1073-1075`). An override made in `build_table_context` would therefore be lost. So:
  - add `pub collapsed_border: Option<taffy::Rect<f32>>` to `Node` (next to `table_cell_offset`, `node/node.rs:119`), set for the cells and the table;
  - apply it in `flush_styles_to_layout` right after `height_as_table_minimum` (`damage.rs:307-312`): `border` from the halves, and `padding` zeroed for the table.

### 3.2 fulgur in F22a

No code change. Cells keep painting their own borders, but at their layout widths (`box_metrics.rs:7-12`), which are now the halves. This is an interim:

- Two adjacent 0.5 px halves draw the 1 px internal line of 03 and 20.
- Outer lines become 0.5 px.
- Row rules stay unpainted, because the cells' own style is `none`.

Fork tests and goldens are run as described in §4.

### 3.3 fulgur in F22b (design fixed now, implemented in the next PR)

- `blitz_adapter::collapsed_borders(doc, table) -> Option<&CollapsedBorders>`.
- `convert/table.rs` `convert_table` (`:25`) records `Drawables::table_collapsed_borders: BTreeMap<usize /* table */, CollapsedEdges>`:
  - segments in pt;
  - colours via `absolute_to_rgba`;
  - the owning cell ids per segment.
- In the same function, a post-fixup sets `border_styles = [None; 4]` and radii to 0 on every cell of a collapsing table. The widths are kept, so the padding box is unchanged. The table entry gets the same treatment, so `paint_table_outer_frame` (`render.rs:3684`) draws no border.
- `render.rs` `paint_collapsed_borders(canvas, table_id, only_group, page_index, …)`:
  - Line positions come from the cells' fragments on the page: y from the cells of a row, and x from the cells starting at a track. F59 guarantees that every track start has one.
  - Each run of identical segments along a line is merged into one path.
  - Segments are drawn centred on the line with `draw_border_line` (`draw_primitives.rs:1516`, made `pub(crate)`).
  - At a junction, a segment is extended by half of the widest crossing segment.
- **Z-order:** collapsed borders go after the table's cells on the page (above cell backgrounds, §2). Step b2 finds the hook in `draw_v2_page`; `draw_under_clip_table` (`:3709`) paints them inside the clip.
- **Page breaks:** a horizontal line is painted with the page where its upper row ends. Whether it is repeated at the top of the next page follows step 0's Chrome result.
- **F12:** `paint_repeated_table_headers` (`:676`) paints the header group's segments in the copy, inside the existing `/Artifact`, including the header's bottom line.
- **Tagging:** segments are `Artifact` content wherever tagging is on.

## 4. Acceptance

**`crates/htmltopdf/tests/row_borders.rs` (new).** Setup:

- A4, strict, Inter, `Page { margin_mm: 0.0 }`.
- Helpers as in `row_backgrounds.rs`: `rasterise`, `assert_pixel`, `word_bbox`, `page_count`.
- Geometry is checked through `pdftotext -bbox` word `yMin`.
- Pitch values are compared to step 0's Chrome numbers, ±0.2 pt.

F22a tests:

| # | Test | Asserts | Before |
|---|---|---|---|
| G1 | `row_rule_adds_its_width_to_the_pitch` | collapse; 5 rows; `tr{border-bottom:1px solid}`; pitch = Chrome's (expected content + padding + 0.75 pt) | red |
| G2 | `shared_cell_border_counts_once` | `td{border:1px solid}` collapse: pitch +0.75 pt, not +1.5 pt | red |
| G3 | `wider_border_sets_the_line` | row 4 px against cell 2 px bottom: that pitch is +3 pt | red |
| G4 | `hidden_cell_border_suppresses_the_row_rule` | pitch +0 | green (guard) |
| G5 | `separate_model_is_unchanged` | same table with `border-collapse:separate`: PDF bytes equal to main's binary output (stored pitch) | green (guard) |
| G6 | `table_holds_half_the_outer_border` | `width:300px`, 10 px cell borders: first word x and table right edge = Chrome's | red |
| G7 | `fixture_02_page_1_ends_with_row_19` | `IF-1019` on page 1, `IF-1020` on page 2 (Chrome) | red |
| G8 | `fixture_words_match_chrome_below_tables` | for 01 (`úhradě`), 04 (`Daň`), 20 (`IBAN`): `yMin` within 1 pt of the committed reference PDF | red |

F22b tests, committed now as `#[ignore = "F22b"]`:

- **P1** `tr_border_bottom_is_painted`: a red 2 px rule across all cells at the row edge; cell interiors are white.
- **P2** `divide_y_rows`: compiled `tailwind.css` with `divide-y`. A gray-200 line between rows, with a pixel tolerance of 8; none under the last row.
- **P3** `row_group_border_top`: a 3 px blue line above the first body row only.
- **P4** `wider_wins`: cell 4 px green against row 2 px red gives green.
- **P5** `cell_beats_row_at_equal_width`.
- **P6** `solid_beats_dotted`.
- **P7** `left_cell_wins`.
- **P8** `shared_line_painted_once`: `rgba(0,0,0,.5)` cell borders give one layer's grey, not the double-painted darker one.
- **P9** `rule_below_every_repeated_header`: 120 rows.
- **P10** `rule_at_page_break`: per step 0.
- **P11** `separate_model_paints_no_row_borders`: no red pixel.
- **P12** `fixture_04_header_rule_is_indigo`.
- **P13** `table_border_collapses_with_cells`: the outer line is blue and 4 px.

**Blitz unit tests (F22a).**

- `winning_border`: each rule 1-6, and symmetry when the arguments are swapped.
- `resolve`: row and group edges, anonymous rows, colspan segments.
- `cell_half_widths` with a span takes the maximum.
- `table_half_widths`: first row for left and right, maximum for top and bottom.
- `build_table_context` gives a `separate` table no `CollapsedBorders`.

**Fork goldens (F22a).** These four use `border-collapse` and will move:

- `fulgur-vrt` `fixtures/layout/table-caption-top.html` and `layout/multicol-table-width.html`;
- examples `template/template.html` and `tagged-pdf/index.html` (`examples_determinism`).

Look at each `pdftocairo` diff, confirm the only change is collapsed geometry, re-bless, and list them in §7. These lib tests build collapse tables: `pagination_layout.rs:11715`, `:12009` and `render.rs:7661`. Change one only if its assertion encodes separated-model geometry, and say so in §7. Every other golden must stay byte-identical.

**Fixtures.** Almost every fixture has a ruled table, so expect many PDFs to change. Estimates (stop rule below):

| Fixture | Now | F22a | F22b |
|---|---|---|---|
| 01 | 4.36 % | about 3 % | about 2.8 % (header and total rules) |
| 02 | 11.1 % | about 8-9 % (19 rows on page 1) | -0.2 |
| 03 | 5.68 % | about 4.5 % | about 4 % (1 px frame) |
| 04 | 5.67 % | about 4 % | about 3.5 % (indigo rule) |
| 05 | 3.45 % | about 3 % | ±0 (gray-100) |
| 18 | 5.32 % | about 4 % | about 3.8 % |
| 20 | 8.92 % | about 5-6 % | about 5 % |
| 07, 09, 10, 13, 14, 15, 16, 17 | | lower or unchanged | |

- Thresholds are lowered to the new number + 1 point in the same PR.
- **Stop rule:** a page count that changes, or any number that grows.
  - If 03 or 20 grows in F22a (thinner interim frame), do not merge F22a alone. Continue with F22b's steps on the same branch as one PR, "F22".
  - Any other growth or page-count change: stop, look at the PNG, and file it.
- `bench.py --check` must pass; record `b200-items` (1615 ms before).

**Reviews.** `vrt-validator` for both PRs. `pdf-reviewer` for F22b, which adds painted and tagged content on the writer path.

## 5. Steps (each one day or less)

0. **Measure (half a day).** Chrome 141 scratch probes (`getBoundingClientRect` plus PDF):
   - row pitch with 1/2/3 px row rules, cell borders and mixes;
   - outer-edge geometry, including the spill case;
   - a colspan under segments of different widths;
   - `hidden`;
   - collapsed lines at a page break and under a repeated `thead`;
   - junction painting.

   Also measure the word offsets of the fixtures against the reference PDFs. Record everything in §7. **Stop** and re-plan if Chrome does not use the half-border model.
1. **Red tests (one day).** `row_borders.rs`: G1-G8 red or guard as listed; P1-P13 ignored. Record the red set.
2. **Blitz resolver (one day).** `collapsed_borders.rs` and its unit tests, not wired yet.
3. **Blitz data (half a day).** Add `row_owners`, call `resolve` in `build_table_context` and store the result. Output-neutral: fixtures must be `cmp`-identical.
4. **Blitz layout (one day).**
   - half widths, the `Node::collapsed_border` flush hook, table border and padding, row items, column recompute;
   - G1-G8 green;
   - `cargo test -p blitz-dom`.
5. **fulgur fork (half a day).** Run lib, smoke, vrt and examples. Inspect and re-bless the four goldens; clippy; fmt; push both forks.
6. **Ours (one day).** Pin the forks; `cargo test --release --locked`; compare; look at the PNGs; lower the thresholds; `bench.py --check`.
7. **Docs (half a day).**
   - findings: F22a done; file **F22b** (paint, §3.3) and **F22c** (`<col>`/`<colgroup>` borders, which need column boxes in Blitz);
   - `docs/upstream.md`;
   - §7;
   - then `vrt-validator`.

F22b steps (next PR), each one day or less: b1 convert and drawables plus suppressing the cells' own borders, with unit tests; b2 single-page paint, z-order hook and smoke tests (P1-P8, P11-P13); b3 pagination, F12 copy and tagging (P9, P10); b4 ours, compare, `pdf-reviewer`.

## 6. Risks and non-goals

- **Layout churn.** Most fixtures and the bench corpus move. Page breaks of 02, 07, 09 and 13 shift toward Chrome's. `PAGE_COUNT_MAY_DIFFER` is empty, so any page-count change stops the PR.
- **Interim double or half painting (F22a).** Cells paint half-width own borders, so outer lines are 0.5 px until F22b. The stop rule covers it.
- **F13 / F48.** Halves shrink the cell's content box by the change in border, and a `border-box` `height` includes it as Chrome does. Fixture 09, `fixed_height_cells.rs` and `cell_vertical_align.rs` must stay green.
- **Rounding.** 0.5 px halves under Taffy rounding can alternate 0/1 px per cell, but the row pitch total stays right. G1 checks the pitch, not single edges.
- **Page-break slicing and F12 (F22b).** The header's bottom line and the next row's top line can coincide on continuation pages; P9/P10 decide what is painted, from step 0. A cell split across pages (F30b) gets vertical segments per fragment.
- **Speed.** `resolve` is O(rows × tracks): about 24k segments on `b200-items` (4800 rows). F22b merges runs, so a row rule is one path and not five.
- **Determinism.** Grid-indexed `Vec`s and `BTreeMap`s only.
- **Rebase.** `build_table_context`, `collect_row` and `GridBuilder` were touched by F50, F59 and F36.
- **Non-goals:**
  - `<col>`/`<colgroup>` borders (F22c);
  - `rowspan` (F15);
  - `direction: rtl` for rule 6;
  - `border-spacing` in the separated model (F21);
  - `empty-cells`, `visibility: collapse`, `border-image`;
  - radii in collapsed tables, which are ignored per css-backgrounds-3;
  - the totals-table position (F34);
  - structure-tree changes.

## 7. Result

(to be filled)
