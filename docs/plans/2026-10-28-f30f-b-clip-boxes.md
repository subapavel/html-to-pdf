# F30f-b (clip part): a clip box shorter than its content shows, or writes into the PDF, what it hides

Date: 2026-10-28. Finding: F30f-b in `2026-10-15-m0-findings.md`, the clip bullets only. Status: **done** (§7). Fork commits `ce76bbfa` (writer) and `c700b372` (fragmenter) on `htmltopdf/srgb-colours`; ADR-0009.

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
