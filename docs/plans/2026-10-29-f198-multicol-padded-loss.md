# F198: a box taller than a page inside a multi-column container is placed whole and loses everything below the page edge

Date: 2026-10-29. Finding: F198 in `2026-10-15-m0-findings.md` (HIGH, silent). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours` (head `3ee98d70`), carried via `[patch.crates-io]`. Function names are from that head; re-read the line numbers before editing. No `css-spec-researcher` output: §2 is from memory and unverified.

## 1. Problem

`<div style="column-count:2;column-gap:20px"><div style="padding:16px;border:1px solid #888;border-radius:8px;background:#f5f5f5">` holding 2500 words, then `<p>AFTER</p>` (`out/scratch/f30fdv/a/multicol.html`, generator `out/scratch/f30fdv/c1.py`; A4, 15 mm, Inter 14px/1.2). Chrome prints 4 pages (690 / 716 / 719 / 375 words). Main prints 2 pages and 361 / 0 words. The text runs in column 1 from 54.8 pt to the page edge (836 pt, while the content box ends at 799.4), column 2 is empty, AFTER is on page 2, and 2139 words are lost. F30f-d did not change the bytes.

The padding is not the cause (from reading). `multicol_layout::column_flow` records every column child that is not a `<p>` inline root with glyph runs only as one `FlowItem::Block`. That covers a `<div>` of text, padded or not, and a card of `<p>`s. `plan_multicol_pages` places a block taller than the column alone ("a box taller than a page is placed alone and overflows", F30d-b). `record_multicol_row` then logs "a `<div>` …px tall inside a multi-column container is taller than a page; it is placed whole in its column and overflows the page" and records the whole subtree once, at the column's y on page 1. The CLI logs at `warn`, so the validator's "silent" needs checking in step 1.

No fixture shows the shape. The multicol sections of 12, 24 and 25 hold `h3` / `p` / `table` children, none taller than a page. The compiled `tailwind.css` has no `columns-*` utility, only `gap-*` / `column-gap`, so fixtures use `style="columns:2"`, as 12 / 24 / 25 already do.

## 2. Spec (unverified)

- css-break-3 §2–3: column boxes and pages are both fragmentainers (nested fragmentation). Monolithic content is replaced elements, scroll/clip containers with a block size, and line boxes. A block with padding and a border is **not** monolithic.
- css-break-3 §4.1: class A break points sit between sibling blocks, class B between line boxes, class C between a container's content edge and a child with a gap. §4.4: an unforced break inside a box that does not fit is allowed. §5.2: margins adjoining an unforced break are truncated.
- css-break-3 §5.4 `box-decoration-break: slice` (initial value): no border, padding or radius at a break edge, and the background is sliced. Stylo computes only `slice`, as F30f-d found.
- css-multicol-1 §8.2: in paged media, columns that do not fit the page continue on the next page. §7.1 `column-fill: balance`: in a fragmented context only the last fragment is balanced, and `auto` fills in order.

## 3. Where it belongs, and scope

It belongs in the fulgur fork only. Blitz and Taffy do not lay out multicol; fulgur does (F30d). Blitz has already laid each column child out at the column width, so every position the fix needs is in `final_layout`. No upstream PR (standing decision).

**Decision: expand into atoms a column child box that is taller than a page, and only that.** We must not touch boxes that fit, for two reasons. Chrome splits them too, but entering them would move fixtures 12 / 24 / 25 and the fork's multicol goldens. And no word is lost on that path.

- `pagination_layout.rs`
  - New types: `pub(crate) struct McBox { items: Vec<McBoxItem>, tail: f32, x: f32 }` and `enum McBoxItem { Lines { source, advances, lead, x }, Leaf { node, h, lead, x }, Box { node, inner: McBox, lead } }`. `lead` is the gap above the item in `final_layout`: the box's top border and padding plus the child's margin. `tail` is what is left below the last item.
  - New `fn mc_box(doc, node_id, column_styles, fresh, depth) -> Option<McBox>`. It returns `Some` only for a block-flow box (`is_block_flow_container`, inside `flow` / `flow-root`) that is:
    - not a table, flex, grid or multicol box, not floating or abspos;
    - not clipped, transformed or translucent (the `table_subtree_is_wrapped` checks);
    - without a block size of its own (`block_size_constrained`);
    - not `break-inside: avoid` while it fits `fresh`.

    An inline root with glyph runs only becomes `Lines` with itself as the source. A child that is not describable but fits a page becomes a `Leaf`. Otherwise, or past a depth of 16, it returns `None`, the box stays a `Block` and keeps today's warning. Nothing is read in the layout pass, so `multicol_layout.rs` is unchanged.
  - `McContainer.boxes: BTreeMap<usize, McBox>`, filled in `multicol_container` for every non-table `FlowItem::Block` with `height > fresh + 1`.
  - `plan_multicol_pages(…, tables, boxes)` flattens a box into `Line` / `Block` atoms. Each atom carries `lead` / `trail` (counted when it is fitted) and an index into a table of enclosing-box chains. In `fill_multicol_columns`, an atom that opens a column drops the margin part of its `lead` (§5.2) and keeps the box's border and padding.
  - New `McPiece::Frame { node, x, y, height, open_top, open_bottom }`: one per box per column piece. `McPiece::Lines` gains `x: Px`, the offset of the source's content box from the column. It is 0 for an unpadded direct `<p>`, which keeps 12 / 24 / 25 byte-identical.
  - `flow_first_unit` takes `boxes`, so the F28 move uses a box's lead plus its first line.
  - `record_multicol_row` gets a `Frame` arm (a fragment of the box at `col_x + x`). The `Lines` arm adds `x`, and skips its per-page fragment when the source is a `Frame` box (that box's piece fragments stand in). A nested `Leaf` goes through `record_descendant` at its x.
- `convert/mod.rs` `convert_paged_multicol_slices`: origin x adds `piece.x`.
- `render.rs`
  - New field `PaginationGeometry.piece_edges: Vec<FrameEdges>`, parallel to `fragments` and set by the `Frame` arm. When it is non-empty, `frame_fragment` takes the edges by fragment index, not page: two pieces share a page, and keying by page gets first/last wrong. Everything else is unchanged.
  - `paint_multicol_paragraph_slices` paints from the source's first fragment on a page only. Otherwise a box with pieces in both columns would paint its lines twice.
- Ours: `Cargo.lock`, tests, fixture 30, `compare.py`, findings, `docs/upstream.md`. No library code.

**F193 stays separate.** Its atoms are table row bands (`McTable`), and its trigger is a container started in the last 100 px of a page with `tr { break-inside: avoid }` (F174d's move decision). This change does not touch `McTable`, the `Row` fill or the F28 move for tables, so folding F193 in would double the validation surface for a different root cause (one finding per PR). Its sweep (`out/scratch/f183v6`, `c_mc*`, `C2mc*`) is run as a byte-identity guard. Recorded in findings, not done here:

- F174c: a table in a wrapper can later become `McTable` rows inside an `McBox`.
- F173(f): the spanner gate can exempt boxes in `boxes`, as F174 exempted tables.
- F173(h): a box shorter than a page overflowing under a padded container is the fill rule for the first atom on a partial page, not this path.

## 4. Acceptance

`crates/htmltopdf/tests/multicol_boxes.rs`. Helpers as in `multicol.rs` (strict render, `pdftotext -bbox`, order rebuilt by page, then column by `xMin` against the gutter, then `yMin`). The expected numbers are Chrome's from step 1, hard-coded.

| Test | Shape (2500 words unless noted) | Asserts | Main |
|---|---|---|---|
| A | headline card of direct text | w0000–w2499 once each in column-major order, then AFTER once; Chrome's 4 pages; first word of every column of every page within one line of Chrome's; no `yMax` past 799.4 pt + 0.5; column-2 text at `col_w + gap + 17 px` ±1 pt | 361 |
| B | the same `<div>` without padding, border or background | as A | red (step 1) |
| C | the card holding 10 `<p>` of 250 words | as A | red |
| D | A with `column-count:3` | words, Chrome's pages | red |
| E | A with `column-fill:auto` | words, Chrome's pages, last page not balanced | red |
| F | `<p style="padding:16px;border…">` as a direct child | words; text x 17 px inside the frame ±1 pt | step 1 |
| G | guards: direct `<p>`s; bare text (Case A); a 300-word card that fits a page; a 300-word card that starts mid-page; `multicol_in_card`; a card holding a 130-row table | every word once (the table case only stays ≥ main, F174c) | — |

Fork tests:

- `mc_box` unit tests: a div of text; a card of `<p>`; a nested card; a clip, `height` or avoid box gives `None`; an inline root with an image gives `None`.
- Planner unit tests: a box shorter than `page_room` stays one atom (an empty `boxes` map reproduces today's plans); lead and trail are glued to the first and last atom; the margin lead is dropped at a column top; `Frame` extents and edges per column; n = 3; `auto`.
- A lib test: two `Frame` fragments of the card on page 1 at both column xs, with `piece_edges` top-closed then top-open.
- `render_smoke`: every glyph once; tagged, no text-show operator outside a BDC.

**Fixture `fixtures/invoices/30-terms-card-columns/`** (30, or the next free number). Recipe of fixture 24: "Faktura 2026-0030", header, parties, a short items table and the total, then `<section class="text-xs" style="columns:2;column-gap:24px">`. Inside it one card `<div style="padding:12px;border:1px solid #d1d5db;border-radius:6px;background:#f9fafb">` of Czech clauses (`<h3 style="margin:0">` + `<p style="margin:0">`), taller than a page in one column. It is tuned in Chrome to exactly 2 pages, with the card crossing the column break and the page break, and a signature `<p>` below.

- The PNG must show the card's background and side borders in both columns of both pages. The top border and radius appear only at its start (page 1, column 1), the bottom ones only at its end. No text below the content box.
- `fixtures.rs` asserts every clause heading in source order, the last word of every clause, the signature and 2 pages.
- Threshold = worst page + 1. Main's "before" is not comparable (it prints 1–2 pages); record its page count and word count instead.

Fixtures 01–29, the bench corpus (`slow-multicol` is a single `<p>`: no `Block`), the fork's VRT goldens and the examples stay byte-identical.

## 5. Steps

1. **Gate (½–1 day).**
   - Chrome m14x and main on A–G. Record pages, words per page, first and last word per column, and stderr.
   - Add a scratch dump in `record_multicol_row` (removed again) listing the `Block` atoms taller than `fresh` for each shape, and for fixtures 12 / 24 / 25 (expected: none). Run the F193 sweep as a baseline.
   - Proceed only if every lossy shape is a tall `Block` atom and the warning names it.
   - Any loss with another cause (for example `multicol_in_card` in the nested call site, or F173(h)) is filed, not folded.
   - **Fallback:** if A or C cannot be described by `mc_box` (Blitz did not lay the child out at the column width, or `shape_multicol_source` cannot shape a non-`p` source), re-plan to the smallest change that stops the loss. Cut such a box in place in its column, as the tall-spanner branch of `fragment_multicol_container` does (`block_walk_line_split` / `fragment_block_subtree` at the column x and width, the next atoms planned below it). All words print, column 2 stays empty beside the box, and the warning says the box was "printed in one column". File the balanced flow as F198b.
2. **Red tests and fixture 30 with its Chrome reference (½ day).**
3. **Fork, pure (1 day):** `McBox`, `mc_box`, flattening in `plan_multicol_pages`, `lead` / `trail` and chains, the margin drop, `McPiece::Frame`, `Lines.x`, `flow_first_unit`, unit tests. Nothing calls it yet; VRT byte-identical.
4. **Fork, wiring (1 day):** `McContainer.boxes`, the `Frame` arm, nested `Lines` / `Leaf` records, the convert x offset, lib tests. A–E green on words.
5. **Fork, render (1 day):** `piece_edges` in `frame_fragment`, slices painted once per page, `render_smoke` (tagged too). F green. Check VRT and examples byte-identical, fmt, `clippy -D warnings`.
6. **Ours (1 day):**
   - Lock bump, tests, `cmp` fixtures 01–29 against main's binary, the F193 sweep `cmp`.
   - `compare.py` for fixture 30 and its PNGs, threshold.
   - `bench.py --check` plus the `slow-multicol` row.
   - Findings (F198 done; notes on F174c, F173(f), F173(h)), `docs/upstream.md`.
   - `vrt-validator`; `pdf-reviewer`, because `render.rs` is on the writer path (tagged nested paragraphs, link rects).

## 6. Risks and non-goals

- **Byte identity of 12 / 24 / 25 and `slow-multicol`.** Expansion fires only for non-table boxes taller than a page (step 1 proves they have none). `Lines.x` is 0 without padding, and `piece_edges` is empty everywhere else. Any moved golden is a regression.
- **Speed.** `mc_box` walks only tall boxes (depth ≤ 16) and the planner stays linear in atoms. Measure A (2500 words) against Chrome's page count. `bench.py --check` must pass.
- **Determinism.** Only `BTreeMap`s and source-order `Vec`s; leads come from `final_layout`.
- **Accepted differences.** Chrome splits boxes shorter than a page at a column end; we move them whole (no loss, F30d-b). Orphans / widows are not implemented, so column ends may be a line off. Top-level column children's margins are still ignored (F30d-b). Readers of the container's Taffy height are unchanged (F30d risk).
- **Still lost, now warned.** An inline root with images or inline-blocks, a clip / `height` box, a table or a leaf taller than a page inside the card keeps the warning (F30d-b, F174c).
- **Non-goals:**
  - F193, F174c, F173(f) and (h), F30d-c (a fitting card is not balanced over two columns), F96 (page-split block frames);
  - `box-decoration-break: clone`; multicol in flex / grid / cells;
  - Tailwind `columns-*` (not compiled; rebuild only when a fixture needs the class).

## 7. Evidence

(Filled in when it lands: step 1 measurements, red/green tests, gates, sweeps, deviations.)
