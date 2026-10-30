# F30c (pre-advance): a body-level paragraph that does not fit the rest of its page moves whole; Chrome starts it in place and splits it

Date: 2026-10-30. Finding: F30c in `2026-10-15-m0-findings.md`, bullet 3 (the pre-advance, with the 2026-10-30 F30f-l validator note) and bullet 4 (the F87 consequence: `break-inside: avoid` is invisible on a body-level `<p>`). Bullets 1-2 were closed by F30f-l. The `orphans` / `widows` properties (bullet "not read") stay open. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, head `5ec1ac67` (the `Cargo.lock` rev; F30f-l's `widow_safe_break`). Line numbers below were read at that head; re-read them before editing.

## 1. Problem

A body-level inline root (a `<p>` or a text `<div>` directly in `<body>`) of two or more lines that does not fit the room below the cursor is moved whole to the next page. Chrome starts it where it is and breaks it between line boxes. No committed fixture shows this today (fixture 13 is the `avoid` twin, where moving it is right); fixture 36 (§5) will.

Repros (A4, 15 mm, `font:14px/21px Inter`, `p{margin:0}`, lines `w0000<br>…`; a page holds 48 lines). `out/scratch/f30flv/c_two/two_N_M.html` holds an N-line `<p>`, an M-line `<p>` and `<p>AFTER</p>`, with Chrome m141 PDFs beside them; the numbers are from `out/scratch/f30flv/res_c_two.txt`. Lines per page are counted over both paragraphs:

| case | Chrome | ours (`5ec1ac67`) |
|---|---|---|
| `two_49_47` | 47 / 47 / 2, AFTER p3 76 pt | 47 / 2 / 47, AFTER p3 785 pt |
| `two_49_48` | 47 / 48 / 2 | 47 / 2 / 48 / 0 (4 pages) |
| `two_49_49` | 47 / 48 / 3, AFTER 92 pt | 47 / 2 / 47 / 2 (4 pages) |
| `two_49_50`, `_60` | 47 / 48 / 4, 47 / 48 / 14 | 47 / 2 / 48 / 2, 47 / 2 / 48 / 12 |
| `two_50_47` … `_60` (already on main) | 48 / 47 / 2 … 48 / 48 / 14 | 48 / 2 / 47 … 48 / 2 / 48 / 12 |

Other shapes from the finding:
- An `h1` followed by a 420-word `<p>` leaves page 1 with only the heading.
- A 600-word `<p>` after a 650 px spacer moves whole to page 2, where Chrome splits it 245 / 355 words (F30d review L3).
- F87: a 100-word `auto` `<p>` near the page bottom moves whole, where Chrome puts 14 words on page 1. It is placed exactly like a `break-inside: avoid` one, so the property cannot be seen.

The repro files for these three shapes were not kept; step 1 rebuilds them. Since F30f-l pulls a one-line widow back, two long paragraphs in a row also lose a page against Chrome.

## 2. Spec (cited by the planner; css-spec-researcher not run; items marked * are unverified wording)

- **css-break-3 §4.1**: Class A break points are between sibling boxes; Class B are between adjacent line boxes. Both are ordinary break opportunities. Nothing ranks a break before the paragraph above one inside it.
- **css-break-3 §3.3 / §4.4**: a Class B break needs ≥ `orphans` lines before it and ≥ `widows` after it (initial value 2). These rules are relaxed only when no allowed break keeps the content inside the fragmentainer. A paragraph whose first two lines fit and that has ≥ 4 lines therefore has an allowed break on this page. A paragraph with ≤ 3 lines, or one whose first two lines do not fit, has none, and the break before it (Class A) is the allowed one.
- **css-break-3 §4.4\***: the UA should fill the fragmentainer ("as much content as possible") and may optimise further (§4.5\*). CSS 2.1 §13.3.5 ("break as few times as possible", informative) points the same way. Chrome takes the last allowed break that fits.
- **css-break-3 §3.2** `break-inside: avoid`: avoid breaks inside the box. F83 relaxes it for a box taller than a page. Then Chrome starts the box on a fresh page (fixture 13 reference; F83b rule "an avoid box taller than a page starts the next page when content precedes it").
- **CSS 2.1 §13.3.3**: a line box is never split.
- **Headings\***: CSS 2.1 Appendix D (an informative sample sheet) has `@media print { h1…h6 { page-break-after: avoid } }`. The HTML rendering section does not, as far as the planner knows. Whether Chromium's UA sheet has it is unverified; step 1 probes it. Our fork parses `break-after: avoid` as "no forced break" (F87c) and does not honour it.

## 3. Cause, traced (fork `crates/fulgur/src/pagination_layout.rs`)

`fragment_pagination_root` (2958), body child loop, line path:

1. `collect_inline_line_metrics` (3440) gives the lines. `avoid_inside_holds` (3445) empties them for an `avoid` paragraph that fits a page; that one takes the block path, whose advance (3874) moves it whole. That is correct and stays.
2. A clip box (`clip_limit_here`, 3455-3462) or a padded box that does not fit (`inset_split_here`, 3477-3484) goes to `fragment_nested_inline_root` (3485-3539). That branch already starts the box in place when `LineSplit::leading()` (6896: inset + `ORPHANS` lines) fits (3494-3501). This is the rule the finding names, as in `fragment_block_subtree` / `nested_line_split`.
3. Every other multi-line paragraph takes the fitting branch (3540). **The pre-advance at 3553-3558 is the cause:**

   ```rust
   if cursor_y > 0.0
       && cursor_y + para_total_h > self.strips.bottom(page_index) + clip_end_slack
   { page_index += 1; cursor_y = 0.0; }
   ```

   Its comment (3541-3548) says it is the fallback for when "the inline split path can't honour widow / orphan constraints within the strip". The condition it implements is "the whole paragraph does not fit", so it also fires when an allowed break exists. It fires for paragraphs taller than a page too: `two_49_49`'s second paragraph and the 40 000-word `slow-paragraph` start on a fresh page.
4. `fragment_inline_root` (8134) can split in place. At the first line that does not fit (8168) it breaks before line `i`, or pulls back with `widow_safe_break` (8080). But when fewer than `ORPHANS` lines fit (8172-8186) it `continue`s and lets the first fragment overflow. That is why the pre-advance cannot simply be removed: it must keep firing exactly when no allowed break exists on this strip.

Unchanged, and why:
- The F30f-h cursor rule (3595-3601) needs `frag_count == 1`. A split gives ≥ 2.
- `clip_whole_now`, `clip_end_slack` (3265) and the block-path advance (3874) are untouched.
- Exact fits keep `<=`.
- Sub-pixel cases (F183h, single-line → block path) are untouched.
- F197 is the table-cell path (`fragment_block_subtree`), not this branch.

### Where it belongs

fulgur fork only. Pagination is fulgur's own code; Blitz / Taffy / Parley only supply the line metrics. htmltopdf bumps `Cargo.lock` and records the commit in `docs/upstream.md`; no upstream PR (standing decision).

## 4. Fix

Narrow the pre-advance to "no allowed break here". A new pure helper next to `widow_safe_break`:

```rust
/// html-to-pdf F30c: whether a paragraph with these lines, placed with `room` px left on its strip, has an allowed
/// break there: its first `ORPHANS` lines fit and a break after them can leave `WIDOWS` lines (`len >= ORPHANS + WIDOWS`).
fn starts_in_place(lines: &[(f32, f32)], room: f32) -> bool;
```

The fit test is `lines[ORPHANS-1].1 - lines[0].0 <= room`, the comparison `fragment_inline_root` makes at 8168. In the fitting branch, `room = strips.bottom(page_index) - cursor_y` (strict, no `clip_end_slack`, so the helper and the splitter agree). The pre-advance fires only when, in addition to today's condition, `!start_here`, where:

```text
start_here = !avoid_inside && !is_float && insets == 0 && clip_limit_here.is_none() && body_block_flow
             && !is_multicol_container(child) && !block_size_constrained(child)
             && child_h <= para_total_h + LINE_BOX_SLACK && starts_in_place(&line_metrics, room)
```

Each exclusion keeps a shape whose current output is right or owned by another finding:
- **`avoid_inside`**: only a relaxed (taller-than-a-page) avoid paragraph reaches this branch, and Chrome starts it on the next page (fixture 13, `b200-text`).
- **insets, `height` / `min-height` beyond the lines**: `fragment_inline_root` does not know the insets (the F30f-d cause) or the box's extra height (F30f-h LOW notes). These shapes reach here only when the padded branch excluded them.
- **floats**: not placed by this path (F30f-h comment).
- **clip and multicol boxes**: F30f-b and F30d own them.

Why `fragment_inline_root` then always breaks on the first strip: n ≥ 4 and lines 0-1 fit. At the first line `i` that does not fit, `first_size >= 2`. Either `remaining >= 2` and the break is at `i`, or `widow_safe_break` returns `n - 2 >= 2`. So no overflow and no new code in `fragment_inline_root`.

Comments to update:
- the pre-advance comment (3541-3548);
- `nested_line_split`'s doc (7666-7669), which says "as body level does";
- the F30c notes at 3469 and 7155.

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/pre_advance_split.rs`

Head and helpers as in `sub_line_overflow.rs` (`render`, `lines`, `per_page`, `after`, `assert_inside`). Every number is Chrome's, from `res_c_two.txt` or measured in step 1. "Red" means red at `5ec1ac67`.

1. `a_second_long_paragraph_starts_below_the_first`: the 10 cases `two_49_{47,48,49,50,60}` and `two_50_{47,48,49,50,60}`. Lines per page and AFTER page and y (±1 pt) must equal the table in §1. Red, all 10.
2. `a_paragraph_below_a_spacer_starts_where_two_lines_fit`: a `<div style="height:Hpx">`, then a 30-line `<p>`, then AFTER. H=650 gives 17 / 13 (red). H=966 gives 2 / 28 (red). H=975 gives 0 / 30, moved whole (guard). H=967-968 is avoided: there our room is 41.13-42.13 px and Chrome's is 42-43 (1010 px page, F30f-c).
3. `a_heading_then_a_long_paragraph_starts_below_the_heading`: `<h1>` plus a 46-line `<p>`. Page 1 holds the heading and as many lines as Chrome's (step 1). Red.
4. `six_hundred_words_after_a_650px_spacer_start_on_page_one`: words per page within one line of Chrome's (finding: 245 / 355; re-measured in step 1), every word once, none below 799.37 pt. Red.
5. `break_inside_avoid_is_visible_on_a_body_level_paragraph`: 100 words after a 960 px spacer, measured with `style="break-inside:avoid"` and with `class="break-inside-avoid"` plus the compiled sheet. `auto` puts its first 2 lines on page 1 (red); `avoid` puts 0 on page 1 (guard).
6. Guards (green at `5ec1ac67`, must stay):
   - a 3-line `<p>` after H=966 moves whole;
   - a relaxed `avoid` `<p>` of 60 lines after a 300 px spacer starts page 2 (48 / 12);
   - a padded `<p>` (F30f-d branch) keeps its bytes;
   - a float `<p>` keeps its bytes;
   - the whole of `sub_line_overflow.rs`.

   A 4-line `<p>` after H=945 (3 lines fit) splits 2 / 2: that one is red.
7. Updated to Chrome's numbers (measured in step 1), each with a comment naming F30c:
   - `padded_paragraph_split::an_unpadded_paragraph_keeps_its_page_break_rule` (today 0 / 801 / 699) becomes `an_unpadded_paragraph_starts_where_its_first_lines_fit`;
   - the module comment of `tailwind_breaks.rs` (lines 10-14), plus a body-level `<p class="break-inside-avoid">` case;
   - `abspos_flow.rs:324-341`: tighten "+2 for F30c" if the count reaches Chrome's 9.

### Fork

- `starts_in_place` unit tests: fewer than 4 lines, two lines fit, an exact fit at `<=`, negative room.
- Lib tests through `fragment_pagination_root`:
  - 10 lines of 20 px after a 100 px spacer on a 200 px page give fragments `0..5` on page 0 at y 100 and `5..10` on page 1;
  - an inline image on line 2 is placed on page 0;
  - a relaxed avoid paragraph starts page 1;
  - a 3-line paragraph moves whole.
- `a_paragraph_after_a_spacer_moves_whole_when_page_zero_has_no_room` (18385) asserts the old rule. Rewrite it to keep its F29 intent with a 125 px spacer: with `body{margin:40px}` 1 line fits and the paragraph moves; with `margin:0`, 3 / 2 on pages 0 / 1.
- One `render_smoke` case.

### Fixture `36-terms-after-items`

Czech invoice. Page 1: header, party grid and an items table as in 32, sized so that the terms start about 300 px above the page-1 content bottom. Then `<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>` and one body-level `<p class="text-xs text-gray-600 leading-relaxed">` of about 450 words of Czech terms (about 32 lines of 19.5 px: fits a page, not the room), then `<p class="mt-6 text-sm">Převzal: …</p>`. All these classes are in the compiled sheet; check any others with grep.

- Expected: like Chrome, page 1 holds the heading and the first lines of the terms, page 2 the rest and the signature (2 pages).
- Main's binary leaves the heading alone at the bottom of page 1 and moves the whole paragraph to page 2.
- PNG: page 1 must show the terms below the heading down to the content bottom.
- `fixtures.rs` asserts the first word on page 2 against Chrome's (±1 line).
- Threshold: worst page + 1 point. The diff should drop on page 1 (missing text now rendered) and page 2 (no longer shifted).

### Byte gates (1 and 4 rayon threads)

- **Fixtures 01-35** are expected byte-identical. Their multi-line body-level paragraphs fit, start a page (34), are `avoid` (13), are padded (29, 32, 33), are clip boxes (28), or the body is the inline root (19, F111). A fixture that moves must be at Chrome's split and named in §8. A page-count change is a stop (`PAGE_COUNT_MAY_DIFFER` is empty, ADR-0003).
- **Corpus**: `slow-paragraph` (one 40 000-word `<p>` after the header and parties) is expected to move: it starts on page 1, and its 59 pages may become 58. Compare lines per page with a Chrome render of it, as F30f-l did for `b200-text`. `b200-text` (avoid paragraphs) and `b200-mixed` (clip card) are expected byte-identical; `b200-items`, `slow-*` likewise.
- **Sweeps**: `out/scratch/f30fl/sweep.py` (3588 cases) and `out/scratch/f30flv/tools/sweep.py`. Every changed case must be closer to Chrome in lines per page and page count. Anything further away stops the work.
- **Fork `fulgur-vrt`**: `.actual.pdf` identical before and after, or the difference is only an in-place split; not re-blessed on this host.
- **`python3 scripts/bench.py --check`**: the helper is O(1) per paragraph.

## 6. Steps

1. **Chrome** (`scripts/reference/render.mjs` on scratch HTML; ≤ ½ day): tests 2-7, fixture 36's split, and the heading probe. The probe is `h2` at the page bottom with room for the heading plus 0, 1 or 3 lines of a long `<p>`: does Chrome move the heading with the paragraph? Fix every expectation; drop a test whose Chrome split contradicts §2, with the reason in §8.
2. **Ours, red**: `pre_advance_split.rs` and fixture 36 with its reference and README; commit with the red list.
3. **Fork**: `starts_in_place` (stub `false`, tests red, then implement), the `start_here` wiring, comments, the fork tests of §5. Fork lib tests, `render_smoke`, clippy `-D warnings`, fmt.
4. **Ours, green**: bump `Cargo.lock`; update the three tests of §5 item 7; `cargo test --release`; byte gates; sweeps. Stop and write a postmortem on a moved fixture that is not at Chrome's split, a page-count change, or a sweep case further from Chrome.
5. **Finish**:
   - `render-fixtures.sh`, `compare.py`, look at 36's side-by-side, set its threshold;
   - run `vrt-validator` (no `pdf-reviewer`: layout only);
   - findings file: F30c bullets 3-4 done, the nested half filed (§7), the heading result filed if Chrome keeps headings with their lines;
   - `docs/upstream.md`; this plan's §8.

## 7. Risks and non-goals

**Risks**
- **Page counts.** Documents with long body-level paragraphs lose the page Chrome does not have. Only `slow-paragraph` is expected to move.
- **First fragment below other content.** A body-level split whose first fragment sits at `y > 0` was barely exercised before: only the `clip_end_slack` window, and F30f-l's geometry-only test. The renderer's line slices, inline-box placements (`record_split_inline_root_descendants`) and link rects must be checked in the PDF, which is why test 1 asserts every line's page and position.
- **F30f-l pullback.** It is reached on the first strip now (`two_49_47`). §4 shows it always finds a break there. Later strips are unchanged.
- **Headings.** The fix never orphans a heading more than today: a paragraph that moves still leaves the heading behind, as now. Fewer headings are left alone, because a paragraph with two fitting lines now stays.
- **Capacity.** Rooms within 1 px of a line boundary differ from Chrome's 1010 px page (F30f-c); the tests avoid them.
- **Performance.** One comparison per paragraph placement.

**Non-goals**
- **Nested paragraphs.** One shorter than a page still moves whole when it does not fit (`nested_line_split` 7666-7707, 16045 test). This is F30c's nested half; file it (suggested id F30c-n). F30f-f depends on that half, not on this plan.
- **`orphans` / `widows` properties.** Still not read (F30c, F183 §7.6).
- **`break-after: avoid` on headings.** F87c parses it only.
- **Shapes excluded by `start_here`.** Padded, constrained, float, clip and multicol paragraphs.
- **Other findings.** The 1010 px capacity (F30f-c, needs an ADR); F197; unifying `fragment_inline_root` with `plan_line_breaks`.

## 8. Evidence

(filled in at implementation)
