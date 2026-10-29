# F198b: a tall box in a multi-column container loses words when it is an auto-height clip card or its lines hold inline boxes

Date: 2026-10-29. Finding: F198b in `2026-10-15-m0-findings.md` (HIGH, warned). Main `45be070`; fork `subapavel/fulgur` `htmltopdf/srgb-colours` head `26137aa7`. Names are from that head; re-read line numbers before editing. No `css-spec-researcher` output: §2 is unverified.

## 1. Problem

F198 cuts a column child taller than a page between its lines (`mc_box`, `McPiece::Frame`). `mc_box` returns `None` for every other shape, and the box falls back to the old path: one `Block`, placed whole, warned, everything below the page edge lost. Repros: `out/scratch/f198v/` and `out/scratch/f198pdf/` (A4, 15 mm, Inter 14px/1.2). Words printed by main / total (Chrome prints all of them):

- `overflow.html` (card with `overflow:hidden`): 361/2500.
- `imginline` (inline `<img>` in the text): 360/2100.
- `f198pdf/imgp-*` (a `<p>` with an inline image, taller than a page): 248/1200; 185 inside a card (`imgcard-*`).
- `inlineblock.html`: 337/1920.
- `svg.html` (inline `<svg>` in a `<p>` of the card): lost, same warning.

Also lost, and filed in §3: `abspos.html` 361/2500, `float.html` 344/2500, `spanbefore/after/mid` 361/2500 and 733/2400, `table.html` 311/771, `height`, opacity, transform, % padding, an image taller than a page, depth > 16, multicol in the card. No fixture covers these shapes inside columns. Fixture 16 has the clip card (`overflow-hidden rounded-lg border` terms card) and fixture 27 an inline logo in a `<p>`, both outside multicol.

## 2. Spec (unverified)

- css-break-3 §2, "monolithic": replaced elements and line boxes are monolithic. A box with `overflow` other than `visible` counts only when its block size is not `auto`. So an auto-height clip box is fragmented like any block (ADR-0004 notes Chrome does this).
- css-break-3 §4.1: class B break points sit between line boxes. An atomic inline (CSS 2.1 §10.8, css-inline-3) is part of its line box and moves with it. §4.4: a box that does not fit breaks inside it.
- css-break-3 §5.4 `box-decoration-break: slice` (the initial value; F30f-d): no border, padding or radius at a break.
- css-overflow-3 §3: content is clipped to the padding box. With slice, each fragment clips to its own sliced padding box, rounded only at its unbroken corners. An auto-height box hides nothing in the block axis: no band, ADR-0009.
- css-multicol-1 §8.2 (columns continue on the next page) and §7.1 (`column-fill`), as in F198.
- For the filed F198e: css-position-3 §3 (containing block) and css-break-3 (abspos in a fragmented containing block) are unverified. Chrome's behaviour is measured in step 1.

## 3. Where it belongs, and scope

Fulgur fork only. Blitz and Parley lay the content out correctly: the positions are in `final_layout` and in the Parley layout. Fulgur's multicol fragmenter refuses the shapes (F30d / F198). No upstream PR (standing decision).

**This PR fixes two shapes, ranked by how likely each is in a real invoice's tall multicol box:**

1. **Auto-height clip card.** `overflow-hidden rounded-lg border` is Tailwind's card idiom: it clips a header strip's background to the rounded corners. Fixture 16's terms card uses it, so any terms card moved into columns hits this.
2. **Lines with atomic inline boxes.** One mechanism covers heroicon `<svg>`s, `inline-flex` badges and inline logos (fixture 27, fixture 08).

**Filed, not done here:**

- **F198e:** an abspos child, e.g. a PAID badge. It usually sits in the header, not in a terms card taller than a page, and it needs a placement rule measured in Chrome.
- **F198f:** floats (rare in terms text).
- **F198g:** a constrained clip box or `height` taller than a page, opacity, transform, % padding (blocked on F30f-g), a line or image taller than a page, depth > 16, multicol in the card, a clip ancestor outside the container.
- **Spanners** stay F173(f). Add the `span*` repros and the wrong "paragraph of 423 lines" warning there. The fix touches the F161 spanner path.
- **A table in a card** stays F174c: `McTable` rows inside an `McBox` is a planner change.

**Root cause and design, per shape:**

**A. Clip card.** `mc_box_eligible` rejects it via `table_subtree_is_wrapped(doc, node.id)`, which rejects `overflow != visible` on the box and on every ancestor.

- `pagination_layout.rs`: replace that call with `fn mc_box_wrapped(doc: &BaseDocument, node: &blitz_dom::Node) -> bool`.
  - It stays true for transform or `opacity < 1` on the node.
  - It stays true for a clip with `block_size_constrained` (the ADR-0009 band case, F198g).
  - It is true for any wrapped ancestor from the multicol container upward (unchanged).
  - Boxes between the container and the node are themselves `McBox` boxes and are checked by the same rule.
- `render.rs` `draw_under_clip`: when `geom.piece_edges` is non-empty, each `Frame` fragment is one clip piece. Its clip path comes from `sliced_style`, so there is no radius at an open edge and no band.
  - Only descendant fragments whose origin lies inside this piece's border box are painted: new `fn piece_contains(frag: &Fragment, x: Px, y: Px) -> bool`. Pieces on one page do not overlap, because `mc_box` refuses `x < 0`.
  - Paragraph sources that have `paragraph_slices` (and the card itself, when it is the inline root) are skipped by the descendant loop. They are painted through `paint_multicol_paragraph_slices(…, within: Option<Rect>)`, which keeps only the slices whose origin is in the piece.
  - **We must not use** the "last piece on page" rule (F198 deviation 3) here. Under a clip it would cut column 1's text out of view while still writing it.

**B. Inline boxes.** `mc_line_advances` requires `parley_layout_is_glyph_run_only`. The reason: the slice painter cannot paint inline boxes. `shape_paragraph_glyph_runs` makes no `InlineBox` items, `draw_slice_tagged` passes `None` as the `InlineBoxRenderCtx`, and the `Lines` arm of `record_multicol_row` records no geometry for the inline boxes.

- `fn mc_line_advances(node, fresh: f32) -> Option<Vec<Px>>` accepts inline boxes when every line is at most `fresh + 1`. A taller line keeps the warning: ADR-0004 shape, F198g.
- `record_multicol_row`, for every `Lines` piece (not only the first on its page): `fn record_line_inline_boxes(geometry, doc, source: usize, lines: Range<u32>, page: u32, x: f32, y: f32)` calls `record_descendant` for each inline box of those lines, at the box's Parley position relative to the piece.
- `convert/mod.rs` `convert_paged_multicol_slices`: for a source whose layout has inline boxes, take its lines from `out.paragraphs[source].lines` (the `extract_paragraph` lines, with `InlineBox` items). **We must not** re-shape a glyph-run-only source this way: that would move the bytes of 12 / 24 / 25 / 30.
- `render.rs` `draw_slice_tagged`: pass `Some(InlineBoxRenderCtx)`. It is a no-op for slices without inline boxes.
- One consequence: a `<p>` with an image that fits a page inside a tall card becomes `Lines` instead of a `Leaf`, so it is cut like Chrome cuts it.

`multicol_layout.rs` is unchanged. Ours: `Cargo.lock`, tests, fixture 31, `compare.py`, findings, `docs/upstream.md`.

## 4. Acceptance

`crates/htmltopdf/tests/multicol_boxes.rs` (helpers as in F198). Chrome's numbers come from step 1 and are hard-coded. Every test asserts:

- every `wNNNN` exactly once (`pdftotext`), in column-major order, then AFTER once;
- Chrome's page count, and the first word of every column within one line of Chrome's;
- no `yMax` past the content box + 0.5 pt;
- per-column dark-pixel counts > 0 (no frame covers text).

| Test | Shape | Extra asserts | Main |
|---|---|---|---|
| H | `overflow.html` | pages and words as F198 shape A (expected identical) | 361/2500 |
| H2 | Tailwind `overflow-hidden rounded-lg border`, `bg-gray-100` header strip | strip colour absent outside the radius at the card's top corners (page 1, column 1); side borders in both columns | red |
| I | `imginline` | the image drawn once (`pdfimages -list` count), its box beside its line ±1 pt | 360/2100 |
| I2 | `imgp-*`, `imgcard-*` | as I | 248, 185/1200 |
| J | `inlineblock`, plus an `inline-flex` badge | badge border painted once, text inside it once | 337/1920 |
| K | `svg.html` | icon once per occurrence | lost |
| G | guards: constrained clip box (`h-[2000px] overflow-hidden`) and abspos / float cards still warned and byte-identical to main; `clip_boxes.rs` green | — | — |

**Fork tests:**

- `mc_box` unit tests: a clip card is described; transform, opacity and a constrained clip give `None`; an inline root with an image or inline-block gives `Lines`; a line taller than a page gives `None`.
- A lib test: `draw_under_clip` of a two-piece card paints each paragraph slice exactly once.
- `render_smoke`, tagged: inline img (Figure), inline-block holding a `<p>`, link on an image. No nested `start_tagged` panic, no text-show outside a BDC for `<p>`.

**Fixture `fixtures/invoices/31-terms-clip-card-columns/`.** Recipe of fixture 30. The card is `overflow-hidden rounded-lg border border-gray-300`, with an `<h3 class="bg-gray-100 px-3 py-2">` strip, clauses with inline heroicon `<svg class="inline size-3">` and one inline `<img>` logo. It is taller than a page in one column and tuned in Chrome to 2 pages.

- The PNG must show: the strip's corners rounded (page 1, column 1 only); text in both columns of both pages; every icon once, next to its word; the bottom border and radius only at the card's end.
- `fixtures.rs` asserts the clause headings, the last word of each clause and 2 pages.
- Threshold: worst page + 1. Main's "before": record its pages and words.

Fixtures 01–30, the bench corpus, the fork VRT goldens and examples stay byte-identical. So do every `f198v` / `f198pdf` repro outside H–K and the F193 sweep.

## 5. Steps

1. **Gate (½–1 day).**
   - Run Chrome m141 and main on H–K. Also run the filed shapes (abspos with `top` / `bottom` badges, float, span*, table, height, opacity, pctpad, imgtall, depth, multicol-in-card, a clip ancestor outside the container), record their numbers and put them in F198e/f/g / F173(f) / F174c.
   - Add a scratch dump in `mc_box_at` naming the first predicate that fails, per shape.
   - Check that `out.paragraphs[source].lines.len()` equals the Parley line count, and that every `InlineBox` item has a `node_id`.
   - Baseline sums: fixtures, corpus, f198v / f198pdf, the F193 sweep.
   - **Proceed** only if A's sole refusal is the wrapped check, B's sole refusal is glyph-run-only, and the line counts agree.
   - **Re-scope:** if A needs more (offsets differ in an `overflow:hidden` BFC), ship B and file A as F198h. If B needs `extract_paragraph` changes (marker or pseudo lines), ship A and file B as F198h. If both fail, write a postmortem and stop.
2. **Red tests and fixture 31 with its Chrome reference (½ day).**
3. **Fork, B layout (1 day):** `mc_line_advances(fresh)`, `record_line_inline_boxes`, unit tests. VRT byte-identical.
4. **Fork, B paint (1 day):** source lines from `out.paragraphs`, ctx in `draw_slice_tagged`, tagged smoke. I, I2, J, K green.
5. **Fork, A (1 day):** `mc_box_wrapped`, per-piece `draw_under_clip`, `piece_contains`, `paint_multicol_paragraph_slices(within)`, lib test. H, H2 green.
6. **Ours (1 day):**
   - Lock bump; `cmp` of everything in the step 1 baseline.
   - `compare.py` and the PNGs of 31; `bench.py --check`.
   - Findings: F198b done, F198e/f/g filed, notes on F173(f) and F174c; `docs/upstream.md`.
   - `vrt-validator`; `pdf-reviewer` (writer path: clip, tagged inline boxes, link rects).

## 6. Risks and non-goals

- **Byte identity.** Each change is gated: `boxes` (only boxes taller than a page), `piece_edges` non-empty, a source with inline boxes. A golden that moves anywhere else is a regression.
- **Double or hidden text under a clip.** The containment filter has to be exact. "Every word once" plus per-column ink catch both failures.
- **Tagging.** Krilla's `start_tagged` does not nest. An inline-block dispatched inside a tagged slice could panic. The tagged smoke test and `pdf-reviewer` cover it.
- **Speed.** Containment is pieces × descendants per page. `bench.py --check`.
- **Determinism.** `BTreeMap` only; positions come from `final_layout` / Parley.
- **Accepted differences** are those of F198 §6 (boxes that fit move whole; no orphans / widows).
- **Non-goals:** F198c (column-rule), F198d (horizontal geometry), F193, F96, `box-decoration-break: clone`, Tailwind `columns-*`.

## 7. Evidence

### Step 1 (gate), 2026-10-29

Chrome (Playwright, `out/scratch/f30fd/chrome.mjs`, m141) and main `45be070` (`htmltopdf-main-f198`, byte-identical to the F198 branch binary), A4, 15 mm, Inter 14px/1.2, words `w0000…`. Repros: `out/scratch/f198v/`, `out/scratch/f198pdf/` and the new `out/scratch/f198b/` (git-ignored; generators were one-off scripts). Words per page, column 1 + column 2 (Chrome in the last column):

| Shape | Main: pages / words | Chrome: pages / words per page | Refusal named by the scratch dump in `mc_box_at` |
|---|---|---|---|
| H `overflow.html` (`overflow:hidden` card, 2500 words) | 2 / 361 | 4: 346+344 / 353+363 / 360+359 / 192+183 (= shape A of F198) | `overflow(wrapped)` via `table_subtree_is_wrapped`; nothing else |
| `clipcard` (Tailwind-like card: border, radius, `overflow:hidden`, blue header strip, 10 `<p>` with padding) | 2 / 358 | 4: 334+352 / 344+365 / 347+342 / 208+208; column 1 starts at 73.8 pt | the card: wrapped only; the `<p>` are plain boxes |
| I `imginline` (14 inline `<img>` in the text of the card, 2100 words) | 2 / 360 | 4: 337+336 / 346+354 / 352+352 / 12+11; `AFTER` 84.3 pt | `mc_line_advances` (glyph runs only) |
| I2 `imgp` (a `<p>` with an image, direct child, 3 columns, 1200 words) | 2 / 248 | 2: 236+236+235 / 164+164+165; `AFTER` 556.2 pt | same |
| I2 `imgcard` (the same `<p>` in a card) | 2 / 185 | 3: 177+177+177 / 179+180+183 / 39+48+40; `AFTER` 190.9 pt | the `<p>` inside the card: same |
| J `inlineblock` (60 `inline-block`, 1920 words) | 2 / 337 | 3: 314+313 / 320+331 / 324+318; `AFTER` 772.1 pt | `mc_line_advances` |
| J2 `badge` (50 `inline-flex` badges, 2500 words) | 2 / 349 | 4: 330+329 / 334+343 / 341+341 / 246+236; `AFTER` 586.8 pt | same |
| K `svgdiv` (50 inline `<svg>` in the text of the card) | 2 / 358 | 4: 343+340 / 344+362 / 360+357 / 202+192; `AFTER` 470.6 pt | same |
| K `svgp` (the same in one `<p>` of the card) | 2 / 358 | the same numbers | the `<p>`: same |

The plan's K repro `f198v/svg.html` is not a lost shape on main: its `<svg>` are siblings of the `<p>`, so each `<p>` of 300 words is a `Leaf` that fits a page and all 1200 words print (placed whole, as every `Leaf`). The lost shape is an `<svg>` inside the paragraph or the text of a card taller than a page: `svgdiv` / `svgp`.

Filed shapes, main against Chrome (all of them warn on main; none is touched here):

| Shape | Main | Chrome | Refusal |
|---|---|---|---|
| F198e `abspos` (badge child in the card) | 2 / 361 | 4 / 2500 | `float/abspos child` |
| F198f `float` | 2 / 344 | 4 / 2500 | the same |
| F198g `height` (card `height:2000px`, no clip) | 2 / 361 | 4 / 2500 | `block_size_constrained` |
| F198g `clipheight` (`height:2000px; overflow:hidden`) | 2 / 361 | 2 / 696 words printed: the box hides the rest, ADR-0009 | `overflow(wrapped)`, `block_size_constrained` |
| F198g `opacity`, `transform` | 2 / 361 | 4 / 2500 | `opacity` / `transform(wrapped)` |
| F198g `pctpad` (`padding:5%`) | 2 / 378 | 4 / 2500 | `pct-padding` |
| F198g `imgtall` (image 1500 px tall) | 2 / 300 | 2 / 600 | child taller than a page, not describable |
| F198g `deep20` (20 nested divs) | 2 / 240 | 5 / 2000 | depth 16 |
| F198g `clipanc` (`overflow:hidden` around the multicol container) | 2 / 361 | 4 / 2500 | wrapped ancestor: stays refused |
| F173(f) `spanbefore` / `spanafter` / `spanmid` | 3 / 361, 2 / 361 (2600 expected), 4 / 733 | 4 / 2500, 4 / 2600, 4 / 2400 | the spanner path warns "a paragraph of 423 lines" |
| F174c `table` (130-row table in the card) | 2 / 112 | 2 / 260 | child taller than a page |

Gate checks:

- A's sole refusal is the wrapped check (H, `clipcard`); B's sole refusal is the glyph-run-only check (I, I2, J, J2, K): nothing else fails for these shapes.
- With the refusal lifted (scratch, env var), `out.paragraphs[source].lines.len()` equals the Parley line count for every source (358 / 332 / 427 / 432 / 300 / 391 lines for `imginline` / `inlineblock` / `svgdiv` / `badge` / `imgp` / `imgcard`), equals the shaped line count of `shape_multicol_source`, and none of the 14 / 60 / 50 / 50 / 1 / 1 `InlineBox` items has `node_id: None` (`extract_paragraph` only skips absolutely positioned pseudo-elements, before it makes the item).
- Baselines (main, `out/scratch/f198b/base-main/`): fixtures 01 to 30 + the bench corpus at 1 and 4 threads, the F193 sweep (48 files), every `f198v` and `f198pdf` repro (PDF + stderr).

Decision: **both shapes proceed** (plan §5 step 1).

