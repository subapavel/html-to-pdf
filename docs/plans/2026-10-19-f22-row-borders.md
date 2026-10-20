# F22: collapsed table borders (row and row-group rules, shared cell borders)

Date: 2026-10-19. Finding: F22 (`2026-10-15-m0-findings.md:209-213`). Status: **done, one PR "F22"** (F22a and F22b landed together, see §7). The work was planned as two PRs. **F22a** put the collapsing border model into Blitz *layout*, on `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` after `310878c`. **F22b** *paints* the resolved borders, on `subapavel/fulgur` `htmltopdf/srgb-colours` after `19c4782`. F22b's design is fixed here (§3.3) so both PRs share one contract. Line numbers are from those heads. I had no `css-spec-researcher` output.

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

Fork commits (review fixes below included). Blitz (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`, after `310878c`): `6973fd6` resolver, `25f4f90` row owners and resolve (output-neutral), `3e5a924` layout, `fadaabb` origin of each segment. fulgur (`htmltopdf/srgb-colours`, after `19c4782`): `7e97cfa` convert, paint and lib tests, `1d32ddf` two VRT goldens, `0a97ce1` `examples/tagged-pdf/index.pdf`, `95104ff` review fixes, `622dae7` goldens again, `0779580` bare cells of a div table. Ours: this branch.

### F22b was folded into F22a, and why

The plan expected the cells' own style to be `none`, so that an F22a that only changes layout would paint nothing new. It is not: Tailwind's preflight sets `border: 0 solid; border-color: currentColor` on everything, so a cell is `solid` at computed width 0. With the half widths the cell got in F22a, fulgur painted a 1 px line in the cell's own colour (gray-900) under every ruled row, where Chrome has the row's gray-100. F22a alone grew fixture 09 from 5.1 / 4.9 / 0.7 % to 5.2 / 5.2 / 0.9 % (stop rule). The coordinator decided to continue with F22b on the same branch, as one PR, rather than suppress the cells' borders as an interim (that would have dropped the whole grid of 03 and 20 until paint landed). With F22b 09 is 4.0 / 3.8 / 0.7 %.

### Step 0: Chrome 141 measurements

Probes `probe.mjs`, `probe-pdf.mjs`, `g-chrome.mjs` (`getBoundingClientRect` and Chrome's PDF read with `pdftotext -bbox` / `pdftoppm -r 96`; scratch, not committed). 5 rows, 28 px each without borders:

| Probe | Result |
|---|---|
| `tr` rule 1 / 2 / 3 px | pitch 29 / 30 / 31 px (21.75 / 22.5 / 23.25 pt); first row 28.5 / 29 / 29.5, table 145 / 150 / 155 px |
| `td` border 1 / 2 px | pitch 29 / 30 px, the table is 1 / 2 px taller than the rows, not 5 |
| row 4 px against cell 2 px bottom | pitch 32 px (24 pt): the wider wins, not 2 + 4 |
| cell `border-bottom: hidden` against a 3 px row rule | pitch 28 px: `hidden` suppresses |
| `tbody { border-top: 3px }` | first body row 29.5 px high, table 87 px; only the first row |
| `divide-y` (1 px) | rows 28.5 / 29 / 29 / 29 / 28.5 px: bottom border of every row but the last |
| `width: 300px`, 10 px cell borders | table border-box 300 px, 5 px of the outer line in the table, 5 px in the cell; first word at 5 + 5 + 4 = 14 px; `box-sizing: content-box` gives 310 |
| spill: row 1 cells 1 px, a row 2 cell `border-left: 10px` | the table's left border is 5 px (half of the **maximum** over the whole edge), right 0.5, top 0.5, bottom 0; **not** the first row's, which is what CSS 2.1 §17.6.2 says |
| table border 6 px, cells 2 px | the line is 6 px, 3 px in the table, 3 px in the cell; rows 34 px high (28 + 3 + 3) |
| colspan under 2 px and 6 px segments | both rows are 31 px high: a spanning cell takes the maximum (3) |
| `thead { border-bottom: 2px }` | the header row and the first body row are 29 px, the others 28 |
| break, rows with a 3 px rule (`brk_rows`) | the line on the break is cut: its upper half (1.5 px, drawn 2) ends page 1, its lower half starts page 2 (red at y 0-1); the first whole rule of page 2 is at y 30-32 against 28-30 on page 1 |
| repeated `thead` with only a header rule (`brk_headonly`) | the copy has its own rule, blue at y 28-30 on every page |
| repeated `thead` and a 3 px row rule on the body (`brk_head`) | the line under the copy is the row's red, not the header's blue (equal width, row beats row group) |
| junction (10 px blue vertical, 10 px red horizontal) | the vertical line is painted over the whole red band, to the band's bottom edge |

Equal contents in G6: Chrome divides a table's free width between auto columns in proportion to their content, Taffy equally (F129), so G6 uses identical words in both columns. Fixture words against the committed reference PDFs, `yMin` in pt (reference / main / now): 01 `úhradě` 425.67 / 420.94 / 425.44; 04 `Daň` 550.03 / 545.30 / 549.80; 20 `IBAN` 622.83 / 630.85 / 622.60; 02 `IF-1019` page 1 / page 1 / page 1, `IF-1020` page 2 / page 1 / page 2.

### Tests

`crates/htmltopdf/tests/row_borders.rs`: 21 tests. On main's binary G1, G2, G3, G6, G7 and G8 were red (6 of 8), G4 and G5 guards. With Blitz `3e5a924` all of G1-G8 are green. P1-P13 (the paint tests) were written ignored at that point and enabled with F22b; they pass against the pinned forks (I did not run them red one by one: P1, P3-P8 and P12-P13 cannot pass without paint; P11 and G5 are guards). Blitz: 20 unit tests in `collapsed_borders.rs` (every rule 1-6, symmetry, groups, anonymous rows, colspan, the table halves). fulgur: 5 unit tests in `table_borders.rs`, 3 in `convert/table.rs` (a collapsed table's edges and its cells' `none` styles, a separated table untouched, a nested table inheriting `collapse`), 4 smoke tests in `render.rs` (transform, overflow clip, opacity, tagged, 120 rows with a repeated header, spans, hidden and mixed styles). The plan's Blitz unit test "build_table_context gives a separate table no `CollapsedBorders`" needs a document; the fulgur test `a_separated_table_keeps_its_own_borders` covers it. G5 is also byte-checked: two separated documents are byte-identical between main's binary and the pinned one, as is `b200-text` and fixtures 01-20 for the data-only step (`25f4f90`).

### Fixtures: worst page (%), pages unchanged on all 20

| | main | F22a only | F22 | | main | F22a only | F22 |
|---|---|---|---|---|---|---|---|
| 01 | 4.4 | 3.3 | 3.0 | 11 | 6.9 | 3.2 | 3.1 |
| 02 | 11.1 | 6.6 | 4.8 | 12 | 14.6 | 9.0 | 8.8 |
| 03 | 5.7 | 4.7 | 4.7 | 13 | 11.7 | 11.7 | 11.7 |
| 04 | 5.7 | 4.2 | 3.8 | 14 | 7.5 | 4.6 | 3.9 |
| 05 | 3.5 | 3.5 | 3.3 | 15 | 8.7 | 5.3 | 4.5 |
| 06 | 15.9 | 11.8 | 11.8 | 16 | 20.8 | 20.8 | 20.8 |
| 07 | 7.2 | 5.3 | 3.7 | 17 | 12.0 | 7.6 | 5.7 |
| 08 | 13.2 | 12.1 | 12.1 | 18 | 5.3 | 4.0 | 3.1 |
| 09 | 5.1 | **5.2** | 4.0 | 19 | 3.1 | 3.1 | 3.1 |
| 10 | 5.9 | 5.3 | 5.2 | 20 | 8.9 | 3.4 | 3.3 |

No page of any fixture is above main's. Thresholds of the 17 that improved are lowered to the new worst + 1 point (`scripts/compare.py`); 13, 16, 19 did not move (their worst page has no collapsed table). Looked at the PNGs of 01, 02, 03, 04, 09, 18, 20: rows, rules, the totals bar and the page break of 02 (19 rows on page 1, like Chrome) line up with Chrome's, 04's indigo header rule is there, 09's row rules are the light gray of Chrome's (the dark half-lines are gone), the repeated header of 09 and 02 carries its rule. What is left in the diffs is text drift, the footer position (F103), the totals table of 01 placed too far right (F34), and on 03 and 20 grid lines that stand 1 px off Chrome's (the table is 1 px higher, all text with it). Estimates of §4 against the result: 03 4.0 expected, 4.7 got; 04 3.5 / 3.8; 20 5 / 3.3; 02 8-9 / 4.8; 18 3.8 / 3.1.

### Goldens re-blessed (fork)

- `fulgur-vrt` `layout/multicol-table-width` and `layout/table-caption-top`: `pdftocairo` diffs looked at, doubled shared borders became single lines, rows are one pixel shorter; nothing else moved.
- `examples/tagged-pdf/index.pdf`: the doubled lines of its collapsed table are single.
- Not touched, though they differ from a fresh render on this VM: `examples/svg/index.pdf` (no table in it; `committed_svg_matches_rendered` fails here, the SVG text uses host fonts), `examples/wasm-demo/index.pdf`, `examples/template/output.pdf` (not reproducible from `template.html` with the pinned fonts). Filed as F133. `examples/table-header/index.pdf` is byte-identical (its table is separated).
- The lib tests at `pagination_layout.rs` and `render.rs` that build collapse tables did not need a change.

### Speed

`bench.py --check` ok. `b200-items` 1662 ms median (1615 ms before; the relative gate allows 2053 ms), `b200-text` 878 ms and `b200-mixed` 890 ms. `b200-items` grows from 204 to 210 pages and `b200-mixed` from 201 to 203, because every ruled row is now as tall as Chrome's; the bench prints a NOTE (210 outside 195-205). `b200-text` is byte-identical.

### Deviations from the plan

- F22b folded in (above).
- The table's left and right border is half of the maximum over the whole edge (Chrome, probe spill), not of the first row.
- Segment widths are stored in CSS px (`Px`), not pt: a line is a band of `w` px centred on the cell edge, snapped down to the px grid (`floor(c - w/2)`), which is where Chrome puts it. Blitz rounds the cell edges, so an odd-width line can stand 1 px off Chrome's.
- Painting is a post-pass at the end of `draw_v2_page` (as the multicol rules are) and at the tail of the transform, overflow-clip and opacity wrappers, not "after the table's cells" in dispatch order: content painted after the table that overlaps it draws under its lines. `paint_collapsed_borders(canvas, table_id, only_group, ...)` of §3.3 became `paint_page`, `paint_tables` and `paint_header_copy` in `table_borders.rs`.
- A line cut by a page break gets `ceil(w / 2)` px on each side (Chrome 1.5 px, drawn 2). The line under a repeated header is the better of the header's bottom line and the line the break cut (width, style, then origin; Blitz keeps the origin of every segment for it).
- Junctions: a run is extended over half of the widest crossing segment and verticals are painted after horizontals. Translucent borders are painted twice where they cross (F131).
- `border-radius` is zeroed on the cells only; a collapsed table keeps its own radius.
- A table with no cells or no tracks keeps the separated geometry.
- P2 uses `divide-gray-100` (only that colour is in the compiled CSS, tolerance 4 against white); P12 expects Chrome's indigo rgb(79, 57, 246); P7 and P8 expect 100 px columns because Blitz does not stretch explicit-width columns to the table's width (F130); P9 and P10 use text rows, since the fragmenter cuts an empty block row in two.
- Not done: the Blitz unit test for a separated table (see Tests), `<col>` borders (F22c).

### Review fixes (vrt-validator, pdf-reviewer)

- **M1** The line under a repeated header copy was the header's own rule, Chrome's is the body row's cut line (fixtures 02 and 09, (300, 89): gray-300 against gray-100; 09 p3 had grown from 0.67 to 0.72 %). At a full tie of width, style and origin the cut line now wins (`better`). `line_under_a_repeated_header_is_the_cut_line_at_equal_width`. 02 p2/p3 4.76 / 4.58 %, 09 p2/p3 3.75 / 0.65 %.
- **M2** A collapsed table inside `display: inline-block` or with `display: inline-table` was painted at page coordinates, but the cells of an inline box are dispatched under the box's translation (`dispatch_inline_box_content`): the lines stood 12 pt above the text. Tables in `inline_box_subtree_skip` are left out of the page pass and painted at the tail of the inline-box dispatch (`paint_wrapper_tables` with the body offset). `collapsed_table_in_an_inline_block_is_painted_where_its_text_is` (three variants: inline-block, inline-block with `width: 150px`, inline-table). Red on the pinned forks before the fix.
- **M3** `convert_table` took `<table>` by tag only, so a `display: table` box got Blitz's half borders and nothing painted them. It now takes every table box (`is_table_box`) as well. `div_tables_collapse_like_tables` and a convert test. Red before. Switching the dispatch to the table box broke `table_fixup.rs` (two tests: a `display: table` box with bare `display: table-cell` children lost their text, because `convert_table` looked for cells inside each child); a direct child that is a cell is now converted as one (`0779580`, `bare_cells_of_a_div_table_are_converted`). I found it only in the final `cargo test --release --locked` of ours, which is why that gate matters.
- **D1** The lines are an `/Artifact` sequence when the document is tagged (`table_borders::as_artifact`), on the page, in the wrappers and in inline boxes; the header copy was one already. `collapsed_table_lines_are_artifacts_in_tagged_output` counts stroke operators outside marked content (page streams and form XObjects), 13 before, 0 after.
- **I1** Lines of one width, style and colour are one path with a subpath each (`emit`): about two strokes per page on `b200-items`, whose PDF is 619 kB (627 kB before, 555 kB on main with 204 pages). Group order follows the first line of each group, so the relative order of two runs of different paint on one line can differ from the paint order before; they only overlap in a junction square.
- Filed, not fixed: F135 (rounded clipped table), F136 (junction tick, `double`), F137 (header copy inside a transform / opacity wrapper).
- Gates: fork lib 2197 tests, all integration targets, VRT (two goldens re-blessed, pixel-identical to before, a few bytes smaller), examples (only the known `examples/svg`); clippy and fmt in both forks and ours. Fixtures: no page above main's at two decimals (per page, main -> now): 01 4.36 -> 2.99; 02 10.22 11.12 9.46 -> 4.84 4.76 4.58; 03 5.68 -> 4.72; 04 5.67 -> 3.80; 05 3.45 -> 3.34; 06 15.88 11.80 -> 10.69 11.80; 07 7.18 7.14 -> 3.65 3.34; 08 13.16 12.10 -> 8.20 12.10; 09 5.10 4.90 0.67 -> 4.02 3.75 0.65; 10 5.87 1.52 -> 5.22 1.01; 11 6.88 -> 3.11; 12 14.57 7.15 -> 8.81 7.15; 13 1.87 11.66 1.88 -> 1.47 11.66 1.88; 14 7.45 1.58 -> 3.92 0.98; 15 8.71 4.92 -> 4.46 3.40; 16 16.58 20.83 -> 9.53 20.83; 17 12.00 9.97 -> 5.72 4.64; 18 5.32 -> 3.10; 19 3.05 2.81 unchanged; 20 8.92 -> 3.31. `bench.py --check` ok, `b200-items` 1680 ms.
