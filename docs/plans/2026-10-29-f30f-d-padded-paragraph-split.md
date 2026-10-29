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

### Step 1: Chrome m141 measurements (A4, 15 mm, Inter, 14px/1.2, `padding:16px;border:1px solid #888;border-radius:8px`, 1500 words, `AFTER` paragraph below; harness `out/scratch/f30fd/{gen1,meas,tab}.py`, git-ignored)

The gate holds: **gate passed**. Chrome puts the first line of the continuation page at 42.3 pt (the content box top is 42.5 pt; word boxes start 0.2 pt above the line box) with no top border on that page, for a body-level `<div>`, `<p>`, `<div>` in a `<section>`, a nested `<p>` and a padding-only box with a background, with and without `overflow:hidden` (the two PDFs have identical words and positions).

| shape (spacer above) | Chrome words per page | last line yMax p1 | first line p2 | AFTER | main words per page / AFTER |
|---|---|---|---|---|---|
| div, direct text, `overflow` or not (0) | 763 / 737 | 798.3 | 42.3 | p2 760.8 | 776 / 724, p2 735.8; yMax 810.8 (below the content box, 799.4) |
| same (300 px) | 530 / 798 / 172 | 796.8 | 42.3 | p3 231.3 | **0** / 776 / 724 (the whole card moves to page 2, F30c) |
| same (700 px) | 223 / 773 / 504 | 794.5 | 42.3 | p3 533.6 | 0 / 776 / 724 |
| `<p>` with the padding (0 / 300 / 700) | 763 / 737, 530 / 798 / 172, 223 / 773 / 504 | as above | 42.3 | as above | the same as the div: lost line and shifted frame |
| `<div>` with a `<p>` inside (nested, 0 / 300 / 700) | as above | | 42.3 | | 763 / 737, AFTER 761.3; 530 / 793 / 177; 222 / 774 / 504: within 5 words of Chrome, lines at 42.1 on page 2 |
| `<section>` > padded div, direct text (0 / 300 / 700) | as above | | 42.3 | | 763 / 737 but lines at **54.8** on page 2, AFTER 774.1 (+13.3 pt); 530 / 779 / 191; 222 / 761 / 517 |
| `td` with `padding:16px;border` (0) | 742 / 758 / 0 and AFTER alone on page 3 | | 42.3 | | 763 / 737, p2 lines at 54.8 |
| padding only, background (0) | 769 / 731 | 797.5 | 42.3 | p2 747.3 | 779 / 721, AFTER 723.1 |
| `lh20` (14px/20px, 0 / 300 / 700) | 634 / 666 / 200; 442 / 647 / 411; 184 / 644 / 658 / 14 | 789.3 | 43.8 | | 646 / 663 / 191 (and 0 first with a spacer) |
| `t16/A_direct_pad`, `B`, `E_leading` (Tailwind, section direct text) | 385 / 617; 385 / 617; 385 / 810 / 688 (words of the whole page) | 796.8 | 42.6 | | 84 / 684 / 238; 84 / 891 / 30; 84 / 792 / 782 / 228: **the card starts on page 2**, page 1 holds 84 words |
| `pad16_h300.s700` (700 px spacer, `height:300px`, padding 16) | 199 / 13 | 799.0 | 43.8 | p2 60.3 | 185 / 26, p2 60.1 |

What the PNG shows (`c286_ovf_s0`, 150 dpi crops of page 1's bottom, page 2's top and bottom): on page 1 the left and right borders run straight down to the page's content-box bottom (row 1065 px of 1065.8), there is no bottom border and no corner there; the top border and the radius are at the top of the card. Page 2 starts with the side borders at the content box top, no top border, no radius; the last fragment has the bottom border with its radius and `AFTER` right below it (the padding bottom is in the frame). So the frame of a non-last fragment runs to the page's bottom (css-break-3 §5) and `slice` is exactly what the plan predicted. A `td` is sliced the same way (its page-2 lines start at 42.3 and there is no top border), which is outside this plan's routed shapes and is left to F96.

The step's scratch build with `inset_top = 0` on continuation pages was not made: step 3 produces that geometry in the fork, and the words, line positions and AFTER above are the numbers the tests are written against. The Tailwind shapes differ from Chrome first in where the card starts (page 1 holds 84 of Chrome's 385 words on main), which is the start rule the new branch changes; test 4 is decided after step 3 (§4).

### Correction to step 1: the `t16/A`, `B`, `E` rows

The three Tailwind rows of the table above (Chrome 385 / 617, 385 / 810 / 688 words of the whole page) came from `out/scratch/f30fbv/t16/*.chrome.pdf`, which were rendered with a relative `<link href="../../tailwind/tailwind.css">` that does not resolve from that directory: Chrome printed them unstyled (Liberation Serif; `pdffonts`). They are not references. The valid ones are `out/scratch/f30fd/t16` and `v29` (absolute stylesheet path): A 497 / 505, B 665 / 337, E 549 / 453 words on 2 pages (E: 3 pages with the section in front, 549 / 779 / 555). The "card starts on page 2 (84 words on page 1)" on main is real and is the start rule the new branch changes; the "385 words" was not Chrome's. Test 4 is therefore asserted (§4), on fixture 29's card in six Tailwind shapes.

### Steps 2-4: what was built and what the gates said

- **Step 2** (`5918522`): `padded_paragraph_split.rs` tests 1-3 and the guards, fixture 29 with its Chrome reference; red on `ab1fc537` as recorded in that commit (not re-run by the engineer who finished the work).
- **3a** (fork `40f3e7fd`): the body-level branch `inset_split_here` (insets > 0, block flow, no float, no `break-inside: avoid`, not multicol, no block size of its own, the lines and insets do not fit the room), `LineSplit::slice`, continuation room without `inset_top`, placements, `line_split_slice`. **3b** (`779b0c28`): `frame_fragment` returns the open edges (it returns `(Fragment, FrameEdges)`; the lines' dy is taken off by `sliced_style`'s caller, not returned), `sliced_style` in `draw_primitives.rs`.
- **Step 4** (`7a33d58c`, `87946cc3`): `slices_at_breaks` (the child of a block or flow-root in `fragment_block_subtree`, with border or padding, no block size of its own, not a table cell or a cell of a table) sets `slice` for the paragraphs of the block walk (`block_walk_line_split`); fixture 16's `p.p-4` inside the clip `<section>` is one. The `own_box_height` / `frame_inset` arithmetic of 3a already drops `inset_top` on continuation pages; it is only reached by boxes with `slice == false` now.
- **Step 4 stop condition, taken for the clip box with a block size.** The first version of step 4 also sliced the body-level clip box that is fragmented in place (`overflow:hidden; height:300px; padding; border`). 15 sweep cases that main and 3b had at Chrome's words and lines exactly (`f30fbv/c/stradpad950`, `tall2500`, `f30fbv3/pad16t_700/800/900`, `f30fbv4/sw2/notesD.s888 ... s954`) ended one line further from Chrome. Cause (measured on `pad16t_700`: spacer 700 px, `height:300px`, `padding:16px`, 20 px lines): the box's last fragment is 23 px tall; Chrome shows the first line only, at 43.1 pt, and hides the second, which starts 20 px down, i.e. past the end of the fragment's content box (23 - 16 = 7 px). Ours, sliced, drew the second line (57.9 pt) because the writer clips at the padding box (23 px). Main's closed frame hid it by accident (its lines sat 16 px lower). Chrome's rule is therefore "on the last fragment of a box with a block size, a line is shown only if it starts above the content box end", not a paint clip. Per the plan, clip boxes with a block size keep the F30f-b geometry; the first line of their page 2 stays `inset_top` low (12 pt in that case, AFTER is where Chrome has it). Filed as F30f-i. Pinned by the fork test `a_padded_clip_box_with_a_height_keeps_its_closed_frame`.
- **Gates.**
  - Fork (`87946cc3`): lib 2388 passed; the 42 integration binaries one at a time all ok (`render_smoke` 323); `clippy -p fulgur --all-targets -D warnings` and `fmt --check` clean. `fulgur-vrt` (`f183-vrt.sh`): 35 of 64 differ, as on the base (the known env failures); the 35 `actual.pdf` are byte-identical to the base run, the other 29 match their goldens: **no golden is re-blessed**, no fulgur-vrt fixture holds a sliced paragraph.
  - Byte identity (`f183-gate.sh`, 1 and 4 threads, identical to each other): against main's binary (`htmltopdf-main-f30fb`) only `16-terms-card` and the new `29-card-direct-text` differ; fixtures 01-15, 17-28 and the 7 corpus documents are byte-identical. For 3b alone (run before the container restart on the working-tree build; not repeated on `779b0c28` itself) only 29 differed, so 16 is the one fixture step 4 moves.
  - html-to-pdf: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --release --locked`: 74 result lines, all ok, 0 failed (`clip_boxes` 29, `wrapper_line_split` 26, `padded_paragraph_split` 9). The first run of clippy found two lints in the tests written before the engine change was finished (`filter().next_back()`, a complex return type); fixed in `e6d7344`.
  - `compare.py` (all 29 fixtures ok): **16: 9.5 / 20.8 % on main -> 9.5 / 12.9 %** (page 1 did not move; threshold 0.218 -> 0.139); **29: 3 pages, 11.3 / 23.0 / 5.5 % on main -> 2 pages, 9.9 / 12.6 %** (threshold 0.136). Every other fixture's number is as in its threshold comment. `bench.py --check` ok (b200-items 1.34-1.38 s here, the 1.25 s goal is "not met on this machine" on main's binary too: 1.33-1.53 s measured side by side).
  - What the PNGs show. 29, page 1: the card has its top border and radius, the side borders run to the page bottom, no bottom border, no line in the margin, like Chrome. Page 2: no top border, the first line at the content top, the last line inside the frame, bottom border and radius, `PLATEBNI UDAJE` below with no overlap and at Chrome's y; the diff is dense text (a word wraps differently a few lines in). 16, page 2: the first line is at the content top and the payment block at Chrome's y (721 px both; main 734); the `<section>` card's own frame is still closed on page 2 (a top border and radius at the head of the page that Chrome does not draw): F96, block boxes.
- **Sweep** (`out/scratch/f30fd/sweep.py`, main's binary against the final one, 2623 cases from `f30fbv*`, `f30fd/t16`, `f30fd/v29`, `f30fbv2/pad`; Chrome references beside each; metrics: words per page, distinct line rows per page, 60 dpi ink per page, `AFTER`'s y): 247 changed. Words per page distance to Chrome summed over the changed cases 28879 -> 3625; ink L1 1.68 M -> 0.29 M; exact words-per-page matches 32 -> 95; page counts different from Chrome's 17 -> 1 (the one is `sc2/c14`, named below). Words: 182 closer, 51 same, 14 farther; ink: 186 closer, 42 same, 19 farther; `AFTER`: 163 closer, 57 same, 27 farther.
- **Exceptions: cases further from Chrome than main in words or ink (20, named).** All are `overflow:hidden` cards with direct text where main happens to be exact, because its old path ignored the insets in the fit test and painted the lines inset lower, which cancelled. Two causes, both outside this plan:
  1. The fit test measures the lines' glyph-run extents (`collect_inline_line_metrics`: `(-1, 16)` for 15 px lines of 14 px Inter), which are up to 2 px taller than the line boxes when `line-height` is below the font's content height (1.21): 67 lines of 15 px plus a 3 px padding fit Chrome's page, ours counts 1010 px against 1009.13 and puts 66. Cases: `sc/c110, c128, c140` (n has Chrome's lines per page; only the ink count is farther), `c142, c236, c238, c260, c290, c292, c296, c304, c334, c352, c52, c80, c82`, `sc2/c138`. Filed as F30f-j.
  2. Chrome's page is 1010 px and ours 1009.13: a line that ends in that last pixel fits there and not here (`sc2/c62`: the 47th line ends at 1008.9 px; `sc2/c16`, `sc2/c142` with `line-height: normal`, wrapping differs by a line as well). F30f-c.
  3. **`sc2/c14` is the one page-count change** (`10px/24px`, padded card, 1500 words: main 2 pages like Chrome, now 3). Our text wraps to 82 lines and Chrome's to 81 (a word wraps differently, sub-pixel advances); main printed 42 on page 1 where Chrome and the fix print 41 (main's lost last line of page 1, which is how it kept `AFTER` on page 2), so with the fix the card ends at about 798 px and the 24 px `AFTER` line no longer fits under it on page 2 (Chrome: `AFTER` at 780 pt, the card is a line shorter). A scratch case; no fixture changes its page count.
  The causes above are read from the line geometry of `c260` (debug print of the fit test: first room 1006.13 px, 67 lines span 1007 px) and `c62` (47 lines span 992.7 px against 992.13), and from `lines per page`, not bisected case by case; no exception is a lost line or a line in the margin (`inside the content box` holds in the new tests; the sweep does not measure it).
- **Not changed:** inline-block wrappers (`display:inline-block; width:100%` around a padded paragraph give main's output byte for byte, so ADR-0004 is not amended); table cells (Chrome slices them too, F96); the body (F37).


### Reviewer defects: radius and shadow of a sliced frame (pdf-reviewer, fork `3ee98d70`)

Two defects of 3b / step 4, found by `pdf-reviewer` and confirmed by the `vrt-validator` pass (`out/scratch/f30fdv/`: findings 1 and 5; crops `f29corners.png`, `f29bot.png`, `crop_shadow.png`). Spec: css-backgrounds-3 §5.5 (corner radii) and css-break-3 §5.4 (the box is rendered as if unbroken, then sliced: radii apply at unbroken corners, nothing is painted beyond a cut edge).

- **Radius.** `sliced_style` zeroes the width of an open edge, so the widths were no longer uniform and `draw_block_border` fell to its per-side straight-line branch, which ignores `border_radii`, while `draw_background` rounded the fill: page 1 had square top corners, the last page square bottom corners, and with a background a white gap showed between the square border and the rounded fill. Fix: `BlockStyle::open_edges` (set by `sliced_style`; empty for a box painted whole) and, in `draw_block_border`, for a box with an open edge, radius and one colour: `open_rounded_border_segments`, one open stroke path along the left side, round the two corners that keep their radius, up the right side, butt ends on the cut edge (any uniform border style, so dashed works); sides of different widths or styles with solid style take `draw_border_ring` (it already skipped zero-width sides), mixed colours already did. `sliced_style` already zeroes both radii of an open edge (checked, unit test `sliced_style_zeroes_the_open_edges_only`). A box with no open edge reaches none of the new code: **the latent case on main (`border-b rounded`, non-uniform widths and a radius on a box painted whole) is deliberately not touched.** Checked for it: no fixture 01-29 and no corpus document is byte-different against main outside 16 and 29 (byte gate below); a general fix would be its own finding with Chrome numbers, not filed here because no fixture or corpus document has such a box (not searched beyond the byte gate: a box of that kind that is rendered the same by both binaries would not show).
- **Shadow.** `draw_box_shadows` painted the whole shadow round the fragment's frame (page 1's shadow ran to 812.0 pt below the content box end 799.4, page 2's started at 36.5 above the content top 42.5). Fix (`sliced_shadow_rects`, `draw_sliced_box_shadow`): for a box with an open edge the shadow is drawn for the frame extended by its reach (|offset| + spread + blur + 1) over each open edge and clipped to the frame at that edge, so the side shadow runs at full strength up to the cut and nothing is painted beyond it; the closed sides keep their whole reach. Chrome (Playwright Chromium, `out/scratch/f30fdpdf/cards/chrome/shadow.html`, crops `s1-bottom.png`, `s2-top.png`): no shadow below page 1's frame or above page 2's, and the side shadow continues to the cut: the same picture. The opacity (`draw_under_opacity`), clip (`draw_under_clip`) and `<li>` paths go through the same two functions with the sliced style: 8 curves each in the `qpdf` content stream of the 3-page repro (`cards/opacity`, `cards/li`), 12 with the clip.
- **Tests (red first).** Fork `tests/sliced_box_decoration.rs`: curves in the content stream of a 3-page sliced card (border only: 0 before, 4 after: two per end; with a background 4 before, 8 after) and a 72 dpi raster of the shadow rows in the margins (page 1 rows 787-841 and page 2 rows 0-55 were not white); unit tests for the segments (`open_border_*`), the shadow rectangles (`sliced_shadow_*`) and `open_edges` of `sliced_style`. html-to-pdf `padded_paragraph_split.rs`: `a_sliced_card_keeps_the_radius_of_its_border_at_the_unbroken_corners` (plain and background, 96 dpi corner pixels outside a 24 px radius, white; red on `87946cc3`) and `a_sliced_card_has_no_shadow_beyond_its_cut_edges` (red on `87946cc3`). The first run of the html-to-pdf corner test sampled x = 640 for the right corner, which is inside the page (right edge 737 px); fixed to 735 before it was green, so it was red for a second reason once.
- **Gates.** Fork (`3ee98d70`): lib 2395 passed; the 43 integration binaries one at a time all ok (`render_smoke` 323, the new `sliced_box_decoration` 3); `clippy -p fulgur --all-targets -D warnings` and `fmt --check` clean; `fulgur-vrt` (`f183-vrt.sh`): 35 of 64 differ as on the base (the known env failures), the output directory is identical to the run before this change, so no golden moves. html-to-pdf: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --release --locked` (74 result lines, all ok), release build. Byte gate (`f183-gate.sh`, 1 and 4 threads identical): against main's binary only 16 and 29 differ, against the previous branch head only 29 (its card has a radius; 16's bytes are unchanged: its sliced box has no radius). `compare.py`, all 29 ok: **29: 9.9 / 12.6 -> 9.8 / 12.5 %** (threshold 0.136 -> 0.135), 16 unchanged at 9.5 / 12.9 %. The side-by-sides show the rounded corners at the top of page 1 and the bottom of the last page of 29, and none of the square corners or gaps. Page 2 of 16 still has the closed frame of the `<section>` (F96).
- **Vrt-validator pass on the branch** (`out/scratch/f30fdv/`, repros `b1..b9.py`, `c1.py`, `c2.py`): findings 1 and 5 are the two defects above; the validator's repros `two_plain`, `bg`, `radius30`, `shadow` now show the rounded corners and no seam (crops `crop_r30.png`, `crop_sh.png`), `thick10` and the others keep their numbers.
  - **Finding 4 (widows at an exact fit)** is not a widows bug: `padding:0 10px 21px; border:1px` with 47 / 48 `<br>` lines of 21 px splits 46 / 1 and 47 / 1 (Chrome 45 / 2 and 46 / 2). A probe of `plan_line_breaks` shows `n = 48` lines for 47 words: the trailing `<br>` makes an empty extra line box (F37d / F66f), the widows rule counts it, and the page after the break holds "one word plus the empty line". Not fixed here (it would change every paragraph that ends in `<br>`); noted in F66f.
  - **The sweep exceptions are not only `overflow:hidden` cards.** Besides the two causes above: (finding 2, filed as F30f-k) with a fractional line pitch (14px/1.2) and a padding of 18 to 32 px Chrome restarts its 16 / 17 px snapping on every fragment and we keep the absolute phase, so page 2 holds a line less (`pd*`, `lh16_pd18`: 834 / 902 / 886 / 378 against 839 / 912 / 889 / 360 words); (finding 6, F30f-k) thick borders are the same cause with a larger inset (`bw10_p16`, `bw6_p8`); (finding 3, folded into F30f-j) a 1 px border without padding (`bw1_p0`, `pd00`, `pad1`): page 1 drops a line because the glyph-extent fit test plus the 1 px inset passes 1009.13 px, `AFTER` at 762.1 against Chrome's 748.8 (main 748.6).
  - Pre-existing and equal on main, filed in the findings file: a padded `div` in a `column-count: 2` container prints 361 of 2500 words (F198, HIGH, silent; `f30fdv/a/multicol.html`), `outline` is not painted (F199), tagged output leaves text directly in a `<div>` and pages without a link or figure unmarked (F200), an `<img>` data URI without size does not set the line height (F201).
- **Not verified:** the segments for a rounded border with elliptical radii (`rx != ry`) have no Chrome comparison; a sliced box with `border-style: double`, `groove` or `ridge` takes `apply_border_style` like the whole-box rounded branch and was only smoke-tested; a shadow with a negative spread on a cut edge is covered by the reach arithmetic and a unit test, not by a raster.
