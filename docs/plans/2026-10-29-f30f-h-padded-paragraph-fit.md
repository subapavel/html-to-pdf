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

### What was built

- Fork `17af1637` (`fragment_pagination_root`, fitting branch): `fit_top` / `fit_page` are taken before `fragment_inline_root`; when it returns one fragment on the same page, the box is not a float and `insets > 0.0 || child_h > lines_end + LINE_BOX_SLACK`, the cursor is `fit_top + child_h`. The second condition catches `height` / `min-height` taller than the lines (`min-height` is not `block_size_constrained`, so that predicate alone was not enough). Fork `02292ef8`: floats keep the old cursor (first version moved `x5`, a float, from 118.6 to 133.6 pt while Chrome has 43.8: a float is out of flow, separate defect, finding F202).
- Fork tests (red first with the fix switched off: the three new-behaviour tests failed, the plain-paragraph guard stayed green; the float guard was added with the float exemption): `a_body_level_box_that_fits_ends_at_its_border_box` (7 shapes and two cards), `a_padded_card_that_fills_the_page_pushes_what_follows_to_the_next_page`, `a_plain_body_level_paragraph_keeps_the_end_of_its_lines`, `a_floating_padded_box_keeps_the_cursor_it_had`, `a_padded_body_level_paragraph_that_fits_is_not_sliced` corrected from the pinned wrong 100 px to 120 px; `render_smoke_padded_body_level_paragraph_that_fits_is_followed_below_its_frame`.
- Ours `padded_paragraph_fit.rs`, 7 tests: 6 red on main for the expected reasons (AFTER 118.6 against 133.8, two lines 73.6 / 88.8, two cards 138.1 / 168.3, a card that fills the page leaves AFTER over L9), the guards test green on main; all 7 green with the fix.
- Fixture 32 and its threshold (below).

### Chrome against the fix (`out/scratch/f30fh/h`, 33 shapes; `q`, the fit-or-span pairs)

Every shape of §3 is within 0.2 pt of Chrome (line box against glyph box), e.g. a 133.6 / 133.8, c 88.6 / 88.8, g 151.6 / 151.8, h 177.1 / 177.3, l 204.1 / 204.3, s 168.1 / 168.3, t 156.1 / 156.3, u 193.6 / 193.8, x1 156.1 / 156.3, x2 193.6 / 193.8. The pairs under an 800 px spacer: 8 lines fit, AFTER 778.6 (Chrome 778.8; main 763.6); 9 lines fill the page to 793 pt, AFTER moves to page 2 at 43.6 (Chrome 43.8; main left it over L9 on page 1 at 778.6); 10 lines span (F30f-d), AFTER 22.5 pt below the last line in fit and in span. Guards that were right stay: b, o (one line), r (nested), x3, x4 (clip box `height:60px`), x6 (flex item), x7 (`li`), y1 (`break-inside: avoid`); `x5` (float) is as on main.

### Gates

- Fork (`02292ef8`): lib 2420 passed; the 43 integration binaries one at a time all ok (`render_smoke` 326); `clippy -p fulgur --all-targets -D warnings` and `fmt --check` clean. `fulgur-vrt` (`f183-vrt.sh`): 35 of 64 differ, as on the base (env failures); 34 of the 35 `actual.pdf` are byte-identical to the base run. **One moves: `basic/box-shadow`** (not re-blessed, below).
- Byte gate (`f183-gate.sh`, 1 and 4 threads identical; also `render-fixtures.sh` outputs compared with `cmp`): against main's binary only `32-note-card` (new) differs; fixtures 01-31 and the 7 corpus documents are byte-identical. Fixtures that hold a padded multi-line paragraph that fits did not exist before 32.
- `compare.py`: all 32 ok, no number moved from its threshold comment; **32: 5.0 % on main -> 3.8 %** (1 page like Chrome's; threshold 0.048 = 3.8 + 1). What the side-by-side shows: on main "PLATEBNÍ ÚDAJE" and the first bullet sit over the card's bottom border and last line ("Faktura je osvobozena ...", heading at 501.1 pt); now the card closes with its bottom border and radius, `mb-6` of white space, then the heading at 526.6 pt (Chrome 526.8) and the bullets, signature line below. The diff panel shows only the 1 px text drift every fixture has.
- Sweep (`out/scratch/f30fh/sweep.py`: main's binary against the final one, 2901 cases from `f30fbv*`, `f30fd/t16`, `f30fd/v29`, `f30fdv`, and the new `f30fh/{h,p,q}`; Chrome beside each): 189 changed, 2712 byte-identical. `AFTER` within 1 pt of Chrome on the same page: 1 -> 154; page counts different from Chrome's 23 -> 3; summed `AFTER` distance 44006 -> 6263 (page misses count 1000 pt); words-per-page distance 614 -> 574; line-rows-per-page distance 100 -> 55; 60 dpi ink counts: 74 cases show a larger count difference, by 1 to 14 pixels of 500-1000 (not a position measure: they are cases whose text now sits at Chrome's y to 0.2 pt).
- **Exceptions: the 17 cases whose `AFTER` is farther from Chrome than on main (named).**
  1. `f30fbv/sp/c118, c134, c6, c70` (4): our text wraps to 9 lines where Chrome's wraps to 8 (a word wraps differently, sub-pixel advances); `AFTER` is one line (plus the bottom inset) below Chrome's, where main's overlap had cancelled the extra line by accident.
  2. `f30fbv4/sw/notesD.s927 ... s954` (10): `overflow:hidden; height:120px; padding:12px` clip card at the page bottom; Chrome cuts the card over two pages (lines 2 / 3), ours moves it whole to page 2 (as main does: F30f-i / the start rule, not this change); `AFTER` is now below the whole card (153.1 pt) instead of over it (main 106.6; Chrome 91-111 because its card started on page 1).
  3. `f30fdv/a/exact_n44_pb22`, `exact_n45_pb20`, `exact_n45_pb21` (3, the only page-count changes: 1 page in Chrome and on main, now 2): a card whose text ends in a `<br>` at an exact fit. The layout gives the trailing `<br>` an empty line box (F66f: `f30fh/z/z1`), so the card's box, and its frame on main too, is one line taller than Chrome's; `AFTER` (and, at an exact fit, the page break) now follows that frame instead of overlapping it. A card with a trailing `<br>` that does not sit at a page end is 15 pt lower than Chrome (main 0.2 pt, by cancellation); a plain or nested paragraph with a trailing `<br>` already was (z2, z3). Filed in F66f.
- html-to-pdf: `cargo fmt --all --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --release --locked`: 76 result lines, 869 passed, 0 failed (`--no-fail-fast`; `padded_paragraph_fit` 7, `padded_paragraph_split` 11, `clip_boxes`, `wrapper_line_split`, `nested_paragraph_split` unchanged). The first full run stopped at one failure: the F30f-d guard `a_padded_paragraph_that_fits_its_page_is_laid_out_as_before` had pinned main's `AFTER` at 333.1 pt, over the card's last line (345.1 pt); Chrome puts it at 357.3 pt and ours is now 357.1 (`out/scratch/f30fh/g/fit300`), so the pinned number was the bug and the guard now pins 357.1 (lines unchanged). Release build `scratchpad/htmltopdf-f30fh`. `bench.py --check` ok (fixture 32 median 25 ms; b200-text 862 ms, b200-mixed 873 ms; b200-items 1.67 s on this loaded host, the accepted-exception line says "not met on this machine" as it did on main's binary; the corpus is byte-identical to main's, so the render path is the same).

### The one fulgur-vrt golden that moves

`basic/box-shadow` (seven `.box` divs: `width:120px; height:40px; padding:10px; margin:0 0 20px`; the sixth, `negative spread`, wraps to two lines): the golden puts the seventh box's text at 368.1 pt, over the sixth box's end and its margin (that box ends at 360 pt, the margin makes 375: the rectangle of the seventh was drawn at 359.25); with the fix it is at 382.1 pt, the box at 375..420 pt. Chrome (`out/scratch/f30fh/bs/box-shadow.chrome.pdf`, letter page, the text on one line there): the seventh box's text at 382.14 pt, i.e. where the fix puts it and a 14.0 pt move away from the golden, the other six boxes are at the same y as before. This is the `height`-taller-than-lines shape of this finding on a one-page document. It is not re-blessed from this host: the host's VRT run already differs from the committed goldens for 35 of 64 fixtures including this one (its fonts and line metrics are not the goldens'), so a regenerated PDF would bake this host's font into the golden. The golden needs regenerating on the pinned font environment (`FULGUR_VRT_UPDATE=1`, `FONTCONFIG_FILE=examples/.fontconfig/fonts.conf`).

### Not verified

- The fork's `fulgur-vrt` golden for `basic/box-shadow` was not regenerated (above); fork CI was not run.
- Float, `display: inline-block` wrappers, table cells and flex / grid items are untouched; their shapes were measured only where listed in §3.
- A paragraph with `height` smaller than its lines and `overflow: visible` (text spilling out) now ends at its box (`fit_top + child_h`) per CSS 2.1 §10.5; checked in no Chrome case beyond `x4` (a clip box) and the fork tests.
- The Tailwind `h`, `i` shapes use the compiled sheet; fixture 32 is the committed proof for it.
