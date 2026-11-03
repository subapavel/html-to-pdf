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

### Step 1: Chrome and ours (2026-11-03)

Chrome: Google Chrome for Testing 153.0.8010.12 (`Skia/PDF m153`, the version of the committed references), Playwright, A4,
15 mm. "Ours" is main's binary at `ed60ea2` (fork `d5b4a994`). Probes P1-P9 were run on Tailwind markup (`font-sans`, `text-sm`,
`rounded-lg border p-4`); the shapes of `block_box_slice.rs` are plain CSS (`font:14px/1.2 Inter`), run through Chrome with the
Inter TTF of `fixtures/fonts` as `@font-face` (without it Chrome embeds the system Inter as Type 3 and `pdftotext` reports its
lines 2.7 pt higher, which is an artefact of the font descriptor, not of the layout). Rows are 96 dpi raster rows (1 px = 1 CSS
px; content box rows 57 to 1065). Probe sources are not committed; the shapes of the tests are, in `block_box_slice.rs`.

**Gate (section 3): passed.** For P1, P2 and P4 (and P3, P5a, P6) Chrome opens the edges at the break, runs the sides and the
background to the content bottom and puts the bottom inset on the last fragment only. Chrome squares the corners at a cut
(`(57, 1065)` of page 1 and `(57, 57)` of page 2 are border colour); the unbroken corners keep their radius.

| Probe | Chrome | Ours (before) |
|---|---|---|
| P1 (06's card, one paragraph) | p1: top rule 757, sides 762..1065, no bottom rule. p2: sides 57..528, no top rule, bottom rule 533. First line of p2 at 43.8 pt | closed: p1 rules 757 + 1065 (sides 761..1061); p2 rules 57 + 533 (sides 61..529). First line of p2 at 43.6 pt, so page 2 starts at the content top as the plan says (the 0.2 pt difference is font drift) |
| P2 (20 clauses, split between children; the test shape has 14) | p1 sides to 1065 although the last clause ends at 1032 (rule 757 only). p2 (3-page probe) sides 57..1066, no rules. Last page: sides 57..380, bottom rule 385. `AFTER` at 308.3 pt | p1 closed at 1041 (sides to 1037, rules 757 + 1041); p2 closed (rules 57 + 1060); last page bottom rule 360. `AFTER` at 289.3 pt: **19.0 pt = 25.3 px too high** |
| P3 (P2 with `bg-gray-50` and `box-shadow`) | background to 1066 on p1 and from 57 on p2; no shadow ink in the margins; the shadow only under the last fragment (385..392) | background ends 1039 on p1; the frame closes with its shadow at 1040-1047; 3 rows of shadow ink in p2's bottom margin (1067..1069) |
| P4 (nested box across the break) | p1: rules 757 (outer top) and 954 (inner top), both sides to 1065. p2: rules 541 (inner bottom), 878 (outer bottom). `AFTER` 678.1 pt | p1: rules 757, 954 and 1062 (inner closed), outer side runs to **1122** (page bottom, 56 rows into the margin: the `fresh()`-tall page-0 frame); p2: rules 57, 516, 819. `AFTER` 634.3 pt (**43.7 pt = 58 px too high**: the inner box and the card both lose their bottom inset) |
| P5a (`grid gap-4` card, paragraph taller than a page, 4 pages) | p1 open at the bottom, p2 and p3 open at both ends (sides 57..1066), p4 bottom rule 113 | closed on every page (rules 757 + 1065, 57 + 1065, 57 + 1065, 57 + 133) |
| P5a with a paragraph shorter than a page | splits the grid item (2 pages) | moves the paragraph to page 2 whole, p1 holds the heading only (`AFTER` 601 pt against 434): a grid item is split only when taller than a page (F30b). **New, not F96**: see below |
| P5b (flex row: bordered item beside a tall `<p>`) | the bordered item is sliced over 4 pages (sides on every page) | the bordered item is painted on page 1 only (side 761..1122, into the margin) and not on pages 2-4. Text matches Chrome (`AFTER` 343.3 against 343.6). F96e, record only |
| P6 (`overflow-hidden` card with a header strip) | strip on p1 only (rules 757, 790, bg 758..789); p1 sides to 1065; p2 sides 57..1066; last page bottom rule 385 | strip on p1 only; p1 outer side runs to **1122**; p2 closed (rules 57, 1065). The clip at each fragment's padding box was not measured separately |
| P7 (first clause does not fit under the top padding) | moves the whole card to page 2 (page 1 holds the spacer only; p2 top rule 57) | the same (first line at 56.3 pt against 56.6). With an `h2` before the first clause both leave the heading alone on page 1. **No start-rule finding** |
| P8 (`box-decoration-break: clone`) | honoured by m153: p1 closed (rule 1066), p2 top rule at 57, the first line of p2 at 56.6 pt (top inset reserved) | slice-like layout, closed frames (first line of p2 at 43.6 pt). Record only |
| P9 `td.border.py-4` (paragraph < page) | sliced: p1 rule 757 and sides to 1066; p2 sides 57..437, bottom rule 437, first line at 43.8 pt | moves the row to page 2 whole (page 1 empty) |
| P9 `td` with 1500 words (guard shape) | 3 pages: p1 rule 757, p2 open, p3 bottom rule 644 | p1 sides 757..1060, p2 open (57..1064), p3 bottom rule 677; the first line of p2 and p3 at 54.1 pt (**the top inset is reserved: F96d confirmed**, text 33 px lower by page 3) |
| P9 abspos card | sliced; first line of p2 at 43.8 pt | closed frame, first line of p2 at 56.3 pt (reserved inset). F96c |
| P9 `height:1500px` box | sliced over 3 pages, `AFTER` 178.6 pt on p3 | moves the box to page 2 whole, `AFTER` 411.5 pt on p3. F96b |
| P9 `height:3000px` box (guard shape, after a 300 px spacer) | 4 pages | 5 pages, closed on pages 2-4 |
| P9 `inline-block` card | moves the card to page 2 whole (monolithic) | splits it across two pages with a closed frame (ADR-0004); `AFTER` 505 pt against 720 |

**Where the plan's numbers were wrong or missing**

- Defect (c) is **25.3 px** (19.0 pt) on P2, not 17 px: the last clause's 8 px margin, the 16 px padding and the 1 px border.
  With a nested box (P4, fixture 42) both boxes lose it: 43.7 pt in 42, 58 px in P4.
- Defect (c) **reproduces**, so it stays in scope.
- The page-0 frame of a box whose nested box splits (P4, P6, 42, and the flex item of P5b) runs to the page's bottom edge (row
  1122, 56 rows into the bottom margin), not 0.87 px past the content bottom. The plan's `slice_block_frames` replaces it.
- The "before" compare numbers of section 6 are stale: on main 06 is 10.7 / 11.8 % (the plan says 15.9 / 11.8), 08 is 8.2 / 12.1
  (13.2 / 12.1), 16 is 9.5 / 12.9 (as planned). The thresholds are 0.129, 0.132 and 0.139 and the worst pages did not move, so
  "worst page + 1 point" will still mean the page-2 numbers.
- Fixtures 40 and 41 do not exist on this branch, and fork `7a79be15` (F215) is not in `d5b4a994`.
- New divergence, **not F96**: a grid item shorter than a page that straddles the break moves to the next page whole where
  Chrome splits it (P5a, short paragraph). F30b covers only items taller than a page. File it with the F96b-F96h batch.
- Our `td` is already open at the breaks (no rules on page 2); what differs is the reserved top inset on page 2 (F96d), as the
  plan says.

### Step 2: red tests (2026-11-03)

Fixture 42 `42-terms-card-clauses` (21 clauses: 1-8 in the card, 9-14 in the nested box, 15-21 in the card; 2 pages; clause 9
ends page 1, clause 10 heads page 2 at 43.8 pt; signature line at 693.3 pt on page 2). Reference committed with
`scripts/reference/render.mjs` (Chrome m153). **Before the fix, compare: 5.7 / 12.1 % (worst 12.1 %).** No threshold is set yet
(`compare.py` defaults to 10 %, so CI's `--threshold-check` is red on this commit); it is set after the fix. The payment block
stands 43.7 pt too high (649.6 against 693.3).

Run with `cargo test --release --locked --offline -p htmltopdf --test block_box_slice --test fixtures` on main's sources:

| Test | State | Why |
|---|---|---|
| `card_around_a_split_paragraph_is_open_at_the_break` (P1, test 1) | red | p1 has 2 rule groups (top and a bottom border) |
| `background_runs_to_the_cut_and_the_shadow_stops_there` (P3, test 2) | red | the background is gone 3 px above the content bottom (grey 235). The shadow assertions pass today (the closed frame keeps its shadow inside the page) and guard the fix |
| `corners_are_square_at_the_cut_and_round_elsewhere` (test 3) | red | the bottom corners of p1 are round. The unbroken-corner assertions pass today |
| `what_follows_a_box_split_between_children_starts_below_its_bottom_inset` (P2, test 4) | red | `AFTER` at 448.6 pt, Chrome 467.6 |
| `nested_boxes_are_both_open_at_the_break` (P4, test 5) | red | p1 has 3 rule groups |
| `text_layer_of_line_split_cards_does_not_move` (test 6) | **green** | pins words per page and a hash of the word positions: P1 [222, 279], 06 [453, 498], 08 [407, 473], 16 [467, 541] |
| `guard_table_cell_keeps_its_closed_frame` (F96d) | green | |
| `guard_box_with_a_height_keeps_its_closed_frame` (F96b) | green | |
| `guard_absolutely_positioned_card_keeps_its_closed_frame` (F96c) | green | |
| `guard_flex_row_item_keeps_its_closed_frame` (F96e) | green | |
| `terms_card_clauses_invoice_slices_both_cards` (`fixtures.rs`) | red | the signature line at 649.6 pt, Chrome 693.3. The rule assertions behind it are also red (3 and 3 groups; Chrome's PNG gives 2 and 2) |

The other 19 tests of `fixtures.rs` pass with fixture 42 added. The assertions of tests 1-5 and of the fixture test were checked
against Chrome's own PDFs and pass there. Not run: the fork tests of the plan (step 3), the rest of the suite, `fulgur-vrt`,
the byte gate.

### Fork notes for step 3 (read from `git show d5b4a994:<path>`, nothing edited)

Every function the plan names exists at `d5b4a994` under that name: `fragment_block_subtree` (`pagination_layout.rs:5090`),
`lines_split_below` (declared 5159, set 5938), `closing` (computed 6447, used 6500 and 6516-6523), `block_size_constrained`
(7223, takes one node), `frame_fragment` (`render.rs:3745`), `PageStrips::chrome_bottom` (2536) and `fresh` (2528), `line_split_insets`
and `box_slices` on `PaginationGeometry` (2426, 2448). `is_cell_of_table(parent, child)` (7929) takes two nodes, not one.

The fork branch `htmltopdf/srgb-colours` already contains F215 (`7a79be15`, on top of `d5b4a994`). Its diff against `d5b4a994`
touches `fragment_block_subtree` at 5257, 5757-5769, 5876, 5908, 5930 and 5986 (pre-F215 numbering) and adds
`pagination_layout/keep_with_next.rs`, 24 lines in `draw_primitives.rs` and 2 in `render.rs`. The `closing` block (6446-6523)
and the end of the function come after F215's last hunk there, so the plan's edits (entry, before `closing`, after the final push)
should not conflict textually; the function's line numbers move by about 130. Re-read the function on top of `7a79be15` before
step 3 and add fixture 41 to the gates when it is in `main`.

### Steps 3-5: fork, byte gates, compare (2026-11-03)

Fork `htmltopdf/srgb-colours`: `abcd5217` (the fragmenter and renderer; on top of F215 `7a79be15`) and `ffdf0c49` (the new helpers had been
inserted inside `fragment_block_subtree`'s doc comment; moved after `has_page_name_change_below`, no code change). The doc comment
that sits above `RowState` on `d5b4a994` was already misplaced there and was left as it is. html-to-pdf `Cargo.lock` follows `ffdf0c49`.

**Fork gates:** `cargo test -p fulgur --lib` 2519 passed; `cargo test -p fulgur --tests --no-fail-fast` (44 test binaries, the lib
included) all passed; `cargo clippy -p fulgur --all-targets -- -D warnings` and `cargo fmt --check` clean. Run with the Blitz / Taffy / Parley fork
overrides of the html-to-pdf `Cargo.toml`.

**Red to green** (`cargo test --release --locked -p htmltopdf --test block_box_slice --test fixtures`): tests 1-5 of
`block_box_slice.rs` (P1 `card_around_a_split_paragraph_is_open_at_the_break`, P3 `background_runs_to_the_cut_and_the_shadow_stops_there`,
`corners_are_square_at_the_cut_and_round_elsewhere`, P2 `what_follows_a_box_split_between_children_starts_below_its_bottom_inset`, P4
`nested_boxes_are_both_open_at_the_break`) and `terms_card_clauses_invoice_slices_both_cards`. The pinned text-layer test (6) and the four
guards (F96b-e) stayed green. New in this step: `outer_frame_ends_at_the_content_bottom_not_in_the_margin` (P4 and fixture 42: the side
borders reach row 1065 and none is inked from row 1067 down; the coordinator's "+-1 px" check). It is red on `7a79be15` (fixture 42 page 1: the card's
side runs to row 1122) and green now (the side ends at row 1066, as the reference does).

**Byte gate** (`htmltopdf-f215d`, fork `7a79be15`, against `htmltopdf-f96`, fork `ffdf0c49`; 40 fixtures + 7 corpus documents; 1 and 4
rayon threads: identical to each other for both binaries):

| Moved | Why |
|---|---|
| 06, 08, 16 | paint only; `text_layer_of_line_split_cards_does_not_move` pins the text layer |
| 42 | new (base has no fixture 42 reference run; the base binary renders it with closed frames) |
| `b200-mixed` | **198 -> 199 pages, which the plan said must not happen, and is the right number**: the first difference is on page 3, where "Bankovní účet" after a clip card that splits stands 1 px (0.75 pt) lower (277.60 -> 278.35 pt), the card's bottom border that `closing` now counts. Chrome 153 prints `b200-mixed` with 199 pages (F30c-n's note said "Chrome 199" too) and puts that line at 278.58 pt. The drift accumulates over the 181 cards and moves the last unit to a new page. Page counts of all 47 documents are otherwise unchanged. |

Identical: 01-05, 07, 09-15 (**15 included**: the relative wrapper with `padding-bottom:2.5rem` did not move), 17-39, `b200-items`,
`b200-text`, the four `slow-*` documents. No moved document is outside the plan's table.

**compare.py** (`HTMLTOPDF_BIN` = each binary, `scripts/render-fixtures.sh`, then `compare.py`): worst page, before -> after, per page:

| Fixture | Before | After |
|---|---|---|
| 06 | 10.69 / 11.80 % | 10.61 / 11.80 % |
| 08 | 8.20 / 12.10 % | 8.20 / 12.10 % (identical to the digit: the frame lines are light grey, under the metric's tolerance) |
| 16 | 9.53 / 12.90 % | 9.45 / 12.90 % |
| 42 | 5.7 / 12.1 % | 5.7 / 5.9 % |

The thresholds of 06, 08 and 16 stay (the worst page did not improve). 42: 0.069 (5.86 + 1 point, comment in `compare.py`). The side-by-side PNGs
of 06 p1 / p2, 08 p1, 16 p2 and 42 p1 / p2 were looked at: the cut cards are open at the break with the sides to the content bottom, the corners at the cut are
square, the closing frames on page 2 carry the bottom border and radius like Chrome's, and the payment block of 42 is where Chrome has it.
`compare.py --threshold-check` passes.

**Bench** (`bench.py --check`, this host, 4x Xeon 2.80 GHz, `htmltopdf-f96`): ok. Median ms: `b200-items` 1265 (210 pages), `b200-text` 744, `b200-mixed` 802 (199 pages); every fixture
under 35 ms; start-up 2.8 ms; scaling r = 3.98. Interleaved 9-run medians, base against new: `b200-mixed` 830 / 805 against 785 / 794 ms, `02-long` 39.7 / 32.8 against
32.0 / 33.5 ms, 42 21.7 / 21.7 against 20.0 / 19.9 ms: no slowdown beyond the noise.

**`fulgur-vrt`** (`f183-vrt.sh`, base `7a79be15` against `ffdf0c49`, the host's goldens): 35 of 64 fixtures differ from their goldens on both, and they are the same 35. Of those, two
outputs changed between the base and F96: `bugs/grid-row-promote-background` and `gcpm/string-set`. Text layers identical; the pixel difference is only the cut rows of the card
that crosses the break (bottom of page 1, top of page 2). Neither golden is re-blessed here: both already differ from their golden on this host for font reasons, and a golden
written on this host would pin the host's bytes (see `docs/upstream.md`, "Fork VRT goldens"). No golden that passes on this host moved (examples/break-inside, mo-006 and mo-008 are unchanged).

**For `pdf-reviewer`: what changes in frame painting.** No new operator, font, metadata or structure. The same paint calls run with different edges and heights:
1. `render.rs::frame_fragment`: a new branch for `geom.block_slice && geom.is_split()` returns `FrameEdges { top_open: !is_first, bottom_open: !is_last }` (judged by page index). The frame
   rectangle itself is the fragment's, unchanged by the renderer.
2. Every block paint path already reads `frame_fragment` and `sliced_style` (generic dispatch incl. `<li>`, `draw_under_clip`, `draw_under_opacity`, the transform path): the border is drawn as
   open segments (`open_rounded_border_segments`, F30f-d), the radius is dropped at open corners (square corners at a cut), the background is clipped to the fragment, and the box shadow
   uses `draw_sliced_box_shadow` (no shadow past a cut edge). An `overflow` clip is the fragment's padding box.
3. Fragment heights (fragmenter side): every fragment but the last runs to `PageStrips::chrome_bottom` (up to 0.87 px into the bottom margin, as F30f-d's sliced paragraphs); the page-0 frame that
   used to run to `fresh()` (row 1122, into the margin) now ends there too; the last fragment includes the bottom padding and border (`closing`).
4. Boxes outside `block_slices_at_breaks` (block size, float, transform, abspos and what is in it, multicol, table parts, atomic inlines, inline roots, frameless boxes, `<body>`, `<html>`) keep their
   frames, byte for byte (byte gate and `boxes_outside_f96_keep_their_frames`).
The moved PDFs are 06, 08, 16, 42 and `b200-mixed` (`gate/new-1` in the session scratch).

**Not verified:** the Chrome pixel agreement of 06 / 08 / 16 beyond the compare numbers above (no new Chrome run of them; `b200-mixed` was re-run in Chrome m153: 199 pages, "Bankovní" 278.58 pt); `box-decoration-break: clone` (not supported by Stylo here); the fork's own VRT job in the pinned font environment; `vrt-validator` and `pdf-reviewer` (run by the coordinator); a merge with F215 (PR #86) and fixture 41 in the gates.
