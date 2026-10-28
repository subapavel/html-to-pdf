# F30f-b (clip part): a clip box shorter than its content shows, or writes into the PDF, what it hides

Date: 2026-10-28. Finding: F30f-b in `2026-10-15-m0-findings.md`, the clip bullets only. Status: **done** (§7). Fork commits `ce76bbfa` (writer), `c700b372` (fragmenter), `16d281a3` (only constrained boxes, §7 "Auto-height clip boxes"), `820c6e3e` (table boxes, padded and bordered boxes) and `aecc013a` (boxes between the clip box and its paragraph; §7 "Padded and bordered clip boxes") on `htmltopdf/srgb-colours`; ADR-0009.

- The fix goes on `subapavel/fulgur` `htmltopdf/srgb-colours` (`Cargo.lock` pins `516558c`), carried via `[patch.crates-io]`.
- Line numbers are from the fork checkout, in `crates/fulgur/src/pagination_layout.rs` unless another file is named. Re-read them before editing.

The other F30f-b bullets stay open as their own PRs (one finding per PR):

- inline-blocks that share their line;
- inline-flex / inline-grid;
- an inline-block that is not first on its page (ADR-0004);
- `overflow:hidden` on a `td` (F51) or on a grid item;
- the 60-row table's cells per page.

## 1. Problem

A box clips its block axis when its computed `overflow-y` is not `visible`. If such a box has a `height` or `max-height` smaller than its content, today one of three things happens:

- it shows the text it should hide;
- it writes the hidden text into the PDF;
- it loses the part it should show.

The probes come from F30f step 1 (Chrome m141, A4, 15 mm, Inter, `W` = 4000 words, 20 px lines). Their files are gone; recreate them under `out/scratch/f30f-b/` with the F30f §1 setup.

| # | Body content | Chrome | Ours (main) | Kind |
|---|---|---|---|---|
| R1 | `<div style="overflow:hidden;height:300px">W</div>` | 1 page, 105 words | 12 pages, 4000 words | reveals |
| R2 | `<div style="display:flex;overflow:hidden;height:300px"><p>W` | 1 page, 105 | 1 page, visible part right, text layer 372 | hidden text in the PDF |
| R3 | `<div style="overflow:hidden;height:600px"><p>W` | 1 page, 211 | 1 page, text layer 372 | hidden text in the PDF |
| R4 | `<div style="overflow:hidden;max-height:2000px"><p>W` | 2 pages, 701 | 1 page, 372 | visible text lost |

Causes, from reading the code (step 1 confirms them):

1. **R1.** The body-level line path of `fragment_pagination_root` (`:2533-2607`, `collect_inline_line_metrics` → `fragment_inline_root`) checks no barrier.
   - It gives the div one fragment per line range over 12 pages.
   - `draw_under_clip` (`render.rs:2564-2654`) then clips each page at that fragment (`frame_fragment`), so nothing is hidden.
   - The same happens to any multi-line body-level clip box whose lines cross the page bottom, even when the box itself fits.
2. **R2, R3.** `LineBarrier::ClipsItsContent` (`box_barrier`, `:5904`) keeps the `<p>` whole.
   - The clip path hides its lower lines, but `draw_paragraph_inner_paint` (`render.rs:4611`) writes every glyph anyway.
   - So `pdftotext`, copy, search and screen readers read text nobody sees.
3. **R4.** The same barrier keeps the `<p>` whole, so the box never advances a page.
   - Its frame closes as one fragment at the end of the content: `closing` (`:5232-5265`) is `parent_h - children_bottom`, which is 0 here.
   - Inferred, step 1 checks: the cursor returned is below the whole paragraph, so content after the box (R3 too) is placed off the page.

## 2. Spec and Chrome

Spec citations are from memory and unverified (css-spec-researcher was not run). Where the spec leaves a choice, the Chrome probes decide.

- **css-overflow-3 §3.**
  - `hidden`, `clip`, `scroll` and `auto` clip the content at the padding box (`overflow-clip-margin` applies to `clip` only).
  - `visible` computes to `auto` when the other axis is neither `visible` nor `clip`. So `overflow-x:hidden` clips the block axis too, and `overflow-x:clip` alone does not.
  - Therefore "clips the block axis" means computed `overflow-y` ≠ `visible` (horizontal writing modes).
- **css-break-3 §2 (monolithic).** A UA may treat as monolithic elements with `overflow` `auto` or `scroll`, and elements with `overflow: hidden` "and a non-auto logical height (and no specified maximum logical height)".
  - So R4 (`max-height`) must fragment.
  - `height` with `hidden` is the UA's choice.
  - `clip` is never monolithic.
- **css-break-3 §4.1 / §4.4.** The gap between two line boxes is a class B break point; orphans and widows are 2 (the F78 rule).
- **css-break-3 §5, fragmented box size** (section number and wording unverified). A box with a definite block size uses that size up across its fragments, and a break does not make it taller. The clip applies per fragment (F30f §2).
- **CSS 2.1 §11.1.1.** Content outside the clip "is clipped". No spec says anything about a PDF's text layer.

**Chrome, measured** (table above). R1 shows 15 lines (105 = 15 × 7 words), R3 30 lines, R4 100 lines over 2 pages: Chrome fragments a `max-height` clip box and clips each fragment.

**Chrome, inferred.** Chrome writes no text for line boxes wholly outside the clip (Blink's paint culling). A partly visible line is written and clipped (P8 measures it).

**Chrome, unknown until step 1:**

- `height:3000px` (taller than a page);
- a 300 px box that straddles the page bottom: fragmented or moved whole;
- `auto` / `scroll`;
- links in clipped text.

**Decisions.**

1. **A line box wholly outside the block-axis clip on its page is not written to the PDF**: no glyphs, no link, no marked content. A partly visible line is written and clipped. Reasons:
   - the reader sees none of it;
   - Chrome's numbers match it;
   - hidden text holding an old amount or VS would be read as data by `pdftotext`, invoice matching or a screen reader.

   This is recorded in **ADR-0009**, because it is a writer policy that no spec states.
2. **A clip box whose content reaches past its clip is fragmented by its own block size.**
   - Its frames add up to its border-box height H.
   - Its content fragments as usual (lines split between line boxes, F78).
   - Nothing at or below the clip bottom gets a page.
   - What follows the box starts at the box's bottom.
   - Where Chrome keeps such a box whole (P4 and P5 decide; the spec allows both), we keep it whole too.
3. **Content-sized clip boxes are unchanged** (F30f; fixtures 05 and 16).

## 3. Where it belongs

**Only the fulgur fork.**

- Blitz and Taffy size the box right (300 px), and Parley gives the right lines.
- What is placed and what is drawn is decided in fulgur's fragmenter and writer. Upstream fulgur has the same code.
- No upstream PR or issue (standing decision).
- Our crate gets only tests, fixture 28 and docs.

**Fragmenter (`pagination_layout.rs`):**

```rust
fn clips_block_axis(node: &blitz_dom::Node) -> bool // clips_overflow && computed overflow-y != visible
fn clip_block_limit(doc: &BaseDocument, node: &blitz_dom::Node) -> Option<f32> // padding-box bottom (own coords) when content reaches past it
fn ancestor_clip_limit(doc: &BaseDocument, node: &blitz_dom::Node) -> Option<f32> // tightest clipping ancestor below body, in node coords (layout_parent chain)
impl LineSplit { fn clip_to(self, limit: f32) -> LineSplit } // drops lines whose top >= limit
```

- **`box_barrier`** tests `clips_block_axis` instead of `clips_overflow`. Reason: an `overflow-x:clip` box hides nothing in the block axis.
- **`fragment_pagination_root`, body-level line path (`:2533`).**
  - A child with a `clip_block_limit` is not line-split while it fits the room below the cursor; it is placed whole, like a nested one.
  - Otherwise it is split with `clip_to`, and the cursor after it is its top plus H.
- **`nested_line_split` / `fragment_block_subtree`, for a child under a clip (`ancestor_clip_limit`).**
  - When the box does not fit the room, `ClipsItsContent` no longer keeps the child whole; the split is cut with `clip_to`.
  - A child that starts at or below the limit gets one zero-height fragment and no page advance. Reason: counters, string-set and bookmarks keep their page.
  - The last frame is H minus what the earlier frames used (`closing` is capped), and the cursor returned is the box's bottom.
- **`PaginationGeometry.clipped_tail: bool`.** The last `LineSlice` may end before the last line. `line_slices_tile` and `paragraph_lines_for_page` (`render.rs:4684`) accept that instead of falling back to measuring.

**Writer (`render.rs`, `paragraph.rs`, `draw_primitives.rs`):**

```rust
pub struct Canvas<'a, 'b> { /* … */ pub clip_band: Option<(Pt, Pt)> } // page-space block-axis clip, intersected
fn line_is_clipped_out(line: &ShapedLine, origin_y: Pt, band: (Pt, Pt)) -> bool // 0.5 pt slack
```

- **`draw_under_clip`** intersects the band with its padding box when `style.overflow_y` clips, and restores it on exit.
- **`draw_under_transform`** sets the band to `None` inside. Reason: the coordinates there are not page space, so output there stays as today.
- **`draw_shaped_lines`** (`paragraph.rs:794`) skips clipped-out lines before it writes glyphs, links, inline boxes or tags.

## 4. Acceptance

**Ours: new `crates/htmltopdf/tests/clip_boxes.rs`.**

- Helpers from `wrapper_line_split.rs`: `printed_words`, `page_count`.
- Setup: A4, 15 mm, Inter, strict, `p{margin:0;line-height:20px}`.
- Expected numbers are Chrome's from step 1.
- Every case puts `<p>AFTER</p>` after the box. AFTER must appear once, below the box, within 2 pt of Chrome's y.

1. **C1 = R1:** 1 page, 105 words, none below the box. **Red.**
   - C1p (`> p`) and C1n (`> div > p`): as Chrome. **Red** if main reveals.
2. **C2 = R2:** the text layer holds Chrome's 105 words. **Red.**
3. **C3 = R3:** 1 page, 211 words. **Red.**
4. **C4 = R4:** 2 pages, 701 words ±7 (one line), AFTER on page 2. **Red.**
   - C4t (direct text in the box). **Red.**
5. **C5** (`height:3000px`) and **C6** (300 px box straddling the page bottom, direct text and `> p`): pages and words per page as Chrome.
6. **C7:** `overflow:clip`, `auto` and `scroll` for C1 and C3: as Chrome.
7. **C8, axis combinations.**
   - `overflow-y:hidden` alone: as C1.
   - `overflow-x:hidden` alone: as C1 (y computes to `auto`).
   - `overflow-x:clip` alone with `height:300px`: every word once, pages as Chrome.
8. **C9** (`height:310px`, a partly visible line): words as Chrome (16 lines predicted).
9. **C10** (an `<a href>` in the hidden part): no link annotation; Chrome's count recorded.
10. **Guards, green before and after:**
    - `wrapper_line_split.rs`, `unsplit_paragraph_warning.rs`, `abspos_flow.rs`, `fixed_height_cells.rs`;
    - `--tagged` renders of C1, C3 and C4 succeed.

**Fork tests.**

- **Lib:**
  - `clips_block_axis_reads_overflow_y`;
  - `clip_block_limit_*` (auto, height, max-height, padding, `overflow-x:clip`);
  - `a_body_level_clip_box_that_fits_is_placed_whole`;
  - `a_clip_box_taller_than_the_room_is_fragmented_at_its_block_size` (frames add up to H, nothing below the limit, cursor at the box bottom);
  - `content_below_the_clip_gets_no_page`;
  - `a_clip_box_shorter_than_its_paragraph_keeps_it_whole` and `paragraphs_that_are_not_in_plain_flow_stay_whole` stay as they are.
- **Render units:** `a_line_outside_the_clip_band_is_not_drawn`, `a_partly_visible_line_is_drawn`, `no_band_under_a_transform`.
- **`render_smoke`:** `render_smoke_clip_box_hides_lines_from_the_text_layer`, `render_smoke_max_height_clip_box_spans_pages`. Reason: codecov does not see these paths otherwise (fulgur `CLAUDE.md`).

**Fixture `28-notes-box-clip` (new).** The shape: a template that reserves a fixed box for a note, and an ERP that pastes in a longer one.

- **Content:**
  - the 01 header, parties, 4 items and totals;
  - then, as a **direct child of `<body>`**, `<div class="h-28 overflow-hidden rounded-lg border border-gray-300 p-4 text-sm">Poznámka: …</div>` with about 25 lines of direct Czech text;
  - the box ends on page 1, but its hidden lines would run past the page bottom.
- **Classes:** all are in the compiled `tailwind.css` (checked by grep), so there is no rebuild.
- **PNG:**
  - Chrome: 1 page, about 4 lines of the note with the 4th cut, then the box edge.
  - Main, predicted: 2 pages, with the hidden lines printed on page 2 (cause 1).
  - After: 1 page, like Chrome.
- **Test:** the note's words in ours equal Chrome's (`reference/chrome.pdf`), and the page count is 1. A one-line wrapping difference is recorded, not failed.
- **Step 1 check:** main must reveal on this markup; if not, tune the number of items. If it never reveals, the fixture still pins the text layer.

**Pixel diff.**

- **Fixtures 01–27 byte-identical.**
  - Only 05 and 16 use `overflow-hidden`, and both are content-sized.
  - No fixture uses `max-h-*`.
  - `h-28`, `h-12` and `h-full` never appear together with a clip.
- **Bench `b200-mixed` byte-identical.** Its `overflow-hidden` cards are content-sized.
- **Fixture 28 is new.** Its threshold is the worst page + 1 point.

## 5. Steps

1. **Gate (scratch).**
   - Recreate R1–R4. Render them with main's binary and with Chrome (`probe.py`, as in F30f); record words per page and look at the PNGs.
   - Additional probes:

     | P | Shape |
     |---|---|
     | P1p / P1n | C1p / C1n |
     | P4 | `height:3000px` |
     | P5 | straddling box |
     | P6 | `clip` / `auto` / `scroll` |
     | P7 | axis combinations |
     | P8 | 310 px box, partly visible line |
     | P9 | link in clipped text |
     | P10 | position of AFTER |
     | P11 | fixture 28 markup |

   - Log R1's `fragment_inline_root` and R4's `closing` to confirm the causes.
   - Fill in the unknowns in §2.
   - **Stop and write a postmortem** if Chrome's text layer does hold clipped lines (decision 1 then needs a new decision), or if main already leaves them out.
2. **Red tests.** Ours and the fork; record which are red.
3. **Fork, writer:** `clip_band`, `line_is_clipped_out`, and the band in `draw_under_clip` and `draw_under_transform`; write ADR-0009.
   - C2, C3, C9 and C10 go green.
   - Checks:
     - fork `--lib` on both Blitz versions, the 42 test binaries, `render_smoke`;
     - VRT and the 21 examples byte-identical;
     - fmt and clippy `-D warnings`.
4. **Fork, body path:** `clips_block_axis`, `clip_block_limit`, and the place-whole rule in `fragment_pagination_root`.
   - C1 and C8 go green. Then the step 3 checks.
5. **Fork, fragmenting at the block size:** `ancestor_clip_limit`, `LineSplit::clip_to`, `clipped_tail`, the capped `closing` and the cursor, on both paths.
   - C1p, C1n, C4, C4t and C5–C7 go green. Then the step 3 checks.
   - **An output change nobody predicted means stop and write a postmortem.**
6. **Ours:**
   - lock bump;
   - `cargo test --release --locked --no-fail-fast`;
   - fixtures 01–27 `cmp` against main's binary;
   - `bench.py --check` (01, 02 and `b200-mixed` within noise);
   - fixture 28: HTML, README, Chrome reference (`scripts/reference/render.mjs`), threshold, and look at the PNGs.
7. **Docs and review:**
   - findings: the F30f-b clip bullets done, the rest kept; a new finding for `text-overflow` and inline-axis culling;
   - `docs/upstream.md`, ADR-0009, §7 here;
   - then run `vrt-validator`, and `pdf-reviewer` because the writer path changes (merge policy item 5).

## 6. Risks and non-goals

- **Tagged PDF.** A paragraph clipped out completely leaves a structure element with no content; a partly clipped one has a shorter MCID run. `pdf-reviewer` checks; if an empty element is a defect, the converter drops it at tag time.
- **Band coordinates.** A wrong band drops text that is visible. Mitigations:
  - no band under a transform;
  - 0.5 pt slack;
  - C9, C10 and the guards pin it.
- **Page counts can change.** The P4 and P5 shapes may gain a page, but only where Chrome has that page. Where Chrome keeps the box whole, the barrier stays.
- **Metadata in hidden content.** Counters and string-set keep their page through the zero-height fragments. Bookmarks to hidden headings still exist; what Chrome does there is unverified.
- **Speed.** One band test per line; the bench check guards it.
- **Rebase.** `box_barrier`, the body path and `draw_under_clip` were also changed by F30e, F30f and F30g.
- **Non-goals:**
  - the other F30f-b bullets (listed at the top);
  - `text-overflow` and inline-axis culling: Tailwind's `truncate` is not in the compiled CSS, so this becomes a new finding;
  - `overflow-clip-margin` and `line-clamp`;
  - scrollbars of `overflow:scroll` in print;
  - vertical writing modes;
  - `box-decoration-break` (F96);
  - abspos boxes under a clip (F95);
  - a clip box that is itself an `inline-block`.

## 7. Evidence

### Step 1: probes (Chrome m141 vs main's binary; `out/scratch/f30f-b/probe.mjs`, git-ignored)

A4, 15 mm, Inter, `p{margin:0;line-height:20px}`, `W` = 4000 words, `<p>AFTER</p>` after the box. Words per page; AFTER as page@y (pt, `pdftotext -bbox`).

| P | Box | Chrome | main |
|---|---|---|---|
| R1 | `overflow:hidden;height:300px` + direct text | 1 p, 105, AFTER 1@268.1 | 12 p, 4000, AFTER 12@327.9 |
| R1p | same, `> p` | same as R1 | 2 p, 372 / 0, AFTER 2@42.9 (not a reveal: hidden text, AFTER on page 2) |
| R1n | same, `> div > p` | same as R1 | 12 p, 4000 (reveals) |
| R2 | `display:flex`, `> p` | 1 p, 105 | 1 p, 372 in the text layer, AFTER 1@267.9 |
| R3 | `height:600px > p` | 1 p, 211, AFTER 1@493.1 | 2 p, 372 / 0, AFTER 2@42.9 |
| R4 / R4t | `max-height:2000px`, `> p` / direct | 2 p, 351 / 350, AFTER 2@785.6 | 2 p 372 / 0 / 12 p 4000 |
| P4 | `height:3000px` | 3 p, 351 / 350 / 344, AFTER 3@778.1 (fragmented, both shapes) | 2 p, 372 (`> p`) / 12 p, 4000 (direct) |
| P5 | 900 px spacer, then 300 px box | fragmented: 2 p, 35 / 70, AFTER 2@185.6 | 3 p (`> p`) / 13 p (direct), 0 words on page 1 |
| P5s | same, 6-line paragraph | fragmented, 28 / 12 (4 + 2), AFTER 2@185.6 | 2 p, box moved whole, 0 / 40, AFTER 2@267.9 |
| P6 | `clip`, `auto`, `scroll` (direct, `> p`, `max-height`) | the same numbers as `hidden` | the same as `hidden` |
| P7 | `overflow-y:hidden`, `overflow-y:clip`, `overflow-x:hidden`, each `height:300px` | the same as R1 | 12 p |
| P7xc | `overflow-x:clip;height:300px` | 12 p, 4000 words once (351 / 350 / 351 / 365 …), AFTER 1@268.1: on top of the text of page 1 | 12 p, AFTER 12@327.9 |
| P8 | `height:310px` | 112 words (16 lines, the 16th partly), AFTER 1@275.6 | 12 p / 2 p |
| P9 | link in the hidden part (word 200), one in the visible part | 104 words, annotation for the visible link only, `HIDLINK` not written | both annotations, both link texts written |

- **Stop conditions:** Chrome's text layer holds no clipped line (R1: 105 words, not 372), and main does not leave them out (R2, R3: 372). Neither fired.
- **Predictions:** R1 to R4, the P8 line count (16) and the missing link (P9) are as the plan predicted. Unknowns settled: `height:3000px` and a straddling box are fragmented (no case where Chrome keeps the box whole, so the "keep whole" clause of decision 2 never applies); `auto` and `scroll` behave like `hidden`; `overflow-y:clip` clips. New: R1p does not reveal (it hides text and puts AFTER on page 2) and R1n reveals; a straddling box with short content is fragmented by Chrome too (P5s: not done here, new finding F30f-c); `overflow-x:clip` alone puts AFTER on top of the overflowing text.
- **Chrome's page content box is 1010 px**, ours 1009.13: AFTER (20 px) fits under a spacer of 990 px in Chrome, under 989.13 here (probes `sp989.5`, `sp990`, `sp990.5`). The box heights of C4, C5 and C6 only match with that px (990.0 / 980.0 / 190.0 px of box on the last page).
- The causes were confirmed by reading the code and by the results (R3's AFTER on page 2 comes from the cursor below the whole paragraph); the log lines of `fragment_inline_root` and `closing` that step 1 asked for were not added.

### Steps 2 to 6: tests

- Ours, `crates/htmltopdf/tests/clip_boxes.rs`: 16 tests (C1, C1p, C1n, C2, C3, C4, C4t, C5, C6, C7, C8 (two tests), C9, C10, the tagged guard, fixture 28). On main (fork `516558cc`) **13 of the first 15 were red**; the green ones were the `overflow-x:clip` case and the tagged guard. With the writer only (`ce76bbfa`) C2 and C10 turned green (C3 and C9 `> p` did not: their AFTER and page count come from the fragmenter, the plan's step 3 list was wrong there). With the fragmenter all 15 are green; fixture 28's test is red on main (2 pages, 491 words) and green after.
- Fork lib: `clips_block_axis_reads_overflow_y`, `clip_block_limit_is_the_bottom_of_the_padding_box_when_content_reaches_past_it`, `ancestor_clip_limit_is_in_the_coordinates_of_the_node`, `clip_to_drops_the_lines_that_start_at_or_below_the_limit`, `a_body_level_clip_box_that_fits_is_placed_whole`, `a_clip_box_taller_than_the_room_is_fragmented_at_its_block_size`, `a_clip_box_that_straddles_the_page_bottom_is_fragmented_in_place`, `content_below_the_clip_gets_no_page`, `a_clip_box_with_direct_text_that_fits_is_not_split_by_the_body_line_path`; render units `a_line_outside_the_clip_band_is_not_drawn`, `a_partly_visible_line_is_drawn`, `no_band_under_a_transform`, `clip_boxes_inside_each_other_intersect_their_bands`; `render_smoke_clip_box_hides_lines_from_the_text_layer`, `render_smoke_max_height_clip_box_spans_pages`. (The new functions do not exist on main, so "red" is a compile error there; the behaviour is pinned by the red tests of ours.)
- Fork gates: `cargo test -p fulgur --lib` 2371 passed (2362 before the fragmenter), the 42 integration test binaries all passed (`render_smoke` 323), clippy `-D warnings` and fmt clean; fulgur-vrt: 35 of 64 differ, the same 35 environment failures as before with byte-identical `.actual.pdf` files; the 21 examples: 10 of 11 tests pass, `committed_svg_matches_rendered` fails exactly as on `516558cc` (23890 vs 24816 bytes, the stale committed SVG example). The fork at this head builds only with the Blitz, Taffy and Parley forks patched in (it did not build against upstream Blitz before this change either), so "both Blitz versions" was one configuration.
- Ours: fmt, clippy `--release --all-targets --locked -D warnings`, `cargo test --release --locked --no-fail-fast` (73 test binaries, 811 passed, 2 failed before the next line, both in `wrapper_line_split.rs`: `a_clip_box_with_a_max_height_below_its_paragraph_is_left_as_it_was` (2 pages now, Chrome's) and `a_clip_box_shorter_than_its_paragraph_keeps_hiding_it` (211 words in the text layer now, Chrome's): both pinned the bug, rewritten).
- **Output identity:** fixtures 01 to 27 and the 7 corpus documents (`b200-*`, `slow-*`, `big_*`) are byte-identical to main's binary at 1 and 4 threads (`f183-gate.sh`), and the `scripts/render-fixtures.sh` outputs of 01 to 27 are `cmp`-identical; fixture 28 is new. `bench.py --check` ok (`b200-mixed` 853 ms vs 860 ms on main's binary, 02 and 01 within noise).
- **Fixture 28** (`28-notes-box-clip`): compare 3.58 % (1 page of 1; threshold 4.6 %). Main's binary: 2 pages, 4.2 % / 1.5 %, 491 words (the box starts page 2 and shows every line of the note); now 1 page, 184 words = Chrome's. The side-by-side matches Chrome: header, parties, items and totals line up (the totals table sits 43 px too far right in ours, as in 01, part of the usual drift), the note box ends at the same row with the 5th line cut by its edge and the rounded border under it. The first draft of the note (18 lines) did not reveal on main (its lines still fit page 1); extended to 28 lines.

### Deviations from the plan

- Step 3 does not make C3 and C9 `> p` green (see above); steps 4 and 5 are one fork commit (`c700b372`).
- `clipped_tail: bool` is not a field: a clip box that hides the last lines of a paragraph gives it one more `LineSlice` than fragments, for the hidden lines (`line_slices_tile` still covers every line; `paragraph_lines_for_page` accepts `fragments + 1` slices when the last ends at the last line).
- The band is `ClipBand::{Open, Band, Off}`, not an `Option`, so that a clip box inside a transform or an inline box painted under a translate does not set a band in non-page coordinates.
- `box_barrier(.., through_clips)`: `line_split_candidate` lifts `ClipsItsContent` when the parent is a block-flow container (a clip box that is a flex or grid container, a table cell or a grid item keeps its paragraph whole as before) and cuts the lines at `tightest clip` (`clip_to`).
- `leading_block_size` counts a paragraph cut by a clip as its first lines (otherwise a straddling clip box was moved to the next page).
- `PageStrips::chrome_bottom` and a 1 px slack for the child after a fragmented clip box at body level (Chrome's fragmentainer, above). Not done for the child after a nested clip box.
- `a_clip_box_shorter_than_its_paragraph_keeps_it_whole` (fork) and two tests of `wrapper_line_split.rs` (ours) pinned the old behaviour and are rewritten, not "unchanged" as the plan said.
- Writer slack: a line with less than 0.5 pt of its height inside the band is not written (the fragmenter drops a line that starts less than 0.5 px inside the clip).

### Not verified

`vrt-validator` and `pdf-reviewer` (to run: tagged output of a paragraph clipped out completely, empty structure elements; a clip box under a transform; table cells with `overflow:hidden`, F51); the Chrome comparison of bookmarks and counters in hidden content; clip boxes nested in flex or grid items (unchanged by design).

### Auto-height clip boxes (vrt-validator, HIGH, 2026-10-28; fork `16d281a3`)

**What the validator found.** An auto-height `overflow:hidden` box (no `height`, no `max-height`) that spans a page break lost lines, against decision 3 ("content-sized clip boxes are unchanged"). Scratch shapes (`out/scratch/f30fbv/`, git-ignored; words in the text layer, main / the fork at `c700b372` / Chrome):

| Case | main | c700b372 | Chrome |
|---|---|---|---|
| `sc/c318` (`16px/12px`, `<div style="overflow:hidden">`, 1500 words) | 1500 | 1488 | 1500 |
| `sc/c300` (`16px/0.9`) | 1500 | 1488 | 1500 |
| `fz/s12_40` (lh 0.7, bordered padded card, h2 + p, 700 px spacer) | 1187 | 762 | 1187 |
| `sc/c286` (`14px/1.2`, padded bordered card, 1500 direct words) | 1500 | 1484 | 1500 |
| `c/auto-lineheight-small`, one page, AFTER y | 740.4 | 729.1 | 725.6 |

In 157 of the 901 auto-height scratch cases below the output differed from main.

**Why an auto-height box was treated as clipped.** Not a rule, a measurement. `clip_block_limit` returned a limit whenever `clip_hides_nothing` said "no", and `clip_hides_nothing` compares the bottom of the last *line box* with the box's height (plus `LINE_BOX_SLACK` = 2 px). Blitz's line boxes are as tall as the font's ascent + descent when the line height is below that, and they sit one line height apart. Logged for the repros (`collect_inline_line_metrics`, px from the content top):

| Case | box height | first lines | last line | past the box |
|---|---|---|---|---|
| c318 (Inter 16 px in 12 px lines) | 1500 | (-4, 16), (8, 28) | (1484, 1504) | 4 px (> 2): "hides content" |
| c300 (`0.9`: 14.4 px) | 1800 | (-3, 17), (11, 31) | (1783, 1803) | 3 px (> 2) |
| c286 (`1.2`: 16.8 px, 17 / 17 inset) | 1966 | (-1, 17), (16, 34) | (1914, 1932) | -2 px: "hides nothing" |

The validator's guess (px quantisation of the line heights) is half of it: it is the line box's height (the font's, 20 px) against the pitch (the line height, 12 px), which makes the last box end past a box that is exactly its lines' pitch high. With such a limit set, `clip_to` dropped the last lines of every fragment (c318, c300, s12_40), the place-whole rule and the capped closing changed the layout of a one-page box (`auto-lineheight-small`), and the writer's band (set for every clipping box) dropped the lines that main paints outside the fragment's frame (c286: no limit, only the band; those lines are not visible on main either, see F30f-d).

**Decision (task giver, 2026-10-28).** The clip semantics of this change (fragmenter `clip_block_limit` / `ancestor_clip_limit` / `clip_to` / place whole / capped closing, and the writer's band) apply only to a box whose computed block size is constrained: a non-auto `height` or a `max-height` other than `none`. An auto-height clip box behaves as on main in fragmentation and drawing, and has no band.

**Change (fork `16d281a3`).**

- `pagination_layout::block_size_constrained(node)`: computed `height` is a `<length-percentage>` (not `auto`, `min-content`, `max-content`, `fit-content`, `stretch`) or `max-height` is a `<length-percentage>` (not `none` or a keyword). A percentage counts only when its basis is definite (`percentage_basis_is_definite`: walking the `layout_parent` chain, a `height` that is a length is reached before an `auto` or keyword one; the root's basis is the viewport; css-sizing-3 §5.2.1). `<html>` and `<body>` are never constrained. `min-height`, `box-sizing` and `overflow` do not matter. Style-only, so box-sizing (padding and border inside or outside) needs no handling.
- `clip_block_limit` returns `None` for an unconstrained box; so do `ancestor_clip_limit` and `tightest_clip_limit` (built on it): every place of the fragmenter that reads a limit sees none.
- `box_barrier(.., through_clips)`: an unconstrained box keeps main's rule (`clips_overflow` and not `clip_hides_nothing` → `ClipsItsContent`, whatever `through_clips` says; also for `overflow-x: clip` alone); a constrained one the rule of the first version (lifted in block flow, `clips_block_axis` only).
- `BlockStyle::block_size_constrained`, set in `extract_block_style` for a box that clips `overflow-y`; `draw_under_clip` sets a `ClipBand` only for such a box. A constrained box inside an unconstrained one still has its band, and the unconstrained one adds none.

**Red tests first.** Ours, `crates/htmltopdf/tests/clip_boxes.rs`: `an_auto_height_clip_box_with_lines_smaller_than_the_font_loses_no_word` (c318 and c300 shapes), `an_auto_height_card_with_direct_text_loses_no_word_in_the_text_layer` (c286), `an_auto_height_card_after_a_spacer_loses_no_word` (s12_40, 425 words missing), `an_auto_height_clip_box_that_fits_the_page_is_laid_out_as_a_box_without_the_clip` (AFTER 234.1 pt against 245.4 without `overflow`), and the other side, `a_clip_box_with_a_height_still_drops_the_lines_it_hides_when_lines_are_small` (green before and after). The first four are red on `c700b372` (words lost: 12, 16, 425; AFTER 11.3 pt higher) and green on `16d281a3`; the other 17 tests of the file stay green. Words are checked in the text layer: every word once and in order; page counts are Chrome's (2). AFTER's position is compared with the same box without `overflow`, not with Chrome: main is 14 pt below Chrome there for a reason of its own (the line boxes' height against the pitch).

Fork lib, new: `block_size_constrained_reads_height_and_max_height`, `a_content_sized_clip_box_has_no_limit_however_tall_its_line_boxes_are` (line boxes of 19 px at 6 px, `clip_hides_nothing` false, limit `None`; with `height:180px` the limit is 180), `a_content_sized_clip_box_keeps_the_barrier_it_had_and_a_sized_one_loses_it_in_block_flow`. (Written with the change, not before it: they need the new function, so "red" is a compile error on `c700b372`; the behaviour was red in ours.)

**Evidence.**

- **Scratch set**, 1090 HTML files of the validator (`sc/`, `sc2/`, `lh/`, `fz/`, `t16/`, `c/`, `sp/`, `sd/`), each rendered by main, `c700b372`, `16d281a3` and the F30f-d patch (`sweep3.py`, git-ignored scratch), words per page and 40 dpi pixel diffs, Chrome PDFs generated for every case that differs:
  - **901 cases with an auto-height clip box and 25 with none in a style attribute (Tailwind, `t16/`): `16d281a3` is byte-identical to main in all 926.** `c700b372` differed from main in 157 of them.
  - 164 cases with a `height` or `max-height` clip box: `16d281a3` is byte-identical to `c700b372` in all 164; 156 differ from main, and in 146 of those the words per page are within 14 of Chrome's (main: 1 of 156); the 9 that are not are shapes this change leaves alone (table cells with `overflow:hidden`, a transform, a multi-column container, an inline-block: `c/td-*`, `transform-*`, `multicol-in-clip`, `inlineblock`, `clipmargin`).
  - The five repros of the validator: c318, c300, c286, s12_40 and `auto-lineheight-small` equal main byte for byte (words 1500, 1500, 1500, 1187; AFTER 740.4 pt).
- Fork: lib 2374 passed (3 new), the 42 integration test binaries pass one at a time, clippy `--all-targets -D warnings` and fmt clean; `fulgur-vrt`: the same 35 of 64 environment failures as on main, with byte-identical `.actual.pdf` files.
- Ours: fmt, clippy `--release --all-targets --locked -D warnings`, `cargo test --release --locked --no-fail-fast` 818 passed in 73 binaries (`clip_boxes.rs`: 21, of which 5 new).
- **Output identity:** fixtures 01 to 27 and the 7 corpus documents are byte-identical to main's binary at 1 and 4 threads (`f183-gate.sh`); fixture 28 is unchanged from `c700b372` (same bytes). `scripts/render-fixtures.sh` + `scripts/compare.py`: all 28 fixtures ok, 28 at 3.6 % like before, page counts unchanged; the side-by-side of 28 looked at.

**The pre-existing loss of a line per page break (b).** Root cause found, fix not included: it is a new HIGH finding, **F30f-d** in `2026-10-15-m0-findings.md`, at the front of the queue. In short: main's body-level line path fits the lines of a paragraph with a border or padding as if they started at the top of its border box, but the renderer paints them `inset_top` lower on every fragment, so the last line of each page ends outside the page's frame and a clip box cuts it off the picture (c286: the 13 words of w0763-w0775 are on page 1 of main's text layer at y 798-811 pt, the content box ends at 799.4 pt, and page 2 starts at w0776; Chrome 763 / 737 words). The fix (route such a paragraph through the splitter that knows the insets) is 20 lines and gets c286 to Chrome's words per page, but over the 238 scratch cases it changes only 69 reach Chrome's numbers exactly and 86 stay more than 14 words off, because it then runs into `box-decoration-break: slice` (F96): Chrome does not repeat the top padding on the continuation page and ours does. So it is not an improvement everywhere yet. The patch:

```diff
diff --git a/crates/fulgur/src/pagination_layout.rs b/crates/fulgur/src/pagination_layout.rs
index 277755f7..2308c962 100644
--- a/crates/fulgur/src/pagination_layout.rs
+++ b/crates/fulgur/src/pagination_layout.rs
@@ -2570,9 +2570,25 @@ impl<'a> PaginationLayoutTree<'a> {
             .flatten();
             let clip_whole_now = clip_limit_here.is_some()
                 && cursor_y + child_h <= self.strips.bottom(page_index) + 0.5;
+            // html-to-pdf F30f-b (plan §7, separate fix): a paragraph with a border or padding that spans pages is cut
+            // by the splitter that knows its insets. `fragment_inline_root` fits the lines as if they started at the
+            // top of the border box, but the renderer paints them at `y + inset_top` on every fragment, so the last line
+            // of each page ended `inset_top` below the page's bottom, in the margin, and a clipping box (`overflow`) cut
+            // it off the picture: a line lost per page break, while the text layer still held it.
+            let has_insets = layout.border.top
+                + layout.padding.top
+                + layout.border.bottom
+                + layout.padding.bottom
+                > 0.0;
+            let inset_split_here = clip_limit_here.is_none()
+                && has_insets
+                && body_block_flow
+                && !is_float
+                && !avoid_inside
+                && !crate::blitz_adapter::is_multicol_container(child)
+                && cursor_y + child_h > self.strips.bottom(page_index) + 0.5;
             if line_metrics.len() > 1
-                && clip_limit_here.is_some()
-                && !clip_whole_now
+                && (clip_limit_here.is_some() && !clip_whole_now || inset_split_here)
                 && let Some(body_node) = self.doc.get_node(body_id)
                 && let Some(split) = line_split_candidate(self.doc, body_node, child)
             {
@@ -2602,7 +2618,7 @@ impl<'a> PaginationLayoutTree<'a> {
                 page_index = new_page_index;
                 cursor_y = new_cursor_y;
                 emitted += 1;
-                after_clip_box = true;
+                after_clip_box = clip_limit_here.is_some();
                 prev_bottom_y_in_body = this_top_in_body + child_h;
                 if let Some((row_page, row_end, row_bottom)) = parallel_row {
                     if page_index == row_page {
```

**Deviations from this plan.** The plan's decision 3 said "content-sized clip boxes are unchanged" and tested it with fixtures only; §7 now tests it with shapes whose line boxes are taller than their pitch. `overflow-x: clip` alone on an auto-height box keeps main's barrier (the first version lifted it; no Chrome probe covers that shape, P7xc has a `height`).

**Not verified (added).** `bench.py --check` (the change adds one style read per clipping box; not timed); Percentage `height` / `max-height` with an abspos or flex-stretched basis (`percentage_basis_is_definite` walks `layout_parent` heights only, so a flex item stretched to a definite height and an abspos box with `top` and `bottom` count as auto: they keep main's behaviour); `min-height`-only boxes against Chrome; the Tailwind `t16/` shapes against Chrome (they are far from it on main already, see F30f-d).


### Padded and bordered clip boxes, table cells (vrt-validator second pass, HIGH, 2026-10-28; fork `820c6e3e`, `aecc013a`)

**What the validator found.** (1) A constrained clip box (`height:300px`) with padding or a border that straddles a page lost most of its visible text: `pad/pad16_h300.s700` 26 words in the PDF (main 700, Chrome 212), `pad/bd2_h300.s800` 66 (Chrome 204), `pad/pt16_h300.s700` 13; page 1 showed an empty frame. Without padding or border it matched Chrome. (2) `td{height:300px;overflow:hidden}` straddling a page (`b/td_h.s880`): 120 words and AFTER 65 px above main's, where Chrome grows the cell.

**Cause of (1).** Not the F30f-d line fit. `nested_line_split` splits a paragraph that is inside a box only when it is taller than a page or "the first thing on its strip" (`child_page_y <= strip_top + frame_above + 0.5`), and `frame_above` (the border and padding that may precede the first child) was counted only for table cells (F183). A clip box's clip cuts the paragraph to the lines that show (`clip_to`), so it fits a page; it sat `padding-top` below the box's top, so it was not first either, and the F30c rule moved it whole to the next page: the box's frame on page 1 was its padding alone and its lines were on page 2. Direct text in the same box went through the body-level path and was right (Chrome's 98 / 7 words at spacer 700). A padded content-sized box *inside* the clip box (`a_in_c_pad`: `overflow:hidden;height:300px;padding:20px > overflow:hidden;padding:10px > p`) had the same cause one level down: its own `clip_block_limit` is `None` (content-sized), so the first change did not help it (25 words against Chrome's 201, page 1 empty where main showed 15 lines); the ancestor's clip decides, so the test is `tightest_clip_limit(parent)`.

**Cause of (2).** `height` on a table, row group, row or cell is a minimum (CSS 2.1 §17.5.3), so such a box is not block-size constrained. `block_size_constrained` returned true for a `td` with a `height`, so the clip semantics applied to it.

**Change.** Fork `820c6e3e`: `block_size_constrained` is false for `display` inside `table`, `table-row-group`, `table-header-group`, `table-footer-group`, `table-row` and `table-cell` (the output of such a box is main's, byte for byte). `aecc013a`: in `nested_line_split`, `frame_above` is the parent's border and top padding also when the parent or an ancestor is a clip box that hides content (`tightest_clip_limit(parent).is_some()`). Chrome splits the paragraph where the box starts; so do we now, with the same slices as the direct-text box (fork test `a_padded_clip_box_with_a_paragraph_is_fragmented_in_place_like_one_with_direct_text`). The paragraph that is not the first child of a padded box still moves whole (F30c, unchanged). About 15 lines of code.

**Red tests first.** Ours (`clip_boxes.rs`, 5 new; 4 red on `16d281a3`, committed as `6c8c87b`, the nested one added with `aecc013a` and measured red by the validator's `a_in_c_pad` run): padding 16 at spacers 700 / 800 / 900 / 960 with `<p>` and with direct text (Chrome: 98 / 7, 63 / 42, 28 / 77, 0 / 112 words; AFTER 59.6, 134.6, 209.6, 292.1 pt), border 2 px at 800 / 900 / 960 (70 / 35, 35 / 70, 14 / 91), `padding-top` and `box-sizing:border-box`, the nested padded box (85 / 12 and 31 / 66), and the table cell (main's output: 2 pages, 371 words on page 2, AFTER 269.4 pt; Chrome grows the cell to 13 pages, a gap of its own). Fork lib, new: `a_padded_clip_box_with_a_paragraph_is_fragmented_in_place_like_one_with_direct_text`, `a_paragraph_in_a_padded_box_inside_a_clip_box_is_split_where_the_box_starts`, `a_table_box_has_no_constrained_block_size` (each checked red with its change reverted, except the nested one, which was measured on the fork before the change: frames of 636 px on page 0).

**Evidence** (`three3.py` over `f30fbv2/b` 488, `f30fbv2/pad` 75, `f30fbv/*` 1196 = 1759 cases, main / `16d281a3` + F30f-b second-pass binary / this binary, ink at 60 dpi per page and words, Chrome PDFs next to the files):

- Against the binary of the previous pass (`f30fb2`): 31 cases changed, 28 closer to Chrome (L1 of the per-page ink), 3 equal (the `td` cases that now equal main's bytes), none farther. 1728 byte-identical to it. Against main: 1422 byte-identical (every auto-height case and every case without a clip box).
- The 75 `pad/` cases: `pad16_h300`, `bd2_h300`, `pt16_h300` and the `bb` variants at spacers 700 to 900 are within 1 to 2 lines of Chrome: `pad16_h300.s700` words 212 against Chrome's 213 (26 before), L1 936 against 12585 for the previous binary; `bd2_h300.s800` 205 / 205 (66 before); `pt16_h300.s700` 205 / 220 (13 before). `a_in_c_pad.s700` 201 / 201 (25 before), ink [11056, 1380] against Chrome [10996, 1351] and main [11056, 34296].
- `td_h.s880` / `s960` / `s0` / `s700` and `c/td-hidden`: the same bytes as main's (ink [0, 13166], AFTER 270.1 pt).
- No visible text of main is dropped: per page, no case has a page where main has more than 1500 dark pixels and this binary less than 60 % of it while Chrome is within 20 % of main. Cases where the total ink is below both main's and Chrome's (11 of 1759, all unchanged from the previous pass except `pad16_h300.s700` / `pt16_h300.s700`, which were worse there): they are shapes that main moves whole to page 2 (page 1 empty in main) and where ours now shows one line fewer than Chrome on page 1 (`lh05_h.s960`, `lh08_h`, `short-fits2` (F30f-c), `strad950`, `stradp950`: 167 against 179 words), `overflow-clip-margin` (`clipmargin`), `height:0` with padding (`h0pad.s700`: 14 words, same as Chrome) and the two 4000-word shapes above (one partly cut line, 212 / 213 words).
- Fork: lib 2377 passed, the 42 integration binaries passed one at a time, clippy `--all-targets -D warnings` and fmt clean; fulgur-vrt: the same 35 of 64 environment failures, `.actual.pdf` files byte-identical to `f30fb2` and to main's.
- Ours: fmt, clippy `--release --all-targets --locked -D warnings`, `cargo test --release --locked --no-fail-fast`: 823 passed in 73 binaries (`clip_boxes.rs`: 26).
- Output identity (`f183-gate.sh`, 1 and 4 threads): fixtures 01 to 27 and the 7 corpus documents are byte-identical to main's binary; fixture 28 is byte-identical to `f30fb2`'s (differs from main's, as in the first pass). `render-fixtures.sh` + `compare.py`: all 28 ok, 28 at 3.6 %, page counts unchanged. `bench.py --check` ok (b200-items 1359 ms with this binary, 1316 ms with main's on this loaded machine: the accepted-exception line prints "not met" for both; the corpus output is byte-identical).
- The side-by-side of `pad16_800` (both pages) and `bd2_900` (page 1) against Chrome was looked at: same lines on both pages, the frame ends in the same place.

**Not verified (added).** Boxes with padding where the paragraph is not the first child (a heading above it moves the paragraph whole, F30c: unchanged); nested clip boxes with two padded levels and `border-radius`; the box-decoration-break `slice` (F96: Chrome does not repeat the top padding on page 2, ours does in the nested path), which is why `pad16_h300.s700` is 26 words on page 2 against Chrome's 13 on the 14 px / 1.4 shape (tests use the 20 px shape, within one line); a `table` with `overflow:hidden` in the table-wrapper sense (Chrome: no clip on the table box itself) was not probed; `vrt-validator` and `pdf-reviewer` re-runs.
