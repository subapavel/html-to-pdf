# F30e — A paragraph in a `display: table` wrapper loses its lines

Date: 2026-10-18. Finding: F30e in `2026-10-15-m0-findings.md` (with the F30f update). Status: **done** (§7).

- **Fix:** goes on `subapavel/fulgur` `htmltopdf/srgb-colours`, after `48aad32`, carried via `[patch.crates-io]`.
- **Fork refs:** at `48aad32`, in `crates/fulgur/src/pagination_layout.rs` unless another file is named.
- **Blitz refs:** to `/home/user/blitz` `packages/blitz-dom/src/` at the pinned `htmltopdf/blitz-dom-0.2.4` (F30f recorded `52b22ef`).
- **Builds on:** the F30f plan §3 G (what F30e reuses) and ADR-0004.

## 1. Problem

The finding says that `<div style="display:table">` around a `<p>` of 120 lines prints nothing and gives no warning. The repro (`/tmp/claude-0/vrt-f30b/p/dtable_p.html`) is gone. Recreate it under `out/scratch/f30e/` (git-ignored) with F30b's probe setup:

- **Page and CSS:** A4 with the default 15 mm margins (content box 1009 px), the `CSS` constant of `crates/htmltopdf/tests/nested_paragraph_split.rs:26` (`body{margin:0} p{margin:0;line-height:20px}` …), rendered like its `render()` with `RenderOptions::default()`.
- **Lines:** `L` = `Line0001<br>Line0002<br>…<br>Line0120`, 120 forced line boxes of 20 px, so 50 per page.
- **R0 `dtable_p.html`:** `<body><div style="display:table"><p>{L}</p></div></body>`.
  - Chrome is expected to print 3 pages (50 / 50 / 20), as for F30b's `<td>` and `grid > p` at 120 lines.

**Why nothing was printed: from reading, Blitz layout, already fixed by F50.**

1. The finding was written from the F30b validator's probes (PR #21). F48 reused that probe set before F48 landed, and F50 landed after both.
2. **Before F50, the `<p>` was never laid out.** Blitz skipped every `Flow` child of a table box ("Probably a table caption: ignore", F50 §1 cause 1). The `<p>` was never constructed, so its `final_layout` was zero.
   - No lines means nothing to paint.
   - It also means `inline_lines_below_page` finds nothing, so no warning. That is exactly the symptom.
3. **Since F50, Blitz wraps the `<p>` in an anonymous row and cell.**
   - The anonymous cell is created by `layout/table.rs` `plan_table_children` :621-649 → `push_row_slots` :775-796 → `create_anonymous_block` (`layout/construct.rs:558-589`).
   - The fork test changed from "placed as one unit" to `a_paragraph_that_is_a_non_cell_child_of_a_table_box_splits_in_its_anonymous_cell` (:12476).
4. **The table paint path is not involved for a `div`.**
   - `convert::table::try_convert` accepts only the `<table>` tag (`convert/table.rs:18`).
   - A `display:table` div goes through `convert/block.rs:9-75`. That function walks `layout_children` (:21-22): the anonymous cell, then the `<p>`.
   - The `<p>` is painted by the main loop (`render.rs` `dispatch_page_node` :717) like any split paragraph.
   - `draw_table_v2` (:3418) and `draw_under_clip_table` (:3578) only run for `<table>` elements. Even there, cells are painted by the main loop or from `clip_descendants`, which include the F50 wrapper (`convert/table.rs:63-72`, :98-107).

**What still loses lines, from reading.** All of it goes through `split_barrier`'s `TablePart` arm (:4947-4951), which F30f left for F30e.

- **An anonymous cell that is itself the paragraph.** This covers direct text or inline content in a table box (`<div style="display:table">{L}</div>`, `<span>`s) and in a `display:table-row` inside one.
  - The table's layout children are its cells (Blitz `table.rs:157-161`), so the walk parent of the anonymous cell is the table.
  - The anonymous block's computed `display` is `inline`, not `table-cell` (F50 §6).
  - So `split_barrier` returns `TablePart`. The cell is placed whole (warning at :4140) and what lies below the page box is lost.
  - F30b already splits the same shape as a direct-text `<td>`.
- **A `table-cell` box outside a table box.** Examples: `<div><div style="display:table-cell">{L}</div></div>`, and a cell in an orphan `display:table-row`.
  - Blitz lays such a cell out as a block (`construct.rs:244`, stylo_taffy `convert.rs:163`), but its walk parent is not a table.
  - So `(child is TableCell) != cell_of_table` fires. The comment at :4934-4935 names it F30e.
- **`inline-table` alone on its line.**
  - `box_barrier` (:4898-4914) returns `AtomicNotFlowRoot` because `alone_on_its_line` (:4850-4893) accepts only inside `flow-root`. This blocks direct text.
  - `record_carrier_slices_after` (:4356-4387, check at :4373) does not give the carrier its slices. A split carrier then paints its one line on no page (F30f cause 6).
  - Separately, F15 notes "`display: inline-table` renders nothing" on a single page (findings :155). Step 1 checks whether that still holds.

Step 1 must confirm each of these with the main binary before anything is changed.

## 2. Spec (cited from memory, unverified; css-spec-researcher not run)

- **CSS 2.1 §17.2.1, anonymous table objects.**
  1. Whitespace between table parts is ignored.
  2. Missing wrappers: a run of children of a table or row group that are not rows gets an anonymous row, and a run of children of a row that are not cells gets an anonymous cell.
  3. Missing parents: a cell outside a row gets an anonymous row, and a row or cell outside a table gets an anonymous `table` (or `inline-table` in an inline parent).
  - css-tables-3 §2.2 "fixup" restates this.
  - Rule 2 is F50; rule 3 is F50b (not done).
- **CSS 2.1 §9.4.1 and §17.4.** A table cell establishes a block formatting context and is a block container, so its line boxes are class B break points (css-break-3 §4.1). The table wrapper box holds the table box and its caption boxes; `caption-side` places the caption above or below.
- **css-display-3 §2.4.** A box's table role comes from its computed `display`.
- **css-break-3 §4.1 and §4.4.**
  - Breaks between line boxes; orphans and widows 2.
  - "inline-block and inline-table boxes … may be considered monolithic". Monolithic overflow may be sliced in print (ADR-0004).
- **css-tables-3 §8, fragmentation.** A table breaks between rows and inside cells; the cells of a row fragment in parallel; headers may repeat.
- **Chrome.** LayoutNG fragments table cells between lines. For `inline-table` it is expected to slice like `inline-block` (ADR-0004). Both are predictions until step 1.

**Decision per shape.** The Chrome column is a prediction.

| Shape (probe) | Chrome, predicted | Main, predicted | F30e |
|---|---|---|---|
| `display:table > p` (P1, R0) | 3 pages, all lines | all lines (F50) | guard |
| direct text / `<span>` in `display:table`, Tailwind `.table` (P1t, P1i, P1w, P1b) | 3 pages | ≈ 50 lines, warning "in a `display: table` box" | **split** (anonymous cell = cell) |
| `display:table > table-cell > p` or direct text; `table > table-row > table-cell` (P2, P2r) | 3 pages | all (F30b) | guard |
| `display:table > display:table-row >` direct text (P2a) | 3 pages | lost (`TablePart`) | **split** |
| orphan `display:table-row > p` (P3) | 3 pages | all, plus Blitz stdout noise (F50b) | guard |
| orphan `table-cell` with direct text, with or without an orphan row (P3c, P3rc) | 3 pages | lost | **split** (a cell is a block container) |
| `inline-table` alone on its line: `p`, direct text, bordered carrier, after `<h1>` (P4, P4c, P4h) | sliced, all words | lost or page 1 only | **split between lines** (ADR-0004 extended) |
| `inline-table` sharing its line (P4s) | sliced | page 1 | unchanged, warning (F30f-b) |
| `display:table` with a `display:table-caption` div (P5) | caption above, all lines | lines as P1, **caption missing** (Blitz `release_skipped` :666-675; `CaptionRestructurePass` handles only `<caption>` in `<table>`, `blitz_adapter.rs:1629-1634`) | lines guard; caption filed as F30e-b |
| nested `display:table > display:table > p` (P6) | all | all | guard |
| `display:table` around a 60-row `<table>` (P6t, F30 `wr_dtable`); `<td>` holding a 60-row `<table>` (P6n) | all rows | P6t predicted all since F50; P6n is the known gap "Nested tables do not fragment" | record; F30e-b if lost |
| `<table><tr><td><p>` (P7); clipped `<table>` with `<td style="display:block">{L}` (P7c, the `draw_under_clip_table` path) | all | all | guard |

## 3. Where it belongs and design (fulgur fork)

**Where.** All of this is fulgur's fragmenter.

- **Blitz:** produces the right boxes since F50, and laying out the missing parents is F50b.
- **Paint path:** paints whatever geometry it is given (§1).
- **Taffy and Parley:** unaffected.
- **Our crate:** changes only tests and docs. No upstream PR or issue (standing decision).

**Model.** The design reuses F30b's direct-text-cell machinery unchanged: `nested_line_split`, `row_mate_can_split` :5052, the `SplitRow` row end, and frame closing. It also reuses F30f's carrier slices and continuation. Only the predicate that says "this is a cell" changes.

**A. Recognising an anonymous cell.** This goes in `blitz_adapter.rs`, next to `anonymous_table_wrapper` :1013, because it reads Blitz internals. Signature only:

```rust
/// F30e: `child` is an anonymous table cell of the table box `table` (CSS 2.1 §17.2.1): an anonymous block laid out
/// as one of `table`'s cells, not its `::before` / `::after` box.
pub fn is_anonymous_table_cell(table: &Node, child: &Node) -> bool
```

- **True when** all of these hold:
  - `is_table_box(table)`;
  - `child.is_anonymous()`;
  - `child.layout_parent == Some(table.id)` (set by `document.rs:1081` for every layout child; the table's layout children are cells);
  - `child.id` is not `table.before` / `table.after` (pseudo boxes are `AnonymousBlock`s too, `construct.rs:410-417`, and are pushed into a table's layout children at :362-368).
- **Why `layout_parent` and not `parent`:** a row-level anonymous cell has its row as `parent` (`push_row_slots` style parent), but the table as `layout_parent`.

**B. `split_barrier` (:4923-4968).**

- `cell_of_table` becomes: the parent is a table box, and the child is a `table-cell` box or `is_anonymous_table_cell(parent, child)`.
- `TablePart` is returned only when the parent is a table box and the child is not one of its cells. With F50 that leaves a table's pseudo boxes and direct unit-test calls.
- **The "cell outside a table" clause is dropped.** Reasons:
  - Blitz lays the cell out as a block container (`construct.rs:244`; CSS 2.1 §9.4.1), so splitting between its lines is consistent with its layout.
  - Once F50b adds the anonymous table, the walk parent becomes that table and the first clause holds.
- **Comments to update:** `LineBarrier::TablePart`'s description becomes "a table box's `::before` / `::after` box"; the doc of `line_split_candidate` (:4970-4979) and the comment at :4934-4935 are updated.
- `abspos_line_split` is unchanged: it asks no parent-side question.

**C. `inline-table` alone on its line.**

- `alone_on_its_line` accepts outside `inline` with inside `flow-root` **or `table`**.
- `box_barrier` returns `SharesItsLine` for both, and `SharesItsLine` / `AtomicNotFlowRoot` change their descriptions to "an inline-block or inline-table that shares its line" and "an inline-flex or inline-grid box".
- **Reason:** an `inline-table` alone on its line is the carrier shape of ADR-0004, and its inside (a table box whose anonymous or real cells are block containers) splits through B.
- **No other code changes; what is reused unchanged:**
  - `record_carrier_slices_after` (:4356);
  - `inline_box_continues_on` / `paint_inline_box_continuation` (`render.rs:1177`, :1213; they are not keyed on display);
  - `dispatch_inline_box_content` (`render.rs:976`), which reaches `dispatch_fragment` and therefore `draw_table_v2` for a `<table>` element.
- **ADR-0004:** amended in the same PR to name `inline-table` (Decision and Consequences, "Amended 2026-10-18 by F30e"), only if P4 shows Chrome printing every word. One decision with a wider scope; a second ADR would split it.
- **If P4 shows Chrome clipping** to page 1, C is dropped, we match Chrome, and the warning names it.

**D. What does not change, and why.**

- **Unchanged:** `would_split_block_subtree`, the gates (:1793 / :1874, :3571-3586), `plan_line_breaks`, `render.rs`, convert, and Blitz. The gates already enter a table whose cell is taller than a page (its layout children are the cells), and F30f found the same for its wrappers. Touching them would move pages in documents that lose nothing today.
- **Side effect:** `leading_block_size` (:2625) and `row_mate_can_split` (:5077) now see an anonymous inline-root cell as splittable. A table whose first cell is such a paragraph starts in place instead of moving whole, as a direct-text `<td>` does since F30b (probe P1m).

**E. Byte identity, predicted.**

- **Fixtures 01–16.**
  - Grep of every `index.html`: no `display:table`, `inline-table`, `table-caption` or `table-cell`. The only table display is 02's `thead{display:table-header-group}` on a real `thead`, which is a part in its place: no anonymous object, no `TablePart`.
  - Fixture 10 has F50 anonymous cells, but each wraps a `td.block` / `td.flex` / `tr.block`, all block-level, so the anonymous cell is not an inline root.
  - Every `td` / `th` in fixture 10 has `block` or `flex`, so no `table-cell` box has a non-table walk parent.
  - Compiled `tailwind.css` has `.table{display:table}` and `.table-header-group`, and neither `.table-cell` nor `.table-row` nor `.inline-table` (no fixture uses `.table`).
  - Prediction: 01–16 `cmp`-identical.
- **Fork VRT and the 21 examples.** The only hit is `fulgur-vrt/fixtures/layout/table-caption-top.html`, a `<caption>` moved out before Blitz. There is no display override on `tr` / `tbody` / `td`. Prediction: byte-identical.
- **Our tests that must stay green:**
  - `table_fixup.rs` 4 and 7 (anonymous cells of one line, bare cells in a `display:table` div);
  - `fixed_height_cells.rs` :354-415 (`display:table` divs);
  - `nested_paragraph_split.rs` (F30b cells);
  - `wrapper_line_split.rs`.
  - The warning test `unsplit_paragraph_warning.rs` changes on purpose (the `SharesItsLine` text).
- **Fork tests that change on purpose:** `split_barrier_names_each_arm` (:12269) gains cases; its "table part" case (:12347, a non-anonymous non-cell child passed directly) keeps `TablePart`. Any other changed expectation is a regression.

## 4. Acceptance

**Ours: new `crates/htmltopdf/tests/table_wrapper_split.rs`.**

- **Setup and helpers:** the CSS and `render` of `nested_paragraph_split.rs`, plus its helpers (`lines`, `words`, `assert_paginated`: every `LineNNNN` once, in order, none below the content box by more than `SLACK_PT`), `page_count`, and Tailwind loaded for the `.table` cases.
- **Expected values:** page counts are Chrome's, as recorded in step 1.
- **Status labels:** "Red" means expected to fail on main; guards are expected green before and after.

1. **B1 (R0, P1):** the finding's markup; every line once, 3 pages. Guard. If it is red, see the step 1 gate.
2. **B2 (P1t, P1i, P1b, P1m):** direct text; `<span>{L}</span>`; `display:table;border:2px solid #000;padding:8px` (no line below the content box, so the bottom inset is reserved); the same after a 900 px spacer (starts on page 1, like Chrome). **Red.**
3. **B3 (P1w):** `<div class="table w-full"><p>{L}</p></div>` (guard) and `<div class="table w-full">{L}</div>` (**red**).
4. **B4 (P2, P2r):** cells of a `display:table` box. Guards.
5. **B5 (P2a):** `display:table > display:table-row > {L}`. **Red.**
6. **B6 (P3c, P3rc):** orphan cells. **Red.** P3 (orphan row with `<p>`): guard; stdout noise is not asserted (F50b).
7. **B7 (P4, P4c, P4h):** `<div style="display:inline-table;width:100%">` with `<p>{L}</p>` and with direct text; inside `<div style="border:2px solid #000;padding:8px">`; after `<h1>Podmínky</h1>`.
   - Assert every line once, `Podmínky` once on page 1, and page count as ADR-0004 states (Chrome's where the box starts a page). **Red.**
   - If P4one shows an `inline-table` paints nothing even on one page, B7 becomes `#[ignore = "F30e-b"]`.
8. **B8 (P4s):** `<p>Úvod <span style="display:inline-table;width:60%">{L}</span> konec</p>`. Words per page identical to main. Guard.
9. **B9 (P5, P5t):**
   - P5: every line once (guard), plus `#[ignore = "F30e-b"]` that `Obchodní` is printed once on page 1 above `Line0001`.
   - `<table class="w-full"><caption>Obchodní podmínky</caption><tr><td><p>{L}</p></td></tr></table>`: caption once, every line once. Guard.
10. **B10 (P6, P6t):** nested `display:table` boxes around `<p>`; every line once (guard). `display:table` around 60 × `<tr><td>RadekNN</td><td>B</td></tr>`: every `RadekNN` once, page count as Chrome's. Guard if main passes; otherwise `#[ignore = "F30e-b"]`.
11. **B11 (P7c):** `<table style="width:100%;overflow:hidden"><tr><td style="display:block">{L}</td></tr></table>`: every line once, so a split anonymous cell is painted inside the clip on every page. Guard (it settles the F30f planner's paint-path question). If it is red, see step 1.

**Ours: `unsplit_paragraph_warning.rs`** (one test, global logger).

- B2's direct text no longer warns.
- B8 warns and names "an inline-block or inline-table that shares its line".
- The existing `inline-block` assertion is updated to the new text.

**Fork: lib tests** next to :12476 (`nested_geometry`, 200 px pages, `br_lines`).

- **Red:**
  - `direct_text_in_a_table_box_splits_in_its_anonymous_cell`;
  - `direct_text_in_a_table_row_splits_in_its_anonymous_cell`;
  - `a_table_cell_outside_a_table_is_split`;
  - `an_inline_table_alone_on_its_line_is_split` (`p` and direct text);
  - `the_carrier_line_of_an_inline_table_is_on_the_page_where_it_starts` (slices `{0,1}`, `{1,1}` …).
- **Guards:**
  - the existing :12476, which also asserts `line_slices_tile` for the `<p>`;
  - `an_inline_table_sharing_its_line_keeps_its_paragraph_whole`;
  - every F30b cell test.
- **Unit:**
  - `split_barrier_names_each_arm` gains: anonymous table-level cell → `None`, anonymous row-level cell → `None`, orphan cell → `None`, `inline-table` alone → `None`, `inline-table` sharing → `SharesItsLine`; "table part" unchanged;
  - `alone_on_its_line`: `inline-table` alone true, `inline-table` with text false;
  - `blitz_adapter` `is_anonymous_table_cell`: table-level run, row-level run, a table's `::before` box (false), and the anonymous block wrapper of a plain `div` (false).
- **`render_smoke.rs`:** `render_smoke_display_table_direct_text_spans_pages` and `render_smoke_inline_table_paragraph_spans_pages`. Reason: otherwise codecov does not see these paths (fulgur `CLAUDE.md`).

**Fixture: none (predicted).**

- The invoice-realistic shape is a "label: value" CSS table: `<section class="table w-full">` with inline `display:table-row` / `display:table-cell` (the classes are not compiled, and rebuilding Tailwind needs npm, which cloud agents must not need), holding about 1.4 pages of terms in the value cell.
- That shape uses real cells, which F30b already splits.
- Step 1 renders it on main. A fixture `17-terms-css-table` (HTML, README, Chrome reference, threshold = worst page + 1 point) is added only if main loses words on that exact markup.
- **Why no fixture otherwise:** the shapes F30e changes (text directly in a table box, orphan cells, `inline-table`) are not invoice templates. Their pixels move only in the tests above, which show them. As in F30f, the PR names this.

**Pixel diff.** Fixtures 01–16 and their `compare.py` numbers are unchanged.

## 5. Steps

1. **Confirm (gate).** Scratch only; temporary logs are removed afterwards.
   - Recreate R0 (§1) and render it with main's binary.
   - **If R0 prints nothing:** find the layer with temporary `eprintln!`s, in this order:
     - the `<p>`'s `final_layout` (zero → Blitz);
     - its geometry fragments (none or one → fragmenter);
     - whether `drawables.paragraphs` holds it and `dispatch_page_node` reaches it on each page (no → paint).
   - **Optional:** confirm the pre-F50 explanation with a binary built at the `Cargo.lock` before the F50 merge (`git log -- Cargo.lock`), with its own `CARGO_TARGET_DIR` (never symlink `target/`, postmortem 2026-10-15). Expected: 0 lines, no warning. If not built, record "not verified".
   - Log the barrier at the `split_barrier` return and at :4140 for P1t, P2a, P3c and P4.
   - **Chrome probes** (Playwright m141, `probe.py` as in F30f): every probe of §2, plus P4one and P8 (the fixture candidate).
     - Record lines per page and any line printed twice or cut (slicing).
     - Look at the PNG at each break.
   - Fill in the §2 table.
   - File **F30e-b** for each Blitz construction loss: P5 caption, P6t / P6n rows, P4one if `inline-table` paints nothing at all.
   - **Stop and write `docs/postmortems/2026-10-18-f30e-*.md`** if:
     - R0 prints nothing on main and the cause is not one of §1's: the harness and the engine disagree, and this plan's premise is wrong;
     - or B11 loses lines while its geometry is right (a paint-path loss): re-plan §3 with the walk at fault before coding.
   - **If every "red" shape already prints every line on main:** F30e closes as fixed by F50. Land the guard tests, §7 and the findings update only, with no fork change.
2. **Red tests.** Ours: `table_wrapper_split.rs` and the warning test. Fork: the lib, unit and `render_smoke` tests above. Record which are red.
3. **Fork, output-neutral.** Add `is_anonymous_table_cell` and its unit tests, and refactor `split_barrier` to use it while still returning `TablePart` exactly as before. Then:
   - `--lib` on crates.io Blitz (the same 13 failures as at `48aad32`) and on local Blitz (0);
   - the `tests/*.rs` binaries one by one;
   - VRT byte-identical (`fonts-noinInter.conf`);
   - the 21 examples byte-identical;
   - fmt and `clippy --all-targets -D warnings`.
4. **Fork, lift `TablePart`** (§3 B). B2, B5 and B6 plus the matching lib tests go green. Run the step 3 checks. **An unpredicted mover means stop and write a postmortem.**
5. **Fork, `inline-table`** (§3 C), if P4 slices. B7, B8 and the carrier lib test go green, the ADR-0004 amendment is written, and the step 3 checks are run again.
6. **Ours.**
   - Lock bump (`cargo update -p fulgur`);
   - `cargo test --release --locked --no-fail-fast`;
   - fixtures 01–16 `cmp` against main's binary;
   - `compare.py` unchanged;
   - `scripts/bench.py --check`;
   - a `--tagged` render of B2 and B7, which must succeed.
7. **Docs.**
   - Findings: F30e done (or "fixed by F50"), F30e-b filed;
   - the `docs/upstream.md` fulgur row;
   - ADR-0004 if amended;
   - §7 here.

   Then run `vrt-validator`. `pdf-reviewer` is not required: no writer, font or metadata change.

## 6. Risks and non-goals

- **Blitz internals.** `is_anonymous_table_cell` depends on F50's `AnonymousBlock` cells and on `layout_parent`. A Blitz bump that changes them fails its unit test loudly; it does not fail silently.
- **Rows that now start in place** (§3 D). A table whose first cell is a long anonymous paragraph starts mid-page instead of moving. This matches Chrome and F30b's `<td>`; P1m pins it.
- **Orphan rows are laid out as Taffy flex** (stylo_taffy `convert.rs:170`), while the fragmenter treats them as block flow.
  - Two orphan cells side by side, one taller than a page, may misplace the short one (F30h-like).
  - The real fix is F50b's anonymous table. Record it there; it is not fixed here.
- **Tagging.** Text in an anonymous cell has no structure element (`walk_semantics` walks the DOM). This is pre-existing on one page, and more pages now carry it. Check it with `--tagged` and file it if confirmed.
- **Bordered `<table>` frames.** A split `<table>` paints its outer frame at full layout height on every page (`render.rs` `table_box_size` :3527-3546). This is pre-existing, unchanged, and visible in B11's PNG.
- **Rebase.** `split_barrier`, `alone_on_its_line` and `box_barrier` (F30f) will conflict on a fulgur bump.
- **Non-goals:**
  - `display: table-caption` outside a `<caption>`, nested `<table>` fragmentation and an `inline-table` that paints nothing (F30e-b);
  - missing-parent fixup and the stdout noise of orphan rows (F50b);
  - an `inline-table` / `inline-flex` that shares its line (F30f-b);
  - backgrounds of `display:table-row` divs (F19 runs for `<table>` only, `convert/table.rs:18`);
  - `box-decoration-break` (F96);
  - centred cells beside a split cell (F35);
  - compiling Tailwind `table-cell` / `table-row`.

## 7. Result

Landed on branch `f30e-display-table-wrapper`; fulgur fork `htmltopdf/srgb-colours` **`7c1389b`** (two commits on top of `48aad32`: `a013184` `is_anonymous_table_cell` and its unit test, output-neutral; `7c1389b` the fix and the fork tests). The Blitz fork is unchanged (`52b22ef`). `Cargo.lock` pins `7c1389b` (`cargo update -p fulgur` changed that one entry). Status: **done**.

**Step 1 (gate), no stop.** Scratch: `/tmp/claude-0/vrt-f30e/` (`probe.py`, `forkcheck.sh`). Chrome m141, A4, 15 mm, Inter, 120 lines of 20 px (`line-height:20px` on `body`, so 50 per page), the binary of main `d532069`.
- **R0 prints every line on main** (3 pages, 50 / 50 / 20, no warning): the finding's repro was fixed by F50, as §1 argued. Not verified: the pre-F50 binary (optional step, not built), so "0 lines before F50" rests on reading.
- **Not every "red" shape is fixed by F50**, so F30e does not close as "fixed by F50": the shapes below that print less than Chrome on main are real.
- **Causes**, from a temporary `eprintln!` at the `split_barrier` return in the fork's debug CLI on local Blitz (removed): P1t and P2a: `parent` a `display:table` box, `child` an anonymous block (`display: inline`) -> `TablePart`; P3c / P3rc: `table-cell` child of a non-table (or of an orphan row) -> `TablePart`; P4t / P4s: parent `inline-table` -> `AtomicNotFlowRoot`; P4 (`<p>` in an inline-table): the walk parent of the `<p>` is the anonymous cell, so `None` (it already split on main); P4c / P4h: `None` too, but the carrier had no line slices (F30f cause 6), so lines 1-49 were printed on no page. B11 (P7c) is green on main with its geometry right: the paint-path stop condition does not apply.

| Probe | Chrome | main | F30e |
|---|---|---|---|
| R0 `display:table > p` | 3 pp, 50/50/20 | same (F50) | same, guard |
| P1t text / P1i `<span>` / P1w `.table.w-full` text | 3 pp, 50/50/20 | 1 p, 53 lines, warning "in a `display: table` box" | 3 pp, 50/50/20 |
| P1w_p `.table.w-full > p`, P2 `table > cell > p` / cell text / row > cell | 3 pp | 3 pp | same, guards |
| P1b `display:table` + 2 px border, 8 px padding | 3 pp (50/50/20) | 2 pp, 53 lines | 3 pp, 49/50/21 (frame closed on every page, F96) |
| P1m 900 px spacer, then P1t | 4 pp, 5 lines on page 1 | 1 p, 8 lines | 4 pp, 5/50/50/15, like Chrome |
| P2a `display:table > table-row` text | 3 pp | 1 p, 53 | 3 pp |
| P3 orphan `display:table-row > p` | 3 pp | 3 pp | same, guard |
| P3c orphan cell text / P3rc orphan row > cell | 3 pp | 1 p, 53 | 3 pp |
| P4 `inline-table` width 100 % `> p` | 3 pp, 51/51/19 (line on both pages) | 3 pp, 50/50/20 (already) | same |
| P4t / P4tbl (`<table style="display:inline-table">`) text | 3 pp, sliced | 1 p, 53 / 53 | 3 pp, 50/50/20 each, every line once |
| P4c / P4ct (bordered carrier) | 4 pp (box moved to page 2, empty frame on page 1) | P4c: 71 of 120 lines (1-49 on no page); P4ct 1 p, 52 | 3 pp, 49/50/21, every line once (ADR-0004) |
| P4h / P4ht (after `<h1>`) | 4 pp | P4h: 71 of 120; P4ht 1 p, 52 | 3 pp, every line once, `Podmínky` once on page 1 |
| P4one / P4onet (10 lines, one page) | 1 p | 1 p, all 10 | same: **`inline-table` renders its text** (F15's "renders nothing" no longer holds) |
| P4s `inline-table` sharing its line | sliced, 3 pp | 1 p, 53, warning | unchanged, the warning names it (F30f-b) |
| P5 caption `<div display:table-caption>` + `<p>` | caption above, 48/50/22 | lines all there (50/50/20), **caption missing** | unchanged (F30e-b) |
| P5t `<table><caption>` | 3 pp | 3 pp, caption once | same, guard |
| P6 nested `display:table` | 3 pp | 3 pp | same |
| P6t `display:table` around 60 rows / P6n `<td>` around 60 rows | 2 pp, 50 + 10 | 2 pp, all 60 rows | same, guards: **the nested-table gap does not reproduce at 60 rows**, no F30e-b for it |
| P7 `table > tr > td > p`, P7c clipped table with a block `td` | 3 pp | 3 pp | same, guards |
| P8 label / value CSS table, 30 articles in the value cell | 4 pp | 4 pp, 30 of 30 | same: **no fixture** (the plan's condition, main loses nothing on that markup) |

PNGs of P1b, P4c (ours and Chrome) were read at the breaks: lines continue, the frame closes on each page.

**Red before:** ours `table_wrapper_split.rs` 10 of 20 (`direct_text_in_a_display_table_box_keeps_every_line`, `spans_in_a_display_table_box_keep_every_line`, `a_bordered_display_table_box_reserves_its_bottom_inset`, `a_display_table_box_that_is_not_first_on_its_page_starts_where_it_is`, `a_tailwind_table_box_keeps_every_line`, `direct_text_in_a_display_table_row_keeps_every_line`, `a_table_cell_outside_a_table_keeps_every_line`, `an_inline_table_alone_on_its_line_keeps_every_line`, `an_inline_table_in_a_bordered_frame_keeps_every_line`, `an_inline_table_below_a_heading_keeps_every_line`) and `unsplit_paragraph_warning.rs`'s one test; the 10 green are the guards and the unchanged shapes. Fork lib: 7 (`direct_text_in_a_table_box_splits_in_its_anonymous_cell`, `direct_text_in_a_table_row_splits_in_its_anonymous_cell`, `a_table_cell_outside_a_table_is_split`, `an_inline_table_alone_on_its_line_is_split`, `the_carrier_line_of_an_inline_table_is_on_the_page_where_it_starts`, `split_barrier_looks_through_anonymous_and_orphan_cells`, the extended `alone_on_its_line_wants_one_line_with_only_the_box_on_it`); the guards (`...non_cell_child_of_a_table_box_splits_in_its_anonymous_cell`, `an_inline_table_sharing_its_line_keeps_its_paragraph_whole`, `is_anonymous_table_cell`'s test) were green. Fork `render_smoke`: the three new ones (`render_smoke_display_table_direct_text_spans_pages`, `render_smoke_orphan_table_cell_spans_pages`, `render_smoke_inline_table_paragraph_spans_pages`). **Green after:** all of them.

**What was built** (fork): `blitz_adapter::is_anonymous_table_cell` (`layout_parent` is the table, not its `::before` / `::after`); in `pagination_layout.rs` `split_barrier` counts an anonymous cell as a cell of its table and drops the "cell outside a table" clause (`TablePart` is left for a table's pseudo boxes and direct calls), `is_inline_block_or_table`, `alone_on_its_line` / `box_barrier` accept `inline-table`, the `LineBarrier` descriptions and docs. `record_carrier_slices_after`, `render.rs` and the gates are unchanged, as §3 D predicted.

**After:**

| Check | Result |
|---|---|
| fork `--lib`, crates.io Blitz | 2131 passed, 20 failed: the same 13 as at `48aad32`, plus 7 new tests that need Blitz's F50 anonymous cells, like their sibling `a_paragraph_that_is_a_non_cell_child_of_a_table_box_splits_in_its_anonymous_cell` |
| fork `--lib`, local Blitz (`/home/user/blitz`, `52b22ef`) | 2151 passed, 0 failed (2143 before) |
| fork `render_smoke` | crates.io Blitz 257 passed, 5 failed (the same 3 plus `render_smoke_display_table_direct_text_spans_pages` and `render_smoke_inline_table_paragraph_spans_pages`, F50-dependent); local Blitz: no failure |
| fork other test binaries | all 42 `tests/*.rs` one by one (local Blitz): 0 failing |
| fork `fulgur-vrt` (`fonts-noinInter.conf`) | 31 passed, goldens untouched |
| fork 21 examples (debug CLI against the F30f outputs, `/tmp/claude-0/vrt-f30f/ex-before`) | byte-identical, 21 of 21 |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked --no-fail-fast` | 442 passed, 0 failed, 3 ignored (20 new in `table_wrapper_split.rs` plus one ignored, the F30e-b caption); fmt and `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-16 | `cmp` byte-identical to the render of main's binary (`/tmp/claude-0/htmltopdf-main-d532069`): 16 of 16; `compare.py` numbers unchanged (16: 16.6 / 20.8 %) |
| `scripts/bench.py --check` (after merging main with B1) | ok; render time 01 / 02, 25 runs, median: 20.2 / 57.6 ms on main, 19.3 / 53.3 ms now: noise |
| `--tagged` renders of P1t, P2a and P4c | succeed, 3 pages each |

Extra probes (12 small documents, `/tmp/claude-0/vrt-f30e/x/`, main's binary against the new one): short table text, short orphan cell, short `inline-table`, two cells side by side, text between cells, an `inline-table` that shares its line with a `<p>` or a block in it, an `inline-table` around a real `<table>`: byte-identical to main. Two differ, both better: `display:table` text followed by `<p>AFTER</p>` (main 53 of 100 lines, AFTER on page 2; now 100 lines, AFTER on page 3 like Chrome) and two orphan cells side by side (see below).

**Deviations from the plan:**

1. **Step 3 was a commit of the function and its unit test only** (nothing calls it, so there is nothing to check for output): the full fork battery ran once, on the final code, not after step 3 and again after steps 4 and 5. Steps 4 and 5 are one commit (`7c1389b`), the unit tests having been written red first.
2. **`split_barrier_names_each_arm` is untouched**; the new arms (anonymous cells have no `id`) are in `split_barrier_looks_through_anonymous_and_orphan_cells`, which finds the anonymous cell as the first layout child of its table. The "table part" case there still returns `TablePart`.
3. **The crates.io-Blitz `--lib` count is 20 failures, not 13**: seven new fork tests assert on F50's anonymous cells. They pass on local Blitz, as their sibling does.
4. **The `inline-table` smoke test also covers direct text and a frame**, because `inline-table > p` already printed on main (P4); the red case is direct text.
5. **B1 / B2 / B3 / B7 expectations**: bordered shapes assert 3 pages (ours), not Chrome's 50 / 50 / 20 split (the frame is closed on every page, F96); the frame / `<h1>` cases accept 3 or 4 pages (Chrome 4, ADR-0004).
6. **The plan's `table_fixup.rs`, `fixed_height_cells.rs`, `nested_paragraph_split.rs`, `wrapper_line_split.rs`** stayed green in the full run; no expectation of an existing test changed apart from the warning text.
7. **`vrt-validator` and `pdf-reviewer` were not run by the implementer** (merge policy items 2 and 5 are open for the caller; `pdf-reviewer` is not required by the plan).

**Filed:** F30e-b (findings): a `display: table-caption` box that is not a `<caption>` is dropped (P5); `::before` / `::after` on a `display:table` box with direct cell children or text panics in Blitz (`layout/table.rs:1020`, index out of bounds; on main too, F50-adjacent); two orphan cells side by side with the taller one split put the short one on page 2 (Chrome keeps it beside, F50b / F30h-like); text in an anonymous cell has no structure element when tagged (`walk_semantics` walks the DOM; pre-existing, more pages carry it now).

Not verified: the pre-F50 behaviour of R0 (no binary built); the spec citations of §2 (from memory; Chrome probes stand in); the structure tree of a tagged render beyond "succeeds" and MCID counts; a split `inline-table` under a transform, an opacity group or a clip (the F30f continuation walks are keyed on the box, not its display, and are exercised for `inline-block` only); an `inline-table` inside multicol, flex or grid; `display:table` with `border-collapse` or `table-layout` styling on the same box; right-to-left and vertical writing modes; `line-height: normal` (all probes use 20 px lines).
