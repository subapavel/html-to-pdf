# F30c-n: a paragraph nested in a block that does not fit the room moves whole; Chrome starts it in place and splits it

Date: 2026-11-02. Finding: F30c-n in `2026-10-15-m0-findings.md`. This is the nested half of F30c; the body-level half is done (`2026-10-30-f30c-pre-advance-split.md` §8, fork `d26eeec2`, `starts_in_place`). F30f-f depends on this item. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, head `d26eeec2` (the `Cargo.lock` rev). Line numbers are from that head; re-read them before editing.

*Coordinator note (2026-11-02):* since this plan was written, F209 (`0feb944f`, PR #81) and F30c-o (`55154081`, PR #82, author `orphans` / `widows`: `LineRules`, `line_rules`, `LineRulesScope`) landed on the fork branch. Implement on top of `55154081`: where this plan uses the constant 2 / 2 for orphans / widows, use the F30c-o `LineRules` of the paragraph (F30c-o2 lists nested paragraphs as not covered by its keep-the-orphans relaxation; close that part here if it falls out naturally, otherwise leave it filed). The acceptance fixture is 39 (37 and 38 are taken); "red at `d26eeec2`" means red at `55154081`.

## 1. Problem

A paragraph inside a block (`<section><h2/><p/></section>`, `<div class="wrapper"><p/>`, an abspos box) is moved whole to the next page when it does not fit the room left on its page but fits a page. Chrome starts it where it is and breaks it between line boxes. Since F30c the body-level paragraph does this, so the same text now pages differently depending on whether it has a wrapper. No committed fixture shows it; fixture 39 (§5) will. `abspos_flow::an_abspos_wrapper_of_ten_paragraphs_is_fragmented_from_its_start` prints 10 pages against Chrome's 9 for this reason (`crates/htmltopdf/tests/abspos_flow.rs:322-343`).

Repros: the 52 cases of `out/scratch/f30c/c/` (A4, 15 mm, `14px/21px`, `p{margin:0}`, 48 lines a page), in two wrappings:

- **Shape A**: everything inside one `<section>`, so the paragraph follows a sibling inside the wrapper.
- **Shape B**: the paragraph is the first child of its own `<section>` and follows content outside it.

Expected Chrome numbers are the body-level ones (`out/scratch/f30c/chrome.txt`: `two_49_47` 47 / 47 / 2; spacer H=650 gives 17 / 13, H=966 gives 2 / 28, H=975 gives 0 / 30). Step 1 measures them wrapped. Ours at `d26eeec2` is expected to give 0 / N, moved whole, in both shapes.

## 2. Spec (cited by the planner; css-spec-researcher not run; * = wording unverified)

- **css-break-3 §4.1**: Class A break points are between siblings, Class B between line boxes, Class C between a container's content edge and its children where there is a gap. Nesting depth does not rank them.
- **css-break-3 §3.3 / §4.4**: a Class B break needs at least `orphans` lines before it and `widows` after it (initial value 2). These rules are relaxed only when no allowed break keeps the content inside the fragmentainer. So a nested paragraph of at least 4 lines whose top inset and first 2 lines fit has an allowed break on this page. Chrome takes the last allowed break that fits (F30c §8.1).
- **css-break-3 §3.2** `break-inside: avoid`: it holds while the box fits a page (F83). A relaxed avoid box starts the next page when content precedes it (F83b, `starts_with_relaxed_avoid`).
- **css-break-3 §5.2\***: margins adjoining an unforced break are truncated. A paragraph that now starts in place keeps its margin.
- **css-break-3 §5.4**: `box-decoration-break: slice` (initial value): the top border and padding are on the first fragment only (F30f-d).
- **CSS 2.1 §13.3.3**: a page may break between line boxes in a block container. Line boxes are never split.

## 3. Cause, traced (fork `crates/fulgur/src/pagination_layout.rs`)

**Shape A** (`fragment_block_subtree`, 5006):

- The block walk asks `block_walk_line_split` (5677, 5890; defined 7664), which calls `nested_line_split` (7685). That function returns a split only when one of these holds:
  - the paragraph is taller than a page;
  - it is first on its strip and overflows (7713-7714). "First" means at the entry cursor of its parent, plus `frame_above` for cells and clip boxes (7707-7712);
  - it is itself a clip box (7718).
- Its doc comment (7678-7684) says it moves a paragraph that fits a page whole.
- With `None`, the paragraph is a leaf. The gate does not enter it, because an inline root's text children have no height (`would_split_block_subtree`, 4367). `first_break_h = child_h + leaf_tail` (5841), and the F28 cut (5843-5925) moves it to the next page.

**Shape B** (body child loop, block path):

- The gate enters the `<section>` because its paragraph overflows the room. But the F28 cut (3808-3833) first asks `leading_block_size(section)` (4470).
- Its line-split arm (4563-4569) counts only a paragraph taller than a page (or a clipped one) by `LineSplit::leading()`. Any other paragraph falls through to `leading_block_size(p)`, which returns its whole height (4598-4599).
- So the whole section moves. Once entered, the first child is already split in place by `first_and_overflowing`. The fork test `leading_block_size_counts_two_lines_of_a_paragraph_taller_than_a_page` (18372) pins "on a page the paragraph fits, it counts whole".

**What already works, and stays:**

- `fragment_nested_inline_root` (7807) places the first fragment at `y_in` with `first_room = bottom - y_in - inset_top` (7832-7838).
- `plan_line_breaks` (6816) puts at least 2 lines on the first strip when 2 fit, and pulls widows back (F30f-l). With at least 4 lines, `n - WIDOWS >= ORPHANS`, so no orphan is left.
- A sliced padded paragraph (`slices_at_breaks`, 7638) gets its top inset on page 1 only, and its non-last fragments run to `chrome_bottom` (7929-7935).
- `record_split_inline_root_descendants` places images and inline boxes on their line's page.
- The cut's `leading` for a split (5786-5796) is `LineSplit::leading()`, so nothing new is needed after the split.

### Where it belongs

This belongs in the fulgur fork only. Pagination is fulgur's own code; Blitz, Taffy and Parley only supply the line metrics. htmltopdf bumps `Cargo.lock` and records the commit in `docs/upstream.md`. No upstream PR (standing decision).

## 4. Fix

New pure helpers, next to `LineSplit::leading` (6911) and `in_block_flow` (7191):

```rust
impl LineSplit {
    /// F30c-n: the paragraph has a Class B break that leaves ORPHANS and WIDOWS lines (len >= ORPHANS + WIDOWS).
    fn has_inner_break(&self) -> bool;
    /// F30c-n: `has_inner_break` and `y + self.leading() <= bottom`, the negation of the F28 cut's own test (5845).
    fn starts_in_place(&self, y: f32, bottom: f32) -> bool;
}
/// F30c-n: every box from `node`'s parent up to the pagination root is in block flow (`in_block_flow`) and is not a float,
/// a table part or cell, a multicol container or transformed.
fn block_flow_up_to_root(doc: &BaseDocument, node: &blitz_dom::Node) -> bool;
/// F30c-n: the room-independent shape a nested paragraph needs to start in place.
fn may_start_in_place(doc: &BaseDocument, parent: &blitz_dom::Node, child: &blitz_dom::Node, split: &LineSplit,
                      avoid_inside: bool) -> bool;
```

`may_start_in_place` requires all of the following:

- `!avoid_inside`;
- `split.has_inner_break()`;
- `split.hidden_tail == 0 && split.own_box_height.is_none()`;
- `!block_size_constrained(child)`;
- `child_h <= inset_top + span + inset_bottom + LINE_BOX_SLACK`;
- `!is_cell_of_table(parent, child)`;
- `block_flow_up_to_root(doc, child)`.

**Shape A.** `block_walk_line_split` gains `avoid_inside: bool` (the caller has `avoid_inside_child`, 5559). It returns `nested_line_split(..)`, or else a split when all of these hold:

- `may_start_in_place`;
- the lines overflow: `child_page_y + inset_top + span > strips.bottom(page) + 1.0`. This is the same slack as `first_and_overflowing`, so nothing Chrome's 1010 px page would keep is split;
- `split.starts_in_place(child_page_y, strips.bottom(page))`.

`slice` is set as today. `body_line_split` and the column-span caller keep calling `nested_line_split` unchanged.

**Shape B.** `leading_block_size` gets a new arm after the avoid arm (4575): a first-row child whose `line_split_candidate` satisfies `column_styles.is_some() && may_start_in_place(.., has_avoid(column_styles, child_id))` counts `split.leading().min(height)`. This is the "first place the fragmenter can break inside it" that the function's doc promises. When `column_styles` is `None`, callers mean "this box cannot move" (3817, 5817) or it is a table cell (`row_mate_can_split`, 7778); the arm stays off there, so those paths stay byte-identical. The engine always passes `Some` (`engine.rs:561`).

**Each exclusion keeps a shape that is right today or owned by another finding:**

- **`avoid` that holds**: it moves whole. Relaxed avoid is unchanged: `relaxed_start` sends it to the next page (5749).
- **Clip boxes** (F30f-b / F30f-f): unsupported shapes are still a `ClipsItsContent` barrier (7416). A supported one has only a first paragraph.
- **Table cells** (F183): the cell is its row's unit (`row_whole`, `kept_rows`, `row_state`, the header band). Not the same path.
- **Flex/grid descendants** (F30b), **multicol** (F198 / F30d-b), **floats** (F30c-f).
- **3-line paragraphs** (F30c-w).
- **Boxes with their own block size** (F83c).

Abspos boxes (F30g) are in scope: they reach the block walk through `abspos_flow.rs:945`.

Comments to update:

- `nested_line_split`'s doc (7678-7684) and the F30c-n note at 7702;
- `clip_shape_supported` (7170-7173);
- the test comment at 17397;
- `abspos_flow.rs:324-341`;
- `clip_boxes.rs:719-721`.

**F30c-o** (author `orphans` / `widows`, planned in parallel) touches the same helpers:

- `ORPHANS` / `WIDOWS` (6789, 6792);
- `LineSplit::leading`, the new `has_inner_break` / `starts_in_place`;
- `plan_line_breaks`, `widow_safe_break` (8095), the body-level `starts_in_place` (8107);
- `fragment_inline_root`'s `ORPHANS_MIN` (8171).

This plan changes no signature of the existing helpers. Whichever item lands second rebases.

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/nested_paragraph_split.rs`

Head and helpers as in `pre_advance_split.rs` (`render`, `lines`, `per_page`, `after`, `printed_words`). Every number comes from Chrome, measured in step 1. "Red" means red at `d26eeec2`.

1. `a_second_long_paragraph_in_a_section_starts_below_the_first`: the 10 `two_*` cases in shape A. Lines per page and AFTER page / y (±1 pt) equal Chrome's. Red.
2. `a_paragraph_below_a_spacer_in_a_section_starts_where_two_lines_fit`: 30 lines, H = 650 / 966 (red) and 975 (moves whole, guard), in shapes A and B.
3. `a_paragraph_in_a_section_after_a_heading`: `<section><h2/><p 46 lines/></section>`. Red.
4. `a_padded_nested_paragraph_slices_its_top_inset_on_page_one`: `<section><div h=650/><p style="padding:8px;border:1px solid">`, 30 lines. Lines per page equal Chrome's. The first line of page 2 is at the content top (±1 pt), with no top padding there. Red.
5. `a_padded_section_breaks_inside_its_paragraph`: `<section style="padding:16px;border:1px solid"><h2/><p/>`. Lines per page are Chrome's. The section's frame is not asserted (F96).
6. `a_long_list_item_starts_in_place`: `<ul class="pl-4" style="list-style:disc">` of short items and one 12-line `<li>` near the page end. Its marker and first lines are on page 1. Red.
7. `margins_collapse_through_the_wrapper`: `<section style="margin-top:24px"><p style="margin-top:12px">`, shape B. First line y and AFTER y match Chrome's. Red.
8. Inline boxes and links: `<a href>` on lines 5 and 25 and an inline `<svg>` on line 3 of a shape A paragraph. Link rects and the image are on Chrome's page and line. Red.
9. Guards, green at `d26eeec2`, must stay:
   - a nested 3-line `<p>` with 2 lines of room moves whole (F30c-w);
   - a nested `p{break-inside:avoid}` and `class="break-inside-avoid"` with the compiled sheet both give 0 / 30;
   - a relaxed avoid `<p>` of 60 lines starts page 2;
   - an avoid `<section>` that fits a page moves whole;
   - `div{display:flex} > div > h2 + p`, `td > h3 + p` and `column-count:2 > h2 + p` keep their bytes;
   - all of `pre_advance_split.rs`, `sub_line_overflow.rs` and `clip_boxes.rs:716`.
10. Relaxed avoid ancestor: `<section style="break-inside:avoid">` taller than a page, holding several paragraphs, after content. It starts page 2, then splits in place (Chrome confirms in step 1). If Chrome disagrees, pin ours and file it.
11. Updated: `abspos_flow.rs:339-341` becomes `pages == 9` if Chrome's 9 is reached; otherwise tighten to what is reached, with the cause named.

### Fork

- Unit tests for the helpers:
  - `has_inner_break`: 3 vs 4 lines;
  - `starts_in_place`: exact fit at `<=`, negative room, `inset_top`;
  - `block_flow_up_to_root`: flex item child, cell child, float wrapper, abspos wrapper.
- Through the pass, 200 px pages, 20 px lines:
  - `<div><div h100/><p 10/></div>` gives fragments `0..5` on page 0 at y 100 and `5..10` on page 1 (`nested_geometry`);
  - shape B `<div h100/><div><p 10/></div>`, same result. Use the `avoid_fragments` helper: `leading_block_size` needs column styles;
  - a padded `<p>` gives `line_split_slice`, page-0 fragment height to the page bottom, and page-1 lines at y 0;
  - a nested avoid `<p>` moves whole;
  - a flex item's inner `<p>` and a cell's inner `<p>` are unchanged.
- Rewrite `a_paragraph_that_fits_a_page_still_moves_whole` (16236) to 5 / 3. Keep its intent as `..._moves_whole_with_one_line_of_room` (spacer 180 px).
- Extend `leading_block_size_counts_two_lines_..` (18372) with a `Some(column_styles)` case that gives 40.
- One `render_smoke` case.

### Fixture `39-terms-in-section`

Czech invoice. Page 1 has the header, party grid, items table and "Celkem k úhradě" as in 36. Then a `<section class="mt-6 text-xs text-gray-600 leading-relaxed">` holding:

- `<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>`;
- a 2-line `<p class="mb-2">`;
- one `<p class="mb-2">` of about 30 lines of numbered clauses, starting about 250 pt above the content bottom;
- a 6-line `<p>`.

After the section: `<p class="mt-6 text-sm border-t pt-2">Převzal: …</p>`. All these classes are in the compiled sheet (checked by grep). `space-y-*`, `mb-3`, `list-decimal` and `pl-5` are not, so do not use them.

- **Expected**: Chrome's split; the long paragraph starts on page 1; 2 pages.
- **Main's binary**: page 1 ends after the intro, and the whole paragraph is on page 2.
- **PNG**: page 1 shows the terms down to the content bottom.
- **`fixtures.rs`**: asserts the first word on page 2 against Chrome's (±1 line) and 2 pages.
- **Threshold**: worst page + 1 point. Both pages should drop against main (text now rendered on page 1, no shift on page 2).

### Byte gates (1 and 4 rayon threads, main's binary against ours)

- **Fixtures 01-38**: expected byte-identical. Their nested multi-line paragraphs are:
  - taller than a page: 06, 16;
  - in grids or multicol: 08, 12, 24, 25, 30, 31;
  - inside holding avoid wrappers: 14;
  - 3 lines or fewer: 15, 34's list;
  - body-level: 13, 19, 28, 29, 32, 36.

  A fixture that moves must be at Chrome's split and named in §8. **A page-count change is a stop** (`PAGE_COUNT_MAY_DIFFER` is empty, ADR-0003).
- **Corpus**:
  - `b200-mixed` is expected to move. Every unit is a `<section class="mb-6">`, and its clip card's padded `<p class="p-4">` sits below an `h2`. Compare lines per page and page count with a Chrome render of it, as F30c did for `slow-paragraph`.
  - `b200-text` (its sections are `break-inside-avoid` and hold), `b200-items`, `slow-paragraph` and `slow-multicol` are expected identical.
  - `slow-breaks` and `slow-avoid` were not inspected. A move there needs a Chrome comparison.
- **Sweep**: `out/scratch/f30cn/sweep.py` reruns the 4616 F30c cases in shapes A and B. Every changed case must be closer to Chrome in lines per page and page count. Anything further away stops the work.
- **Fork `fulgur-vrt`**: `.actual.pdf` identical before and after, or the only difference is an in-place split. Not re-blessed on this host.
- **`bench.py --check`**: the chain walk is at most `MAX_DOM_DEPTH` hops, done only for candidates of at least 4 lines; cheap checks come first. Watch `b200-items` and `b200-mixed`.

## 6. Steps

1. **Chrome** (≤ ½ day): render shapes A and B of `out/scratch/f30c/c/` plus tests 3-10, a `td > h3 + p` probe (number recorded for the filed cell item) and fixture 39's reference. Fix every expectation; drop or pin any test whose Chrome split contradicts §2, with the reason in §8.
2. **Ours, red**: `nested_paragraph_split.rs`, fixture 39 (index, README, `reference/` via `scripts/reference/render.mjs`) and its `fixtures.rs` test. Commit with the red list.
3. **Fork**:
   - write the helpers as stubs returning `false`; the fork tests go red;
   - implement them, wire `block_walk_line_split` (both call sites) and the `leading_block_size` arm;
   - update the comments and add the fork tests;
   - run lib tests, `render_smoke`, clippy `-D warnings` and fmt.
4. **Ours, green**:
   - bump `Cargo.lock`;
   - update test 11 and the comments of §4;
   - run `cargo test --release --locked --no-fail-fast`, the byte gates and the sweep.

   Stop and write a postmortem on a page-count change, a moved fixture that is not at Chrome's split, or a sweep case further from Chrome.
5. **Finish**:
   - run `render-fixtures.sh` and `compare.py`; look at 39's side-by-side; set its threshold;
   - run `vrt-validator` (no `pdf-reviewer`: layout only);
   - findings file: F30c-n done, F30f-f unblocked, and the table-cell probe filed as F30c-t if Chrome splits there;
   - `docs/upstream.md`; this plan's §8.

## 7. Risks and non-goals

**Risks**

- **Page counts.** Documents whose text sits in wrappers lose the page Chrome does not have. `b200-mixed` is the expected one.
- **`leading_block_size` is shared** by both F28 cuts and `row_mate_can_split`. The arm is gated on `column_styles.is_some()` and on the chain check, so cells and flex rows are not reached. A fork test pins the cell case.
- **F96.** A bordered wrapper now breaks mid-page more often, and its frame is still closed on both pages. Fixture 39 uses an unbordered `<section>` on purpose; test 5 does not assert the frame.
- **Margins.** A paragraph that starts in place keeps its top margin; the gap after a split paragraph is kept (`origin_pending_prev_bottom`, test `the_gap_below_a_split_paragraph_is_kept`). Test 7 checks collapse through the wrapper.
- **Capacity.** Rooms within 1 px of a line boundary differ from Chrome's 1010 px page (F30f-c); the tests avoid them.
- **F30c-o merge order.** See §4 for the shared helpers.

**Non-goals**

- Table cells (`td > h3 + p`, F183 rules; probe only).
- Flex/grid descendants (F30b), multicol (F198, F30d-b), floats (F30c-f).
- The 3-line 2 / 1 split (F30c-w). The body-level padded branch (3494) still starts a 2- or 3-line padded paragraph in place; that inconsistency belongs to F30c-w.
- `orphans` / `widows` properties (F30c-o).
- Widening `clip_shape_supported` (F30f-f, unblocked by this item; it can reuse the new `avoid_inside` parameter).
- A box whose lines fit and only its bottom inset or tail does not (F30f-c).
- F96 block-frame slicing; F197.

## 8. Evidence

### Step 1: Chrome (m141), `out/scratch/f30cn/{gen.py,chrome.txt,ours.sh}`

Every shape-A / shape-B twin of the 52 F30c cases (117 documents with the extra shapes below) gives Chrome's body-level number:
`two_49_47` 47 / 47 / 2, spacer 650 17 / 13, 966 2 / 28, 975 0 / 30, h1 + 46 lines 44 / 2, `hp_2` 2 / 28, `hp_3` 3 / 27, 600 words
232 / 368, and so on. §2 holds: nesting depth does not change Chrome's split. Further Chrome numbers (test file
`crates/htmltopdf/tests/nested_start_in_place.rs`):

- padded `<p>` (8 px + 1 px border) after 650 px, shape A and B: 16 / 14, AFTER 271.8 pt, first line of page 2 at 44.58 pt;
- `<section style="padding:16px;border:1px solid">` + h2 + 46 lines: 44 / 2; with padding 20 px and 60 lines: 45 / 15 (16 px and 60
  lines is on a line boundary, 46 / 14 on Chrome's 1010 px page and 45 / 15 on ours, F30f-c, so not used);
- 40 one-line `<li>` and a 12-line `<li>`: 8 / 4, marker on page 1;
- `<section style="margin-top:24px"><p style="margin-top:12px">` after 650 px: 16 / 14, first line at 550.08 pt (the margins collapse to 24 px);
- relaxed avoid ancestor (three 30-line paragraphs in an avoid section after 300 px): 0 / 48 / 42, as §5 item 10 expects;
- links and inline images: on the page of their line, 17 / 13;
- `td > h3 + p` after 650 px: 16 / 14 (Chrome splits inside the cell; ours 0 / 30; filed as F30c-t);
- flex item: Chrome 15 / 15, ours 0 / 30 (F30b, pinned);
- fixture 39: "28." heads page 2 at 44.5 pt, "65." 358.0 pt, "Převzal:" 411.3 pt, 2 pages.

### Deviations from the plan

- The test file is `nested_start_in_place.rs`, not `nested_paragraph_split.rs`: that name is taken by F30's tests (taller than a page).
- Test 8 (links and one inline `<img>`): green at `55154081` (a paragraph with inline boxes takes another path and already splits
  there); a variant without the images is red. Both are in the test.
- Test 5 uses padding 20 px for the 60-line case (see above).

### Red at `55154081` (12 of 18 in `nested_start_in_place.rs`, fixture test)

Red: second long paragraph (A and B), spacer H=650 / 966, 4 / 5 lines with 3 lines of room, running text, heading + paragraph (46, 48,
`hp_2`, `hp_3`), padded paragraph, padded section, list item, margins, links without images, relaxed avoid ancestor
(30 / 30 / 30 against 48 / 42); `terms_in_section_invoice_starts_the_long_paragraph_in_place` ("1." on page 2). Green guards: 3-line,
avoid paragraph (style and class), relaxed avoid paragraph, avoid section, table cell (pinned 0 / 30), flex item (pinned 0 / 30).

### Fork (`subapavel/fulgur` `htmltopdf/srgb-colours`)

Built on `55154081` (F30c-o). Commits: `a699c0c5` (the change), `e9765f05` (the `orphans + widows` length rule moved into one predicate,
`has_allowed_break`, shared by the body-level `starts_in_place` and `LineSplit::has_inner_break`, so that F30c-o3 changes it in one
place; coordinator note), `cd9fb62e` (sweep finding below; head, the `Cargo.lock` rev).

- `block_walk_line_split` (now with `avoid_inside`) returns the candidate split also when it overflows the room by more than 1 px, has an
  allowed break and its inset and first `orphans` lines fit (`LineSplit::starts_in_place`, the negation of the F28 cut's own test), and
  `may_start_in_place` holds: no `avoid` on the paragraph, no hidden tail or own block size, not block-size constrained, not a table cell,
  and `block_flow_up_to_root` (every box up to `<body>` in block flow, not a float / table part / transformed / clip box with a block size).
  The cheap tests run first, the ancestor walk only for a paragraph that overflows and has an allowed break.
- `leading_block_size`: the arm after the taller-than-a-page one, for a wrapper's first-row paragraph, `column_styles.is_some()` and no
  forced `break-before` on the paragraph.
- `nested_line_split` is split in a candidate step and `nested_split_needed`; `body_line_split` and the multicol caller are unchanged.
- Comments of `nested_line_split`, `clip_shape_supported`, `frame_above` and one test updated. `abspos_flow.rs` and `clip_boxes.rs` comments too.
- Fork tests: 2476 lib tests pass (13 new / rewritten: `has_inner_break`, `starts_in_place` exact fit / negative room / inset,
  `block_flow_up_to_root` (14 shapes), shape A and B fragments 0..5 on page 0 at y 100 and 5..10 on page 1, one line of room moves whole,
  3 lines move whole, padded paragraph sliced, avoid moves whole, flex item and cell unchanged, forced `break-before` counts whole,
  `a_paragraph_that_fits_a_page_still_moves_whole` rewritten to 5 / 3, `leading_block_size` with column styles gives 40), all
  integration tests, one `render_smoke` case. Clippy `-D warnings` and fmt clean. The forced-`break-before` test was not run red against the
  unguarded arm (the sweep case showed it).
- `fulgur-vrt` on this host (35 `.actual.pdf`, not blessed here): byte-identical to the F30c-o fork's (`55154081`).

### Ours

- Byte gate (RAYON_NUM_THREADS 1 and 4; main-side binary = the F30c-o binary `htmltopdf-f30co`, fixtures 01-37 + 39 + the 7 corpus
  documents): identical except `39-terms-in-section` (new) and `b200-mixed`. One and four threads give identical bytes. Fixture 38 does not
  exist on this branch; the gate is re-run when it is rebased onto the merged F30c-o.
- Wrapped F30c cases (`out/scratch/f30cn/c`, 116 documents: the 52 F30c cases in shapes A and B plus the plan's tests 3-10 and probes):
  lines per page and `AFTER` page / y (±1 pt) at Chrome's in 108, against 51 on the F30c-o binary. The 8 left: 3-line paragraphs with two
  lines of room (F30c-w, pinned), `sp968_n30` A / B (room of 2 lines to the pixel: Chrome's 1010 px page against our 1009.13, F30c's
  body-level twin has the same, F30f-c), the table cell probe (F30c-t, filed).
- Sweep (`out/scratch/f30cn/sweepgen.py`, `sweep.py`): 756 documents, the 378 body-level F30c / F30f-l cases of `out/scratch/f30flv`
  (`c_two`, `c_mid`, `c_mid2`, `c_tiny`, `c_first`, `c_h`, `c_wrap`, `c_wrap2`, `c_lh`, `c_long`, `c_rich`, `c_edge`, `c_pg`, `c_hf`, `c_just`,
  those with the `AFTER` paragraph) in shapes A and B, each with a Chrome PDF. 704 unchanged, 52 changed, **52 closer to Chrome, 0 further**
  (lines per page and `AFTER`); at Chrome: 648 on the F30c-o binary, 700 now. The first run had one case further (`c_h_h_ba_n48_B`:
  `<p>PRE</p><section><p style="break-before:page">48 lines</p></section>`; the old binary got 0 / 48 by accident, because the whole
  wrapper did not fit; the forced break on the first child of a wrapper is not honoured by the fragmenter when the wrapper stays on the
  page, which is an old gap: with 10 or 30 lines it is ignored on both binaries). Fixed by not counting the paragraph by its first lines
  when it has a forced `break-before` (guard test `a_forced_break_before_the_first_paragraph_of_a_wrapper_is_not_started_in_place`); the gap
  itself is filed as F30c-b. The cases left not at Chrome are unchanged ones (heights, `min-height`, line-height families, F30f-c capacity).
- `b200-mixed`: 203 pages before, **198 now, Chrome 199** (`out/scratch/f30cn/mixed`, the corpus document with the sheet linked). Every
  word once: the multiset differs from Chrome's only in four hyphen-split tokens (`e-` + `mailem`); the F30c-o binary printed 54 extra words
  (10 repeated table headers). The heading `Doklad N` is on Chrome's page for 159 of 181 units, against 29 before (the others one page off).
- `abspos_flow::an_abspos_wrapper_of_ten_paragraphs_is_fragmented_from_its_start`: 10 pages, now 9 like Chrome, with Chrome's words per page
  (342 / 342 / 343 / 356 / 343 / 341 / 333 / 342 / 258); the test asserts `== 9`.
- `compare.py --threshold-check`: all fixtures ok. Fixture 39: 7.5 % / 13.4 % on the F30c-o binary, **5.9 % / 4.9 %** now; threshold 0.069
  (worst + 1 point). No other fixture moved. Side-by-side of 39 looked at: both pages match Chrome's text lines, page 1 ends with clause 27.
- `bench.py --check` ok: b200-items 1331 ms (byte-identical output; the "1250 ms" note is the machine, as before), b200-text 806 ms,
  b200-mixed 824 ms (was ~780 before; the chain walk is not measurable), scaling r 3.78.
- Findings filed: F30c-t (table cell), F30c-g (flex / grid item), F30c-b (forced `break-before` on the first child of a wrapper that stays
  on its page is ignored); F30f-f unblocked; F30c-o2 / F30c-o3 (keep-the-orphans for short nested paragraphs) left to F30c-o3.
- Not verified: Chrome numbers for orphans / widows author values on nested paragraphs; shapes with an `avoid` ancestor that holds and is
  entered for a forced break inside it (reasoned, not measured); the plan's step 5 `vrt-validator` run (not run from this session); a
  full `cargo test --release --locked` is run at the end (see the final report).
