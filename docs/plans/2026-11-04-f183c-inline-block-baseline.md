# F183c — an `inline-block` sits on its line by its last line box's baseline, not by its bottom edge

Date: 2026-11-04. Finding: F183c (`2026-10-15-m0-findings.md`, with the F213 and F213e notes), re-scoped to the baseline of atomic inlines with
in-flow content; folds in F183b (section 3.2). Pilot item (`docs/roadmap.md`, *Pilot bar*): it must land before F217 (PR #95), which exposes it.
Status: steps 0 to 7 done (7.1 to 7.5); the 1 px band accepted (7.5, "Decision"); the `vrt-validator` and `pdf-reviewer` runs of step 7 are the coordinator's.

## 1. Problem

Parley 0.6 puts every inline box bottom-on-baseline and sizes the line that way: the box counts only as ascent
(`parley/src/layout/line/greedy.rs:493-496`), and the box is placed at `y = baseline - height` (`layout/line/mod.rs:249`). Blitz
copies that `y` into the box's layout (`blitz-dom/src/layout/inline.rs:313-316`), and that is the position the fragmenter records.
fulgur then paints the box `height - last_line_baseline` lower (`convert/inline_root.rs:633-638`,
`inline_box_baseline_offset_from_drawables`), so that the last line inside the box lands on the outer baseline. So the line box and the
recorded geometry say "box above the baseline", and the paint says "box hanging below it by the free space under its last line".

Fixtures that show it today:

- **44:** the 70 mm signature box is painted 91.3 pt below Chrome's position.
- **45:** the 843 px invoice box is 5.8 pt low ("Faktura" at 49.1 pt, Chrome 43.3), and that is why page 2 is at 9.2 %.
- **43:** the QR box and its line are 14 pt low.

F217's validator probes (`scratchpad/f217v/r/u01_stamp_text.html`, `u03_qr_caption.html`) show the invoice shapes: a 140 x 60 "ZAPLACENO"
stamp, or a 112 px "QR kód" caption box, in a flex row. Once F217 gives such a box its real height, it hangs 17-76 pt below its row and
paints over the next block, or into the bottom margin near a page end. The same wrong seat loses text: in F183c-b, the lines of a fixed
box that overflow it are pushed above the page top.

## 2. Spec (cited from memory; no `css-spec-researcher` output; unverified)

- **CSS 2.1 §10.8.1, `vertical-align: baseline`.** The baseline of an `inline-block` is the baseline of its last line box in the normal
  flow. When it has no in-flow line boxes, or its `overflow` is not `visible`, its bottom margin edge is the baseline. The baseline of an
  `inline-table` is the baseline of its first row. Every line box starts with a strut.
- **CSS 2.1 §10.8.** The line box runs from the top of the highest box to the bottom of the lowest box; for an `inline-block`, that box is
  its margin box.
- **css-align-3 §9.1, `baseline-source: auto`.** Last baseline for `inline-block`, first baseline for every other box. So `inline-flex`
  and `inline-grid` use their first baseline.
- **css-flexbox-1 §8.5 (flex container baselines), and css-grid-1's grid container baselines.** The baseline is that of the first item
  that participates in baseline alignment, otherwise that of the first item.
- **css-overflow-3: `overflow: clip` does not make a scroll container.** CSS 2.1's "other than `visible`" predates `clip`; step 0
  measures what Chrome does.
- **css-break-3 §4.1.** A line box is monolithic. This matters for the fragmenter effects in section 3.3.

## 3. Where it belongs

### 3.1 Which repos change

Parley owns the line geometry. Blitz owns the box tree that the baseline comes from. fulgur must stop moving the box a second time.
There is no Taffy change and no change in our crate.

| Repo (fork branch) | Change |
|---|---|
| Parley `htmltopdf/inline-box-strut` (on `1f23226`) | `InlineBox::baseline: Option<f32>`: layout units from the top of the box (its margin box, like `height`) to its alignment baseline. It may exceed `height` when content overflows below. `None` keeps bottom-on-baseline, bit for bit. In `BreakLines::finish`, a line holding a box with `Some(b)` takes the explicit branch that F183 added, with the strut made optional: above = max(text and strut above, `b`, flagged `None` box heights); below = max(text and strut below, `height - b`, 0). Unflagged `None` boxes keep the rule that the baseline sits at least their height down. In `GlyphRunIter::next`, `y = baseline - b.unwrap_or(height)`. The field name follows upstream 0.12's `InlineBox::baseline` (its exact meaning there is not checked). |
| Blitz `htmltopdf/blitz-dom-0.2.4` | New `layout/baseline.rs`: `fn atomic_inline_baseline(doc: &BaseDocument, node_id: usize) -> Option<f32>` (CSS px from the box's border-box top) and `fn last_line_baseline(doc: &BaseDocument, node_id: usize, depth: usize) -> Option<f32>`, a port of fulgur's `pageable_last_baseline_from_drawables` walk: an inline root gives its border-top + padding-top + last line's baseline; otherwise the in-flow `layout_children` are walked in reverse; `overflow` other than `visible` gives `None`. `compute_inline_layout` sets `ibox.baseline = baseline.map(\|b\| (margin.top + b) * scale)`. Each non-replaced, in-flow inline box is laid out with `run_mode: PerformLayout`, also when the root is only being measured, because otherwise the subtree is not there in a measuring pass and the measured height would differ from the laid-out one. This is the same contract Taffy's block layout uses for its in-flow children (`compute/block.rs:223-232` runs `perform_final_layout_on_in_flow_children` in `ComputeSize`). `whitespace.rs` `replay` passes `baseline: None`. |
| fulgur `htmltopdf/srgb-colours` (on `09d4d25a`) | `convert/inline_root.rs`: `computed_y = positioned.y - accumulated_line_top`; the `baseline_shift` term and both `*_baseline_*_from_drawables` functions are removed (their five lib tests move to Blitz). Otherwise the box would be moved twice. Stale comments in `render.rs` (about lines 946 and 5068) and `pagination_layout.rs` (`record_carrier_slices`) are updated. |

**Fork rules.** The branches are append-only: no force-push and no rebase. There are no upstream PRs or issues. Every commit is recorded
in the Parley, Blitz and fulgur rows of `docs/upstream.md` in the same PR.

**Blitz sequencing.** The Blitz branch head is `5305fa41`. That is main's lock (`3cea708f`) plus F217's `4c3af6ae` (red tests) and
`5305fa41` (fix), which are pushed but unmerged. Cargo can only lock a commit reachable from the branch head. So F183c first appends
`git revert 5305fa41` and `git revert 4c3af6ae`, then its own commits. `git diff 3cea708f <F183c head>` must touch only F183c files.
After F183c merges, F217 re-applies its two commits on top (by reverting the two reverts), moves PR #95's lock and runs its gates again.
Taking F217's engine change into this PR instead would break merge-policy item 4. Step 0 records this as ADR-0012, because it will
happen again.

### 3.2 F183b and `overflow`

F183b is folded in. It is one predicate on the same Parley branch, and landing it apart would re-baseline the same fixtures twice.
`strut_box` flags every atomic inline with `vertical-align: baseline`, not only `<img>` and `<svg>`, so an empty or clipped box keeps
the strut's descent under it (G3: next block at 52, today 48). `inside_atomic_inline` is removed: it exists only because the outer
line seated the box by its bottom (F183 plan §7.3 item 2). The `img_in_button` case is re-measured in step 0.

A box whose `overflow` is not `visible` keeps `None`, its bottom margin edge, as fulgur paints it today. Its only change is the strut
below it (F183b). The fragmenter's clip semantics (ADR-0009, F30f-b) are untouched.

### 3.3 The fragmenter (F213, F213d, F213e)

The fragmenter reads three inputs: Parley's line heights (F78 line slices), Blitz's box geometry, and the predicates built on them
(`monolithic_line_carrier`, `content_ends_inside`, `ancestor_insets`, `first_box_overflows`). None of that code changes.

What changes in its inputs:

- **Box geometry.** It moves down by at most `ceil(descent / 2)` (about 2 px). The old negative-leading offset, which put the box top
  above its line, goes away.
- **The carrier line's height.**
  - Unchanged where the box's part below its baseline is at least the strut's: fixed boxes with free space under the last line (43,
    44, 45, the stamp).
  - Up to about 1 px taller for a box whose last line is at its bottom (03's badges).
  - About 4 px taller for an empty or clipped box (F183b).
  - Much taller for a box whose content overflows below its height (the F183c-b shape: the line now covers the overflow, as Chrome's
    does).

So page breaks move only at a room's edge and for overflowing boxes; we expect them to move toward Chrome (the F213d-e "fixture 44
with 5 rows" and A5 cases are candidates). The paint now equals the recorded geometry. That removes the hangs that the F213d safety
net reports, and the split and unsplit branches of `paragraph::inline_box_offset` now agree.

## 4. Acceptance

### 4.1 Red tests

New file `crates/htmltopdf/tests/inline_block_baseline.rs`, with Chrome m153 numbers from step 0. Y positions must be within 1 px
(0.75 pt), unless a test states otherwise.

- **B1.** The stamp (140 x 60, `padding-top: 12px`, 3 px border, "ZAPLACENO") alone in a block: box top, text baseline, next block.
- **B2.** `<p>Zaplaťte <span ib 112x112>QR kód</span> nebo…</p>`: the two baselines are level, and AFTER is at Chrome's position.
- **B3.** The 43 shape (auto height, label, QR, amount).
- **B4.** A badge on a text line (the 03 shape).
- **B5.** `overflow: hidden`, `clip` and `auto` boxes with text.
- **B6.** An empty 48 px box: next block at 52.
- **B7.** An `inline-flex` badge, and an `inline-flex` box with items of different heights.
- **B8.** Nested boxes: the last block child holds an inline-block.
- **B9.** The F183c-b box (300 px fixed, 40 lines): every line printed once, none above the page top.
- **B10.** `pg_t_*` from `scratchpad/f217v/pg`, in a plain block at the room's edge: pages, words, no warning.
- **B11.** Guard: two `vertical-align: top` boxes keep today's placement relative to each other (F183i).
- **B12.** `margin-top` on the box.
- **B13.** A float or absolutely positioned last child is skipped.

Each fork gets its own red tests:

- **Parley:** `None` is bit-identical to today; placement with `Some(b)`; a negative part below the baseline; with and without a strut.
- **Blitz:** the baseline walk, and the measured height equals the laid-out height for B1, B2 and B9 inside a grid item and a table
  cell.
- **fulgur:** `computed_y` equals Parley's `y`.

Existing tests that turn red and get Chrome's numbers:

- `inline_img_strut.rs`: the F183b guards (about lines 420, 492 and 568).
- `page_bottom_warning.rs` "F183c, a fixed 600 px box": now no warning.
- `fixtures.rs` (45, "Faktura"): the 7.0 pt bound becomes 1.5 pt.
- `tall_inline_box_line.rs` and `alone_inline_block_line.rs`: y is compared where they now skip it because of F183c.

### 4.2 Fixtures that will move (expected direction: toward Chrome)

| Fixture | Inline box | Expected | Worst page now (threshold) |
|---|---|---|---|
| 03 | two `inline-block` badges in a `p` | badges about 3 px up, line about +1 px | (5.8) |
| 27 | `.meta div` (`vertical-align: top`), `.pay dt` | line heights | 6.1 (7.1) |
| 35 | 6 px `::before` dot | F183b strut path only | 2.7 (3.7) |
| 37 | hidden STORNO stamp, `.meta` | line heights | 3.17 (4.2) |
| 43 | QR box | box and line 14 pt up | 3.7 / 1.4 (4.7) |
| 44 | signature box | box 91.3 pt up on page 2 | 3.3 / 0.8 (4.3) |
| 45 | 843 px invoice box | 5.8 pt up; page 2 about the text-drift level | 0.0 / 9.2 (10.2) |

Page counts stay at Chrome's for every fixture.

### 4.3 New fixture `48-paid-stamp-header`

A one-page Czech invoice:

- A `grid grid-cols-2 gap-8` header. The right column has an `h1` and, under it, an `inline-block` stamp `.razitko`, styled in the
  fixture's `<style>` (140 x 60, 3 px `#c00` border, "ZAPLACENO" and a date line, `text-align: center`; `text-center` and the red
  classes are not in `tailwind.css`).
- An items table.
- A payment paragraph `Zaplaťte QR kódem <span class="inline-block w-28 h-28 border border-gray-300 bg-gray-50 rounded-md text-xs">QR
  kód</span> nebo převodem…`, ending within about 40 px of the content bottom in Chrome (N rows tuned in step 1).

The header is a grid, not a flex row, so that F217 is not needed. F217 adds the flex copy to its own tests. Every class used was checked
with `rg -o '\.cls\{'`: `grid`, `grid-cols-2`, `gap-8`, `text-right`, `text-2xl`, `font-bold`, `mt-2`, `inline-block`, `w-28`, `h-28`,
`border`, `border-gray-300`, `bg-gray-50`, `rounded-md`, `text-xs`, `text-sm`, `mt-6`, `mb-8`.

What the PNG must show: the stamp's frame sits under the title like Chrome's; the QR frame sits on the line of its text, not below it;
nothing is in the bottom margin. The test asserts the y of "ZAPLACENO", "QR", "Zaplaťte" and "Děkujeme", and no warning. The reference
comes from `scripts/reference/render.mjs`. The README is in English, the text in Czech. The threshold is the first number plus 1 point.

### 4.4 Sweep against Chrome m153

Families:

- context: block `p`, grid item, table cell, flex-column item;
- box: fixed 60, 112, 300 and 600 px with text at the top; auto height with the last line at the bottom; empty; `overflow-hidden`; an
  `inline-flex` badge;
- room: fits; the room's edge in 4 px steps; no room;
- plus the `pg_t_*` and `pg_e_*` probes, and the HTML of the F213, F213d and F213e test files.

What is compared: pages, words per page, the y of marker words, and warnings.

Pass condition: no case loses a word or gains a warning, and no case is further from Chrome than main. Every changed case is listed in
§7.

### 4.5 Byte gate

These must be identical to main at RAYON 1 and 4: fixtures 01, 02, 04-26, 28-34, 36, 38-42 and the seven `bench/corpus` documents
(no atomic inline other than replaced boxes, which keep `None`).

These differ and are judged by `compare.py` and their PNGs: 03, 27, 35, 37, 43, 44, 45 and 48. Thresholds are lowered in the same PR to
the new number plus 1 point. No growth is expected. A growth in 35 or 37 is a regression to fix; merge-policy item 3 does not cover it.

The fulgur VRT goldens `layout/inline-block-basic`, `-nested`, `-overflow-hidden`, `inline-flex-smoke`, `inline-grid-smoke` and
`review_card_inline_block` will move. They are added to the deferred re-bless list in `docs/upstream.md`.

## 5. Steps (each at most a day)

0. **Measure and prepare.** Measure B1-B13 in Chrome m153, plus `overflow: clip / auto`, `img_in_button`, inline-flex, inline-grid and
   inline-table, into §7. Write ADR-0012. Append the two Blitz reverts.
1. **Red tests and fixture.** Ours: section 4.1, the flipped guards, fixture 48 with its reference and README. Commit them red.
2. **Parley.** The field, the `finish` branch and the `y` change, with unit tests. Blitz gets one output-neutral commit (`baseline:
   None`). The byte gate must be identical.
3. **Blitz and fulgur together.** Blitz: the baseline walk, `PerformLayout` for the boxes, `ibox.baseline`, lib tests. fulgur: remove
   `baseline_shift`. Both go in one lock move, because either alone moves the box twice or not at all. Green: B1-B4, B8, B9, B12, B13.
4. **F183b.** `strut_box` covers atomic inlines; `inside_atomic_inline` is removed. Green: B5, B6 and the `inline_img_strut` guards.
5. **First baseline.** `first_line_baseline` for `inline-flex`, `inline-grid` and `inline-table` (first item, first row; css-align-3
   §9.1). Green: B7. If step 0 shows Chrome's table rule needs cell `vertical-align`, the table part is filed instead.
6. **Gates.** `cargo test`, the byte gate, `render-fixtures.sh` and `compare.py` with the PNGs looked at, `bench.py --check` (the
   measuring layout pass is new work), the sweep, thresholds lowered.
7. **Docs.** `docs/upstream.md` (three rows); findings: F183c and F183b done, F183c-b re-checked (only its "above the top page edge"
   warning may stay open), F183i filed, the F213d-e notes. Then `vrt-validator` and, because the fulgur paint path changes,
   `pdf-reviewer`.

## 6. Risks and non-goals

- **Every inline-level box with text moves vertically.** Badges, stamps, `.meta` columns and the VRT goldens all move. A wrong baseline
  walk moves them all at once, so the tests pin one case of each shape.
- **Taffy's cache.** Laying a box out fully in a measuring pass leaves its subtree as last laid out. A later cache hit on the root's
  final layout could then keep the measuring pass's subtree, a hazard Taffy's block layout already lives with. The Blitz tests compare
  measured and laid-out heights inside grid items and table cells, and B10 checks the paint.
- **Speed.** Inline boxes are laid out in measuring passes too. The corpus has none, but an item table with a badge per row is probed
  in step 6.
- **Fractional insets** give fractional line heights. No extra rounding is applied; if Chrome rounds differently, that goes in §7.
- **Carriers whose content overflows** (F30f split, ADR-0004) grow tall; B9 and the sweep cover this.
- **Non-goals:**
  - `vertical-align` other than `baseline` on any inline box: these boxes stay aligned by their last line, as they are painted today.
    Filed as F183i (the original F183c text).
  - `align-items: baseline` in flex and grid (Blitz gives Taffy no item baselines).
  - The baseline of form controls beyond today's walk.
  - F183d, F183e and F183f.
  - Flex rows with a fixed-size box: that is F217, re-validated after this PR.

## 7. Evidence

### 7.1 Step 0: Chrome m153 against main (f550a8e)

Chrome 153.0.8010.12 (`/opt/pw-browsers/chromium-1243`), `print` media, A4, 15 mm margins, `body { font: 14px/20px Inter; margin: 0 }`,
`p { margin: 0 }`, `preferCSSPageSize`, `printBackground`: the settings of `scripts/reference/render.mjs`. Main is the binary of f550a8e. Words are the `yMin` of
`pdftotext -bbox` in pt; boxes are the magenta border at 192 dpi in pt (top-bottom). **Inter reaches Chrome through `@font-face` from
`fixtures/fonts/*.ttf`** (as the fixtures do through `tailwind.css`): with the host's Inter OTF Chrome embeds Type 3 fonts and `pdftotext` reports a `yMin`
2.9 pt smaller for the same glyph position (the glyph rows are identical, checked on a raster). The older Chrome numbers in `tall_inline_box_line.rs`
(41.7 pt, 267.4 pt, 116.7 pt) come from that host font: on the scale of ours they are 44.6, 270.3 and 119.6 pt.
All probes, in about 130 files, are kept in the scratchpad of the step-0 session; the shapes are in `crates/htmltopdf/tests/inline_block_baseline.rs`.

What the table says: **every number of the plan holds except the four marked CONTRADICTS below.** AFTER is the next block; "main" is the hang of the box.

| Probe | Chrome | main | Plan |
|---|---|---|---|
| B1 stamp 140x60, pad-top 12, 3 px border, alone in a block | box 57.8-116.2, ZAPLACENO 70.1, AFTER 117.3 | box 93.4-151.9, ZAPLACENO 105.8, AFTER 117.1 | holds: box 35.6 pt low, line height equal |
| B2 `Zaplatte [112x112 QR] nebo` | box 57.8-146.2, all text 61.1, AFTER 147.3 | box 130.9-219.4, text 134.3, AFTER 147.1 | holds |
| B2b the same, text at the bottom of the box (padding-top 80, height 32) | text 121.1 (level with the box's own last line) | 134.3 | holds |
| B3 fixture-43 shape (label, 112 QR, amount; 16 px padding) | box 57.8-197.2, label 72.3, amount 171.3 = sentence 169.8 | box 73.1-212.6, 87.8, 186.8 = 185.3 | holds |
| B4 two badges (12 px / 16 px, 2 px padding, round) | box 57.8-77.2, Stav 60.3, Zaplaceno 61.8, AFTER 78.3 | box 63.0-82.1, Stav 65.3, Zaplaceno 66.8, AFTER 78.1 | holds; this probe 5.2 pt low, fixture 03 itself 2.0 pt (Chrome 72.3, main 74.3) |
| B5 `overflow: visible` / **`clip`** (60 px box, text at top) | text 61.1 (last line), AFTER 108.3 | text 95.3, AFTER 108.1 | **CONTRADICTS**: the plan treats `clip` as "not visible" (bottom edge); Chrome gives `clip` and `overflow-x: clip` the last line's baseline, like `visible` |
| B5 `overflow: hidden` / `auto` / `scroll` | text 97.1 (bottom edge), AFTER 112.1 (box bottom + strut descent) | text 95.3, AFTER 108.1 | holds (the F183b strut under the box) |
| B5 `hidden` / `clip`, auto height | hidden 67.1 / AFTER 82.1; clip 61.1 / AFTER 78.3 | 65.3 / 78.1 for both | same |
| B6 empty 48 px box, 3 px border | box 57.8-98.2, AFTER 103.1 (54 + strut descent 4.9 = 59 px) | AFTER 99.1 | holds (inline_img_strut shape without a border: block at 52) |
| B7 `inline-flex` badge, center | Stav 60.3, Zaplaceno 61.8 (first item) | 65.3 / 66.8 | holds: first baseline |
| B7 `inline-flex` items 40 px / 20 px, centered or column | Stav = the first item's first line | Stav = the last | holds |
| B7 `inline-flex; align-items: baseline` | Stav 70.1 = ITEMA's baseline, ITEMB level with it | | holds, but Blitz gives Taffy no item baselines: not a goal (section 6) |
| B7 `inline-grid` 2 columns | Stav = GRIDA = GRIDB (first row) | all 95.3 | holds |
| B7 `inline-table` of `display: table-row/-cell` spans | Stav = TROW1 (first row) | | holds |
| B7 real `<table style="display:inline-table">`, **default (middle) cells** | Stav 82.1, TROW1 70.8: Stav is level with the **bottom of the first cell** (a 40 px cell) | Stav 98.3 | **CONTRADICTS** step 5's "first row" without a condition |
| the same with `td { vertical-align: baseline }` | Stav 63.3 = TROW1 | | the first row's cell baseline |
| the same with `top` | Stav 82.1 (TROW1 63.3): again the cell bottom | | |
| the same, auto-height cells (20 px line) | Stav 67.1, TROW1 63.3: 3.8 pt (5 px) below the text, at the cell's bottom edge | | |
| B8 outer box, last block child holds an inline-block (INNER, 60x30) | X = INNER = 78.3, NEST1 61.1, AFTER 105.3 | X 92.3, NEST1 61.6 | holds |
| B8 last block child is empty (height 30), the one before has FIRST | X = FIRST 61.1 (the empty block has no line box) | 87.8 | holds: the walk skips a childless block |
| B8 FIRST block, text `BB`, block, text `DD` | X = DD, the last line box (106.1) | 110.3 | holds |
| B9 300 px box, 40 lines of 20 px | one page, L01 46.1 ... L40 631.1 (15 pt steps), AFTER 646.1, box 42.8-272.2 | 19 words: L23 at 6.8 pt, L01 to L22 on no page, AFTER 273.1 | holds: the line box reaches down to L40's baseline, 803 px |
| B9b 300 px box, 14 lines | L01 46.1, L14 241.1, AFTER 273.3 | 66.8 / 261.8 / 273.1 | holds |
| B10 `pg_t_N` and `pg_e_N` in a plain block, N 700 to 1000 step 12 | no warning; pages, 8 / 7 words | `pg_t_856` to `892`: "content extends 2.4, 11.4, 20.4, 29.4 pt past the bottom page edge"; pages and words equal Chrome's everywhere; `pg_e_892`: the row is on page 1, Chrome moves it to page 2 (F183b) | holds |
| B11 two `vertical-align: top` boxes (60 / 40 px), and one next to a baseline box | all texts at 61.1 | all at 95.3 | the guard holds (the texts are level in both) |
| B12 `margin-top: 10px` | box 65.2-114.8, text 68.6 (+7.5 pt), AFTER 115.8 | text 102.8 | holds; `margin-bottom: 10px`: text 61.1, AFTER 115.8; margin of the last child inside the box: text on the last line (61.1) and the box 37.9 px tall: not part of the baseline |
| B13 float or abspos as last child | X = LINE2 (the last in-flow line) 76.1; a float under it is inside the box (a BFC root); fixed 100 px box: X = LINE1 | 95.3 / 80.3 | holds |
| fixed 60, 112, 300, 600 px, text at the top | text 61.1, AFTER 108.3 / 147.3 / 288.3 / 513.3 | text 95.3 / 134.3 / 275.3 / 500.3 (and a "past the bottom page edge" warning for 600) | holds |
| contexts: block, grid item, table cell | text 61.1 (cell 61.8), AFTER 147.3 | text 134.3, AFTER 147.1 | holds; main's layout agrees with itself in all three |
| contexts: flex row / column item with a 112 px box | box 112 px tall, AFTER 147.3 / 162.3 | box 26 px tall, AFTER 78.1 / 93.1 | F217, not this PR |
| `<img>` in an `inline-block` (60 px) | text 94.8, AFTER 112.1 (the strut inside the box) | text 95.3, AFTER 108.1 | holds (F183b: `inside_atomic_inline`) |
| `<button><img 30></button>` (`img_in_button`) | button 57.8-78.0, text 61.8, AFTER 79.1; `inline_img_strut` shape: block at **34** | button 61.1-80.6, AFTER 78.1; block at 30 | the guard becomes 34 (the strut applies inside a button too) |
| `line-height: 1` / `30px`, 30 px font in the box | text at the box top in all (58.8, 64.8, 60.0) | 95.3 / 95.3 / 83.7 | holds |
| F213d Q1 (room 120, 300 px box, BOXTOP) and Q7 (inline-table), Q3 (15 text lines), Q4 (15 blocks) | page 2: BOXTOP 44.6, L01 44.6, L15 254.6, AFTER 270.3 | BOXTOP 258.8, L01 48.8, L15 258.8, AFTER 270.1 | holds (Q3 / Q4 are 4.2 pt low: the free space under the last line) |
| F213e `margin-top` 809 / 500 / 265 + 300 / 600 / 843 px alone | page 2: BOXTOP 44.6, AFTER 270.3 / 495.3 / 677.6 | BOXTOP 258.8 / 483.8 / 666.1, AFTER 270.1 / 495.1 / 677.3 | holds |
| fixture 45 | "Faktura" 43.3 (committed reference) | 49.1 | holds |
| fixture 48 (new), 15 rows | ZAPLACENO 100.1, Zaplaťte 675.3, QR 676.8, Děkujeme 778.0, box 675.0-758.2, one page | 109.6, 746.3, 747.8, 776.3, box 747.8-829.5 (30 pt into the margin), no warning | the fixture shows what F217 will expose |

The four things to change in the plan, all consequences of the table:

1. **`overflow: clip` keeps the last line's baseline** (like `visible`), not the bottom edge. Only `hidden`, `auto` and `scroll` (and `overflow-x`
   / `-y` set to one of them, which makes the other axis `auto`) give `None`. The step 3 walk has to read the computed `overflow-x` and `-y` and compare
   with `Visible` and `Clip`. The B5 test has Chrome's numbers; it is red until step 4 because its `hidden` rows need the strut of F183b.
2. **`inline-table` baseline is not "the first row" for a real table with the default `vertical-align: middle` cells**: Chrome puts it on the bottom of
   the first cell (the cell's `vertical-align` is not `baseline`). Only a cell with `vertical-align: baseline`, and `display: table-cell` boxes whose initial
   value is `baseline`, give their text baseline. Step 5 therefore covers `inline-flex`, `inline-grid` and the table made of `display: table-*` boxes, and the
   real `<table>` goes to a finding (filed in step 7), as the plan's step 5 allowed.
3. **`img_in_button` becomes 34** (the strut applies inside a button), the `inline_img_strut` guard is flipped to it.
4. The old Chrome numbers in the tests are 2.9 pt on the other font scale (above). The flipped and new tests use the numbers on ours.

### 7.2 Step 1: tests, fixture and thresholds

- `crates/htmltopdf/tests/inline_block_baseline.rs` (new): B1 to B10, B12, B13, the fixed shapes, grid / cell, B5 to B7 and the images, the F213d 600 px box, fixture 48.
  B11 is a guard and runs. The rest is `#[ignore = "F183c: red until step N ..."]` (step 3: B1 to B4, B8 to B10, B12, B13, fixed, contexts, 600 px box, fixture 48; step 4: B5, B6, images; step 5: B7).
- Flipped, ignored until step 4: `inline_img_strut.rs` `g3_empty_inline_block_leaves_the_strut_descent_below_it` (52), `v2_image_inside_an_inline_block_has_the_strut_under_it` (64 / 34),
  `v3_inline_block_taller_than_a_baseline_image_keeps_the_strut_descent` (104). The preflight half of `g3` runs.
- Added, ignored until step 3: `fixtures.rs` `letterhead_invoice_starts_at_chromes_y` (1.5 pt; the old 7.0 pt bound stays until step 3 removes it),
  `tall_inline_box_line.rs` and `alone_inline_block_line.rs` `f183c_the_y_of_the_moved_*` (the y they skipped). The warning assertion for the 600 px box left `page_bottom_warning.rs`
  (it asserted the bug) for `a_600px_box_moved_to_page_two_by_f213d_is_printed_whole`.
- Fixture `48-paid-stamp-header` with its Chrome reference and README; `compare.py` threshold 5.3 % (main: 4.25 %).

### 7.3 Step 2: Parley and Blitz, output-neutral

Fork commits (all pushed, append-only):

| Repo, branch | SHA | What |
|---|---|---|
| Blitz `htmltopdf/blitz-dom-0.2.4` | `d3f2a372` | `git revert 5305fa41` (F217's fix), ADR-0012 |
| Blitz | `6feeb2c7` | `git revert 4c3af6ae` (F217's red tests), ADR-0012. `git diff 3cea708f 6feeb2c7` is empty |
| Parley `htmltopdf/inline-box-strut` | `21d3c72e` (on `1f23226`) | `InlineBox::baseline: Option<f32>`; `finish` takes the strut path also without a strut for `Some(b)`: above = max(text and strut, `b`, flagged `None` heights), below = max(text and strut, `height - b`, 0); with `Some` the line is `baseline_above + below`, without it the old formula bit for bit; `GlyphRunIter::next` places the box at `baseline - b.unwrap_or(height)`. Seven unit tests in `test_strut.rs` |
| Blitz | `4488400c` | `whitespace.rs` `replay` passes `baseline: None`; the only change against main's lock `3cea708f` |

`Cargo.lock` (this branch): exactly four lines moved, `parley` and `fontique` to `21d3c72e`, `blitz-dom` and `blitz-html` to `4488400c`; no other crate;
`cargo metadata --locked` passes. fulgur is untouched (`09d4d25a`).

Gate (binary `htmltopdf-f183c-a` of this branch against the binary of main f550a8e, `RAYON_NUM_THREADS` 1 and 4): the 46 fixtures (01-45, 48) and the 7 `bench/corpus` documents are
byte-identical at both thread counts (53 PDFs, 106 comparisons; stderr differs only in paths and timings). Fixtures 03, 27, 35, 37, 43, 44, 45 and 48 are identical too, as expected before step 3.
Parley: `cargo test -p parley --lib` with `PARLEY_IGNORE_DECODING_ERRORS=1` 87 passed (the snapshot PNGs are LFS pointers here, so without the variable 53 snapshot tests fail on main as well).

### 7.4 Where to resume

Done: steps 0, 1, 2 (see 7.1 to 7.3). Not started: steps 3 to 7. The fulgur fork was not touched (the F107 engineer works there; step 3 needs the go).

Next, step 3 (Blitz and fulgur in one lock move): Blitz `layout/baseline.rs` and `ibox.baseline` in `compute_inline_layout`, `PerformLayout` for the boxes of a measuring pass, lib tests, **with the `overflow` rule of 7.1 (only `hidden`, `auto`, `scroll` give `None`; `clip` is like `visible`)**; fulgur `convert/inline_root.rs`: `computed_y = positioned.y - accumulated_line_top`, drop `baseline_shift` and the two `*_baseline_*_from_drawables` functions. Green then: `cargo test --release --locked -- --ignored` for the step 3 tests of 7.2, then remove the old assertions that the ignored tests replace (the 7.0 pt bound in `fixtures.rs`, the 91.3 pt comments), and lower the thresholds of 03, 27, 35, 37, 43, 44, 45, 48 in `compare.py`.
Blitz branch head to build on: `4488400c`; Parley `21d3c72e`.

Build notes: release only, `CARGO_TARGET_DIR=/home/user/html-to-pdf/target` is shared and every cargo run queued 30 to 90 minutes behind the others on its lock; a full `cargo build --release --workspace --all-targets` of this branch took 96 minutes including the wait. After a `--config patch...path` run, restore the `Cargo.lock` of that repo.
`cargo clippy --release --locked --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass (two `type_complexity` fixes in the new tests, one commit after the lock move). Not verified in step 2: the `bench.py --check` speed guard and `compare.py` for the whole set (step 6; fixture 48 alone gave 4.25 % on main).

### 7.5 Steps 3 to 6: what was done (2026-11-09)

Fork commits (all pushed, append-only). Parley is unchanged (`21d3c72e`).

| Repo | SHA | What |
|---|---|---|
| Blitz | `ecb900f6` | step 3: `layout/baseline.rs` (`atomic_inline_baseline`, `last_line_baseline`), `ibox.baseline`, boxes with content laid out with `PerformLayout` also in a measuring pass; 11 lib tests (the five of fulgur, the walk, a measured-at-another-width test that fails without `PerformLayout`) |
| fulgur | `f2bfc96f` | step 3: `baseline_shift` and the two `*_from_drawables` walks removed, `computed_y = positioned.y - accumulated_line_top`, stale comments |
| Blitz | `65bf3726` | step 4: `strut_box` flags every atomic inline with `vertical-align: baseline`, `inside_atomic_inline` removed |
| Blitz | `514d51e0` | `layout.margin` of an inline box is kept (Parley places the margin box, the painter needs the border box inside it; B12 was 10 px high without it) |
| Blitz | `2fd60f3f` | step 5: `first_line_baseline`; `inline-flex` / `inline-grid` give the first in-flow item (an item without a line gives its bottom edge), `inline-table` the first row; a table cell that is not `vertical-align: baseline` gives its bottom edge |
| fulgur | `8a4cd2c4` | `computed_y` adds the top margin (`unrounded_layout.margin.top`); lib test that `computed_y` is the position Blitz recorded |

Deviations from the plan, all decided on Chrome numbers of 7.1: `clip` is like `visible`; the **real `<table style="display:inline-table">` is fixed, not filed** (the cell-bottom rule is three lines; measured on Blitz: 43 px for a 40 px first cell with a 3 px border, 23 px for an auto cell, the text baseline for a `vertical-align: baseline` cell); `overflow` is tested on the box itself only (a nested `overflow: hidden` block keeps its line baseline, as in fulgur's old walk; not measured in Chrome).

Lock of this branch: `blitz-dom`, `blitz-html` to `2fd60f3f` and `fulgur` to `8a4cd2c4`, three lines of `Cargo.lock`.

Gates (binary `htmltopdf-f183c-c` of this head against main `0cbe50f`, binary `htmltopdf-main-f107`):

| Gate | Result |
|---|---|
| `cargo test --release --locked --no-fail-fast` | 1109 passed, 0 failed, 14 ignored (none of them F183c's; all 18 of `inline_block_baseline.rs`, the three `inline_img_strut` flips and the 5 y tests are on) |
| `cargo fmt --check`, `cargo clippy --release --locked --all-targets -- -D warnings` | pass |
| Byte gate, RAYON 1 and 4 | 47 of 54 PDFs identical to main at both counts (fixtures 01, 02, 04-26, 28-36, 38-42, 47, and the 7 corpus documents); 03, 27, 37, 43, 44, 45, 48 differ; **35 does not move** (its `::before` dot is not an atomic inline box); binary is deterministic across 1 and 4 threads |
| `render-fixtures.sh` + `compare.py --threshold-check` | all 47 ok; page counts unchanged (Chrome's) |
| `bench.py --check` | ok: b200-items 1230 ms, b200-text 749, b200-mixed 770, scaling r 4.27; fixtures 9 to 28 ms |
| Item table with a badge per row (b200-items with 4800 inline-block badges, 262 / 268 pages on main / new) | median of 5: main 1608 / 1647 ms, new 1779 / 1669 ms: within the noise, at most 10 % |
| Parley `cargo test -p parley --lib` | 87 passed |
| Blitz `cargo test -p blitz-dom --lib -- --test-threads=1` | 175 passed |
| fulgur `cargo test -p fulgur --no-fail-fast` | 3131 passed, 0 failed |
| fulgur-vrt | 36 of 64 differ with the pinned `FONTCONFIG_FILE`, against the 35 recorded for main; see `docs/upstream.md` |

Fixtures that moved, worst page, main to this head (threshold, set to the new number plus 1 point where the worst page improved): 03 4.7 to 2.4 % (3.5), 27 6.1 to 5.9 (7.0), 37 3.2 to 2.2 (3.2), 43 3.7 / 1.4 to 3.7 / 0.4 (unchanged 4.7: the worst page is page 1, untouched), 44 3.3 / 0.8 to 3.3 / 0.4 (unchanged 4.3), 45 0.0 / 9.2 to 0.0 / 5.3 (6.3), 48 4.3 to 3.6 (4.7). None grew. The PNGs of all seven were looked at: the stamp under the title, the badges level with their text, the QR frame on the line of its text, "Faktura" at the content top of page 2, nothing in the bottom margin.

Sweep against Chrome m153 (per case: pages, every word, y of the marker words, the box rows, stderr warnings):

| Set | Cases | improved / same / further than main | warnings main to new |
|---|---|---|---|
| `pb`: the step-0 probes (B1 to B13, fixed boxes, contexts, images, `pg_t_*`, `pg_e_*`, F213d / F213e shapes) | 132 | 131 / 1 / **0** | 8 to 0 |
| `sw`: context (p, grid item, table cell, flex column) x box (fixed 60, 112, 300, 600, auto, empty, `overflow: hidden`, `inline-flex`) x room (fits, box -8, -4, 0, +4, +8, none) | 224 | 203 / 18 / **3** | 39 to 0 |
| `ed`, `e2`: the room's edge in 1 px steps for an empty box, a clipped box, a box with text and an image | 42 + 50 | 76 / 11 / **5** | |

No case loses a word or gains a warning; the sum of |dy| to Chrome over the `sw` marks falls from 133402 to 75431, the cases within 0.75 pt from 0 to 179 of 224; page counts differ from Chrome in the same 30 `sw` cases as on main (flex-column items, F217).

**Open: the 1 px band.** The 8 cases that are further than main are one case: an **empty or clipped box (`overflow: hidden`) whose line fits the room in Chrome by exactly the strut's last pixel**. For a 52 px box Chrome keeps the row on page 1 from room 56 (box + 4), main from 52 (it has no strut under such a box), this head from 57 (box + 5). So for rooms 52 to 55 main is wrong and this head is right; for room 56 main is right and this head is wrong: the wrong band shrinks from 4 px to 1 px, but at that one room the case is further than main. The cause is not the box: Chrome keeps a line that overflows the room by less than 1 px (room 56.13 for a row of 57; measured: an `<img>` of 40 px needs room 44 in Chrome and 45 in main and here, a box with text 51 against 52 in both: the same 1 px on shapes that F183b did not touch), where our fragmenter wants the whole line inside the room. F183b makes the empty box's row include the strut, as Chrome's does (the row pitch X to AFTER is 15.0 pt in both), so the band moves from "no strut, 4 px early" to "the fragmenter's 1 px". Fixing it means giving the fragmenter that tolerance, which moves page breaks in every document and is another finding; it was not done here. Decision needed: accept the 1 px band (it is a strict improvement in the room count: 1 wrong room instead of 4, and on an invoice a shape that only occurs when a row ends within a pixel of the page end) and file the tolerance as its own finding, or hold F183c until the tolerance lands. The plan's pass condition ("none is further than main") is not met as written for this one case; see the decision below.

**Decision (2026-11-09): accept the 1 px band.** Reasons: (1) no word is lost and no warning is added in any of the 132 + 224 + 92 sweep cases; (2) the wrong band shrinks from 4 px on main (rooms 52 to 55 wrong) to 1 px (room 56 wrong); the sum of |dy| to Chrome over the grid falls from 133402 to 75431 and the cases within 0.75 pt go from 0 to 179 of 224; (3) the cause is the fragmenter's whole-line rule, not the baseline: the same 1 px shows on shapes F183c does not touch (a 40 px `<img>` needs room 44 in Chrome and 45 in ours, a box with text 51 against 52); (4) fixing it moves page breaks in every document, so it is its own finding, **F221**, with the numbers. Not held for it.

New edge cases found:

- **`margin-left` / `margin-right` of an inline box are lost in the paint (x); filed as F220 (2026-11-09).** Main and this head draw `<span style="display:inline-block;margin-left:50px">` at the margin edge, so the box sits 50 px left of where the line holds its space (the text after it is right). Invoice-like (`ml-2` on a badge): findings queue. The fix is the same line as the top margin in `convert/inline_root.rs` (`x_offset` plus `layout.margin.left`); not done here, it moves pixels of fixtures with such boxes and is another finding.
- Filed as B18 to B21 in `docs/backlog/stress-findings.md` (2026-11-09), with `align-items: baseline` (B21). Not measured in Chrome: a nested block with `overflow: hidden` as the last child of an inline-block (the walk keeps its line baseline); `overflow-x: clip` alone on an `inline-flex`; `inline-flex` with `overflow: hidden` (here: the bottom edge, as fulgur's old walk did).

### 7.6 Step 7: docs (2026-11-09)

`docs/upstream.md`: three fork rows and the deferred vrt re-bless note (7.5). Findings: F183c and F183b done; F183i filed (the original F183c text); F183c-b re-checked and done, the 40-line probe and the 15-line probe now print every line at Chrome's y (only the unbuilt "above the top page edge" warning is left, with no known shape); F220 (horizontal margin of an inline-block) and F221 (the fragmenter's whole-line rule) filed; B18 to B21 in the stress backlog. F213d-e's F183c part: the hang of a fixed box below its line is gone (the box sits on its last line), the page-count differences at the room's edge are F221's. `docs/roadmap.md`: F183c off the pilot list, F220 on it after F208. Not run: `vrt-validator`, `pdf-reviewer`.

### 7.7 Validator finding: two boxes on a line taller than a page lost 50 lines (2026-11-14)

**The loss.** `<p>` with two inline-blocks: A of 59 block lines (1182 px, taller than a page), B with a trailing `display:block; height:9px` child (probes `v2`, `v6`, `v8`, F213e `a4_two_ib_8_200`, `a4_two_ib_40_200`). The PR printed no word of A00 to A49 (not even the border) and warned "painted on no page"; main prints them on page 1; Chrome moves the whole line to page 2 and slices it there. Without the spacer in B, or with one in both, the PR was fine.

**Cause.** Not the baseline walk and not B's position (B sits where Parley puts it, `pos.y 1100`, in the PR and in Chrome's order). The line is taller than A: the line keeps `b` above and `height - b` below the baseline of every box (Parley `finish`), B's last line box is 9 px above its bottom, so B hangs 9 px (plus its border) below the baseline where A hangs 6; the line is 1183 px, A is 1182 px. Before F183c every box sat on its bottom edge and the line was as tall as the tallest box. The walk enters the carrier because the line is taller than a page, fragments A between its lines (1001 + 181 px) and closes the carrier's fragments at A's bottom: page 0 is 1182 px high. The renderer, with no line slices for the carrier, measures its one line from the fragment heights (`paragraph_line_range_for_page`): a line of 1183 px does not fit a fragment of 1182 px, so it is painted on no page, and A with it. The `+1 px` is the whole bug; every shape in which a neighbour hangs deeper than the tall box by anything (the probes: 1 px to 14 px) hits it. Checked by printing the Parley line (`min 0 max 1183`) and the fragments (`p0 1182`).

**Fix** (fulgur `caa5e87c`, `pagination_layout.rs`): `record_carrier_slices_after` records the carrier's line slices (the line on the first fragment of the box, none on the others) also for a box that shares the line and spans pages, as it did only for a box alone on its line. The line is then painted once on the page where the box starts, and A's later fragments are painted as continuations, as for an alone box. The output is main's: A00 to A49 on page 1, A50 to A58 on page 2, B41 on, AFTER (B00 to B40 stay lost, as on main, warned: the line paints B below page 1; Chrome is the whole line on page 2, which is the taller-than-a-page line of F213b / F198l, not cheap). Not done: moving a shared line taller than a page whole to the next page (that is F213b's slicing).

**Tests (red then green).** `alone_inline_block_line.rs`: `r_f183c_two_boxes_on_a_line_taller_than_a_page_keep_the_lines_of_the_first` (red before: A00 to A49 missing, 10 words printed; green: every A line and AFTER once, A00 on page 1, A58 on page 2) and the guard `g_f183c_two_boxes_on_a_line_taller_than_a_page_without_a_trailing_child`; fulgur lib test `the_carrier_line_of_two_boxes_is_on_the_page_where_the_taller_one_starts` (red before: no slices; green: `(0, 1), (1, 1), ...`).

**Gates after the fix.**

| Gate | Result |
|---|---|
| `cargo test --release --locked --no-fail-fast` | 1111 passed, 0 failed, 14 ignored (was 1109) |
| fulgur `cargo test -p fulgur --no-fail-fast` | 3132 passed, 0 failed (was 3131) |
| `cargo fmt --check`, `cargo clippy --release --locked --all-targets -D warnings`, fulgur clippy `--all-targets -D warnings` | pass |
| Byte gate, RAYON 1 and 4 | 54 PDFs: the same 7 as main differ (03, 27, 37, 43, 44, 45, 48), byte-identical to the PR before this fix; 1 and 4 threads identical |
| `compare.py --threshold-check` | 47 ok, numbers unchanged (03 2.4 %, 27 5.9, 37 2.2, 43 3.7 / 0.4, 44 3.3 / 0.4, 45 0.0 / 5.3, 48 3.6) |
| Validator's F213d family (3843 cases, `p/dv`), main against the fix | 3333 changed, 510 identical; 11 with fewer words than main, as before the fix (see below); `DIST+` / `PAGES+` 1 each: `adv_a5_10_ib_negmargin_auto_Rpage2_d-6`, B22 |
| Validator's F213e family (2901 cases, `p/ev`) | 702 changed, 2199 identical; **0 with fewer words than main** (was 2: `a4_two_ib_40_200`, `a4_two_ib_8_200`, now main's 74 and 78 words) |

The 11 `FEWERWORDS` of the F213d family are the validator's: the 10 ghost-word cases (`adv_a4_m0_ib_negmargin_fixed_*`: main prints a word twice, with a warning, we print it once) and `adv_a4_m0_ib_negmargin_auto_Rpage2_d-6`, which counts 56 words on main and 55 here only in the validator's wide read (the MediaBox widened downward): `F00` is at y = -0.06 pt (the top of its bbox 0.06 pt above the page edge, the glyphs inside; Chrome's own F00 bbox starts at -3.3 pt for its font), where main has it at 5.9 pt; `pdftotext` on the real page prints it in both (55 lines each). The `margin: -12px` position is B22's family; no word is lost.

Not verified: the PNGs were not looked at again (the byte gate shows the 54 PDFs equal the ones looked at before this fix); the full F213d / F213e sweep against Chrome is the validator's, here only against main.
