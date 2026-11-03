# F198j: a tall box in columns is still placed whole when a child has a relative offset or a negative margin, or a line is taller than a page

Date: 2026-11-02. Finding: F198j in `2026-10-15-m0-findings.md` (medium-high, warned). It extends F198 and F198b (both done). Their siblings F198c to F198i and F198k are open and not touched here. Fork: `subapavel/fulgur`, branch `htmltopdf/srgb-colours`, head `0feb944f` (the rev in `Cargo.lock`). Line numbers are from that head; re-read them before editing. No `css-spec-researcher` output exists, so §2 is from memory and unverified.

## 1. Problem

F198 cuts a column child that is taller than a page between its lines (`mc_box`). For any shape it cannot describe, `mc_box` returns `None` and the box falls back to the old path: one atom, placed whole, warned, and everything below the page edge is lost. The F198b vrt-validator found three more such shapes. The repros are in `out/scratch/f198bv/` (A4, 15 mm, Inter 14px/1.2, generators `c2.py` and `c5.py`). Chrome (version of the committed references) PDFs sit beside them as `*.chrome.pdf`.

| repro | shape | main words / total | Chrome |
|---|---|---|---|
| `r_rel` | clip card (`border; radius 12px; overflow:hidden`) of blue strip `<div>`s with `position:relative; top:3px`, each followed by a padded `<p>` | 330 / 2100 | all |
| `x_relshift` | the same card, its 25 `<p>` with `position:relative; top:-12px` | 354 / 2500 | all (read, pp. 1-2). Every paragraph is drawn 12 px up and overlaps the strip. A line that crosses a column or page edge after the shift is cut, and part of it shows on both sides (`w0672…` at the bottom of p. 1 and the top of p. 2). |
| `x_negmargin` | the same card, `<p style="margin-top:-6px">` | 360 / 2500 | all (not opened) |
| `im1200_pl`, `im1200_cl` | padded card (`_cl`: clip card) of text with six inline `<img>` 60 x 1200 | 298 / 1810 | all (read, pp. 1-3). Column 1 of p. 1 holds `w0000`–`w0298`. The line with the tall image sits alone at the top of column 2 and runs past the page. Chrome slices the overflow onto p. 2, column 2, where `w0299 [img] w0300 w0301 w0302` is printed on the baseline and overlaps the next tall line. |
| `edge1006`–`edge1012` | a 40 px wide image, 1006 to 1012 px tall | 1006 works; 1008 to 1012 are refused | — |

A realistic invoice hits the second cause through Tailwind's full-bleed card header, `overflow-hidden rounded-lg border p-4` with `<h3 class="-mx-4 -mt-4 bg-gray-100 px-4 py-2">`. That header has a negative top margin and a negative inline offset. Today fixtures 30 and 31 use no offsets and no negative margins, so no fixture shows this.

## 2. Spec (from memory, unverified)

- **CSS 2.1 §9.4.3 / css-position-3 §3 (relative positioning).** The box is laid out in normal flow and then shifted. The shift does not affect the layout of boxes that follow. When both values on an axis are set, `top` wins over `bottom`, and `left` over `right` (ltr).
- **css-break-3** (section unverified). Fragmentation happens on the static layout, and the relative shift applies to each fragment. Chrome matches this (`x_relshift`).
- **CSS 2.1 §8.3.1.** Negative margins collapse as the largest positive margin plus the most negative one.
- **css-break-3 §5.2.** Margins next to an unforced break are truncated. Unverified: whether this covers negative margins. Step 1 measures Chrome at a column top.
- **css-break-3 §2 and §4.1.** Line boxes are monolithic. Monolithic content taller than the fragmentainer may be sliced in paged media. Chrome slices it (`im1200`, and ADR-0004 measured the same). ADR-0004 decided that we do not slice a line in two.
- **css-multicol-1 §8.2.** Columns continue on the next page.

## 3. Where it belongs, and the cause

The fix belongs in the fulgur fork only. Blitz and Taffy lay the content out correctly, and fulgur's multicol fragmenter refuses it (F198). No upstream PR or issue (standing decision).

- **Relative offset.** Taffy adds the inset to the item's location (`taffy 7bbedd5 src/compute/block.rs:473-495`). That location is `final_layout`, so a child shifted by `top:3px` ends 3 px low, and the next sibling's `lead = cy - prev_bottom` is -3. `mc_box_at` (`pagination_layout.rs:1354`) refuses any `lead < -1.5` or `cx < -0.5` (`:1414`). A negative `top` makes the shifted child's own lead negative. `position:relative` without an offset adds nothing, which is why it works today. Note: `stylo_taffy` maps `static` to `Relative` (`convert.rs:211`) and passes the insets unconditionally (`wrapper.rs:73-81`). So Blitz also shifts `position:static; top:3px`, which is a Blitz gap and out of scope; step 1 confirms it.
- **Negative margin.** The same check (`:1414`) refuses a real overlap.
- **Tall line.** `mc_line_advances` (`:1229`, refusal at `:1234`) returns `None` for a layout with inline boxes when one line is taller than `fresh + 1`. The planner already places any atom taller than a column alone at a column top (`fill_multicol_columns`, `:588`). The refusal only existed to keep ADR-0004's shape on the old path, and that path loses everything after the line.

## 4. Fix

### 4.1 Tall line (`mc_line_advances`)

- Drop the refusal. The line becomes one `Line` atom and the planner places it alone at the top of a column.
- In the `Lines` arm of `record_multicol_row` (`:2122`), a piece holding a line taller than `fresh + 1` logs a warning, in the same style as the `Block` arm (`:2102`): "a line N px tall inside a multi-column container is taller than a page; it is placed alone at the top of a column and what is below the page edge is not printed".
- **We must not slice it.** ADR-0004 keeps every line in one fragment. Slicing would be a new decision with renderer work; it is filed as **F198l**. The cost: the words on such a line's baseline can fall below the page (24 words in `im1200_pl`). Everything after the line is kept (1810 minus about 24, against 298 today).

### 4.2 Negative margins (`mc_box_at`)

- Lead: `lead = if lead >= -1.5 { lead.max(0.0) } else { lead }`. Inline position: `x = if cx >= -0.5 { cx.max(0.0) } else { cx }`.
  - **Why the two-band form:** every box described today keeps its exact arithmetic, so the bytes do not change.
- Refuse only when either holds:
  - the child starts above the top of the previous item's last atom: `cy < prev_bottom - mc_item_last_unit(prev) - 0.5`, or `cy < -0.5` for the first child (measured from the border-box top);
  - `cx < -left - 0.5`: the child sticks out past the box's own border.
- **Why these refusals:** with them, the cursor in `fill_multicol_columns` never moves above an atom already placed in the column, so it never goes above the column top. The F198b invariant also holds: every item starts inside its box's border box, so clip pieces on one page do not overlap.
- New helper `fn mc_item_last_unit(item: &McBoxItem) -> f32`, the mirror of `mc_box_first_unit` (`:537`).
- At a column top the negative gap is dropped like any other gap (`lead_trail`, `:369`; css-break-3 §5.2).

### 4.3 Relative offsets: plan the static layout, paint the shift

```rust
/// Taffy's relative offset of an in-flow block child (block.rs:473-479): left wins over -right against the parent's
/// content-box width; top wins over -bottom against 0 (Taffy's basis). CSS px; [0, 0] when every inset is auto.
fn mc_relative_offset(doc: &BaseDocument, node: &blitz_dom::Node) -> [f32; 2];
enum McBoxItem { Lines { /* … */ rel: [f32; 2] }, Leaf { /* … */ rel: [f32; 2] }, Box { /* … */ rel: [f32; 2] } }
struct McBox { /* … */ slack: [f32; 2] }  // the largest shift of anything inside, relative to the box
struct McFrame { /* … */ shift: [f32; 2] } // accumulated: the boxes around it plus its own
struct AtomCtx { /* … */ shift: [f32; 2] } // accumulated, PLAIN = [0, 0]
fn push_box(&mut self, node: usize, b: &McBox, parent: Option<u32>, x: f32, gap: f32, shift: [f32; 2]);
pub struct PaginationGeometry { /* … */ pub piece_slack: [crate::units::Px; 2] } // zero unless set
```

1. **Static positions.** `mc_box_at` reads `rel = mc_relative_offset(doc, child)` and uses `cy - rel[1]` and `cx - rel[0]` for `lead`, `x` and `prev_bottom`. It follows the pattern of `resolve_viewport_cb_location` (`:9561`): Stylo computed insets, CSS px basis. The rel goes on the item, not inside the child's own `McBox`. **Why:** a framed inline root's lines inherit the shift through the chain and would otherwise get it twice.
2. **Shift at placement.** `push_box` accumulates the shifts (the call sites at `:523`, `:783` and `:786` pass `[0, 0]` at the top level). `fill_multicol_columns` adds `ctx.shift` to the `Leaf` and `Lines` piece positions (`:682-699`) and `frame.shift` to the `Frame` positions (`:607`, `:726`). It adds the shift only when it is non-zero. **Why:** `-0.0 + 0.0` turns into `+0.0`, and every existing plan must stay bit-identical. `used`, `cursor` and the frame heights stay static, because a shift does not move later boxes (CSS 2.1 §9.4.3). Everything downstream follows the pieces: the `record_multicol_row` fragments, `record_line_inline_boxes`, and the slice origins in `convert_paged_multicol_slices` (`convert/mod.rs:1003-1009`).
3. **Clip attribution.** F198b's `draw_under_clip` paints only what starts inside the piece's border box (`render.rs:2757-2760`, filters at `:2102` and `:2918`). In `x_relshift`, a line shifted 12 px above an open-top piece would fall outside and be lost.
   - The `Frame` arm sets `piece_slack` to `[min(slack_x, gap / 2), slack_y]`.
   - `draw_under_clip` widens the window by that slack.
   - **Why this is safe:** two pieces of one box on one page are in different columns, so the vertical slack cannot attribute an item twice, and capping the horizontal slack below half the gap keeps the windows of neighbouring columns apart. The clip path itself is not changed. A shifted line is clipped at the piece edge, as Chrome does.

## 5. Acceptance

### Ours: `crates/htmltopdf/tests/multicol_boxes.rs`

The helpers and the `check_shape_with` asserts are F198's: every `wNNNN` once in column-major order, then `AFTER`; Chrome's page count; the first word of each column within one line. Chrome's numbers come from step 1.

- **L** `a_relatively_offset_child_keeps_its_offset_and_every_word`, on `r_rel` and `x_relshift` (red).
  - Oracle (b), as in F209: the same body without the offsets prints every word on the same page and in the same column, and its x and y differ only in the shifted elements' words, by exactly the offset (±0.01 pt).
  - The blue strip's top is 2.25 pt below its unshifted position (±0.1 pt).
  - The `no yMax past the content box` check is widened by the offset, because Chrome draws shifted content past the edge too.
- **M** `a_child_with_a_negative_margin_is_cut_like_the_others`, on `x_negmargin` and a Tailwind full-bleed header card (`margin:-16px -16px 12px` inline) (red).
  - The strip's background spans the card's inner border edges (±1 pt).
  - Its top sits on the card's top border, in column 1 of page 1 only.
- **N** `a_line_taller_than_a_page_is_placed_alone_and_the_rest_flows_on`, on `im1200_pl` and `im1200_cl` (red).
  - Every word not on a tall line is printed once, in order.
  - The missing words are a subset of the words on the tall lines, at most 5 per tall line.
  - Chrome's page count, if step 1 confirms it.
  - `edge1008`, `edge1010`, `edge1012`: every word once.
- **G** guards, green on main:
  - shapes A–K are unchanged;
  - `position:relative` without an offset prints the same words and the same xy as no `position`;
  - a child pulled above the previous paragraph's last line keeps the old path and the warning.

### Fork

- `mc_relative_offset` truth table: auto; `top:3px`; `bottom:3px`; top plus bottom; `left:5%`; `top:10%` gives 0; static plus `top` mirrors Taffy.
- `mc_box`:
  - a shifted child gets a static `lead` and `x` and carries its `rel`;
  - the full-bleed first child is described;
  - an overlap above the previous atom, or past the left border, gives `None`;
  - in `mc_box_cuts_an_inline_root_with_inline_boxes_between_its_lines`, the tall-line assertion (`:19920-19926`) flips to `Some`, with one advance.
- `fill_multicol_columns`: a shifted atom's piece is offset while `used` is unchanged; the existing plan tests stay unchanged.
- `render::tests`: in a cut clip card, a slice shifted 12 px above its piece is painted once.
- `render_smoke`: the `r_rel` shape, every glyph once (the coverage gotcha in fulgur's `CLAUDE.md`).

### Fixture `40-terms-card-strips-columns`

Built on fixture 31's recipe: Czech terms in `.terms{columns:2;column-gap:24px}`, as one card `overflow:hidden; border-radius:8px; border:1px solid #d1d5db; padding:16px`. It uses an inline `<style>`, because the compiled `tailwind.css` has no `-mx-*`, `-mt-*`, `relative` or `top-*`, and rebuilding it would move every fixture.

- Each section heading is a full-bleed strip: `margin:0 -16px 8px; padding:6px 16px; background:#f3f4f6`. The first strip also has `margin-top:-16px`.
- One heading is nudged with `position:relative; top:2px`.
- The card is taller than a page in one column, tuned in Chrome to 2 pages.

The PNG must show:

- strips running from border to border in both columns;
- the first strip flush with the top border and clipped to the radius (page 1, column 1 only);
- the nudged heading 2 px low;
- text in both columns of both pages.

`fixtures.rs` asserts the headings in order, the last word of each clause, the signature and 2 pages. Threshold: the measured worst page + 1. Main's "before" is the page and word count.

### Byte gates (1 and 4 rayon threads, `out/scratch/f30fl/gate.sh`)

- **Identical:** fixtures 01–37 (plus 38 and 39 if merged by then), the 7 corpus documents, the F193 sweep (48 files), and every `f198v`, `f198pdf`, `f198b` and `f198bv` repro except the ones listed below.
  - **Reason:** none of them has a cut-box child with a non-auto inset, a lead below -1.5 px, an `x` below -0.5, or a line taller than a page in a cut box. Step 1 checks this with a scratch dump. Every other path keeps its arithmetic (§4.2, §4.3(2)).
- **May move:** `r_rel`, `x_relshift`, `x_negmargin`, `im1200_*`, `edge1008`–`edge1012`, and fixture 40.
- **Fork VRT:** compare the `.actual.pdf` files before and after; do not re-bless on this host.

## 6. Steps (each ≤ 1 day)

1. **Gate (½ day).**
   - Chrome (version of the committed references) on the repros, the full-bleed card, `bottom:3px`, and `static; top:3px`: words per page, first word per column, the strip's y, the column-top truncation of a negative margin.
   - A scratch dump in `mc_box_at` naming the refusal for each repro; baselines for §5.
   - **Proceed** only if the sole refusals are those of §3.
   - Otherwise, ship the causes that are clean and file the rest as F198j-b.
2. Red tests, fixture 40 and its Chrome reference; commit.
3. Fork §4.1 with its tests; byte gate; N green.
4. Fork §4.2 with its tests; byte gate; M green.
5. Fork §4.3 with its tests; byte gate; L green.
   - If the clip slack cannot be kept exact, ship (1)–(2) without the shift for clip cards, file it, and record that in §8.
6. Ours:
   - lock bump; `compare.py` and the PNGs of fixture 40; `bench.py --check`;
   - findings: F198j done, F198l filed; `docs/upstream.md`;
   - `vrt-validator`; `pdf-reviewer` (`render.rs` is on the writer path).

## 7. Risks and non-goals

- **Rounding.** Blitz rounds `final_layout`; the insets are not rounded. A fractional offset leaves an error of up to 1 px in the static position, which is inside the existing 1.5 px tolerance.
- **Shifted content past an edge.** It is drawn once, past the column or content edge by the offset. Chrome slices it and draws part of a line twice. We keep "every word once" (ADR-0004's reasoning).
- **Words on a line taller than a page** stay lost and warned (F198l).
- **Double paint** through the slack needs a box wider than its column plus a shift plus a clip (F198d). The render test and the "every word once" checks would catch it.
- **Non-goals:**
  - the relative offset and margins of a top-level column child (F30d-b, F198d);
  - the frame bottom of a box whose last child has a negative bottom margin (it ends `inset_bottom` below that child; F198d);
  - `position:static` with insets (Blitz);
  - `sticky` (mirrors Taffy);
  - F198c–i, F198k, F173(f), F174c.

## 8. Step 1 and 2 results, and where the plan does not fit the current fork (2026-11-03)

Measured on main `c8f2e07` (fork `ffdf0c49`, which has F30c-n, F30c-o3, F215 and F96 on top of the plan's `0feb944f`) with Chrome (version of
the committed references). Probes are not committed; the shapes of the tests are, and `dump_chrome_inputs` writes their HTML.
The fork was not edited, so the scratch dump of the refusals in `mc_box_at` (step 1) was **not** done: each cause is isolated by a control
instead (the same HTML with the one property removed prints every word), which shows what the refusal is caused by but not that the
refusal is the `mc_box_at` check named below.

### 8.1 Repros: Chrome and main

Words per page as column 1 + column 2; "main" is the binary of main with the old path (one box placed whole, warned).

| shape | main | Chrome | the plan said |
|---|---|---|---|
| `r_rel` (strips `top:3px`, with icons) | 2 pages, 330 / 2100 | 4 pages, all 2100 | 330, all: as planned |
| `x_relshift` | 2 pages, 354 / 2500 | 4 pages, 2536 printed (36 words twice: a line cut by a column or page edge shows on both sides), first words 0, 330 / 672, 1012 / 1367, 1700 / 2042, 2272, `AFTER` p. 4 at 549.3 pt | "all (read, pp. 1-2)": it prints some twice, which the plan only noted for the cut line |
| `x_negmargin` | 2 pages, 360 / 2500 | 4 pages, all 2500 | 360, all: as planned |
| `im1200_pl` / `_cl` | 298 / 300 of 1810 | 7 pages, all 1810. Page 1 column 2 is empty: the tall line stands alone in it and its words (4 to 6, on the baseline) are printed in column 2 of page 2 | as planned (298, 7 pages) |
| `edge1006` | 6 pages, all 1200, like Chrome | 6 pages | works: as planned |
| `edge1007`, `1008`, `1009`, `1010`, `1011`, `1012` | 2 pages, 198 / 1200 (233 for the test's variant), warned | 6 pages, all 1200; 1011 and 1012 print 25 words twice | the plan lists 1008 to 1012; 1007 and 1011 refuse as well, so the threshold is a line taller than `fresh + 1` = 1010.1 px: 1000 px (`t_im1000`) works |

Test shapes (`multicol_boxes.rs`, F198j section): `relstrips` 2100 words (Chrome 4 pages, first words 0, 306 / 612, 924 / 1240, 1543 / 1848, 1974,
`AFTER` p. 4 at 338.6 pt), `relshift`, `negmargin` (4 pages; 0, 342 / 690, 1036 / 1391, 1737 / 2084, 2290; `AFTER` 502.8 pt), `fullbleed` (4 pages; 0, 330 /
671, 1012 / 1366, 1718 / 2071, 2290; 508.8 pt), `im1200_*` (7 pages), `edge1008` / `1010` / `1012` (6 pages). Main prints 330, 354, 360, 347, 298 / 300,
233 words of those.

### 8.2 Does each shape still lose words on main after F96 and F215?

Yes, all three, with the same numbers as before. F96's `block_slices_at_breaks` excludes everything below a multicol container (its doc
comment names F198's `piece_edges`) and F215 only moves headings; neither touches `mc_box`.

Controls (each prints every word on main, 4 pages for the 2500-word ones): `relshift` without `top` (the same text, 330 / 342 / ...),
`position: relative` alone, `margin-top: 6px`, the full-bleed card with its margins 0, the 1000 px image.

- Relative offset: `top:3px` on the strips (`s_static3`, `r_rel`), `bottom:3px` (the first child's gap is -3), `left:-8px` (`cx < -0.5`) and
  `top:-12px` all lose words. Both halves of the full-bleed header refuse on their own: `margin:0 -16px` (cx -16) and `margin-top:-16px`
  (gap -16): 275 and 281 of 2400 words, both together 281.
- Negative margin: `margin-top:-6px` refuses (360 words), `-40px` (the "pulled up" guard) too (418).
- Tall line: refuses above 1010.1 px.

### 8.3 What the plan missed

1. **Positive offsets are not refused today; they are baked into the flow.** `top:12px` on every paragraph passes: the first one's gap grows by
   12 px, the others keep theirs, so every word is printed, but the column breaks fall 12 px later than Chrome's (first words 324 / 666 / 1006 where Chrome
   and the flat layout have 330 / 672 / 1012), and `left:8px` is drawn by moving the item's `x` (right, 6 pt like Chrome's). `position:relative; top:4px;
   bottom:20px` (top wins) and `left:5%` behave the same way, `top:10%` is 0 (also Chrome's). §4.3 changes all of these to "static layout plus a painted
   shift", which is Chrome's behaviour, so their bytes **move** (6 words per column back to Chrome's). §5's byte gates must list them under "may move"
   (whether a committed fixture, a corpus document or a sweep file has one was **not** checked here: step 1's scratch dump has to say, and the step
   that does §4.3 re-checks with the byte gates). The plan's sentence "`position: relative` without an offset adds nothing, which is why it works today" is right and is the
   guard `guard_position_relative_without_an_offset_changes_nothing`; a uniform offset also "works", wrongly.
2. **`position: static; top: 3px` is shifted by Blitz**, as §3 suspected: the first word is 2.25 pt low against Chrome's flat layout. In a cut box it
   refuses like `r_rel` (330 words). Out of scope (non-goal list), but `mc_relative_offset` ("static plus `top` mirrors Taffy") will paint the shift where
   Chrome draws none; the test for it is not written.
3. **Chrome prints shifted lines twice at an edge** (2536 words for `x_relshift`; 2506 for `top:12px`): "every word once" is our rule, not Chrome's.
4. **Line numbers.** The multicol code of `pagination_layout.rs` is untouched between `0feb944f` and `ffdf0c49` except for a shift of +1 line
   around `mc_box_at` and nothing else in the region: `McBox` 274, `McFrame` 318, `AtomCtx` 343, `lead_trail` 370, `push_box` 464 (call sites 524, 784, 787
   instead of 523, 783, 786), `mc_box_first_unit` 538, `fill_multicol_columns` 589 (the `McPiece` pushes at 608, 683, 691-694, 727), `mc_line_advances` 1230
   (the refusal at 1235), `mc_box_at` 1355 (the `lead < -1.5 || cx < -0.5` check at 1415), `record_multicol_row` 2047, `PaginationGeometry` 2419 (it derives
   `Default`, so a `piece_slack` field needs no new constructor; F96 added `block_slice` next to `piece_edges`). `resolve_viewport_cb_location` moved from 9561
   to 10644. In `render.rs` the lines the plan names are exact (`draw_under_clip` 2624, the window at 2757-2759, the filters at 2102 and 2918) and
   `convert_paged_multicol_slices` is at `convert/mod.rs:943` with the slice origin at 1003. F96's `frame_fragment` branch (`block_slice`) does not apply to pieces:
   `piece_edges` is checked first (`render.rs:3755`).
5. **Test details that differ from §5.** The oracle needs unique words, so the strips print `S000` ... and the icons of `r_rel` are left out; the strip
   background is measured at 288 dpi to ±0.3 pt, not ±0.1 pt (poppler snaps the edge of a filled rectangle to the pixel grid: 2.0 pt at 144 dpi for the true
   2.25), the shifted text itself is held to ±0.01 pt by the oracle; the words on a tall line are checked against the sheet's bottom (841.9 pt), not the
   content box, because the baseline of an `edge` line is, by arithmetic and not yet measured, 2 to 6 pt below the content box and still on the sheet; the warning guard is its own test file
   (`multicol_shifted_warning.rs`: the logger is global to the process); the numbers of the older F198b tests in `multicol_boxes.rs` stay Chrome 141's.
6. **Open claim.** §4.1's "everything after the line is kept" and the `edge1008` to `edge1012` "every word once" are asserted by the red test and not
   yet shown: if the engine keeps a line alone at a column top the baseline words land at about 798 to 806 pt (estimated: image bottom plus the descent), on the sheet, so they should print.

### 8.4 State after step 2 (red)

Red on main: `a_relatively_offset_child_keeps_its_offset_and_every_word` (330 of 2100), `a_child_with_a_negative_margin_is_cut_like_the_others`
(360 of 2500), `a_line_taller_than_a_page_is_placed_alone_and_the_rest_flows_on` (1512 missing), `terms_card_strips_columns_invoice_prints_every_clause`
(fixture 40). Green on main (guards): `guard_position_relative_without_an_offset_changes_nothing`, `guard_a_child_pulled_above_the_previous_line_keeps_the_old_path`
(418 of 2500, 2 pages), `a_child_pulled_above_the_previous_line_is_warned_about_and_a_plain_card_is_not`, and every older test of the files.

Fixture `40-terms-card-strips-columns` on main: 2 pages like Chrome's but 290 of 945 words (page 2 holds 9), compare 7.8 / 8.6 %, one warning. Its
threshold in `scripts/compare.py` is 0.096 (the red state's number + 1 point) and drops to the earned number + 1 point with the fix.

### 8.5 What was built (steps 3 to 5, fork `58c78ae2`, 2026-11-03)

The three causes went in as one fork commit on top of F213's `329b4e43` (the fork branch is shared, one commit at a time), with the code where §4
puts it. Differences from §4:

- **§4.1.** `mc_line_advances` lost its `fresh` parameter along with the refusal. The warning is in the `Lines` arm of `record_multicol_row`
  and reads "a line Npx tall inside a multi-column container is taller than a page; it is placed alone at the top of a column and what is below the
  page edge is not printed" (the `Block` arm's "taller than a page" substring, so one test finds both).
- **§4.2.** As written, with `above_previous` computed from `items.last()` (the first child: `cy < -0.5`) and `cx < -left - 0.5`. A first child with a
  negative `margin-top` in a card without a border or padding does not get here (the margin collapses through the parent); the unit test uses a
  border. At a column top the negative gap is dropped like any other: ours starts the columns of `x_negmargin` at Chrome's first words (0 / 342 / 690 /
  1036, Chrome the same), so the measurement §6 step 1 asked for ("column-top truncation of a negative margin") agrees with css-break-3 §5.2.
- **§4.3.** As written: `mc_relative_offset`, `McBoxItem::rel` on all three variants, `McBox::slack` (the sum of absolute offsets down the chain), `McFrame::shift`,
  `AtomCtx::shift`, `push_box(.., shift)` with `add_shift`, `shifted(v, 0.0) == v` bit for bit, `PaginationGeometry::piece_slack`, the window widened in
  `draw_under_clip`. The horizontal slack is `min(slack_x, gap / 2)`. The render test `draw_under_clip_paints_a_slice_shifted_above_its_piece_once`
  is red without the slack (116 of 375 text operators), so the slack is exact for the clip card and §6 step 5's fallback was not needed. A shifted line
  is clipped at the piece edge (the clip path is the padding box), as Chrome does; the words are in the PDF text once.
- **Tests added in the fork:** `mc_relative_offset_is_the_offset_taffy_adds_to_a_block_child` (11 insets, including `static` + `top` and `top: 10%` = 0),
  `mc_box_measures_a_shifted_child_at_its_static_place`, `mc_box_describes_a_full_bleed_first_child_and_a_child_pulled_over_the_one_before`,
  `plan_paints_a_shifted_atom_at_its_offset_and_plans_it_at_its_static_place`, `plan_places_a_line_taller_than_the_column_alone_at_the_top_of_one`, the flipped
  tall-line assertion of `mc_box_cuts_an_inline_root_with_inline_boxes_between_its_lines`, the render test above and `render_v2_smoke_multicol_card_with_shifted_children_negative_margins_and_tall_lines_prints_every_word_once`.
  The fork's `cargo test -p fulgur` is green (2531 lib, 337 `render_smoke`, 45 test binaries) and clippy `-D warnings` is clean.

**Results (release binary, repros and the plan's shapes).** Words printed, main -> now: `r_rel` 330 -> 2100 / 2100, `x_relshift` 354 -> 2500, `x_negmargin` 360 -> 2500,
full-bleed card 347 -> 2500, `im1200_pl` / `_cl` 298 / 300 -> 1783 of 1810 (27 on the baselines of the six tall lines are lost, 4 to 5 per line; Chrome prints them),
`edge1007` to `edge1012` 198 -> 1200 / 1200; pages like Chrome's everywhere (4, 4, 4, 4, 7, 6). The words of the shifted shapes are where the unshifted ones are
(oracle b, ±0.01 pt), the strips 2.25 pt lower. Fixture 40: 290 -> 945 words, compare 7.8 / 8.6 % -> 9.4 / 6.8 %.

**Byte gates** (1 and 4 rayon threads, baseline: the F213 binary, fork `329b4e43`, whose fixtures 01-42 equal main's): fixtures 01-39, 41, 42 and the 7 corpus
documents are identical, only fixture 40 moves. The sweep (498 files: the 24 F193 `c_mc*` cases of `f183v6/s`, the `f198`, `f198b`, `f198pdf`, `f198v`,
`f198bv` repros, and this plan's probes): 48 outputs move, every one a shape of §1 or a probe with an inset (`edge*`, `im1200_*`, `r_rel`, `x_relshift`,
`x_negmargin`, `fb_*`, `p_*`, `s_*`, `t_edge*`), none of the others. 1 and 4 threads give the same bytes.

**Against Chrome (no changed case may end further away).** For each of the 48 outputs, Chrome re-rendered (the committed references' version): words missing
(of those Chrome prints), page-count difference, distance of the first word of every column. Every case that lost words now has fewer missing and the page
count of Chrome's; the first-word distance falls from "column empty" to 2 to 60 words (0 to 12 in a column on the shapes that fit). One case moves both ways:
`p_top_pos12` (`top: 12px` on every paragraph, which main baked into the flow): its columns started 6 words early; they are now the unshifted layout's, six
words late in three columns where main happened to be exact (the flat layout is 6 words from Chrome there too), in total 18 words from Chrome's against 24.
No word is lost in either.
