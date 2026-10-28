# F30f-d: a padded or bordered paragraph that spans pages is sliced at its breaks, not cut at its border box

Date: 2026-10-29. Finding: F30f-d in `2026-10-15-m0-findings.md` (HIGH, silent). It takes the inline-root part of F96 with it (§3). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours` (head `ab1fc537`), carried via `[patch.crates-io]`. The function names below are from that head; re-read the line numbers before editing.

## 1. Problem

Take a body-level inline root (a `<div>` or `<p>` with direct text) that has a border or padding and does not fit its page. `fragment_inline_root` cuts it, and its fit test is `paragraph_top + (line_bottom - first_top) <= strips.bottom(page)`, as if the first line sat at the top of the border box. The renderer paints the lines `content_inset` (border + padding top) lower on every fragment, and `line_split_insets` is not recorded, so the frame (`frame_fragment`) is only as tall as the lines. Results:

- The last line of every page except the last ends `inset_top` below the content box and below its own frame.
- On the last page the frame closes `inset_top + inset_bottom` too early.
- The cursor after the box (`paragraph_top + frag_h`) is that much too high.

Repro `out/scratch/f30fbv/sc/c286.html` (14px/1.2, `overflow:hidden;padding:16px;border:1px solid;border-radius:8px`, 1500 direct words). Main writes w0763-w0775 at y 798-811 pt; the content box ends at 799.4 pt and the clip hides them. Main has 776 / 724 words per page against Chrome's 763 / 737. AFTER is at 735.8 pt against 760.8: page 2 has one line fewer and the bottom inset is missing. Without `overflow` the code predicts the following, and step 1 confirms it: the line is painted in the bottom page margin, where an `@bottom-*` footer sits; the frame's bottom edge runs above the last line; what follows the box starts over the box's last line. 23 of 144 `sc2.py` card configs and `t16/E_leading` (Tailwind) have the shape.

No committed fixture shows it. Fixture 16's `<p class="p-4">` is nested, and the nested splitter reserves the insets. Hence fixture 29 (§4).

The tried patch (`2026-10-28-f30f-b-clip-boxes.md` §7) routed such a paragraph through `line_split_candidate` + `fragment_nested_inline_root`:

- c286 reached exactly 763 / 737 with every word on the page.
- But of 238 changed cases only 69 matched Chrome, and 86 moved further away.
- c286's AFTER landed 13 pt below Chrome. That is the 17 px of padding and border the nested splitter reserves again on page 2, where Chrome reserves none.

## 2. Spec (css-spec-researcher not run; cited by the planner, section numbers unverified)

- css-break-3 §5.4 `box-decoration-break`. The initial value `slice`: "as though the element were rendered with no breaks present, and then sliced by the breaks afterward". No border or padding at a break edge; `border-radius` only at unbroken corners; the background is one box, sliced. It applies to all elements.
- css-break-3 §5 (same section or §5.1, unverified): when a box breaks, its content box extends to fill the remaining fragmentainer extent. So the frame of a non-last fragment would run to the page bottom; step 1 measures this.
- css-break-3 §4.1 (class B break points between line boxes) and §4.4 (`orphans` / `widows`, initial 2).
- css-overflow-3 §3: the clip is the padding box of each fragment. With `slice`, a continuation fragment's padding box starts at the fragmentainer edge.
- Stylo (servo build) does not compute `box-decoration-break`, and the compiled `tailwind.css` has no `box-decoration-*` rule. `slice` is the only value we can see.

## 3. Where it belongs and the scope decision

This goes in the fulgur fork only. Taffy lays the box out correctly without fragmentation (the insets are in `final_layout`). Fragmentation and painting belong to fulgur. No upstream PR (standing decision).

**Decision: fix F30f-d together with `slice` geometry for line-split inline roots, and nothing else of F96.**

1. The patch alone is not an improvement everywhere: 86 of 238 cases moved further from Chrome. The remaining offset is exactly the reserved top inset. `slice` is the spec's initial value, not a policy choice, so we must apply it to the boxes we route.
2. All of F96 would also cover the block fragmenter's frames, abspos and multicol: fixtures 06, 08 and the `<section>` in 16 would move at once. It is not needed to stop the loss, and it would hold a HIGH silent loss behind a LOW cosmetic finding. The open-edge renderer piece built here is what F96 reuses.
3. Rejected: keep `fragment_inline_root` and add `inset_top` to its fit test. It produces the same geometry as the patch, with the same 86 cases further from Chrome.

Changes:

- `pagination_layout.rs`, `fragment_pagination_root`, body-level line path: the patch's `inset_split_here` branch, as tried.
- `LineSplit` gets `slice: bool`, and `fragment_nested_inline_root` reads it:
  - first-strip room stays `bottom - y_in - inset_top`;
  - continuation room becomes `fresh - top_floor`, with no `inset_top`;
  - the bottom inset stays on the last strip only (`last_extra`);
  - if step 1 says Chrome extends a non-last fragment's frame to the page bottom, its frame height goes to `chrome_bottom`, as the `own_box_height` branch already does.
- `PaginationGeometry::line_split_slice: bool`, next to `line_split_insets`.
- `record_split_inline_root_descendants`: apply the inset on the first strip only (inline images on continuation pages).
- `render.rs`: `frame_fragment` returns the open edges and the lines' dy. Its consumers are `draw_block_inner_paint`, `draw_under_clip` and the block-with-inner-content paint (`inner_inset`).
- `draw_primitives.rs`: one helper, `sliced_style`, removes the open side's border width and radii. `draw_block_border`, `draw_background`, `draw_box_shadows` and `compute_overflow_clip_path` are called with its result and stay unchanged. `paragraph_lines_for_page` is unchanged (the slices are authoritative).

```rust
pub(crate) struct FrameEdges { pub top_open: bool, pub bottom_open: bool }
pub(crate) fn frame_fragment(geom: &PaginationGeometry, frag: &Fragment) -> (Fragment, FrameEdges, crate::units::Px);
pub(crate) fn sliced_style(style: &BlockStyle, edges: FrameEdges) -> std::borrow::Cow<'_, BlockStyle>;
```

Which callers set `slice`:

- Step 3: only the new branch.
- Step 4: also the body-level clip-box branch and `fragment_block_subtree`'s paragraph split. Table cells only if step 1 shows that Chrome slices them.
- Never in this plan: `fragment_body_inline_root` (body has F37's `box_slices`) and the multicol `column-span` caller (outside this finding).

## 4. Acceptance

Ours, a new file `crates/htmltopdf/tests/padded_paragraph_split.rs`, using the text-layer helpers of `clip_boxes.rs`:

1. c286: 763 / 737 words; every word written once, in order. Every glyph box is inside its page's content box (page 1: ≤ 799.4 pt) and inside the frame.
2. c286 without `overflow`: the same words; no glyph below the content box; AFTER within 1.5 pt of Chrome's (step 1).
3. The first line of page 2 is within 1.5 px of Chrome's, at the content box top.
4. `t16/A_direct_pad`, `B_direct_pad_xs`, `E_leading`: Chrome's words per page within one line. This is asserted only if step 1 shows the gap is the insets plus the start rule; otherwise it is filed, not asserted.
5. Guards, byte-identical to main:
   - a padded paragraph that fits its page;
   - one with `break-inside: avoid`;
   - an unpadded body-level paragraph (it keeps F30c).

Fork:

- `pagination_layout` lib test `a_padded_body_level_paragraph_is_sliced_at_its_page_breaks`: the continuation fragment is at y 0 and its lines start at the strip top; the first strip leaves `inset_top`; the bottom inset is on the last fragment only.
- An inline image on a continuation page lands at its line.
- `render.rs` test `frame_fragment_of_a_sliced_paragraph_opens_the_broken_edges`.
- An end-to-end smoke test in `render_smoke.rs` (fork coverage rule).

New fixture `29-card-direct-text`, without `overflow`, which is the commoner and more visible case. Shape: header, items, `<div class="rounded-lg border border-gray-300 p-4 mb-6 text-sm">` with about 1.3 pages of direct Czech terms text, then the payment block. Classes are as in 06, so no Tailwind rebuild. The Chrome reference comes from `scripts/reference/render.mjs` and is committed.

- The PNG must show: page 1 card with top border and radius, sides down to the page bottom (if Chrome does that), no bottom border, no line in the margin. Page 2: no top border, the first line at the content top, the last line inside the frame, bottom border and radius, the payment block below with no overlap.
- Its threshold is the worst page + 1 point, with a comment in `compare.py`. It is red on the current fork: a line in the margin and an overlap.

Byte identity (`f183-gate.sh`, 1 and 4 threads):

- Steps 3a and 3b: fixtures 01-28 and the 7 corpus documents are byte-identical (the patch already met this).
- Step 4: only 16 changes. Page 2's lines move up about 16 px (Chrome is about 20 px higher), and the 20.8 % worst page should go down. Lower its threshold in the same PR and update its README, which names F96. Page counts are unchanged.
- The fork's `fulgur-vrt` goldens: re-bless only those whose diff is a sliced paragraph.

Sweep: the patch's 238 changed cases (`sweep3.py`) plus `f30fbv2/pad`. No case may end up further from Chrome than main in words per page or in 60 dpi ink L1 per page; any exception is named in §7.

## 5. Steps

1. **Measure (no engine change; scratch builds only).** Use Chrome m141, main, and the patch with continuation `inset_top = 0` hacked in. Shapes: c286 with and without `overflow`; `t16/A`, `B`, `E`; a padded `<div>` in a `<section>`; fixture 16's `p.p-4`; `pad16_h300.s700`; a `td` with `py-4` and long text across a page.
   - Record: words per page; the y of the last line on page 1 and the first line on page 2; whether there is a top border on page 2 and a bottom border on page 1; the corner shape at the break; where page 1's side borders end; AFTER's y.
   - **Gate:** continue only if Chrome puts the first line of page 2 at the content box top (±1 px) with no top border for the body-level card, with and without `overflow`.
   - **Stop conditions:** if not, write a postmortem and stop. A shape Chrome does not slice (e.g. cells) leaves scope. If the t16 shapes differ for another reason (avoid or orphans start), file it and drop test 4.
2. **Red tests and fixture 29** with its reference. Confirm they are red on `ab1fc537` for the expected reasons; the guards are green.
3. **3a, fork fragmenter:** the branch, `LineSplit::slice`, continuation room, placements, the geometry flag. The fork lib tests go green.
   **3b, renderer:** `frame_fragment` edges and dy, `sliced_style`. Our tests 1-3 and fixture 29 go green. Gates: byte identity of 01-28 and the corpus, then the sweep. Stop if more cases move away from Chrome than toward it; write a postmortem.
4. **Nested callers and the clip-box branch.** Set `slice` in the clip-box branch and in `fragment_block_subtree`, and adapt the `own_box_height` arithmetic (the room and the last frame drop `inset_top` on continuation pages).
   - Gate: `clip_boxes.rs` (29 tests) and `wrapper_line_split.rs` stay green; their numbers may change only toward Chrome.
   - Fixture 16: threshold lowered.
   - Stop condition: if any clip shape moves away from Chrome, keep clip boxes on the old geometry and record why.
5. **Finish:** `render-fixtures.sh` + `compare.py` and look at the 16 and 29 side-by-sides. `vrt-validator`; `pdf-reviewer`, because `render.rs` frame painting is writer path (merge policy 5). Docs:
   - F30f-d done;
   - F96 narrowed to block boxes;
   - this plan's §7;
   - `docs/upstream.md`;
   - ADR-0004's "frame closed on every page" consequence, amended only if step 4 changes split inline-blocks.

## 6. Risks and non-goals

- **Start rule asymmetry.** The new branch starts a padded paragraph where its first two lines fit (`leading`), while an unpadded one keeps F30c's move-whole pre-advance. This is closer to Chrome for the padded ones, and it is intended until F30c.
- **Byte identity:** a change outside the routed shapes is a regression (§4 gates). The gates are needed because fixtures are the spec.
- **Chrome's page height** is 1010 px against our 1009.13: a line in the last pixel can still land differently (finding F30f-b notes).
- **Inline images and links on continuation pages** depend on the placement offset; the step 2 test pins it.
- **Non-goals:**
  - block-level F96 (06, 08, 16's section border, abspos, multicol);
  - `box-decoration-break: clone` (not computed);
  - the body inline root;
  - background-image and gradient positioning across slices;
  - F30c for unpadded paragraphs;
  - F30f-e, F30f-f, F30f-g;
  - the inline axis.

## 7. Evidence

(Filled in when it lands: step 1 measurements, red/green tests, gates, sweep, deviations.)
