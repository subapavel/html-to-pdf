# F30g — An absolutely positioned paragraph, or a wrapper of one, loses its text

Date: 2026-10-18. Finding: F30g in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after `a2b2d3a`), carried via `[patch.crates-io]`. Fork line numbers are at `a2b2d3a`, in
`crates/fulgur/src/pagination_layout.rs` unless another file is named.

Amended by the main session before implementation: fixture 15 sets positions with inline `style` instead of rebuilding
`tailwind.css` (the rebuild needs npm and the `^4.1.0` range is unpinned, precedent fixtures 12 and 13); the spec gate in
step 1 relies on the Chrome probes, not on a separate css-spec-researcher run.

## 1. Problem

Text inside an absolutely positioned box is printed only where it happens to fit the page its box starts on. If no emitter
places the box at all, none of it is printed. Nothing warns. Both repros are pre-existing and the same on main.

Recreate them under `out/scratch/f30g/` (git-ignored). All of them use A4, `@page{size:A4;margin:15mm}` (content box
680.3 × 1009.1 px), Inter 16 px, `body{margin:0}`, and words `word%05d` joined by single spaces.

- **R1** (F83 validator, `/tmp/claude-0/vrt-f83/att/cases3/abspos.html`, still on disk): `<body><div style="position:relative"><p style="break-inside:avoid;
  position:absolute;top:0;left:0;width:400px;margin:0">` 200 words `</p></div>`, with `line-height:normal`. The twin has
  `break-inside:auto`. Ours: 0 of 200 words in both. Chrome: 200, on page 1.
- **R2** (F30 validator, `abs_normal` / `abs_1.3`): `<body><div style="position:absolute;top:0;left:0;right:0"><p>` 5000
  words, with `line-height` `normal` and `1.3`. Ours: 379 words on 14 pages. Chrome: all of them.
  - The `absparent_*` / `absrel_*` shapes were lost. Recreate both readings, (a) abs `div` > static `div` > `p` and
    (b) relative `div` > abs `div` > `p`, and record which prints 0 and which 379.

Causes, from reading (step 1 confirms them):

1. **The fragmenter never places an abspos box.**
   - Out-of-flow children are skipped in the body walk (`:1364-1379`) and in `fragment_block_subtree` (`:3160-3170`).
   - A zero-height in-flow parent is recorded alone, without its descendants (`:1484-1497`).
   - The only other emitters:
     - `record_descendant` (`:2367-2423`). It has no out-of-flow filter. When the plain path emits an ancestor whole, it
       puts the abspos box on the ancestor's page as one whole fragment.
     - The body-direct pass.
   - So an abspos box under a zero-height ancestor (R1), or under an ancestor that `fragment_block_subtree` enters, has no
     geometry and is never drawn.
2. **The body-direct pass uses a "window" model.** The pass is `append_position_absolute_body_direct_fragments`
   (`:5605-5701`, called at `engine.rs:580` and `:1012`).
   - Every node of the subtree gets one fragment per page its box intersects. Each fragment has the **full** height
     (`:5961-5966`) at `y = origin − k·page_h − body_offset` (`:5959`).
   - The pass clears `line_slices` (`:5939-5941`). That is correct for this model: it replaces earlier placements and has
     no line ranges to give.
   - The renderer's `paragraph_lines_for_page` (`render.rs:3873`) then falls back to measuring:
     - Page 0: `consumed = 0`, so it takes every line from the box top. Lines below the page fall outside the page box.
     - Page k ≥ 1: `consumed = k·h`, which is past the whole paragraph, so it returns `None` (`render.rs:3930-3932`).
   - No warning, because the page-0 cut is not partial (`render.rs:3937`, `:3955`). R2's 379 words are what fits on
     page 1.
3. **A nested abspos box placed whole by `record_descendant`** loses whatever falls below that page in the same way.

## 2. Spec (cited from memory, unverified)

- **CSS 2.1 §10.1.** The containing block (CB) of an abspos box is the padding box of its nearest positioned ancestor, or
  else the initial containing block (in paged media the page area).
- **CSS 2.1 §10.6.4.** The vertical constraint `top + margin + border + padding + height + … + bottom = CB height`.
  `top: auto` with `bottom: auto` uses the static position. Over-constrained: `bottom` is ignored. Auto `height` with
  `top` and `bottom` set comes from the constraint.
- **CSS 2.1 §13.3.1 / §13.3.3.** Page breaks are defined only for the normal flow: between block siblings and between line
  boxes. CSS 2.1 leaves breaks inside out-of-flow boxes undefined.
- **CSS 2.1 §9.6.1.** In print, a fixed box "is rendered on every page", fixed to the page box. It is repeated, not
  fragmented.
- **css-break-3 §2.**
  - Fragmentation splits the content of a fragmentation context across fragmentainers.
  - §2.1 (parallel flows): an independent formatting context laid out beside the flow fragments on its own.
  - §4.1: breaks inside any block container are class B, between line boxes.
  - §4.4: `orphans` / `widows`, initial value 2.
- **css-position-3.**
  - A positioned box is positioned against its CB as if the CB were unfragmented (the "stitched" CB), and is then
    fragmented like other content of the context.
  - Fixed boxes repeat on every page in paged media.
- **Derived rule.** An abspos box is laid out once. Its start is found in the CB's stitched coordinates. From there its
  content is fragmented by the in-flow rules: class B between lines, orphans and widows 2. Each line box goes in exactly
  one fragment (the F78 rule).
- **Chrome / LayoutNG** (unverified).
  - Block fragmentation in printing, and out-of-flow boxes fragmenting across fragmentainers since about M108.
  - Pages are appended for abspos content below the last in-flow page.
  - Fixed boxes are repeated whole on each page and clipped to it.
  - Step 1 probes all of this.

## 3. Where it belongs and design (fulgur fork)

**Where.** This is engine behaviour in fulgur's fragmenter.

- Blitz and Taffy lay out one tall page (ADR-0001). Parley only provides the lines.
- The window pass and the out-of-flow skips are fulgur 0.40.0 code, so the bug exists upstream too. No upstream PR or
  issue (standing decision).
- Our crate changes only tests, fixture 15 and docs.

**Model.** As in F78, the fragmenter decides line ranges once and the renderer paints them as given. For a box whose text
crosses a page, the abspos pass drops the window model and fragments the box from its own start with the existing
fragmenters. `render.rs` is unchanged: it already paints `line_slices` and `line_split_insets`.

**Why the window cannot just be given line slices:** a window cuts at page edges, not between lines. Pushing a straddling
line to the next page moves everything below it, which is fragmentation.

**Entry point.** Called in `engine.rs` right after the window pass at both sites (`:580`, `:1012`), before the second fixed
pass (`:588-598`, `:1020-1030`). That pass then repeats fixed boxes on any pages the flows add.

```rust
pub fn fragment_abspos_flows(
    geometry: &mut PaginationGeometryTable,
    doc: &BaseDocument,
    page_height_px: f32,
    viewport_w_px: f32,
    column_styles: Option<&crate::column_css::ColumnStyleTable>,
    multicol: Option<&crate::multicol_layout::MulticolGeometryTable>,
    running_store: Option<&crate::gcpm::running::RunningElementStore>,
)
```

- `PageStrips::new(page_height_px, body_origin_in_px(doc).1)`, as in `PaginationLayoutTree::new` (`:1112`). The flows then
  use the fragmenter's own coordinates: body coordinates on page 0, page coordinates after. Reason: render adds the body
  offset on page 0 only (`render.rs:290-294`).
- `used_page_names = None`. Reason: `page` applies to boxes in the normal flow of the root (css-page-3 §5, unverified).

**New private helpers** (signatures only):

- `fn abspos_boxes_in_tree_order(doc, running_store) -> Vec<usize>`
  - Pre-order walk over `layout_children`, then `children`, plus `before` / `after`.
  - Skips subtrees of `position: fixed` boxes, running elements and multicol containers (non-goals).
  - Outer boxes come first. Reason: a nested box's start reads its CB's fragments.
- `enum Anchor { Top { cb: Option<usize>, offset: f32 }, Bottom { cb: Option<usize>, from_end: f32 }, Static { after: Option<usize>, parent: usize, offset: f32 } }`
  - `cb: None` means the initial containing block.
  - Inset axes come from `explicit_inset_axes` (`:6125`) and values from `resolve_viewport_cb_location` (`:6212`) against
    the CB padding box, as the nested-abs window code already does (`:6005-6041`).
  - `Static` anchors to the previous in-flow sibling's last fragment plus the Taffy gap, or to the parent's first fragment
    if there is no sibling. Reason: a sibling moved whole to the next page leaves a strut. A Taffy offset would put the box
    that many px too high (case A9b).
- `fn locate_forward(frags: &[Fragment], offset: f32, strips: PageStrips) -> (u32, f32)` and
  `fn locate_backward(frags: &[Fragment], from_end: f32, strips: PageStrips) -> (u32, f32)`
  - Map a CB-local offset through the CB's fragments: fragment k covers `[Σ h_<k, Σ h_≤k)`.
  - Past the last fragment, or before the first, continue with full page strips.
  - For the ICB, page = `floor(Y / page_h)` with the window pass's `vh` snap (`:5837-5857`), so WPT `top: N·100vh` lands
    where it does today.
- `fn window_cuts_lines(doc, root_id, root_y_icb, page_h) -> bool`
  - True when some inline root in the in-flow subtree (not crossing a clipping boundary, `clips_overflow` `:6174`) has its
    first line top and last line bottom in different page rows under the window placement.
- `enum Disposition { Keep, Place, Flow }` and `fn abspos_disposition(doc, geometry, id, start, strips) -> Disposition`
  - **Keep:**
    - the box has geometry and none of its lines end below the page they start on;
    - or it is a body-direct root with `!window_cuts_lines`;
    - or it sits under a clipping, transformed or multicol ancestor up to its CB;
    - or it is inside a kept body-direct root.
    - Reason: today's output is complete there; this keeps WPT `fixedpos-*`, the VRT and the examples byte-identical.
  - **Place:** no geometry, and the box fits below its start → one fragment at the start plus `record_subtree_descendants`
    (`:2313`). This is the same shape `record_descendant` gives a whole-emitted parent.
  - **Flow:** otherwise.
- `fn clear_subtree_geometry(geometry, doc, id)`
  - Clears `fragments`, `line_slices`, `line_split_insets` and `multicol_pages` of the box and its in-flow descendants. It
    does not descend into nested out-of-flow boxes; those are handled in their own turn.
  - Reason: text placed once. Without it, `record_descendant`'s or the window's fragments would draw the text a second
    time.
- `fn abspos_line_split(child: &Node) -> Option<LineSplit>`
  - The child-side checks of `line_split_candidate` (`:4636-4678`): at least 2 lines, not atomic inline, not multicol, not
    floating, not clipping, not transformed.
  - `line_split_candidate` is refactored to call it, output-neutral. Reason: its parent-side checks are about the in-flow
    parent's formatting context, which an abspos box does not take part in.

**Flow.** Clear the subtree, then fragment it as its own flow from `(start.page, start.y)` at the box's border-box x. The
box gets `is_repeat = false`.

- **The box is an inline root** (an abspos `<p>`, R1): `fragment_nested_inline_root(…, y_in = start.y,
  page_in = start.page, strips, tail = 0, top_floor = 0, &split, depth)` (`:4761`). It records tiling `line_slices`
  (`:4808-4812`), `line_split_insets` (`:4820`) and the descendants (`:4827`).
- **Otherwise** (a wrapper, R2): `fragment_block_subtree(…, parent_id = id, parent_w, parent_x_in_body, start.page,
  start.y, strips, 0, 0.0, 0.0, false)` (`:3002`).
  - It emits the wrapper's per-page frames and its children.
  - A paragraph child taller than a page is split by `nested_line_split` (`:4687`). A shorter one that does not fit moves
    whole: F30c, the same as in flow.
- **Bound.** At most `MAX_PAGES` strips per flow. A shared `emitted` count against `MAX_SUBTREE_PAGE_FRAGMENTS`; above it
  the box falls back to Keep. Reason: the DoS guard of `:5933-5975`.
- **Page extension.** Fragments on new pages raise `implied_page_count`. Under a clipping boundary the box is Keep
  (fulgur-xa9q).

**Reuse.**

- `fragment_inline_root` (`:4982`) could be reused: it takes a start page, a cursor, `strips` and line metrics, and writes
  tiling slices. But it reserves no padding or border (the body-level gap, F30c), records no descendants and sets no
  `line_split_insets`.
- `fragment_nested_inline_root` does all three and leaves room below the last line through `plan_line_breaks` (`:4553`,
  orphans and widows 2). So the abspos `<p>` uses the nested one.
- Wrappers use `fragment_block_subtree`, which calls the nested one for tall children.
- Neither function needs a change.

**Cases.**

| Case | Disposition | Result |
|---|---|---|
| abspos `<p>` taller than the rest of the page, or than a page | Flow | lines split at line boxes from its start, slices tile |
| abspos wrapper of paragraphs | Flow | `fragment_block_subtree` from the start, wrapper frame per page |
| abspos in a relative parent (zero-height, entered, or whole-emitted) | Place or Flow (Keep if already complete) | start = CB fragment + inset |
| abspos box that fits | Keep (or Place when it had no geometry) | unchanged, or newly drawn |
| `top`, or `top` + `height` | `Anchor::Top` | content fragments past `height` too (overflow is visible) |
| `bottom` with `top: auto` | `Anchor::Bottom`, located back from the CB's last fragment | part above the ICB top stays unprinted, like Chrome (probe) |
| `top` + `bottom` | `Anchor::Top`, Taffy height | content past the box continues on the next pages (probe) |
| no insets | `Anchor::Static` | after the previous sibling's real placement |
| `position: fixed` | **out of scope** | repeated whole per page, unchanged (CSS 2.1 §9.6.1) |

**Byte identity, predicted.**

- **Fixtures 01–14.** There is no `absolute`, `fixed` or `relative` in any `index.html`, and the compiled `tailwind.css`
  has no positioning utility (grep). The pass finds no boxes, so all 14 are byte-identical.
- **Fulgur VRT.** Only `paint/opacity.html`: two abspos boxes in a 200 px `.stack`, recorded by `record_descendant` and
  complete → Keep, byte-identical. (`gcpm/*` match `position: running()`, not abspos.)
- **Examples.** Only `examples/transform/style.css` (`.ghost`, `.box`, `.label` in `.cell{position:relative;
  height:130px}`). It is the only possible mover, and only if a `.cell` is entered across a page: its abspos children have
  no geometry today and would then be drawn. Step 4 names the outcome.
- **WPT `css-page/fixedpos-*`** and `tests/abs_positioned_pagination.rs`: single-line text or height-only boxes → Keep;
  the expectations stay as they are.

## 4. Acceptance

**Ours: new `crates/htmltopdf/tests/abspos_flow.rs`.**

- Setup as in `avoid_wrappers.rs`: A4, 15 mm, Inter, strict, `RenderOptions::default()`, `p{margin:0;line-height:20px}`.
- Helpers copied from it: `printed_words` (`pdftotext -bbox`), `page_of`, `assert_every_word_once` (every word once, in
  order, none below the content box by more than 5 pt, none above it by more than 2 pt).
- Expectations come from the step 1 Chrome probes.
- Each case is the body content.
- Pages for the full-width 2000- and 5000-word paragraphs are measured by the probe (about 6 and 15).

1. **A1 (R1):** `<div style="position:relative"><p style="position:absolute;top:0;left:0;width:400px">` 200 words (`avoid`
   and `auto` twins). 200 words on page 1, `word00000` top within 1 pt of the content top. **Red.**
2. **A2:** `<p style="position:absolute;top:0;left:0">` 2000 words directly in body. Every word once; page count equals
   Chrome's. **Red.** Twin with `body{margin:8px}`: every word once, none above the content top (catches the page-0
   offset).
3. **A3 (R2):** `<div style="position:absolute;top:0;left:0;right:0"><p>` 5000 words, plus R2's (a) and (b) shapes. Every
   word once; page count equals Chrome's. **Red** (today about 400).
4. **A4:** `<div style="position:absolute;top:0;left:0;right:0">` of ten `<p style="margin-bottom:20px">` of 300 words.
   Every word once. The page count is asserted within Chrome's + 2 (F30c moves short paragraphs whole). **Red.**
5. **A5:** `<div style="height:600px"></div><div style="position:relative"><p style="position:absolute;top:0">` 1000
   words. `word00000` on page 1 at 600 px ± 1 pt; every word once; the page count Chrome gives. **Red.**
   - **A5b:** `<div style="height:900px"></div><div style="position:relative;height:60px"><p style="position:absolute;
     top:0">` 300 words (whole-emitted parent, box crosses the page). Every word once. **Red.**
6. **A6:** `<div style="position:relative">` of 60 `<p>` lines of 12 words, and `<div style="position:absolute;top:0;
   right:0">ZAPLACENO</div>`. `ZAPLACENO` once, on page 1, top within 2 pt of Chrome's. **Red** if step 1 confirms the
   entered-parent loss.
   - **A6b:** the same with `bottom:0`: `ZAPLACENO` once, on page 2, near the last line, as in Chrome.
7. **A7 (guard):** `<div style="position:relative;height:200px"><div style="position:absolute;top:20px;left:20px">Poznámka
   </div></div>`. Same words and positions as main.
8. **A8:** 1000 words in `<div style="position:absolute;…">`, each as Chrome gives:
   - (a) `top:300px;height:1500px`;
   - (b) `top:0;bottom:0`;
   - (c) `bottom:0` (expectation: the words Chrome prints, the same per page ± 1 line).
9. **A9:** `<p>before</p><div style="position:absolute"><p>` 1000 words: `word00000` below `before` on page 1.
   - **A9b:** `<div style="height:900px"></div><div style="height:300px"></div><div style="position:absolute"><p>` 1000
     words. `word00000` on page 2 at y 300 px ± 1 pt. **Red.**
10. **A10 (guards):**
    - `<div style="position:fixed;bottom:0">Strana</div>` with 3000 in-flow words: `Strana` once per page, PDF identical
      to main;
    - `position:fixed` around 2000 words: words per page identical to main (Chrome recorded, non-goal).

**Fork: lib tests** next to `:7720-8112`. Through `Engine::build_drawables_and_geometry_for_testing_no_gcpm` (`:7944`) so
the `engine.rs` wiring is covered.

- **Red:**
  - `abspos_paragraph_taller_than_a_page_is_split_between_lines` (fragments on pages 0..=2, `line_slices_tile`);
  - `abspos_paragraph_in_a_zero_height_relative_parent_gets_geometry`;
  - `abspos_child_of_an_entered_parent_is_placed_at_its_cb`;
  - `abspos_wrapper_is_fragmented_from_its_start`;
  - `abspos_bottom_anchored_in_a_split_cb_lands_on_its_last_page`;
  - `abspos_static_position_follows_a_moved_sibling`.
- **Guards:**
  - `abspos_that_fits_keeps_its_geometry` (the table is equal before and after the pass);
  - every existing `position_absolute_*` / `position_fixed_*` test, the `:9971`, `:10021` and `:10363-10420` tests;
  - `tests/abs_positioned_pagination.rs`.
- **Unit:** `locate_forward` / `locate_backward` (across pages, page-0 offset, past the end), `abspos_disposition` (each
  arm), `window_cuts_lines` (single line on a page edge `100vh`: false).
- **`render_smoke.rs`:** `render_smoke_abspos_paragraph_spans_pages`, an end-to-end render. Codecov does not see this path
  otherwise (fulgur `CLAUDE.md`).

**Fixture `15-paid-stamp`** (new). This is an invoice-realistic shape.

- Content: a 02-style two-page invoice whose outer `<main style="position:relative">` holds the header, parties, an items
  table that spans the page break, totals and payment details.
- A stamp `<div class="border-4 border-green-600 px-4 py-2 text-2xl font-bold uppercase text-green-700"
  style="position:absolute;top:0;right:0">Zaplaceno</div>`, plus a bottom-anchored note
  (`style="position:absolute;bottom:0;right:0"`, "Uhrazeno převodem dne 12. 10. 2026"). Czech content, unrotated (transform
  is not under test). Positions are inline because the compiled `tailwind.css` has no positioning utilities and is not
  rebuilt here; any colour/border class missing from the compiled sheet is also set inline. The README says so.
- PNG: the stamp at the top right of page 1 and the note at the end of the invoice on page 2, as in Chrome. Today both are
  missing (prediction; step 1 confirms with A6).
- Record compare numbers with main's binary and the branch. Threshold = worst page + 1 point. Both have 2 pages, so the
  fixture is not in `PAGE_COUNT_MAY_DIFFER`.

**Pixel diff.** Fixtures 01–14 and their numbers are unchanged; 15 is new.

## 5. Steps

1. **Confirm (gate).** Scratch only; temporary logs are removed afterwards.
   - Recreate R1, R2 and the §4 cases. Render with main's binary and count words per page.
   - Log at `:1376`, `:3167`, `:1484` and in the window pass. Expected:
     - A1, A5 and A6 have no geometry entry for the abspos node;
     - R2 has full-height fragments with empty slices, and `paragraph_lines_for_page` returns `None` on page ≥ 1.
   - Check whether window fragments on page ≥ 1 sit `body_offset` too high (`:5959` against `render.rs:290`). If so, file
     it as a new finding.
   - Chrome probes (Playwright, m141) of every §4 case at `20px` and `normal`, plus `break-inside:avoid` on A1/A2.
   - **Stop and write `docs/postmortems/2026-10-18-f30g-*.md`** if:
     - Chrome does not fragment A2 (it prints page 1 only);
     - or A1 has geometry (another cause).
   - If Chrome does not append pages for A5, drop page extension for nested flows and record it.
2. **Red tests.** Ours: `abspos_flow.rs`. Fork: the lib and unit tests and `render_smoke`. Record which are red.
3. **Fork, output-neutral.** `abspos_line_split` (refactoring `line_split_candidate`), `locate_*`,
   `clear_subtree_geometry`, `abspos_boxes_in_tree_order`, with their unit tests. Then:
   - `--lib` on crates.io and on local Blitz (the same 13 / 0 failures as at `a2b2d3a`);
   - the 42 `tests/*.rs` files one by one;
   - VRT byte-identical;
   - fmt and clippy.
4. **Fork, nested boxes:**
   - `Anchor`, `abspos_disposition` (Keep / Place / Flow for boxes that are not body-direct), `fragment_abspos_flows`;
   - wiring in `engine.rs` at both sites;
   - A1, A5, A6, A9 green.

   Then the step 3 checks, plus the 21 examples before and after with the `transform` outcome named. **An unpredicted
   mover means stop and write a postmortem.**
5. **Fork, body-direct roots:** `window_cuts_lines`, and Flow instead of the window model for A2, A3, A4, A8; fixed boxes
   repeat on the added pages. Then:
   - the step 4 checks;
   - the `fulgur-wpt` css-page subset, if the WPT checkout is present (otherwise record that the lib tests stand in);
   - the `fixedpos-*` expectations must not change.
6. **Ours:**
   - lock bump;
   - `cargo test --release --locked --no-fail-fast`;
   - fixtures 01–14 `cmp` against main's binary;
   - compare unchanged;
   - render time of 01 and 02 within noise.
7. **Fixture 15:**
   - HTML, README, Chrome reference (the scratch `render-ref.mjs`, as in F83 §7);
   - compare and threshold;
   - look at the PNGs.
8. **Docs:**
   - findings: F30g done, any new finding from step 1 filed;
   - the `docs/upstream.md` row;
   - §7 here.

   Then `vrt-validator`. `pdf-reviewer` is not needed: `render.rs`, the writer, fonts and metadata are untouched.

## 6. Risks and non-goals

- **Two models for abspos.** Boxes whose text crosses a page switch from the window model to flow. Their backgrounds then
  follow the fragment frames instead of a sliding window. That is visible only in documents that lose text today. A
  fixed-`height` box taller than its content may end where its content ends (the in-flow F83c remainder).
- **Start mapping.** The stitched mapping through the CB's fragments can differ from Chrome by struts inside the CB. A6b
  and A9b pin the common shapes.
- **Clearing.** `clear_subtree_geometry` must not touch fixed or nested out-of-flow entries. The guards pin this.
- **Pages.** Flows can add pages beyond the in-flow content; they are bounded by `MAX_PAGES` and the shared fragment
  budget.
- **Rebase.** The `line_split_candidate` refactor touches F30b code; `fragment_block_subtree` is unchanged.
- **Non-goals:**
  - fragmenting `position: fixed` (repeated whole; the part taller than a page stays unprinted);
  - abspos inside fixed, multicol (F30d), clipping or transformed ancestors;
  - `break-inside: avoid` on the abspos box itself;
  - named pages and forced breaks inside abspos content beyond what `fragment_block_subtree` already does;
  - F30c inside abspos wrappers;
  - paint order and `z-index`;
  - Taffy's parent-as-CB beyond `explicit_inset_axes`;
  - the offset shift of the window path (filed separately if step 1 confirms it);
  - `left` + `right` stretch (F32j);
  - right-to-left text.

## 7. Result

Landed on branch `f30g-abspos-paragraph`; fulgur fork `htmltopdf/srgb-colours` **`129af34`** (three commits on top of `a2b2d3a`: `dff05f3` the pass, `a451cb9` a first bottom-padding fix that did not hold, `129af34` the one that does); the Blitz fork is
unchanged (`52b22ef`). `Cargo.lock` pins `129af34` (`cargo update -p fulgur` changed that one entry). Status: **done**.

**Step 1 (gate), no stop.** Scratch: `/tmp/claude-0/vrt-f30g/` (`probe.py`, `sweep.py`). Chrome m141, A4, 15 mm, Inter, 20 px and `normal` lines (same pages and words per page in both).
- R1 (`abspos.html`, avoid and auto): ours 0 of 200 on 1 page; Chrome 200 on page 1. R2 `abs div > p` 5000 words: ours 372-386 words on 14-15 pages, Chrome all of them on 15. `abs > static > p` (a): 386; `relative > abs > p` (b): 0.
- The causes held, measured with a temporary test (removed) on the engine's passes: the abspos `<p>` of A1 and of A5 has no geometry entry (its parent is recorded alone with height 0); A6's `ZAPLACENO` has none either (the relative parent has 2 fragments, `960 + 240`); A5b's box has one whole fragment `(0, 900) 600` from `record_descendant`; R2 / A2 have three full-height fragments `y = 0, -971, -1943` and empty `line_slices`; reading `paragraph_lines_for_page`, page 0 takes every line from the top and page k >= 1 gets `consumed >= height` and so `None`.
- **Window offset:** confirmed, a new finding (**F92**). With `body{margin:8px}` an abspos box on the window path is 8 px (6 pt) too far left on every page and 8 px too high on pages >= 1 (`ofs8`: 36.5 / 97.5 pt against Chrome's 42.7 / 99.8 pt; `ofs40`: 12.5 / 73.5 pt). Not fixed here (the pass is untouched); boxes F30g fragments do not have it.
- **Stop conditions:** Chrome fragments A2 (6 pages, `avoid` or not); A1 has no geometry (as predicted). Chrome appends pages for A5 (4 pages), so page extension stays.

Chrome (words per page, first word) against main's binary before the change and the branch's after:

| Case | Chrome | main | branch |
|---|---|---|---|
| A1 (R1) 200 words | 200, p1 | 0 | 200, p1 |
| A2 2000 words in body | 6 pages: 351 / 350 / 350 / 364 / 353 / 232 | 372, 1 page | the same numbers |
| A2 with `body{margin:8px}` | same as without | 371 | 2000 words, 6 pages, a few words fewer per page (F94: width) |
| A3 R2 5000 words, (a), (b) | 15 pages | 372 / 372 / 0 | 15 pages, all words, every page within a word of Chrome's |
| A4 ten `<p>` of 300 words | 9 pages | 1821 words, 9 pages | all 3000, 10 pages (F30c moves a short paragraph whole) |
| A5 spacer 600 + relative > abs p | p1 at 600 px, 4 pages | 0 | p1 at 600 px, 4 pages, same split |
| A5b whole-emitted parent | 35 words p1, 265 p2 | 56 | 35 / 265 |
| A6 stamp `top:0` / `bottom:0` | once, p1 level with line 1 / p3 at the last line | not printed | once, p1 / p3, same places |
| A8a `top:300px; height:1500px` | 246 / 350 / 350 / 54 | 510 | the same |
| A8b `top:0; bottom:0` | 351 / 350 / 299 | 0 | the same |
| A8c `bottom:0` | 355 words (645..999) | 369 | 369 (window path, unchanged; F95) |
| A9 `<p>before</p>` + abs div | 3 pages, 15 pt below `before` | 365 | 3 pages, 15 pt below |
| A9b spacers 900 + 300 | page 2 at **191 px** (the block splits) | 302 | page 2 at 300 px, 5 pages (F93) |
| A10 fixed `Strana`; fixed taller than a page | 9 times; 358 words p1 | the same | the same (guards) |

**Red before** (main at `53ef8ab` with fork `a2b2d3a`): ours `abspos_flow.rs` 15 of its 20 tests (the 5 green are guards: A7, A8c, the two fixed tests, the fixed repeat on added pages); fork lib: 12 of the 24 `abspos_flow` tests (the other 12 are the helper unit tests and guards:
`abspos_that_fits_keeps_its_geometry`, `fixed_box_repeats_on_the_pages_an_abspos_flow_adds`, `many_static_abspos_siblings_are_bounded`) and `render_smoke_abspos_paragraph_spans_pages`. The first fork commit's message says 8 red; that was a miscount; 11 were red of the 23 tests it had, 12 of 24 now (pass disabled through a temporary environment check, removed).
**Green after:** all of them.

**What was built** (fork `pagination_layout/abspos_flow.rs`, a submodule, `pub use fragment_abspos_flows`; the plan said "in `pagination_layout.rs`", the logic is as in §3): `fragment_abspos_flows`, `abspos_boxes_in_tree_order`, `Anchor` (`Top`, `Bottom`, `Static`), `locate_forward` / `locate_backward` /
`locate_icb` / `carry_over`, `abspos_disposition` (`Keep`, `Place`, `Flow`), `window_cuts_lines`, `lines_end_below_page`, `clear_subtree_geometry`, `abspos_line_split` (`line_split_candidate` calls it, output-neutral); wiring in `engine.rs` at both sites; a doc paragraph in `lib.rs`.

**After:**

| Check | Result |
|---|---|
| fork `--lib`, crates.io Blitz | 2105 passed, 13 failed: the same 13 as at `a2b2d3a` (9 `seed_image_natural_size_tests`, 3 `convert::table`, 1 `pagination_layout` table test) |
| fork `--lib`, local Blitz (`/home/user/blitz`, `52b22ef`) | 2118 passed, 0 failed |
| fork `render_smoke` | crates.io Blitz 252 passed, 3 failed (the same `outside_*_markers`, `table_parts_with_a_non_table_display`); local Blitz 255 passed |
| fork other test binaries | all 42 `tests/*.rs` one by one (local Blitz): no failure |
| fork `fulgur-vrt` (`fonts-noinInter.conf`, crates.io Blitz) | 31 tests green, goldens untouched |
| fork 21 examples (debug CLI at `a2b2d3a` against the branch) | byte-identical, 21 of 21. **`transform`: unchanged.** Its `.ghost`, `.box`, `.label` (children of `.cell{position:relative; height:130px}`) are placed whole by `record_descendant`, are complete or transformed themselves, so the pass keeps them; no `.cell` is entered across a page |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| fork WPT `css-page` subset | **not run**: no WPT checkout on this machine; the lib tests (`position_absolute_*`, `position_fixed_*`, `abs_positioned_pagination.rs`) stand in and are green |
| ours `cargo test --release --locked --no-fail-fast` | 389 passed, 0 failed (20 new `abspos_flow.rs`; `fixtures.rs` renders 15 strict); `cargo fmt --check` and `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-14 | `cmp` byte-identical to the render of main's binary (`/tmp/claude-0/htmltopdf-main-53ef8ab`): 14 of 14; `compare.py` numbers unchanged |
| render time 01 / 02 (25 runs, median) | 20.1 / 55.4 ms on main, 18.8 / 54.1 ms now: noise |
| fixture 15 | new: **2 pages like Chrome, 8.7 / 4.9 %** against main's binary 8.9 / 4.8 % (stamp and note missing), threshold 0.097 (worst page + 1 point) |
| generated abspos documents against Chrome (`sweep.py`: 11 containing-block shapes x 3 box kinds x 8 inset sets x 3 lengths x 3 spacers; 400 + 300 documents, two seeds) | 0 with fewer words than main; words per document within 8 of Chrome's: 304 / 400 and 233 / 300 (main: 56 and 39); page counts equal Chrome's: 368 / 400 and 274 / 300 (main: 179 and 136). The 38 + 34 documents with "duplicated" words are in-flow text that repeats the box's words by construction, with Chrome's counts. Left: transformed and clipping containing blocks (Keep, F95), `bottom: 0` above the page (F95) |

Fixture 15 (`out/compare/15-paid-stamp-p1.png`, `-p2.png`, git-ignored): the stamp is top right of page 1 level with the title, the note is at the right on page 2 32 pt under the last payment line, both as in Chrome. The remaining diff is text drift, the totals table
starting about 20 px above Chrome's after the split items table (not F30g, not analysed) and the `<tr>` rule of Blitz #386.

**Deviations from the plan:**

1. **A9b's expectation is not Chrome's.** The plan says 300 px; the Chrome probe says 191 px (Chrome splits the empty block). The test pins ours (300 px, after the sibling, as designed in §3) and the difference is **F93**. The plan's reasoning for `Static` (a Taffy offset puts the box inside the moved sibling) holds; its premise that Chrome agrees does not.
2. **A2's body-margin twin** does not assert the words per page of the plain case (the plan: every word once, none above the content top): the box is 16 px narrower in ours because Taffy lays it out against its parent (**F94**), so a page holds a few words fewer; the page count and the first line agree. **A8c** asserts a range (340-390 words, Chrome 355) and the last word: it is the window path, unchanged.
3. **A1 and A5 compare against an in-flow twin** (`content_top_glyph_y`) instead of an absolute y, because the glyph box of a line sits 2.7 pt higher in Chrome's PDF than in ours; A7 the same.
4. **Stretched boxes and wrappers of boxes.** A body-direct box that Taffy makes 0 px tall (`top:0; bottom:0` in an empty body, A8b; a wrapper that holds only abspos boxes) has no window geometry; `abspos_disposition` flows it from `content_extent` / `window_cuts_lines`, and `window_cuts_lines` follows nested abspos boxes the way the window pass places them. Not in §3, needed for A8b and for a paragraph in an abspos wrapper. Also `ensure_zero_height_chain` (a containing block under several zero-height wrappers gets zero-height fragments) and `SIBLING_SCAN_BUDGET` (the static-position search is bounded).
5. **`Anchor::Bottom`** measures to the bottom of the containing block's last in-flow child plus its bottom padding (`closing_deficit`), because the block fragmenter closes the last fragment at the last child. The first attempt used Taffy's height and did not move fixture 15.
6. **Steps 3-5 in one pass** (the fork checks ran on the final code, three times: after the pass, after the nested and chain additions, after the bottom fix), as in F83b. The flow is built in a scratch table and merged, which gives the plan's "falls back to Keep above the budget" without undoing anything.
7. **`vrt-validator` was not run** (a subagent cannot start another); the 700-document sweep against Chrome stands in for part of it.

Filed: **F92** (window path body offset), **F93** (static position after a sibling ours moves whole), **F94** (abspos laid out against its parent), **F95** (text above the page area, transformed and clipping ancestors).

Not verified here: `vrt-validator`; `pdf-reviewer` (not required: `render.rs`, the writer, fonts and metadata untouched); the spec citations of §2 (from memory; the Chrome probes stand in); the WPT `css-page` subset (no checkout); an abspos box inside `display: flex` / grid / table parents beyond the sweep's `flex` shape, inside inline `position:relative` spans (Keep), with `margin` on a `bottom`-anchored box in a split containing block, in a right-to-left or vertical writing mode, with `break-before` inside the box; `line-height: normal` for A4-A10 (only A1 and A3 are tested with it; the probes agree with 20 px); the release CLI against the fork examples (they ran on the debug CLI).
