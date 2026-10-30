# F30f-l: a block that overflows its page by less than a line keeps its last line in the bottom margin

Date: 2026-10-29. Finding: F30f-l in `2026-10-15-m0-findings.md` (medium). The same code is behind the first two bullets of **F30c** ("a one-line widow stays on the previous page and the last fragment overflows by one line", F83 MEDIUM-2). This plan closes those two bullets as well; it is one cause, not a second finding. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, head `aba83f75` (the commit in `Cargo.lock` after F66f; the line numbers below were read at `02292ef8`, the F30f-h head). Function names and line numbers are from that head; re-read them before editing.

## 1. Problem

Take a body-level inline root (a `<div>` or `<p>` with direct text) whose lines exceed the room on a page by less than one line, so exactly one line is left over. Chrome moves the last **two** lines to the next page (widows 2). We keep every line on the page, and the last one is painted below the content box.

Repro `out/scratch/f66f/w/un_n49.nobr.html` (= `f30fdv/a/un_n49.html` since F66f): A4, 15 mm, `font:14px/21px Inter`, `<div>` of 49 `<br>` lines (1029 px; the content box is 1009.13 px), then `<p>AFTER</p>`.

| | lines page 1 / page 2 | `AFTER` (pt) |
|---|---|---|
| Chrome m141 | 47 / 2 | page 2, 76.1 |
| ours (F66f head) | 49 / 0; `w0048` at 800-813 pt, below the content bottom (799.4) | page 2, 44.3 |

With 50 lines we split 48 / 2, like Chrome (`un_n50`). With `@page { margin: 0 }` the extra line runs off the page edge and is cut (F30c bullet 1, `n3385` / `n3858`). Any body-level paragraph whose last page would hold one line has this shape, long paragraphs included: with *n* lines and room for *c* per page, it happens whenever *n* mod *c* = 1.

**Shape 2 of the finding is a different cause and is handed to F30f-c (§3.3).** The finding said Chrome pushes the box's bottom inset to page 2. The Chrome PDFs show otherwise. In `exact_n46_pb22`, `pd_n47` and `pdt_n47`, the border box is exactly 1010 px; Chrome keeps it whole on page 1, bottom border included, and puts only `AFTER` on page 2. In `exact_n45_pb22`, the box (989 px) plus `AFTER` (21 px) ends at 1010; Chrome keeps both on page 1. All four end in (1009.13, 1010]: Chrome's fragmentainer is 1010 px (F30f-c). This is not a widows or inset rule.

## 2. Spec (cited by the planner; css-spec-researcher not run)

- **css-break-3 §3.3** `orphans` / `widows`: the minimum number of line boxes of a block container left at the bottom of a fragment before a break, and at the top of the fragment after it. Initial value 2 for both.
- **css-break-3 §4.1** break points: Class B between adjacent line boxes; Class C between the content edge of a container and its child content *only if there is a non-zero gap*. So between the last line box and the bottom padding of an auto-height box there is no break point, and a box whose lines fit but whose bottom inset does not is broken between lines. Chrome does exactly this when the inset overflows by more than its rounding: `exact_n47_pb20-22`, `pd_n48`, `pdt_n48` split 45 / 2 and 46 / 2 in Chrome and in ours (`out/scratch/f66f/sweep.jsonl`). `plan_line_breaks`' `last_extra` therefore stays as it is.
- **css-break-3 §4.4** (unforced breaks): a Class B break is allowed only with ≥ `orphans` lines before it and ≥ `widows` after it; the rules are relaxed only "if the above doesn't provide enough break points to keep fragmented content from overflowing the fragmentainer". In `un_n49` a break after line 47 satisfies both rules and does not overflow, so overflowing is not allowed.
- **CSS 2.1 §13.3.3**: a line box is never split, so a line either fits or moves.
- **css-break-3 §5.4** `box-decoration-break: slice`: not involved (shape 1 has no insets).
- The 1010 px page (shape 2): css-page-3 makes the page area the page box minus its margins, 297 mm − 30 mm = 1009.13 px. No spec text allows the extra 0.87 px; it is Chrome's whole-pixel snapping. Matching it is a reference-compatibility policy, which belongs in F30f-c with its own ADR (§3.3).

## 3. Cause, traced (fork `crates/fulgur/src/pagination_layout.rs`)

### 3.1 Shape 1: no tolerance; the widows rule skips the break instead of pulling it back

`fragment_pagination_root`, body-level line path for `un_n49`:

1. `collect_inline_line_metrics` (3440) returns 49 lines of 21 px.
2. `insets == 0`, so `inset_split_here` (3477-3484) is false; no clip, so `clip_whole_now` (3461) is false.
3. The fitting branch is taken (3540). The pre-advance (3553) does nothing because `cursor_y == 0`.
4. `fragment_inline_root` (8117) walks the lines. Line 48 is the first whose `projected_bottom_in_body` (1029) exceeds `strips.bottom(0)` (1009.13), at 8148.
5. `first_size = 48`, `remaining_size = 1 < WIDOWS_MIN`, so the loop runs `continue` (8152-8161). Every later line has a smaller remainder, so no break is ever taken. The final fragment (8196-8227) holds lines 0..49 and is 1029 px tall.
6. The F30f-h cursor rule (3595-3601) does not apply, so the cursor is 1029 px and the block path (3874) moves `AFTER` to the top of page 2.

`un_n50` splits because at line 48 `remaining_size == 2`; "less than a line" therefore means exactly "a one-line remainder". The doc comment (8084-8092) and the fork test `widow_minimum_blocks_single_line_tail_fragment` (12646-12676) describe this skip as a relaxation; the spec relaxes the rule only when no allowed break exists.

The nested and body-root paths use `plan_line_breaks` (6801-6840), which already pulls back: when `n - end < WIDOWS`, `end = n - WIDOWS` if that leaves ≥ `ORPHANS` lines (6829-6834). Only `fragment_inline_root` lacks this.

### 3.2 The tolerances on this path: none admits shape 1, none is touched

| where | slack | what it is for |
|---|---|---|
| `clip_whole_now` 3462, `inset_split_here` 3484 | `+ 0.5` px | box geometry rounding (as `frame_is_content_sized`) |
| `clip_end_slack` 3265-3269, used at 3554 and 3874 | 1.0 px after a clip box | the clip box ends on Chrome's whole pixel (F30f-b) |
| F30f-h cursor 3598 | `LINE_BOX_SLACK` = 2.0 (6944) | line box against glyph extent (F30f-j) |
| `plan_line_breaks` 6809 | `EPS` = 0.01 | float noise |
| `fragment_inline_root` 8148 | none (strict `>`) | exact fits stay (F197's exact-fit family) |

A 19.87 px overflow passes none of these; the `continue` alone lets it through. The fix changes no comparison, so exact fits, sub-pixel cases (F183h) and F197 keep their bytes.

### 3.3 Shape 2: the 1010 px page (handed to F30f-c, not fixed here)

`exact_n46_pb22`, `pd_n47`, `pdt_n47`: the box ends at 1010 > 1009.13 + 0.5, so it takes the slice branch; `last_extra` (6826) forces `end = n - 1` and the widows rule pulls back to `n - 2` (44 / 2, 45 / 2). In Chrome the box fits its 1010 px page. `exact_n45_pb22`: the one-line `<p>AFTER</p>` takes the block path's exact test (3874) and moves to page 2. Fixing only 3484 would leave 3874 wrong and add a third capacity rule to one path. F30f-c owns "flow uses Chrome's capacity" across 3484, 3554, 3874, 8148 and the `plan_line_breaks` rooms; `PageStrips::chrome_bottom` (2520) exists, and F183h (1122.52 → 1123) is the same rule. The four repros and this analysis go into F30f-c's entry.

### 3.4 Where it belongs

fulgur fork only, in `fragment_inline_root`. htmltopdf moves `Cargo.lock` to the new fork commit and records it in `docs/upstream.md`.

## 4. Fix

In `fragment_inline_root`, at the first line `i` that does not fit:

- If `first_size < ORPHANS_MIN`, keep today's `continue`.
- If `remaining_size < WIDOWS_MIN` and `j = total_lines - WIDOWS_MIN` satisfies `j >= fragment_start_idx + ORPHANS_MIN`, break before line `j`: emit `[fragment_start_idx, j)` on this page, start the next fragment at `j` on the next page, and re-examine lines `j..=i` against the new page.
- Otherwise keep today's `continue` (orphans-violating case, §7).

One pure helper makes it unit-testable:

```rust
/// The break that leaves the next fragment `WIDOWS_MIN` lines when the natural break before line `i` would leave
/// fewer: `Some(j)` with `start + ORPHANS_MIN <= j < i`, or `None` when no such break exists.
fn widow_safe_break(start: usize, i: usize, total: usize) -> Option<usize>;
```

The signature of `fragment_inline_root` stays. Placements and `LineSlice`s are built from the same fragments, so inline images and links follow their lines. Update the doc comment (8084-8092) and the comment of `widow_minimum_blocks_single_line_tail_fragment` (its assertion stays). Not routed through `plan_line_breaks`: that would change `EPS`, the orphans relaxation and the room arithmetic of every body-level split.

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/sub_line_overflow.rs`

Head: `@page{size:A4;margin:15mm}body{font:14px/21px Inter;margin:0}p{margin:0}`; lines `w0000<br>…` without a trailing `<br>`.

1. `a_block_one_line_taller_than_its_page_leaves_two_lines_for_the_next`: `<div>` with 49 lines, then `AFTER`. Page 1 holds `w0000`-`w0046`, nothing below 799.4 pt; page 2 holds `w0047`, `w0048`, `AFTER` within 1 pt of 76.1. Red on main.
2. The same with `<p>`.
3. `a_long_paragraph_with_a_one_line_remainder_on_its_last_page`: 97 lines; Chrome's 48 / 47 / 2 (measured in step 1), every glyph inside its content box.
4. `at_margin_zero_the_last_line_is_not_cut_off`: `@page{margin:0}`, 54 lines; 52 / 2 (step 1 confirms), all 54 words inside the page box.
5. Guards (green on main, bytes unchanged): `un_n48`, `un_n50` (48 / 2), `un_n51` (48 / 3); the 49-line div nested in a `<section>`; the four shape-2 files (comment naming F30f-c); `padded_paragraph_split::an_unpadded_paragraph_keeps_its_page_break_rule`.

### Fork

Lib tests: `fragment_inline_root_pulls_a_line_back_for_a_widow` (49 × 21 px on 1009.13: fragments `0..47`, `47..49`), a multi-page variant, `widow_safe_break` unit tests, an inline image on line 47 placed on page 1; a `render_smoke` case.

### Fixture `34-terms-page-widow`

Czech invoice. Page 1: header, party grid, items table and payment block as in 32. Page 2 starts with `<div class="text-xs text-gray-700" style="break-before:page;line-height:20px">` holding `<strong>Obchodní podmínky</strong><br>` and 50 numbered one-line clauses (≤ 70 characters), each ending in `<br>` except the last (51 lines, 1020 px), then `<p class="mt-6">Převzal: …</p>`. Expected: page 2 holds 49 lines, page 3 clauses 49-50 and the signature (main: 51 lines on page 2, the last in the margin). Chrome reference via `scripts/reference/render.mjs`; threshold worst page + 1.

### Byte gates (1 and 4 threads)

- Fixtures 01-33 byte-identical to the F66f binary.
- 7 corpus documents: `b200-text`, `b200-mixed` or `slow-paragraph` may move if a body-level paragraph has a one-line remainder; each named in §8 with the line that moved.
- Fork `fulgur-vrt`: only a pulled widow may differ; not re-blessed on this host.
- Sweep `out/scratch/f66f/sweep.py` (3381 cases): changed cases must be one-line remainders now at Chrome's split; anything else stops the work.

## 6. Steps

1. Measure Chrome on tests 2-4 and fixture 34's page 2; adjust or drop a test whose Chrome split is not n−2 / 2, with the reason in §8.
2. Red tests and fixture 34 with its reference; commit.
3. Fork: `widow_safe_break` and the branch in `fragment_inline_root`, fork tests (red first), docs; fork lib tests, clippy, fmt; bump `Cargo.lock`; our tests, byte gate, sweep. Stop on a moved fixture 01-33 or a sweep case further from Chrome; write a postmortem.
4. Finish: `render-fixtures.sh`, `compare.py`, look at 34's side-by-side; `vrt-validator` (no `pdf-reviewer`: layout only); findings file (F30f-l done; F30c bullets 1-2 done; F30f-c gets shape 2); `docs/upstream.md`; §8.

## 7. Risks and non-goals

- Page count: a document ending in such a paragraph gains the page Chrome has; only fixture 34 is expected to.
- The F30f-h cursor rule applies only to one fragment on the same page; a pulled split keeps `fragment_inline_root`'s cursor.
- Not changed: the orphans-violating case (F30c); the pre-advance that moves a paragraph whole (F30c bullet 3); `orphans` / `widows` properties (F30c); shape 2 and the 1010 px capacity (F30f-c, needs an ADR); glyph-extent fit (F30f-j); per-fragment snapping (F30f-k); unifying `fragment_inline_root` with `plan_line_breaks`.

## 8. Evidence

(Filled in by the implementer.)
