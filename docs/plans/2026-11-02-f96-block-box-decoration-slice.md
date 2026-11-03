# F96: a block box split across pages is sliced at its breaks (`box-decoration-break: slice`)

Date: 2026-11-02. Finding: F96 in `2026-10-15-m0-findings.md` (low; queued right after F30c-o3 since F30c-n). Fork: `subapavel/fulgur`
`htmltopdf/srgb-colours` (head `d5b4a994`), carried via `[patch.crates-io]`. **The line numbers below were read from the fork's
working tree, not with `git show d5b4a994:`.** Another engineer was editing that tree for F215, so re-read every function at
`d5b4a994` before editing. F215 also edits `fragment_block_subtree`. Rebase on it if it merges first, and add fixture 41 to the gates.

## 1. Problem

A block box with a border, padding, background or shadow is split across pages because its children split. When that
happens, the renderer closes its frame on every page. Page 1 gets a bottom border and rounded bottom corners under the last
line. A continuation page gets a top border and rounded top corners over its first line. Chrome slices the box instead
(css-break-3 §5.4, the initial value): the top edge is on the first fragment only, the bottom edge on the last fragment only,
and the side borders and background run to the page's content bottom.

Fixture 06 shows this today (`out/compare/06-long-notes-p2.png`): the `rounded-lg border p-4` card on page 2 starts with a
top border and radius, where Chrome has open sides. Fixtures 08 (a one-column `grid` card) and 16 (an `overflow-hidden`
card; README and `compare.py` name F96) have the same defect. F30c-n made it common: any bordered card that breaks inside its
text now shows it (`f30cnv/p1/sec_deep3`).

F30f-d already slices a padded paragraph, i.e. an inline root. The renderer pieces it built are reused here:
`frame_fragment` edges, `sliced_style`, `open_rounded_border_segments` and `draw_sliced_box_shadow`.

**Does content on a continuation page start at the content top?** For block boxes it already does. Read from the code:

- `fragment_block_subtree`'s page advances set `page_start_y = 0`, `cursor_y = band (= top_floor)` and `child_page_y = band`
  (pagination_layout.rs ~5957-5967 and ~6240-6252).
- `fragment_nested_inline_root` places continuation strips at `top_floor` (~8351).

Measured: F30f-d step 1, row "`<div>` with a `<p>` inside", put our page-2 lines at 42.1 pt against Chrome's 42.3 pt. The
06 PNG shows the same thing.

So for block boxes, no top inset is reserved, and slicing does not give a continuation page any extra room. Inline roots
reserved it (fixed in F30f-d), and table cells still do (F96d). The block-box defects are these three:

- (a) closed edges at the breaks;
- (b) a non-last fragment that ends at its last child (when a child moved whole) or at `fresh()` on page 0 (block recursion
  without a line split), instead of at the content bottom;
- (c) the bottom padding and border of a box that split only between whole children. `closing` is computed only when a
  paragraph below was split (`lines_split_below`, ~6447: "other splits keep closing at the last child"). The last frame then
  ends at its last child, and what follows starts the bottom inset too high.

(c) is the only layout change in this plan.

## 2. Spec (css-spec-researcher not run; section numbers cited by the planner, unverified)

- css-break-3 §5.4 `box-decoration-break`, initial value `slice`: "the element is rendered as if its box were not fragmented,
  and then the rendering for this hypothetical box is sliced". At a break there is no border or padding. Backgrounds and
  `box-shadow` are drawn for the unbroken box and sliced. It applies to all elements.
- css-break-3 §5.3 (Splitting Boxes): "when a box breaks, its content box extends to fill any remaining fragmentainer
  extent". So a non-last fragment runs to the page's content bottom even when its content ends higher.
- css-break-3 §5.2: margins adjoining an unforced break are truncated. This is already implemented and unchanged.
- css-backgrounds-3 (corner shaping, as cited in F30f-d §7): radii apply at unbroken corners only.
- css-overflow-3 §3: a fragmented clip box clips each fragment at that fragment's padding box (16's card).
- **`clone` cannot be honoured.**
  - Stylo 0.8.0 `properties/longhands/border.mako.rs:85-93` defines `box-decoration-break` with `engines="gecko"`, so the
    servo build used by Blitz never parses it (verified).
  - The compiled `fixtures/tailwind/tailwind.css` has no `box-decoration-*` rule (grep: 0).
  - `slice` is the only value we can see.

## 3. Chrome behaviour

**Measured (F30f-d step 1, Chrome of that time, A4, 15 mm, Inter):**

- On a sliced padded paragraph, page 1's side borders run to the content-box bottom (row 1065 of 1065.8 at 150 dpi), with no
  bottom border or corner. Page 2 has no top border or radius.
- A `td` is sliced the same way: page-2 lines at 42.3 pt, no top border.
- A nested `<div>` + `<p>` has its page-2 lines at 42.3 pt.
- A box with padding only and a background: 769 / 731 words per page.

**Given by the coordinator, not re-measured:** Chrome (the version of the committed references, Skia/PDF m153) squares the corners at a cut.

**Inferred, measured in step 1:**

- a block box (not an inline root) is sliced the same way;
- after a child moved whole, the frame and background of a non-last fragment still fill to the content bottom;
- nested boxes are each sliced;
- a one-column grid card is sliced;
- the header strip of an auto-height clip card appears on page 1 only;
- the shadow stops at the cut edges;
- what follows a box split only between children starts below its bottom padding, border and the last child's margin.

**Probes for step 1** (`out/scratch/f96/`, git-ignored):

- Chrome PDFs come from the Playwright renderer of `scripts/reference/render.mjs`. Record `browser.version()`.
- Words and y positions come from `pdftotext -bbox-layout`. 150 dpi crops of page 1's bottom and page 2's top come from
  `pdftoppm`.
- The 700 px spacer is there so the break falls inside the box.

| Probe | Shape | Record |
|---|---|---|
| P1 | 06's card: `section.border.p-4.rounded-lg` > `h2` + one long `<p>` | page-2 first line y; borders and radius at the break; where the sides end |
| P2 | P1 with 40 `<p class="mb-2 break-inside-avoid">` clauses (split between children) + `AFTER` | as P1; page-1 frame bottom when the last clause ends high; `AFTER` y |
| P3 | P2 with `bg-gray-50` and `box-shadow: 0 4px 8px #0004` | background reaches the content bottom; no shadow beyond a cut |
| P4 | P2 with a nested `rounded-md border bg-white p-4` box across the break | both boxes open; inner and outer sides to the bottom |
| P5 | 08's `grid gap-4` card; a two-item `flex` row with a bordered item beside a tall `<p>` | grid sliced; row: record only (F96e) |
| P6 | 16's `overflow-hidden` card with a header strip | strip on page 1 only; clip at each fragment's padding box |
| P7 | P2 whose first clause does not fit below the card's top padding | does Chrome move the whole card (no empty first fragment)? |
| P8 | `box-decoration-break: clone` on P2 | record only (to know what we diverge from) |
| P9 | `td.border.py-4`, abspos bordered box, `height:3000px` bordered box, `inline-block` card | record only, for the filed items |

Run the same probes with our current binary. They confirm that page 2 starts at the content top, and defect (c) on P2.

**Gate:** continue only if Chrome, for P1, P2 and P4, opens the edges at the break, runs the sides and background to the
content bottom, and puts the bottom inset on the last fragment only.

## 4. Where it belongs and design

**Location.** The fix goes in the fulgur fork only, carried as above. Taffy lays the unbroken box out correctly; fragmenting
and painting are fulgur's. There are no upstream PRs or issues (standing decision), and nothing in Blitz, Taffy or Parley
changes.

**`pagination_layout.rs`**

- `PaginationGeometry` gets `pub block_slice: bool`. It is set only for a block box that this plan slices. It stays `false`
  everywhere else, so no other output moves.

- The eligibility predicate:

  ```rust
  /// F96: whether block box `node`, fragmented by `fragment_block_subtree`, is painted sliced at its page breaks.
  fn block_slices_at_breaks(doc: &BaseDocument, node: &blitz_dom::Node, closes_like_block: bool) -> bool;
  ```

  True when **all** of these hold:
  - The box is an element, and neither `<body>` (F37's `box_slices`) nor `<html>`.
  - It has something to slice: a border width, vertical padding, a background or a shadow. A box without a frame, like 39's
    section, keeps its bytes.
  - It is not an inline root (F30f-d's `line_split_slice` covers those). A debug-assert checks that `block_slice` and
    `line_split_insets` are never both set.
  - Its inner display is `Flow` or `FlowRoot`; or it is a `Flex` or `Grid` container with `closes_like_block` (items stacked,
    like 08).
  - It is not a table part or cell (`is_cell_of_table`), not a multicol container, and not an atomic inline container
    (ADR-0004).
  - It is not `block_size_constrained`. That excludes `height`, `min-height` and `max-height`, and with them F30f-i's clip
    boxes.
  - It is not a float, not transformed, and not `position: absolute` or `fixed`.
  - No ancestor up to `<body>` is absolutely positioned (the `abspos_flow.rs:959` entry) or a multicol container (F198's
    `piece_edges`).
  - The ancestor walk runs only for a box that actually crossed a page.

- The frame post-pass:

  ```rust
  /// F96: every fragment of a sliced box pushed since `first`, except the last, runs to the page's content bottom
  /// (`strips.chrome_bottom`, as F30f-d's sliced paragraphs, so nested sliced frames end on the same row).
  fn slice_block_frames(entry: &mut PaginationGeometry, first: usize, strips: PageStrips);
  ```

  It sets `height = (chrome_bottom(page) - y).max(0)`, replacing both the short frame of a split where a child moved whole
  and the `fresh()`-tall page-0 frame of the oc51 push.

- `fragment_block_subtree`:
  - At entry, record `own_first = geometry.get(&parent_id).map_or(0, |g| g.fragments.len())`.
  - Before `closing` (~6447), compute
    `sliced = page_index > page_in && block_slices_at_breaks(doc, parent, closes_like_block)`.
  - `closing` applies when `(lines_split_below || sliced) && closes_like_block`. This is defect (c). The existing clamp to
    the room on the page stays.
  - After the final push, if `sliced`, call `slice_block_frames(entry, own_first, strips)` and set `entry.block_slice = true`.
  - The returned `lines_split_below` is unchanged, so the oc51 push and `stretch` are untouched for other callers.

**`render.rs` `frame_fragment`.** After the `piece_edges` and `line_split_insets` branches, add:

```rust
if geom.block_slice && geom.is_split() { edges = FrameEdges { top_open: !is_first, bottom_open: !is_last } }
```

`is_first` and `is_last` are judged by page index, as today. The frame size is unchanged, because a block's fragments
already are its frame.

- Every block paint path already reads `frame_fragment` and `sliced_style`: the generic dispatch (~1756, `<li>` included),
  `draw_under_clip` (~2655, so the clip is each fragment's padding box), `draw_under_opacity` (~3175), and the transform path
  through dispatch.
- The radius, shadow and border segments are F30f-d's and need no change.

**Determinism.** The change is plain f32 arithmetic on existing values, with no new maps or iteration.

## 5. Acceptance: fixture and tests (red first)

**New fixture `42-terms-card-clauses`.**

- Content: the header, party grid, items table and total of 06. Then
  `<section class="rounded-lg border border-gray-300 bg-gray-50 p-4 mb-6 text-sm">` holding:
  - an `h2`;
  - about 30 `<p class="mb-2 break-inside-avoid">` clauses of 2-3 lines;
  - a nested `<div class="rounded-md border border-gray-200 bg-white p-4 mb-2">` ("Reklamace a odpovědnost", 6 clauses).

  Then 16's "Platební údaje" block and signature line.
- Tune the clause count until Chrome's page break falls between two clauses inside the nested box, and the card ends
  mid-page 2.
- Every class is in the committed `tailwind.css` (checked: `rounded-lg`, `rounded-md`, `border`, `border-gray-200` / `300`,
  `bg-gray-50`, `bg-white`, `p-4`, `mb-2`, `mb-6`, `break-inside-avoid`, `text-sm`, `text-xs`, `font-semibold`, `uppercase`,
  `tracking-wider`, `text-gray-500`, `mt-8`, `pt-2`, `border-t`, `w-72`, `pl-4`; `.rounded`, `.p-3`, `.shadow-sm` and
  `.space-y-*` are absent, so they are not used). No rebuild.
- Commit the reference via `render.mjs`.
- The README names css-break-3 §5.3 / §5.4.

**What 42's PNG must show.**

- Page 1: both boxes have their top border and rounded top corners; the grey and white backgrounds and all four side borders
  run to the content bottom; there is no bottom border; the corners at the cut are square.
- Page 2: no top border on either box; side borders from the content top; the first clause line at the content top
  (about 42.5 pt); the inner box closes with its bottom border and radius, then the outer clauses follow; the outer box
  closes the same way; the payment block is at Chrome's y (within 1.5 pt).
- Before the fix (expected red): frames closed at the break on both pages, backgrounds stopping at the last clause, and the
  payment block about 17 px too high (defect (c), if step 1 confirms it).

**Ours: new file `crates/htmltopdf/tests/block_box_slice.rs`** (96 dpi raster and text-layer helpers from
`padded_paragraph_split.rs`):

1. P1 shape: no border-coloured row across the card at page 2's content top; the side columns are present at page 1's last
   content row; no horizontal border line under page 1's last line.
2. P3: the background colour is present 3 px above page 1's content bottom inside the card; no shadow ink below page 1's
   content bottom or above page 2's content top.
3. Corners: square at the cut (the pixel inside the corner has the border colour); at the unbroken top corners, the pixel
   outside the radius is white.
4. P2: `AFTER` within 1.5 pt of Chrome's (defect (c)).
5. P4: nested, both boxes open at the break.
6. **Paint only for line-split cards:** P1, 06, 08 and 16 have the same words per page and the same word positions
   (`pdftotext -bbox`, ±0.01 pt) as before the change. This is pinned with the current numbers in the red commit.
7. Guards that pin today's closed frame (each becomes red when its filed item is fixed): `td.border.py-4` (F96d),
   `height:3000px` (F96b), abspos card (F96c), two-item flex row (F96e).

**`fixtures.rs`:** `terms_card_clauses_invoice_slices_both_cards`: 2 pages; every clause once and in order; the two
bottom-border rows appear on page 2 only and the two top-border rows on page 1 only.

**Fork tests.**

- `pagination_layout` lib tests:
  - `a_bordered_block_split_between_its_children_is_sliced`: `block_slice`; fragment 0 runs to `chrome_bottom(0)`; the
    fragment on page 1 is at y 0; the first child there is at y 0 (a guard, green today); the last fragment includes the
    bottom inset.
  - `a_bordered_block_around_a_split_paragraph_is_sliced`.
  - `nested_bordered_blocks_are_both_sliced`.
  - `a_one_column_grid_card_is_sliced`.
  - `boxes_outside_f96_keep_their_frames`: cell, inline-block, abspos box and its bordered child, multicol and a card in
    it, `height`, `min-height`, clip box with a block size, float, transform, body, frameless box. Each has
    `block_slice == false` and the old fragments.
  - `what_follows_a_box_split_between_children_starts_below_its_bottom_inset`.
- `render.rs`: `frame_fragment_of_a_sliced_block_opens_the_broken_edges` (first / middle / last).
- `tests/sliced_box_decoration.rs`: border curves for a 3-page sliced block card; shadow rows in the margins are white.
- `render_smoke.rs`: one case (coverage rule).

## 6. Byte and pixel gates

**Byte gate** (`f183-gate.sh`, 1 and 4 threads, identical to each other). Baseline: main's binary at `ed60ea2` on fork
`d5b4a994`. Fixtures 01-39 and 42 (plus 40 and 41 if merged), and the 7 corpus documents.

| Expected | What and why |
|---|---|
| Changes, paint only | 06, 08, 16 (text layer identical, test 6) |
| Changes | 42 (new) |
| Changes, page count must stay 198 | `b200-mixed` (16's card ×181; its `padding-bottom:2rem` wrapper may gain `closing`) |
| Expected identical; if it changes, explain it before going on | 15 (`<main style="position:relative;padding-bottom:2.5rem">` paints nothing, but gains `closing` and may move the abspos stamp's containing block) |
| Identical | everything else: 29 (inline root, F30f-d); 30, 31, 12, 24, 25 (multicol); 39 (no frame); `b200-text`, `b200-items`, the `slow-*` documents |

Any other difference is a regression. No page count may change.

**`compare.py`.**

- Expected: a small decrease in 06 (15.9 / 11.8 %), 08 (13.2 / 12.1 %) and 16 (9.5 / 12.9 %). Removing a 1 px border row
  and two radii is a fraction of a point.
- Lower each threshold in the same PR to the new worst page + 1 point.
- Replace the F96 sentences in their `compare.py` comments and READMEs.
- 42: threshold = worst page + 1 point, with a comment.
- No growth is allowed: nothing missing becomes rendered here.
- Look at the side-by-sides of 06, 08, 16 and 42.

**`fulgur-vrt`** (`f183-vrt.sh`; the host's 35 known environment failures are the baseline).

- Expected to move: split bordered or background boxes (`examples/break-inside`'s callout; mo-006 / mo-008, whose page-0
  frame ran into the margin to `fresh()`).
- Re-bless only goldens whose diff is a sliced frame, after looking at the diff PNG. List them in §10.

**Other gates.**

- `bench.py --check`.
- Fork: lib, all integration binaries, `clippy -D warnings`, `fmt --check`.
- Ours: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --release --locked`.

## 7. Steps (each ≤ 1 day)

1. **Measure** (½ day). P1-P9 in Chrome and in ours (§3).
   - Gate as in §3.
   - Stop and write a postmortem if Chrome does not open the edges for block boxes.
   - Drop defect (c) and test 4, and file them, if ours already has the bottom inset on P2.
   - If P7 shows Chrome moving the whole card where we leave an empty first fragment, file it (start rule) and keep the
     slice.
2. **Red** (1 day). `block_box_slice.rs`, fixture 42 with its reference, the `fixtures.rs` test, and pinned test 6. Commit
   with the red list; the guards are green.
3. **Fork fragmenter** (1 day). The field, `block_slices_at_breaks`, `slice_block_frames`, `closing`, lib tests.
4. **Fork renderer** (½ day). The `frame_fragment` branch, the render and integration tests, `render_smoke`, `fulgur-vrt`.
5. **Ours green** (½-1 day). Bump `Cargo.lock`; byte gate; `render-fixtures.sh` and `compare.py`; look at the PNGs; lower the
   thresholds; update the READMEs of 06, 08 and 16. ADR-0004 stays as written (inline-blocks are out of scope).
6. **Finish.**
   - `vrt-validator`.
   - `pdf-reviewer`: frame painting is the writer path (merge policy 5).
   - Findings: mark F96 done and file F96b-F96h.
   - `docs/upstream.md`; this plan's §10.

## 8. Risks

- **`closing` is a layout change.** What follows a box split between children moves down by its bottom inset, which can push
  a page break. The byte gate and the no-page-count-change rule catch it. `b200-mixed` and 15 are the documents to watch.
- **Abspos children of a sliced relative box.** They may read its fragment heights for `bottom:` or `%` insets. The ours and
  fork abspos tests must stay byte-identical, or the change must be explained.
- **`chrome_bottom` paints up to 0.87 px into the bottom margin.** This is the same as F30f-d and keeps nested sliced frames
  on one row.
- **An empty first fragment** (only the top inset, because the first child moved) now runs as an open frame to the page
  bottom (P7).
- **Merge with F215.** The `fragment_block_subtree` edits are at its start and end only.

## 9. Out of scope (filed in the findings file with this PR)

- **F96b:** boxes with their own block size (`height`, `min-height`, `max-height`). This includes the F30f validator's
  `height:3000px` wrapper (4 pages against Chrome's 3). Clip boxes with a block size stay with F30f-i.
- **F96c:** absolutely positioned boxes (F30g LOW 7), and in-flow boxes inside them (a separate fragmenter entry, with F94 /
  F95 open).
- **F96d:** table cells and tables. Chrome slices a `td` (measured); our cell reserves its top inset on page 2 (lines at
  54.8 pt against 42.3), so this is a layout change as well.
- **F96e:** flex and grid rows with more than one item (F30h's stretched siblings).
- **F96f:** background images and gradients are positioned per fragment instead of over the unbroken box. This also applies
  to F30f-d's paragraphs.
- **F96g:** transformed split boxes. Floats are not split today (F30c-f).
- **F96h:** a multicol container's own frame across pages. Boxes inside columns stay with F198 (`piece_edges`).
- **`box-decoration-break: clone`:** recorded only. Stylo parses it for gecko only; supporting it needs a Stylo fork, which is
  not in the standing fork list, so it would be a new ADR if a product ever needs it.
- **Inline-blocks:** monolithic; ADR-0004 stands, including its "closed frame" consequence.

## 10. Evidence

(Filled in by steps 1-6.)
