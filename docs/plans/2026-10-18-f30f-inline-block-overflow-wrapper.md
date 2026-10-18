# F30f — A long paragraph in an `inline-block` or `overflow: hidden` wrapper prints only its first page

Date: 2026-10-18. Finding: F30f in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after `e0aed2e`), carried via `[patch.crates-io]`. Fork line numbers are at `e0aed2e`, in
`crates/fulgur/src/pagination_layout.rs` unless another file is named; Blitz references are to the fork at `52b22ef`
(`/home/user/blitz`, `packages/blitz-dom/src/`).

F30e (`display: table` wrappers) has the same cause and stays a separate finding and PR. §3 ends with what F30e reuses
from this change and what it still needs.

## 1. Problem

A paragraph taller than a page whose parent is an `inline-block`, or a box whose `overflow` is not `visible`, is placed
as one fragment. Lines below the page are drawn below it. What lands in the bottom margin is printed and the rest is lost.
F30's warning (`warn_unprinted_inline_lines`, `:4949`) fires, but the text is still gone. The bug is pre-existing and the
same on main.

The finding's files (`/tmp/claude-0/vrt-f78/ab/`) are gone. Recreate them under `out/scratch/f30f/` (git-ignored) with
the F30g setup:

- A4, `@page{size:A4;margin:15mm}` (content box 680.3 × 1009.1 px);
- Inter 16 px, `body{margin:0}`, `p{margin:0}`;
- `W` = words `word%05d` 0..3999 joined by single spaces;
- render with `RenderOptions::default()`, or with the CLI plus `--margin-mm 15` (the CLI ignores `@page`, F6).

The repros:

- **R1 `ibl_normal.html`:** `<body><div style="display:inline-block;width:100%"><p>W</p></div></body>`, with
  `line-height: normal`.
  - Ours: 386 words, 1 page.
  - Warning: "a paragraph of 569 lines is placed whole and ends 9917px below the content box".
  - `ibl_1.3.html` is the same with `p{line-height:1.3}`.
- **R2 `ovfhid_normal.html`:** the same markup with `<div style="overflow:hidden">`. Same numbers.
- **Recreation check:** the recreation is right when 386 words / 569 lines / 9917 px come out, ±1 line. If not, adjust
  the margins until they do and record the markup in §7.

Causes, from reading (step 1 confirms them):

1. **The splitter excludes these wrappers.**
   - `line_split_candidate` (`:4643-4671`) returns `None` when the parent is `is_atomic_inline_container_node`
     (`blitz_adapter.rs:1058`: outside inline, inside not flow, i.e. inline-block / inline-flex / inline-grid /
     inline-table) or `clips_overflow` (`:6194-6205`: any non-visible overflow on a block-level or BFC box).
   - The child-side `abspos_line_split` (`:4679-4698`) excludes the same for a wrapper that holds its text directly.
2. **So the `<p>` is placed whole.**
   - In `fragment_block_subtree` (`:3009`) it gets no split (`:3590`).
   - The gate does not enter it: its children are text, and `would_split_block_subtree` (`:2482-2500`) sees no height.
   - It is placed whole at `:4135-4154`, right after the warning at `:4134`.
3. **The wrapper itself is entered, but never advances a page.**
   - Both gates (`:1867-1892`, `:3571-3586`) enter it because its `<p>` is taller than a page.
   - No page advance happens inside, so the wrapper closes as one fragment of about 11 000 px.
   - The clip box is clipped to that one fragment (`render.rs:2077-2162`), and the page box cuts off the rest.
4. **Inline-block only, render side.**
   - An inline-block's subtree is in `Drawables.inline_box_subtree_skip` (`convert/inline_root.rs:579-606`). The main
     loop skips it (`render.rs:769`).
   - Only the `LineItem::InlineBox` arm of its *carrier* paints it (`paragraph.rs:913-998`; the carrier is the paragraph
     whose line holds it). It does so on the page that line is drawn on, with that page's fragment (`:943-946`).
   - Nothing paints the box on later pages. So a body-level inline-block with direct text, which the body line path
     (`:1676-1749`, no exclusions) already splits, most likely prints page 1 only and gives no warning. Step 1, P1t.
5. **Blitz turns the parent into an inline root.**
   - A block container whose children are all inline-level becomes an inline root (`layout/construct.rs:244-295`). An
     inline-block is not `is_or_contains_block` (`node/node.rs:186-218`).
   - The inline-block becomes the parent's embedded box: it is in `layout_children`, with `layout_parent` set at
     `construct.rs:691`.
   - So the carrier of R1 is `<body>`. After a heading it is an anonymous block; inside a `div` it is that `div`.
6. **The carrier line goes missing if the carrier is split.** Suppose the carrier is split over several fragments and
   has no `line_slices`. Then `paragraph_lines_for_page` (`render.rs:3900-3932`) finds its one line taller than every
   fragment and returns `None` on every page. Once the box inside splits, the carrier's line would vanish on page 1 too.

## 2. Spec (cited from memory, unverified; css-spec-researcher not run)

- **css-break-3, "monolithic" (§2 / §4.1).**
  - Content that cannot be fragmented is monolithic.
  - In addition, "UAs may consider as monolithic: any elements with `overflow` set to `auto` or `scroll`; any elements
    with `overflow: hidden` and a non-auto logical height (and no specified maximum logical height)".
  - So `hidden` with auto height and `clip` are fragmentable. `auto`, `scroll` and height-limited `hidden` are the UA's
    choice.
- **css-break-3 §4.1 (line boxes).**
  - A line box contains no break point.
  - "inline-block and inline-table boxes (and other inline-level display types that establish an independent
    formatting context) may also be considered monolithic". UA's choice.
- **css-break-3 (monolithic overflow).** In paged media, monolithic content taller than the fragmentainer may be
  sliced at an arbitrary point instead of being lost. WPT `css/css-break/monolithic-overflow-*-print.html` (the fork
  cites one at `:2028-2035`).
- **css-break-3 `box-decoration-break` / css-overflow-3.**
  - A fragmented box clips each fragment to that fragment's padding box.
  - With `slice` (the initial value) there is no border, padding or radius at the break.
- **css-overflow-3 §3.**
  - `hidden`, `auto` and `scroll` make a scroll container; `clip` does not.
  - `visible` on one axis computes to `auto` when the other axis is neither `visible` nor `clip`. So Tailwind
    `overflow-x-auto` gives `overflow-y: auto`.
- **css-display-3.** `flow-root` is a block-level block container that establishes a BFC. It fragments like any block
  and is not atomic.
- **Chrome / Blink (unverified; step 1 decides).**
  - `LayoutBox::IsMonolithic` asks `HasUnsplittableScrollingOverflow()`, which returns false when printing ("If we're
    printing … allow objects with non-visible overflow to be paginated as normally"). If that holds, all four overflow
    values fragment in print. The finding brief assumes scroll containers are monolithic; P8 settles it.
  - LayoutNG lays atomic inlines out unfragmented. Since about M117, printing slices monolithic overflow at the page
    edge, cutting a straddling line in two.
- **Derived rule.**
  - A wrapper the splitter may look through is either:
    - (a) a block-level box whose clip hides nothing in the block axis (its content ends inside its padding box), with
      the scroll-container values only if P8 says so; or
    - (b) an inline-block that is the only content of its carrier's single line, where Chrome prints all of it.
  - Inside such a wrapper the paragraph splits between line boxes: class B breaks, orphans and widows 2, each line in
    exactly one fragment (F78).
  - The clip applies per fragment.

**Decision per wrapper.** The Chrome column is a prediction until step 1.

| Wrapper (probe) | Spec | Chrome, predicted | Ours today | F30f |
|---|---|---|---|---|
| `inline-block`, width 100 % or auto, alone on its line (P1-P4) | may be monolithic | monolithic, sliced: all words, one line cut per break | page 1 only | **split between lines** (rule below) |
| `inline-block` sharing its line (P5) | may be monolithic | sliced | page 1 only | unchanged; precise warning; F30f-b |
| `overflow: hidden` / `clip`, auto height (P6, P7) | fragmentable | fragments | page 1 only | **split** |
| `overflow: auto` / `scroll`, `overflow-x-*` alone (P8) | may be monolithic | fragments in print | page 1 only | **split** if Chrome prints every word; otherwise unchanged + warning |
| `overflow: hidden` with `height` / `max-height` below its content (P9) | may be monolithic | box fragments, clipped at its height | first page of the box | unchanged; no warning for lines the clip hides anyway; F30f-b if a box taller than a page loses visible lines |
| `display: flow-root` (P11) | fragmentable | fragments | splits already (not atomic, not clipping) | guard |

**Rule for sliced boxes.** Chrome may print every word of a row by slicing, i.e. one line per break is cut in two.
Where it does, we split between line boxes instead, and record this as **ADR-0004** in the same PR. Reasons:

- the spec allows both;
- line splitting prints every line once, at the fragmenter's position (F78);
- it is the same mechanism as for the clip wrappers;
- the cost is at most one line per page placed differently from Chrome.

Where Chrome prints only what fits (it clips), we match Chrome and the warning names the case.

## 3. Where it belongs and design (fulgur fork)

**Where.** This is engine behaviour in fulgur's fragmenter (`pagination_layout.rs`) and renderer (`render.rs`).

- Blitz and Taffy lay out one tall page (ADR-0001), and Parley only provides the lines.
- `line_split_candidate`, the inline-box skip set and the carrier paint path are all fulgur code, so upstream has the
  bug too. No upstream PR or issue (standing decision).
- Our crate changes only tests, fixture 16 and docs.

**Model.** As in F78, the fragmenter decides line ranges once and the renderer paints what it is given.

- F30f widens *which* parents the existing `nested_line_split` path may look through.
- It records where the carrier line of a split inline-block is painted.
- It teaches the renderer to paint an inline-box fragment its carrier does not paint.
- `would_split_block_subtree`, the gates and `plan_line_breaks` are unchanged.
  - Reason: they already enter these wrappers (cause 3). Changing them would move pages in documents that lose nothing
    today.

**A. One barrier predicate.** Signatures only.

```rust
/// F30f: why the line splitter does not look through a box (`None`: it does).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineBarrier { SharesItsLine, AtomicNotFlowRoot, ClipsItsContent, ScrollContainer, Multicol, Transformed,
                   Floating, Orthogonal, TablePart }
impl LineBarrier { fn describe(self) -> &'static str }
fn split_barrier(doc: &BaseDocument, parent: &blitz_dom::Node, child: &blitz_dom::Node) -> Option<LineBarrier>
fn clip_hides_nothing(doc: &BaseDocument, node: &blitz_dom::Node) -> bool
fn alone_on_its_line(doc: &BaseDocument, node: &blitz_dom::Node) -> bool
fn inline_root_lines(child: &blitz_dom::Node) -> Option<LineSplit>
```

- `inline_root_lines`: the present child checks minus the atomic and clip ones (at least 2 lines, not multicol, not
  floating, not transformed).
- `abspos_line_split` becomes `inline_root_lines` plus `!atomic && !clips_overflow`, so its behaviour is unchanged.
  - Reason: F30g keeps every clipping or transformed abspos box `Keep` (`abspos_flow.rs:987-1001`), and an abspos box
    is blockified (CSS 2.1 §9.7). Its tests (`abspos_flow.rs:1799-1802`) stay as they are.
- `line_split_candidate` becomes `inline_root_lines(child)` with `split_barrier(..).is_none()`.
  - `split_barrier` takes over the parent checks of `:4660-4667`.
  - It takes over the table checks of `:4658-4665` as `TablePart` (F30e lifts that one).
  - It applies the clip and atomic tests to both the parent and the child.
- `clip_hides_nothing` is true when the content ends within the padding box in the block axis:
  - an inline root: `inset_top + span + inset_bottom <= height + 0.5`;
  - a block: the lowest in-flow child bottom, plus the trailing margin, is at most `height - bottom border - bottom
    padding + 0.5`. Same walk as `frame_is_content_sized` (`:2966`); floats count.
  - Reason: splitting a box whose clip hides content would give it one frame per page, and the per-fragment clip would
    then show text that Chrome and today's output hide. `overflow_hidden_does_not_split_oversize_child` (`:7136`) and
    the `height:400px` clip guards in our `abspos_flow.rs:581-641` pin this.
- `alone_on_its_line` is true when all of these hold:
  - the node is outside-inline with inside `flow-root`;
  - its `layout_parent` is an inline root with exactly one Parley line;
  - that line's items are this inline box plus glyph runs of whitespace only.
  - Reason: then the carrier's line *is* the box, the carrier is never itself a line-split candidate (1 line), and
    nothing else on the line would need a page.
- `ScrollContainer` blocks only if P8 shows Chrome keeping `auto` / `scroll` monolithic and clipped. Otherwise it is not
  produced.

**B. Clip wrappers (R2).** Once `split_barrier` is `None` for a content-sized clip box, `nested_line_split` splits the
`<p>` through it. `fragment_block_subtree` then emits the wrapper's frame per page (the F30 closing at `:4233`).

- The renderer already clips per fragment, so it needs no change:
  - `draw_under_clip` takes `frame_fragment` (`render.rs:2965`) when `is_split()` (`:2077-2088`);
  - it pushes the clip at that fragment's rectangle (`:2152-2162`);
  - it paints the clip descendants' fragments on that page (`:2278-2399`).
- Question 3 of the brief is answered there. What differs from Chrome is the frame: radius and border close on every
  fragment, because `box-decoration-break: slice` is not implemented (F96).

**C. Inline-block wrappers (R1).** Three parts.

1. **Fragmenter.**
   - `split_barrier` lets an inline-block that is `alone_on_its_line` through, as a parent (`ib > p`, the gate enters it
     as today, cause 3) and as a child (direct text).
   - `block_parent` (`:3507`) stays false for it. Reasons:
     - `suppress_page_check` (`:3117-3119`) carries the page-name rule of WPT `page-name-inline-block-001`;
     - F83b's avoid rules are not extended here.
   - The last line still leaves room for the wrapper's bottom inset through `child_tail` (`:3834`), which does not
     depend on `block_parent`.
2. **Carrier slices.**
   - At the end of `fragment_block_subtree`, when the parent is a one-line inline root with more than one fragment and
     one of its embedded boxes now has fragments on several pages, record:

     ```rust
     fn record_carrier_slices(geometry: &mut PaginationGeometryTable, carrier_id: usize, line_page: u32)
     ```

   - It writes one `LineSlice` per carrier fragment: `{0,0}` before `line_page`, `{0,1}` on it, `{1,1}` after.
     `line_page` is the page of the box's first fragment.
   - The renderer's slice branch (`render.rs:3887-3898`) already returns `None` for an empty slice and the line for the
     full one.
   - `LineSlice`'s doc (`:116`) gains the "carrier" case. `line_slices_tile` (`:123`) stays strict; a separate
     `debug_assert!(carrier_slices_ok(..))` checks carriers.
   - Reasons:
     - cause 6;
     - the line must be drawn exactly on the page where the box starts, so that page 1 keeps today's baseline-shifted
       placement (`paragraph.rs:930-939`).
   - `<body>` as carrier keeps its single page-0 fragment and is not split. It needs nothing, but step 1 checks that it
     paints the line on page 0, given the `body` skip at `render.rs:789`.
3. **Render continuation.**

   ```rust
   fn inline_box_continues_on(geom: &PaginationGeometry, page_index: u32) -> bool
   fn paint_inline_box_continuation(canvas: &mut Canvas<'_, '_>, ib_id: usize, page_index: u32,
       geometry: &PaginationGeometryTable, drawables: &Drawables, margin_left_pt: f32, margin_top_pt: f32) -> bool
   ```

   - `inline_box_continues_on` is true when the node has a fragment on `page_index` and its first fragment is on an
     earlier page.
   - `paint_inline_box_continuation` calls `dispatch_inline_box_content` (`render.rs:964`) for that page's fragment,
     under a translate of `(-body_offset.x, -body_offset.y)`.
     - Reason: the helper adds `body_offset_pt` to the root and to every descendant (`:1062-1065`); the carrier arm
       cancels that with its own translate (`paragraph.rs:959-967`). On a continuation page `margin_top_pt` is already
       page-correct (`render.rs:290-299`).
   - The call sites are every walk that skips inline boxes as "painted by their paragraph":
     - `dispatch_page_node` at the skip (`:769`);
     - `draw_under_clip` (`:2274-2285`);
     - `draw_under_opacity` (`:2631-2642`).
   - Each calls it for an inline-box root it skips (a key of `inline_box_subtree_descendants`) when
     `inline_box_continues_on`. Descendants stay skipped, because the helper walks them.
   - Reason: on the box's first page the carrier paints it (unchanged), and on every later page nothing else would.
     This is exactly once per page. A transformed box is never split (`inline_root_lines` excludes transforms and a
     transformed parent is atomic for the strip slicer, `:2036-2039`), so `draw_under_transform` (`:1842`) needs no
     call. A `debug_assert!` there says so.
   - Side effect: an inline-block taller than a page *without* lines (cut by the strip slicer at `:2039`) also gets its
     continuation slices painted now; they are dropped today. It moves toward Chrome. None exists in the fixtures, the
     VRT or the examples (grep below).

**D. A precise warning.**

```rust
fn warn_unprinted_inline_lines(doc: &BaseDocument, parent: &blitz_dom::Node, child: &blitz_dom::Node,
                               child_page_y: f32, page_bottom: f32)
```

- The message names the barrier: "a paragraph of N lines in {LineBarrier::describe} is placed whole and ends X px below
  the content box; lines outside the page box are not printed".
- For `ClipsItsContent` it counts only lines that are inside the clip box and below the page, and is silent when there
  are none. Reason: the clip hides the rest in Chrome too, so "not printed" would blame pagination for a clip.
- The list in today's text (`:4952-4955`) goes away. It is wrong after this change.

**E. Interaction with F30g's guard (`abspos_flow.rs`).**

- Abspos boxes under a clipping ancestor stay `Keep` (`has_guarded_ancestor`, `:169-190`; `:987-1001`), unchanged.
- **Why Keep needs no change:**
  - F30f does not change which clip boxes are entered. The gates ignore the splitter (cause 3).
  - An abspos box under a clip box therefore gets the same geometry as today: none when the box is entered (out-of-flow
    children are skipped, `:3170-3176`; this is F95 b), or one whole fragment from `record_descendant` when it is placed
    whole.
  - Only the clip box's own fragments change, from one to one per page.
  - A kept box adds no fragment and no page, so there is no blank page.
  - It has at most one fragment, painted by the clip scope on that page, so there is no double draw.
- **Lifting the guard for content-sized clips is out of scope.** It would need the anchor to be located in the clip
  box's stitched fragments (`locate_forward`, `:215`) and a per-fragment clip of the flowed pieces. That is the validator
  HIGH shape of F30g. Left to F95.
- **`window_cuts_lines` (`:344`) and `lines_end_below_page` (`:438`) still stop at clipping boxes.**
  - Consequence: an abspos box that holds an `overflow-hidden` card with a long paragraph stays on the window model and
    stays lossy. This becomes F95 (e).
  - Reason: flowing such a root would give its nested out-of-flow boxes window geometry against a placement that no
    longer exists. Their doc comments are corrected to say so.
- **Abspos boxes are blockified, so they are never the inline-block case.** An abspos box that holds an inline-block
  alone on its line is flowed by F30g's `fragment_block_subtree` (`flow_box`, `:931`) and benefits from C as in flow.

**F. Byte identity, predicted.**

- **Fixtures 01–15.** Grep of every `index.html`:
  - `inline-block` appears only in 03: two one-line badges sharing a line, so not a candidate, one fragment each, no
    continuation.
  - `overflow-hidden` appears only in 05: the wrapper of a 3-row table on a 1-page invoice. The gate is false and the
    child has no lines.
  - No `flow-root`, other `overflow-*`, or `inline-flex` / `inline-grid` / `inline-table`.
  - The compiled `tailwind.css` has `.inline-block`, `.overflow-hidden`, `.w-full` and none of `flow-root` /
    `overflow-clip` / `-auto` / `-scroll` / `overflow-x-*`.
  - Prediction: 01–15 `cmp`-identical.
- **Fulgur VRT.** These are all one page, with fixed small boxes or one-line text, so byte-identical:
  - `layout/inline-block-basic`, `inline-block-nested`, `inline-block-overflow-hidden`;
  - `review_card_inline_block`;
  - `overflow-hidden`, `overflow-hidden-rounded`, `overflow-nested`, `overflow-axis-x`, `overflow-axis-y`.
- **Examples.** Both byte-identical:
  - `examples/overflow-hidden` (80 px boxes of block children);
  - `examples/wasm-demo` (`label{display:inline-block}`, one line).
- **Fork tests that change on purpose.**
  - `paragraphs_that_are_not_in_plain_flow_stay_whole` (`:11547`): its "overflow:hidden flex" case is content-sized and
    now splits. Rewrite it as P17's Chrome result, plus a `height:300px` twin that stays whole. "inline-flex" stays.
  - Any other changed expectation is a regression.

**G. What F30e reuses, and what it still needs.**

- **Reuses:**
  - `LineBarrier` / `split_barrier` (F30e lifts `TablePart`);
  - the precise warning;
  - for `inline-table`, `alone_on_its_line`, the carrier slices and the render continuation unchanged.
- **Still needs:**
  - **Its own step 1.** The fork harness already splits `display:table > p` in its anonymous cell
    (`a_paragraph_that_is_a_non_cell_child_of_a_table_box_splits_in_its_anonymous_cell`, `:11576`, since F50). Yet
    `dtable_p.html` prints nothing and gives no warning. So F30e's loss may be in the table paint path (`draw_table_v2`,
    `draw_under_clip_table` `render.rs:3320`), which F30f does not touch.
  - Lifting `TablePart` for a table whose single anonymous cell (`blitz_adapter::anonymous_table_wrapper`, `:1013`)
    holds the paragraph.
  - Allowing `inline-table` in `alone_on_its_line` (inside `table`, not `flow-root`).

## 4. Acceptance

**Ours: new `crates/htmltopdf/tests/wrapper_line_split.rs`.**

- Setup as in `abspos_flow.rs`: A4, 15 mm, Inter, strict, `RenderOptions::default()`, `p{margin:0;line-height:20px}`.
- Helpers copied from it: `printed_words` (`pdftotext -bbox`), `page_of`, `page_count`, `blank_pages`,
  `assert_every_word_once` (every word once, in order, none below the content box by more than 5 pt, none above it by
  more than 2 pt).
- `W` is 4000 words. Expectations, including page counts, come from the step 1 probes; at 20 px about 12 pages are
  expected.

1. **A1 (R1).** `<div style="display:inline-block;width:100%"><p>W`. Every word once; page count equals Chrome's.
   **Red.**
   - A1t: direct text in the inline-block. **Red** (pages ≥ 2 empty).
   - A1n: R1 with `line-height:normal`. Every word once. **Red.**
2. **A2.** `display:inline-block` with auto width: as A1. **Red.**
   - A2c: `<div style="text-align:center"><div style="display:inline-block;width:60%"><p>`. Every word once, and the
     first word's x on page 2 within 0.5 pt of its x on page 1. **Red.**
3. **A3 (anonymous carrier).** `<h1>Podmínky</h1>` + A1. `Podmínky` once on page 1, every word once. **Red.**
4. **A4 (element carrier).** `<div style="border:2px solid #000;padding:8px">` + A1. Every word once; a border row at
   the top of every page's frame (`red_pixels`-style scan, as `avoid_wrappers.rs`). **Red.**
   - A4b: A1 with `body{margin:8px}`. No word above the content top on any page (catches the body offset). **Red.**
5. **A5 (guard, shared line).** `<p>Úvod <span style="display:inline-block;width:60%">W</span> konec</p>`.
   Words per page identical to main, plus the precise warning (warning file, below).
6. **A6 (R2).** `<div style="overflow:hidden"><p>W`. Every word once; page count equals Chrome's. **Red.**
   - A6t: `<div><div style="overflow:hidden">W</div></div>` (child side). **Red.**
   - A6n: R2 with `normal`. **Red.**
7. **A7.** `overflow:clip`. **Red.**
8. **A8.** `overflow:auto`, `overflow:scroll`, `overflow-x:auto`: as Chrome. **Red** if Chrome prints every word;
   otherwise words per page identical to main.
9. **A9 (guards, clip hides content).**
   - `overflow:hidden;height:600px`: 1 page, no word whose top is below 600 px + 1 pt from the box top, same words as
     main.
   - `max-height:2000px`: words per page identical to main (F30f-b).
10. **A10 (guards, already split today).** `display:flow-root` and `<div style="overflow:hidden"><div><p>`: every word
    once.
11. **A11 (guards, 05 shape).**
    - `<div class="overflow-hidden rounded-xl ring-1 ring-gray-200">` with a 3-row table: 1 page, all cell words on
      page 1.
    - The same wrapper with a 60-row table: every cell word once, page count equal to main's and recorded against
      Chrome's. Predicted unchanged, because F30f does not touch tables. If main loses rows here, file it, do not fix it
      here.
12. **A12 (F30g Keep guard).** `<div style="overflow:hidden;position:relative"><p>W</p><div style="position:absolute;
    top:0;right:0">ZAPLACENO</div></div>`. Every `W` word once; `ZAPLACENO` at most once; no blank page; page count
    equals Chrome's. Chrome prints the stamp; ours does not (F95 b).
13. **A13 (inline-block in a clip scope).** `<div class="overflow-hidden rounded-lg border p-4">` + A1. Every word
    once. **Red.**

**Ours: new `crates/htmltopdf/tests/unsplit_paragraph_warning.rs`.** It is a separate binary because each binary has one
global logger (as `fallback_warning.rs`).

- A5 warns and names "an inline-block that shares its line".
- A9 (600 px) does not warn.
- A1 and A6 do not warn.

**Fork: lib tests** next to `:11281-11589`, using `nested_geometry` / `fragments_of` / `br_lines` (200 px pages).

- **Red:**
  - `a_paragraph_in_a_content_sized_clip_box_is_split` (`hidden`, `clip`, plus `auto` / `scroll` per P8);
  - `a_paragraph_in_an_inline_block_alone_on_its_line_is_split`;
  - `an_inline_block_with_direct_text_is_split`;
  - `the_carrier_line_is_on_the_page_where_its_inline_box_starts` (slices `{0,1}`, `{1,1}`…);
  - `an_inline_block_moved_to_the_next_page_takes_its_carrier_line_along` (slices `{0,0}`, `{0,1}`, …);
  - the rewritten `paragraphs_that_are_not_in_plain_flow_stay_whole`.
- **Guards:**
  - `a_clip_box_shorter_than_its_paragraph_keeps_it_whole`;
  - `an_inline_block_sharing_its_line_keeps_its_paragraph_whole`;
  - `overflow_hidden_does_not_split_oversize_child` (`:7136`);
  - every `abspos_flow` test.
- **Unit:**
  - `clip_hides_nothing`: auto, `height`, `max-height`, padding, a float, `overflow-x` only;
  - `alone_on_its_line`: alone, two boxes, text before, a box on a second line;
  - `split_barrier`: each arm;
  - `carrier_slices_ok`;
  - `inline_lines_below_page` with a clip (`:11840`).
- **Render unit (`render.rs` tests near `:5494-5679`):**
  - `paragraph_lines_for_page_paints_a_carrier_line_on_its_page_only`;
  - `inline_box_continues_on_only_after_its_first_page`;
  - `an_inline_box_continuation_is_painted_once_per_page` (synthetic drawables).
- **`render_smoke.rs`:**
  - `render_smoke_inline_block_paragraph_spans_pages`;
  - `render_smoke_overflow_hidden_paragraph_spans_pages`.
  - Reason: codecov does not see these paths otherwise (fulgur `CLAUDE.md`).

**Fixture `16-terms-card` (new).** This is an invoice-realistic shape: `overflow-hidden` is there so that the title
bar's background does not poke out of the rounded corners.

- Content:
  - the 06 header, party grid and a 3-row items table;
  - "Celkem k úhradě";
  - `<section class="overflow-hidden rounded-lg border border-gray-300 mb-6 text-sm">` holding:
    - `<h2 class="bg-gray-50 border-b border-gray-300 px-4 py-2 text-xs font-semibold uppercase tracking-wider
      text-gray-500">Obchodní podmínky</h2>`;
    - `<p class="p-4">` with about 1.4 pages of new Czech terms, articles 1–30 (not 06's text), as the **direct child**
      of the clip box, which is the F30f shape;
  - then a "Platební údaje" section (účet, IBAN, VS, splatnost) and a signature line.
  - Every class is in the compiled `tailwind.css` (grep), so there is no inline style and no rebuild.
- PNG: Chrome continues the terms on page 2 inside the card, then the payment section.
  - Today the text below page 1 is missing. Predicted: the payment block alone on page 2.
  - After: as Chrome, except the card's border and radius close at the break (F96).
- Record compare numbers with main's binary and with the branch. Threshold = worst page + 1 point. Both are predicted at
  2 pages, so the fixture is not in `PAGE_COUNT_MAY_DIFFER`. If main has 1 page, record it; the fixture is new either
  way.
- Before that, step 1 confirms main loses text on this exact markup. If it does not, there is no fixture and the tests
  carry F30f.

**Pixel diff.** Fixtures 01–15 and their numbers are unchanged; 16 is new.

## 5. Steps

1. **Confirm (gate).** Scratch only; temporary logs are removed afterwards.
   - Recreate R1 and R2 (numbers as in §1). Render with main's binary and count words per page.
   - Log the barrier at `:4660` and at `:4134`. Log in `paragraph.rs:940` which node carries the inline box and on
     which page. Expected:
     - `<body>` for R1;
     - the anonymous block for A3;
     - the `<p>` placed whole.
   - Run P1t to check that the body-level direct-text inline-block already has fragments on pages ≥ 1 but paints only
     page 1.
   - **Chrome probes** (Playwright, m141; scratch `probe.py` as in F30g) at `20px` and `normal`. For each probe record
     words per page, any word printed on two pages or cut at a page edge (slicing), and the PNG at each break.

     | P | Body content (`W` = 4000 words) | Question |
     |---|---|---|
     | P1 / P1t / P2 | A1 / A1t / A2 | split, slice or clip |
     | P2c, P3, P4 | A2c, A3, A4 | carrier shapes |
     | P5 | A5 | shared line |
     | P6 / P6t / P7 | A6 / A6t / A7 | clip wrappers |
     | P8 | A8 (`auto`, `scroll`, `overflow-x:auto`, `overflow-x:hidden`) | scroll containers in print |
     | P9 | A9 (`height:600px`, `max-height:2000px`) | clip that hides content |
     | P10 | body-level `<div style="overflow:hidden;height:300px">W</div>` | does ours reveal clipped text? (pre-existing) |
     | P11 / P12 | A10 | controls |
     | P13 | fixture 16 markup | radius and border at the break |
     | P14 / P15 | A11 | 05 shape |
     | P16 | A12 | stamp |
     | P17 | `<div style="display:flex;overflow:hidden"><p>W` | lib-test expectation |

   - Fill in the §2 table.
   - Write ADR-0004 if any row slices.
   - File findings: F30f-b (shared line, height-limited clips taller than a page, plus P10 if ours reveals), and F95 (e).
   - **Stop and write `docs/postmortems/2026-10-18-f30f-*.md`** if:
     - Chrome prints only page 1 for P6 (the finding's premise is wrong; F30f becomes the warning only);
     - or the `<p>` in R1/R2 already has more than one fragment (another cause).
2. **Red tests.** Ours: `wrapper_line_split.rs` and `unsplit_paragraph_warning.rs`. Fork: the lib, unit and render
   tests and `render_smoke`. Record which are red.
3. **Fork, output-neutral.**
   - `LineBarrier`, `split_barrier`, `clip_hides_nothing`, `alone_on_its_line`, `inline_root_lines`;
   - `line_split_candidate` / `abspos_line_split` refactored with the barriers still blocking;
   - the precise warning.

   Then:
   - `--lib` on crates.io Blitz (the same 13 failures as at `e0aed2e`) and on local Blitz (0);
   - the 42 `tests/*.rs` one by one;
   - VRT byte-identical (`fonts-noinInter.conf`);
   - fmt and `clippy --all-targets -D warnings`.
4. **Fork, clip wrappers.** Lift `ClipsItsContent`-free clips (and `ScrollContainer` per P8) on the parent and child
   sides. Rewrite `:11547`. A6–A8, A10–A12 green.

   Then the step 3 checks, plus the 21 examples rendered before and after. **An unpredicted mover means stop and write a
   postmortem.**
5. **Fork, inline-block fragmenter.** Lift `SharesItsLine`-free inline-blocks; add `record_carrier_slices` and
   `carrier_slices_ok`. The lib geometry tests go green; A1–A4 stay red (render). Then the step 3 checks.
6. **Fork, render continuation.** `inline_box_continues_on`, `paint_inline_box_continuation` at the three call sites,
   and the render unit tests. A1–A4 and A13 green, plus `render_smoke`. Then the step 4 checks.
7. **Ours:**
   - lock bump;
   - `cargo test --release --locked --no-fail-fast`;
   - fixtures 01–15 `cmp` against main's binary;
   - `compare.py` unchanged;
   - render time of 01 and 02 within noise.
8. **Fixture 16:**
   - HTML and README;
   - Chrome reference (`scripts/reference/render.mjs`);
   - compare and threshold;
   - look at the PNGs.
9. **Docs:**
   - findings: F30f done, F30f-b and F95 (e) filed, F30e updated with §3 G;
   - the `docs/upstream.md` row;
   - ADR-0004 if written;
   - §7 here.

   Then run `vrt-validator`, and `pdf-reviewer` as well: `render.rs` changes on the writer path, and continuation
   content is tagged outside its carrier paragraph (merge policy item 5).

## 6. Risks and non-goals

- **Carrier slices are a new `LineSlice` shape.** If the mapping is off, the carrier line is painted on no page, or on
  the wrong one, and the box disappears. A3, A4 and the moved-carrier lib test pin it, and `carrier_slices_ok` checks it
  in debug.
- **Double draws.** The continuation must skip the box's first page. A1–A4 assert every word once. A box whose first
  fragment is not on the page of its carrier line would be lost; the lib test asserts both pages agree.
- **Position on continuation pages.**
  - Geometry positions lack the carrier's baseline shift. That is harmless after page 1 (the box starts at the page
    top).
  - The x position relies on Taffy and Parley agreeing; A2c pins it.
  - The body offset is cancelled once; A4b pins it, cf. F92.
- **Tagged PDF.** Continuation content is tagged under the box, not under its carrier paragraph; `pdf-reviewer` checks
  the structure tree.
- **Clip reveal.** `clip_hides_nothing` has 0.5 px of slack. Negative margins or a float that sticks out make it false,
  so the box stays whole: today's output.
- **Frames.** The border and radius close on every fragment (F96). Fixture 16's diff shows it at the break.
- **Chrome slicing against our split.** Where ADR-0004 applies, pages differ from Chrome by at most one line.
- **Rebase.** `line_split_candidate` / `abspos_line_split` (F30b, F30g) and the three render walks will conflict on a
  fulgur bump.
- **Non-goals:**
  - F30e (`display: table` / `inline-table` wrappers);
  - inline-blocks that share their line, and inline-flex / inline-grid (F30f-b);
  - clip boxes whose content overflows them and that are taller than a page (F30f-b);
  - the body-level line path's own exclusions (P10, filed if confirmed);
  - abspos boxes under clipping ancestors, and abspos roots holding a clip card (F95);
  - `box-decoration-break: slice` (F96);
  - paragraphs shorter than a page that move whole (F30c);
  - real slicing of monolithic boxes without lines;
  - transforms and multicol.

## 7. Result

Landed on branch `f30f-inline-block-overflow-wrapper`; fulgur fork `htmltopdf/srgb-colours` **`5c496f3`** (commits on top of `e0aed2e`: `d116cd6` the barrier predicate, output-neutral; `e9ffbc1` the fix; `227dd59` and `5c496f3` doc comments only). The Blitz fork is
unchanged (`52b22ef`). `Cargo.lock` pins `5c496f3` (`cargo update -p fulgur` changed that one entry). Status: **done**.

**Step 1 (gate), no stop.** Scratch: `/tmp/claude-0/vrt-f30f/` (`probe.py`, `h.py`). Chrome m141, A4, 15 mm, Inter, 4000 words, 20 px and `normal` lines (same pages and words per page in both).
- **Recreation:** R1 (`ibl_normal`) gives 386 words, 569 lines, 9917 px below the content box, as in §1; R2 the same. Main prints 372 words at 20 px.
- **Causes held**, with temporary logs in the fork (removed): `line_split_candidate` returns `None` for R1 with `atomic_parent=true` and for R2 with `clips_parent=true`, the `<p>` is placed whole (the warning at the whole-placement site fires, so it has one fragment: stop condition 2 does not apply). The carrier of R1 is `<body>` (one fragment `(0, 0, 10925)`, no slices, painted on page 0); for A3 the anonymous block below the `<h1>` (fragment at y = 29). P1t: ours prints 351 words on 12 pages, 11 of them empty, no warning (cause 4).
- **Stop conditions:** Chrome fragments P6 (12 pages, 4000 words), so the premise holds; the `<p>` does not already have several fragments. No stop.
- **Chrome slices an `inline-block`**: every word, but the straddling line is printed on both pages, half on each (P1: 4042 word occurrences, 42 twice; PNG at the break read). When something precedes the box (P3 heading, P4 bordered frame) Chrome moves the box whole to page 2 and slices it from its top (13 pages, an empty frame on page 1). **ADR-0004** written.

| Probe | Chrome | main | F30f |
|---|---|---|---|
| P1 / P1t / P2 (ib alone, direct text, auto width) | slices, 4000 words, 12 pages | 372 on 1 page / 351 on 12 (11 empty) / 372 | every word once, 12 pages |
| P2c (centred 60 %) | slices, 20 pages | 212 on 1 page | every word once, 20 pages, x equal on every page |
| P3 (`<h1>` before) / P4 (bordered carrier) | box on page 2, 13 pages | 365 / 364 on 1 page | every word once, 12 pages (box starts below the heading, ADR-0004) |
| P5 (ib shares its line) | slices, 20 pages | 212 on 1 page | unchanged, warning names it (F30f-b) |
| P6 / P6t / P7 (hidden, hidden child side, clip) | fragments, 12 pages | 372 / 372 / 372 on 1 page | every word once, 12 pages |
| P8 auto, scroll, overflow-x:auto, overflow-x:hidden | fragments, 12 pages | 372 on 1 page | every word once, 12 pages: **no `ScrollContainer` barrier** |
| P9 `height:600px` / `max-height:2000px` | 1 page, 211 words / 2 pages, 701 | 372 on 1 page | unchanged (F30f-b), no ink below the clip, no warning |
| P10 body-level `overflow:hidden;height:300px` direct text | 1 page, 105 words | **ours prints all 4000 on 12 pages (reveals)** | unchanged (F30f-b) |
| P11 / P12 (flow-root, hidden > div > p) | 12 pages | 4000 on 12 pages | unchanged |
| P14 / P15 (05 shape, 3 / 60 rows) | 1 page / 2 pages, 126 + 54 cells | 1 page / 2 pages, 135 + 45 cells | unchanged |
| P16 (abspos stamp in a clip box) | stamp once on page 1, 12 pages | 372 on 1 page | text 12 pages, stamp not printed (F95 b) |
| P17 flex + overflow:hidden / + `height:300px` / inline-flex | fragments 12 / 1 page, 105 / slices 12 | 372 on 1 page / 372 / 8 words | split, 12 pages / whole / unchanged (F30f-b) |

Decisions (§2 table): the `inline-block` rows are as predicted (ADR-0004); `overflow: auto` / `scroll` / `overflow-x-*` split like `hidden` (P8, Chrome fragments them in print); `overflow: hidden` with `height` / `max-height` stays whole; flow-root is a guard.

**Red before** (fork at `e0aed2e`, red by running the new tests against the refactored, still-blocking `d116cd6`): ours `wrapper_line_split.rs` 15 of 21 tests (A1, A1t, A1n, A2, A2c, A3, A4, A4b, A6, A6t, A6n, A7, A8, A12, A13); the 6 green are guards (A5, both A9, A10, both A11) and `unsplit_paragraph_warning.rs`'s one test is red. Fork lib `pagination_layout`: 8 of the 14 new tests (clip box split, `direct_text_in_a_clip_box_is_split`, the flex clip box, `a_paragraph_in_an_inline_block_alone_on_its_line_is_split`, `an_inline_block_with_direct_text_is_split`, the two carrier tests, `split_barrier_names_each_arm`); the unit tests of `clip_hides_nothing`, `alone_on_its_line`, `carrier_slices_ok`, `unprinted_inline_lines` are green from the start. The render unit tests were red by absence of the functions (they did not compile); `render_smoke` was not run red. **Green after:** all of them.

**What was built** (fork `pagination_layout.rs`, `render.rs`): `LineBarrier` (without `ScrollContainer`), `split_barrier`, `box_barrier`, `clip_hides_nothing`, `alone_on_its_line`, `inline_root_lines`, `line_split_candidate` / `abspos_line_split` on them, `unprinted_inline_lines` + the warning naming the barrier, `carrier_slices_ok`, `record_carrier_slices`, `record_carrier_slices_after` (called at the end of `fragment_block_subtree`), `inline_box_continues_on`, `paint_inline_box_continuation` at the three walks and a `debug_assert!` in `draw_under_transform`. Docs of `LineSlice`, `window_cuts_lines`, `lines_end_below_page`.

**After:**

| Check | Result |
|---|---|
| fork `--lib`, crates.io Blitz | 2127 passed, 13 failed: the same 13 as at `e0aed2e` (9 `seed_image_natural_size_tests`, 3 `convert::table`, 1 `pagination_layout` table test) |
| fork `--lib`, local Blitz (`/home/user/blitz`, `52b22ef`) | 2140 passed, 0 failed (2123 before) |
| fork `render_smoke` | crates.io Blitz 254 passed, 3 failed (the same `outside_*_markers`, `table_parts_with_a_non_table_display`) |
| fork other test binaries | all 42 `tests/*.rs` one by one (local Blitz): no failure |
| fork `fulgur-vrt` (`fonts-noinInter.conf`, crates.io Blitz) | 31 tests green, goldens untouched |
| fork 21 examples (debug CLI at `e0aed2e` against the branch, after step 3, after step 4, after the final code) | byte-identical, 21 of 21 each time |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked --no-fail-fast` | 417 passed, 0 failed, 2 ignored (21 new `wrapper_line_split.rs`, 1 `unsplit_paragraph_warning.rs`, `fixtures.rs` renders 16 strict); fmt and `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-15 | `cmp` byte-identical to the render of main's binary (`/tmp/claude-0/htmltopdf-main-a68154d`): 15 of 15; `compare.py` numbers unchanged |
| render time 01 / 02 (25 runs, median) | 19.7 / 59.8 ms on main, 19.9 / 58.8 ms now: noise |
| tagged output | `--tagged` renders of A1, A2c, A3, A4, A6 and fixture 16 succeed (no Krilla nesting panic); the structure tree was not inspected |
| fixture 16 | new: **2 pages like Chrome, 16.6 / 20.8 %** against main's binary 3 pages, 11.4 / 25.0 / 0.9 % (the terms' tail lost, the payment block on page 3), threshold 0.218 (worst page + 1 point) |

Fixture 16 (`out/compare/16-terms-card-p1.png`, `-p2.png`, git-ignored): every word is on Chrome's page; page 1 breaks after article 13 like Chrome's, the card continues on page 2 and the payment block follows it. What remains is dense text: ours closes the card's frame on page 2 (top border and 16 px of padding, F96), so page 2's lines sit about 20 px lower than Chrome's, and the drift every fixture has. Page 1 grew from main's 11.4 to 16.6 % because on main the paragraph's lines on page 1 were the unsplit ones (a different cut); the pages are not comparable (new fixture).

**Deviations from the plan:**

1. **Order of steps 2 and 3.** The red tests name `split_barrier`, `clip_hides_nothing` and the rest, so the refactor (step 3) came first; the tests were then red against it with the barriers still blocking. Steps 4 to 6 were committed as one fix (`e9ffbc1`); the examples were rendered after step 3, after step 4 and at the end, the full fork suite (lib on both Blitz, the 42 test binaries, smoke, VRT, examples) after step 3 and on the final code (as in F30g and F83b).
2. **`ScrollContainer` is not a variant** (P8: Chrome fragments `auto` / `scroll`; the variant would be dead code under `-D warnings`).
3. **`clip_hides_nothing` also reads inline-root children.** A child paragraph whose lines overflow its own box (a `height` on the `<p>`, a stretched flex item) counts with its lines, and the lines have 2 px of slack (`LINE_BOX_SLACK`): Blitz quantises line boxes, and with Noto at 20 px the lines are `(-1, 21)`, `(19, 41)` .., so the last one ends 1 px below the box (found by `render_smoke_overflow_hidden_paragraph_spans_pages`, which first printed 1 page).
4. **`unprinted_inline_lines`** was split out of the warning so its clip counting is unit-testable; **`record_carrier_slices_after`** is the helper that decides when to call `record_carrier_slices`. Neither is in §3.
5. **A2c** has `text-align:left` on the box (the plan's markup centres its text, so the first word of a page is not the box's left edge); the carrier-line x is the geometry's x on pages after the first and Parley's on the first, they agree within 0.5 pt.
6. **The Orthogonal arm of `split_barrier` has no unit test**: `writing-mode: vertical-rl` on a child or a parent did not produce `Orthogonal` in the test harness (not investigated; the arm is the old check, moved).
7. **A3's page count** is not asserted against Chrome's (13): Chrome moves the box to page 2 (ADR-0004), we have 12.
8. **`vrt-validator` and `pdf-reviewer` were not run by the implementer** (a subagent cannot start another); merge policy items 2 and 5 are open for the caller. The tagged renders above are a smoke check, not a review.

Filed: **F30f-b** (shared line, inline-flex, clip boxes that hide content, ours reveals clipped text at body level, ib that is not first on its page), **F95 (e)**; F30e updated with what it reuses and needs.

Not verified here: the spec citations of §2 (from memory; the Chrome probes stand in); the Blink `IsMonolithic` claim (probes only); a `position: fixed` box or an opacity / transform scope around a split `inline-block` (the continuation call sites in `draw_under_opacity` and the `debug_assert!` in `draw_under_transform` are exercised by unit tests only, not by a render); a split `inline-block` inside a multicol or flex / grid item; `visibility: hidden` on the box (the continuation skips it like its carrier); the structure tree and link rectangles of continuation content; the WPT `css-page` subset (no checkout); right-to-left and vertical writing modes; `line-height: normal` for A2c-A4b, A7, A8, A10-A13 (A1n and A6n only).
