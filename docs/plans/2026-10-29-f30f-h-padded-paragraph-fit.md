# F30f-h: a padded or bordered body-level paragraph that fits its page leaves what follows over its last line

Date: 2026-10-29. Finding: F30f-h in `2026-10-15-m0-findings.md` (medium-high, visible overlap). Fitting counterpart of F30f-d (`2026-10-29-f30f-d-padded-paragraph-split.md`), which fixed the same boxes when they span pages. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours` (head `fde7f859`), carried via `[patch.crates-io]`. Function names are from that head.

## 1. Problem

A body-level inline root (a `<div>` or `<p>` with direct text, `<br>`s or inline spans) with a border or padding that fits its page is placed by the fitting branch of the body-level line path in `fragment_pagination_root` (`line_metrics.len() > 1 && !clip_whole_now`, `fragment_inline_root`). That function returns the end of the *lines* as the cursor: `paragraph_top + (last line bottom - first line top)`. The box also has `border-top + padding-top + padding-bottom + border-bottom` that the lines do not contain, so whatever follows the box starts that much too high, over the box's last line(s).

The frame itself is right: a box that is not split is painted from its layout size (`frame_fragment` only adjusts a split box, `line_split_insets` is not needed for a single fragment). The finding's suggestion to record `line_split_insets` for the fitting case is therefore dropped: only the cursor is wrong.

The same line sets the cursor for a box whose block size is larger than its lines (`height`, `min-height`), with or without insets: the next sibling lands inside the box (measured below, `x1`, `x2`, `t`, `u`). It is the same cause and the same line, so it is fixed together; no other block-size case is touched.

A one-line box does not take this branch (`line_metrics.len() > 1`) and is placed by the block path with its layout height: right already (`b_one_line`).

## 2. Spec

CSS 2.1 §10.6.3 (block-level non-replaced elements in normal flow, `height: auto`): the box's height runs to the bottom of its last in-flow line box *plus the bottom padding and border* (§8.1: the box height includes padding and border; the margin box adds the margins). §8.3.1 (collapsing margins) is untouched: `gap` between siblings is computed from the layout positions as before. With a specified `height` / `min-height` the box is that tall (§10.5, §10.7). css-break-3 §5.4: a box that is not broken is one fragment with all its decoration.

Chrome agrees with the spec on every shape below (differences are 0.2 pt, line box against glyph box).

## 3. Measurements (Chrome m141, A4, 15 mm, Inter 14px/20px, `scripts/reference/render.mjs`'s engine via `out/scratch/f30fh/{gen,run.sh,meas}.py`, git-ignored)

AFTER's top (pt) for `<div …>…</div><p style="margin:0">AFTER</p>`; main = binary of 1a73c1a.

| shape | Chrome | main |
|---|---|---|
| a: border 1px, padding 9px, 5 `<br>` lines (the repro) | 133.8 | 118.6 |
| b: one line, border and padding | 73.8 | 73.6 |
| c: two lines, border and padding | 88.8 | 73.6 |
| d: padding 9px only, 5 lines | 132.3 | 118.6 |
| e: border 1px only, 5 lines | 120.3 | 118.6 |
| f: `overflow:hidden` card, 5 lines | 133.8 | 118.6 |
| g: card with `margin-bottom:24px` | 151.8 | 136.6 |
| h: Tailwind `p-4 border rounded-lg mb-6`, wrapped text | 177.3 | 151.6 |
| i: the same with `overflow-hidden` | 177.3 | 151.6 |
| j: `padding-top` only / k: `padding-bottom` only (12px) | 112.8 / 112.8 | 103.6 / 103.6 |
| m: wrapped text, border and padding | 148.8 | 133.6 |
| n: border 3px, padding 20px 5px | 123.3 | 88.6 |
| o: one line, `p-4 border` | 102.3 | 102.1 |
| q, x8: inline spans and `<br>` in the card | 118.8, 103.8 | 103.6, 88.6 |
| s: two cards in a row, then AFTER | 168.3 | 138.1 |
| l: title, two cards with margins, AFTER | 204.3 | 180.1 |
| t: `height:150px` card (border-box) | 156.3 | 118.6 |
| u: `min-height:200px` card | 193.8 | 118.6 |
| x1 / x2: plain `div`, `height:150px` / `min-height:200px`, 5 lines | 156.3 / 193.8 | 118.6 / 118.6 |
| already right (guards): r nested in `<section>`, x6 flex item, x7 `li`, y1 `break-inside: avoid`, x3 plain paragraph with margin, x4 clip box with `height:60px` | | within 0.2 pt |

Two shapes are wrong on main for another reason and stay out of scope: a `float:left` card (`x5`: Chrome 43.8, the float takes no space; main 118.6 — floats are not placed by this path at all, finding to file).

## 4. Fix

In the fork, `fragment_pagination_root`, fitting branch: when `fragment_inline_root` returns one fragment on the page it started on (`new_page_index == page_index`, `frag_count == 1`) and the box has insets or a block size of its own (`insets > 0.0 || block_size_constrained(child)`), the cursor is the bottom of the border box, `start_cursor + child_h` (`child_h` is the layout height, `final_layout.size.height`), not the end of the lines. Everything else stays: a box that is cut by `fragment_inline_root` (constrained shapes that still split between lines) keeps its cursor; a plain multi-line paragraph keeps `fragment_inline_root`'s cursor (byte identity: its glyph extents and line boxes differ by sub-pixel amounts and F30f-j owns that).

Consistency with F30f-d: a box takes the slice path when `cursor + insets + lines_extent > bottom + 0.5`; otherwise its lines fit and the box ends at `cursor + child_h`, within the 1-2 px by which the glyph extents differ from the line boxes (F30f-j). The two paths therefore use the same box geometry: top at the cursor, frame from layout, end at the border-box bottom (the slice path's last fragment ends at `lines + inset_bottom` too).

## 5. Acceptance

Ours, `crates/htmltopdf/tests/padded_paragraph_fit.rs` (red on main):

1. Shape a (and c, d, e, g, s, t, u): AFTER within 1 pt of Chrome's number in §3.
2. Pixel check on a 96 dpi raster: the card's bottom border row is below the last text row of the card; AFTER's ink does not touch the card's rows (nothing overlaps).
3. Guards, unchanged from main: b, o (one line), x6, r, y1, x3 and a plain multi-paragraph page; and a card that spans a page (F30f-d) with `AFTER` at its F30f-d number.
4. Fit and span use one geometry: the same card with 1 line fewer than the room allows (fits) and with 1 line more (spans): AFTER follows the card's last line by the same distance (inset_bottom + margin) in both.

Fork: a `pagination_layout` lib test that asserts the cursor after a fitting padded paragraph is `top + child_h` (the existing `a_padded_body_level_paragraph_that_fits_is_not_sliced` pins the wrong 100 px on purpose and is corrected), one for `min-height`, and a `render_smoke` case.

Fixture `32-note-card`: an ordinary one-page Tailwind invoice with a bordered, padded note card of direct multi-line text followed by the payment block (on main the block overlaps the card's last lines). Chrome reference from `scripts/reference/render.mjs`; threshold in `compare.py` is the worst page + 1 point.

Byte gate: fixtures 01-31 and the corpus byte-identical to main except any that holds the shape (named with `compare.py` numbers, Chrome-confirmed better, threshold lowered). Fork `fulgur-vrt`: re-bless only a golden whose diff is this shape.

## 6. Risks

- A box with a block size taller than the room: its end moves down by the extra and the next sibling may move to the next page (as Chrome does). Only boxes that already fit by their lines.
- Table-of-contents / flex / grid / `li` / nested shapes do not take this branch (measured right on main).
- `child_h` is layout px; the conversion to pt happens in the existing geometry code (no px/pt arithmetic added).
- Non-goals: floats (`x5`), F30f-j (glyph extents), F30f-i, F96, plain paragraphs.

## 7. Evidence

(Filled in after the implementation.)
