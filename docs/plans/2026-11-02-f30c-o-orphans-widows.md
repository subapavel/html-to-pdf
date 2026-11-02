# F30c-o: author `orphans` / `widows` are not read; a body-level `<p>` with `orphans: 3` splits where Chrome moves it

Date: 2026-11-02. Finding: F30c-o in `2026-10-15-m0-findings.md`: the F30c bullet "`orphans` / `widows` in CSS are not read", with the F183 §7.6 note on table cells and the 2026-10-30 F30c validator note. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, head `d26eeec2` (the `Cargo.lock` rev). Line numbers were read at that head; re-read them before editing.

## 1. Problem

fulgur breaks paragraphs between lines with the constants `ORPHANS` = `WIDOWS` = 2 (`pagination_layout.rs:6786-6792`). An author `orphans` / `widows` declaration has no effect. Before F30c, a body-level paragraph that did not fit moved whole, so `orphans: 3` happened to look right. Since the F30c pre-advance fix (`starts_in_place`), the paragraph starts wherever its first **two** lines fit. A print sheet with `p { orphans: 3; widows: 3 }` is common, so this case now prints a stub at the page bottom where Chrome prints none.

Repros: A4, 15 mm, `font:14px/21px Inter`, `p{margin:0}`, a spacer, then a 9-line `<p>` of 120 words and `AFTER`.

| case | room | Chrome m141 | ours `d26eeec2` |
|---|---|---|---|
| `out/scratch/f30cv/t7/or3_k2.html` (`orphans:3;widows:3`, spacer 956 px) | 2 lines | 0 / 9 | 2 / 7 |
| `t7/or4_k2.html` (`orphans:4`) | 2 lines | 0 / 9 | 2 / 7 |
| `t7/or4_k3.html` | 3 lines | 0 / 9 | 3 / 6 |
| `t3/orph_k2.html`, `orph_k3.html` (`p{orphans:4;widows:4}`) | 2 / 3 lines | 0 / 9 (page 1 empty, checked in the Chrome PDFs) | split; re-measure in step 1 |
| `or3_k0..k5`, `or4_k0..k5`, the other 9 | | at Chrome's split already | |

Table cells (F183 §7.6): `out/scratch/f183v5/s/u_col1_o4_*.html` and `u_col2_o4_*` use `td{orphans:4;widows:4}` on a collapsed table whose cell holds `img60` and 150 words, after fillers of 996..1032 px. Chrome moves the cell whole (0 | 150). We print 48 | 102 or 32 | 118 (`u_col1`) and 47 | 103 or 31 | 119 (`u_col2`): 20 of 6209 sweep cases. No committed fixture shows this today; fixture 38 (§5) will.

## 2. Spec (cited by the planner; css-spec-researcher not run; * = wording unverified)

- **css-break-3 §3.3** `orphans` / `widows`: Value `<integer [1,∞]>`, initial 2, applies to block containers that establish an inline formatting context, inherited, computed value = the specified integer. `orphans` is the minimum number of line boxes of the block container left in the fragment before a break; `widows` is the minimum left after it. Zero and negative values are invalid (also CSS 2.1 §13.3.2). A non-integer number (`2.5`, `2.0`) does not match `<integer>` (css-values-4), so the declaration is dropped. *Note in §3.3: a block with fewer lines than the value keeps all its lines together.
- **css-break-3 §4.4**: a Class B break (between line boxes) is allowed only with ≥ `orphans` lines before it and ≥ `widows` after it. The rules are relaxed only when the allowed breaks cannot keep content from overflowing the fragmentainer. *The `avoid` rules are dropped first and orphans / widows last. CSS 2.1 §13.3.3 has the same order: rules A, B and D are dropped first, then C.
- **css-break-3 §4.1**: the break before a box (Class A) is an allowed break. So a paragraph with no allowed Class B break on its strip moves whole when content precedes it. That is today's pre-advance.
- **Inheritance**: css-cascade-4 says `unset` acts as `inherit` on an inherited property, and `revert` has no UA declaration to fall back on, so it acts as `inherit` too. CSS 2.1 §9.2.1.1: an anonymous block box inherits from its enclosing box; a pseudo-element inherits from its originating element (css-pseudo-4*).
- **Chrome is the reference here, not the truth.** It gives up `widows` when a paragraph is too short for both rules (F30c-w: a 3-line `<p>` with room for 2 splits 2 / 1). This plan keeps the length rule `n >= orphans + widows` and leaves that deviation to F30c-w.

## 3. Cause, traced (fork `crates/fulgur/src/`)

**Stylo cannot help.** Stylo 0.8.0 lists `orphans` and `widows` only in `properties/counted_unknown_properties.py:10-11` (use counters for unknown properties). There is no longhand for the servo or the gecko engine, so no `clone_orphans()` exists. Blitz parses and drops the declaration.

**Every use of the constants** (`pagination_layout.rs`):

- `ORPHANS`, `WIDOWS` (6786-6792); `fragment_inline_root`'s aliases `ORPHANS_MIN` / `WIDOWS_MIN` (8170-8172); its doc says "not parsed today" (8137-8138).
- Body-level fitting branch: `start_here` (3558-3566) calls `starts_in_place` (8107: `len >= ORPHANS + WIDOWS`, and the first `ORPHANS` lines fit). If it is true, the pre-advance (3567-3573) does not fire. **This is the regression.** With `orphans:3` and room for 2 lines, `starts_in_place` is true, and `fragment_inline_root` (8160; decision at 8194-8212) breaks at `i = 2`.
- `widow_safe_break` (8095): pulls back to `total - WIDOWS` if that is `>= start + ORPHANS`.
- `fragment_inline_root`, `first_size < ORPHANS_MIN` and the `None` of `widow_safe_break` (8198-8211): `continue` keeps accumulating lines, so the fragment overflows. With 2 / 2 this is reachable only through lines taller than a third of a page. With author values it is reachable on ordinary text (`orphans: 60`, or `widows: 30` with a 4-line remainder) and **would lose lines below the page**. The fix must therefore define the relaxation.
- `plan_line_breaks` (6816; widow pull-back 6844-6849): used by `fragment_nested_inline_root` (7832). It relaxes greedily and never overflows.
- `LineSplit::leading` (6909-6917: inset + `ORPHANS` lines), used by:
  - the body-level padded / clip branch (3494-3501);
  - `leading_block_size` (4537: an inline-root cell through `abspos_line_split`; 4563-4569);
  - `fragment_block_subtree` `first_break_h` (5784-5796);
  - `row_mate_can_split` (7780).
- `LineSplit` is built only in `inline_root_lines` (7612-7632), behind `line_split_candidate` (7569), `abspos_line_split` (7604) and `nested_line_split` / `block_walk_line_split` / `body_line_split` (7664-7748). Those cover nested paragraphs, table cells (F30b / F30e / F183), body as the inline root (F111), abspos (`abspos_flow.rs:909-919`), the multicol column-span caller (1954) and ADR-0004's inline-blocks.
- **Multicol ignores even the initial value 2**: `multicol_layout.rs` `slice_lines_by_budget` (1528), `slice_lines_with_first_col_remainder` (1580), and `fill_multicol_columns` (`pagination_layout.rs:588`). This is F30d-b's list ("`orphans` / `widows` … inside columns"); out of scope.

**Pinned by tests** (fork): `widow_safe_break` unit (12881-12891), `starts_in_place` unit (12899-12914), `widow_minimum_blocks_single_line_tail_fragment`, the `plan_line_breaks` tests (15776-15835, 15799, 18405-18413), the `LineSplit` literal (16648), the padding test (17479). Ours: `sub_line_overflow.rs`, `pre_advance_split.rs` (including the F30c-w guard), `row_avoid.rs`, `padded_paragraph_split.rs`. All keep their assertions; they run on the initial values.

### Where it belongs

**fulgur fork only.** The right home is Stylo: a servo-engine longhand, so that `ComputedValues` carries the value. But Stylo is not among our forks, and forking it would need the owner plus a slow rebuild. The repo has already decided where properties Stylo's servo build lacks get read: `column_css` (fulgur-ftp, F87), which reads `break-*` and `column-fill` from `<style>`, `style=""` and the injected `--css` sheet (`blitz_adapter.rs:2311`), with layers, `@media` (print and screen swapped by htmltopdf) and the cascade. So no ADR is needed. Record in `docs/upstream.md`: "drop the reader when Stylo computes `orphans` / `widows`". htmltopdf only bumps `Cargo.lock`.

## 4. Fix

**Reader** (`column_css.rs`):

- `ColumnStyleProps` (124-142) gets `orphans: Option<LineCount>` and `widows: Option<LineCount>`, with `merge` / `is_empty`.
- `parse_value_into` (682) parses `<integer>` ≥ 1 (`expect_integer`), `inherit`, `initial`, `unset` and `revert` (the last two as `Inherit`); anything else is dropped. `calc()` is dropped too (noted as a gap).

```rust
pub enum LineCount { Value(u32), Inherit, Initial }
```

**Computed value** (`pagination_layout.rs`, next to `BreakAvoidScope` 7112-7156, same pattern). The free functions see only the DOM. Reasons for this shape:

- A scope is cheaper than threading the table through ~15 functions.
- Inheritance is resolved at lookup by walking `node.parent` (≤ `MAX_DOM_DEPTH`). This covers anonymous blocks and pseudo-element nodes without a second DOM walk.
- An empty declared set short-circuits to `INITIAL`. Every fixture has an empty set, so the default path costs nothing and its bytes cannot move.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineRules { pub orphans: usize, pub widows: usize }
impl LineRules { pub(crate) const INITIAL: LineRules; }            // 2 / 2
pub(crate) struct LineRulesScope { /* previous Rc<BTreeMap<usize, (Option<LineCount>, Option<LineCount>)>> */ }
impl LineRulesScope { pub(crate) fn enter(column_styles: &ColumnStyleTable) -> Self; }
fn line_rules(doc: &BaseDocument, node: &blitz_dom::Node) -> LineRules;  // per property: nearest declared Value / Initial up the parent chain, else 2
```

`engine.rs` enters the scope next to `_avoid_scope` at 507, 1024 and 1104.

**Consumers.** The value used is that of the inline root, i.e. the block container whose lines are split:

- `LineSplit` gets a `rules: LineRules` field, set in `inline_root_lines(doc, child)`. Its callers already hold a `doc`.
- `leading()` uses `rules.orphans`.
- `plan_line_breaks(.., rules)` uses the rules for the pull-back.
- `widow_safe_break(start, i, total, rules)`.
- `starts_in_place(lines, top, bottom, rules)`.
- `fragment_inline_root(.., line_metrics, rules)`; the body-level branch passes `line_rules(self.doc, child)` to `starts_in_place` and to `fragment_inline_root`.
- `ORPHANS` / `WIDOWS` become `LineRules::INITIAL`.

**Relaxation in `fragment_inline_root`** (replaces the `continue` at 8206-8211): when no break at line `i` honours the author's rules, try the initial rules (today's decision, unchanged). Only when that fails too, keep today's `continue`. Why:

- Spec: §4.4 drops the rules only to avoid overflow.
- Bytes: identical when the rules are the initial ones.
- Safety: never more overflow than today, so `orphans: 60` cannot lose lines.
- Consistency: `plan_line_breaks` already relaxes greedily.

Step 1 checks this against Chrome on large values. If Chrome takes a different break, the helper changes and §8 says why.

```rust
fn line_break_at(start: usize, i: usize, total: usize, rules: LineRules) -> Option<usize>; // today's 3-way decision, parametrised
```

The §4 argument of the F30c plan generalises unchanged: when `starts_in_place` holds, `fragment_inline_root` breaks on the first strip without relaxing. Update the comments at 3547, 4455, 4560, 5787, 6786, 6910, 8090-8138 and the module comment of `abspos_flow.rs:12`. Amend ADR-0004 in one line ("orphans and widows 2" becomes "the inner block container's `orphans` / `widows`, initially 2").

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/orphans_widows.rs`

Helpers as in `pre_advance_split.rs` (`render`, `per_page`, `after`, `assert_inside`). Every number is Chrome's, from §1 or from step 1. "Red" means red at `d26eeec2`.

1. `orphans_three_moves_a_paragraph_with_room_for_two_lines`: `or3_k2`, 0 / 9, `AFTER` on page 2 (±1 pt). Red.
2. `orphans_four_moves_it_with_room_for_two_or_three`: `or4_k2`, `or4_k3`, `orph_k2`, `orph_k3`, all 0 / 9. Red.
3. Guards at Chrome's split already: `or3_k0..k5` minus `k2`, `or4_k0,k1,k4,k5`. Green; they must stay.
4. `widows_three_pulls_back_two_lines`: a 50-line `<p>` with `widows:3` at a page top gives 47 / 3 (step 1). Red (48 / 2 today).
5. `inherited_and_cascaded`: the `or3_k2` result through `body{orphans:3}`, `style="orphans:3"`, `@media print{p{orphans:3}}` and `@layer`. `p{orphans:3} .x{orphans:2}` with `class="x"` gives the 2 / 7 split. Red except the last.
6. `invalid_values_keep_the_initial_rule`: `orphans: 0`, `-1`, `2.5`, `foo` all give 2 / 7 (Chrome's for the initial value). Green.
7. `orphans_one_keeps_a_single_line`: a 30-line `<p>` with `orphans:1;widows:1` and room for 1 line; Chrome's number from step 1 (expected 1 / 29). Red if Chrome splits.
8. `a_value_larger_than_a_page_loses_nothing`: `orphans:60`, then `orphans:30;widows:30`, on 100 lines at a page top and after a 300 px spacer. Every word printed once, nothing below 799.37 pt, lines per page equal to Chrome's (step 1).
9. `a_cell_with_orphans_four_moves_whole`: `u_col1_o4_{996..1032 step 4}` and `u_col2_o4_*` give 0 | 150 words. Red. Same shape with `orphans:2` still splits (guard).
10. `nested_and_padded`: a 60-line `<p style="orphans:3">` in a `<section>` with room for 2 lines, and a padded body-level `<p>` (F30f-d branch) with room for 2 lines. Both start on the next page (step 1). Red.
11. `columns_keep_their_output`: `.terms p{orphans:4}` in a two-column box renders byte-identical to the same box without the property (F30d-b; the comment gives Chrome's number). Green.

### Fork

- `column_css` parse tests: values, keywords, invalid, `!important`, layer.
- `line_rules` tests:
  - a `p` inheriting from `body`;
  - `td` to `p`;
  - an anonymous block beside a `<div>` in `div{orphans:3}` (verifies Blitz sets `parent` on anonymous nodes);
  - a `::before` block;
  - `initial` in between;
  - no scope gives `INITIAL`.
- `starts_in_place`, `widow_safe_break`, `line_break_at` and `plan_line_breaks` with rules.
- `fragment_inline_root` with `orphans: 60` on 100 lines of 20 px on a 200 px page: no fragment taller than the page.
- One `render_smoke` case.
- The existing tests pass `LineRules::INITIAL` and keep their assertions.

### Fixture `38-terms-print-orphans`

A Czech invoice like 36, with an inline `<style>@media print { p { orphans: 3; widows: 3 } }</style>`. On page 1 the items table is sized so that the second terms `<p class="text-xs text-gray-600 leading-relaxed">` (about 20 lines) starts with room for about 2.5 lines; that keeps it clear of the 1 px capacity window (F30f-c).

- **Chrome**: page 1 ends after the first terms paragraph; page 2 starts with the second one.
- **Ours today**: 2 lines at the bottom of page 1.
- **PNG**: page 1 shows blank space below paragraph 1; page 2 starts with paragraph 2.
- **Test**: `fixtures.rs` asserts that page 2's first word equals Chrome's.
- **Diff**: should drop on both pages (no stub on page 1, no 2-line shift on page 2).
- **Threshold**: worst page + 1 point.

### Byte gates (1 and 4 rayon threads)

- **Fixtures 01-36** expected byte-identical. Only 24, 25, 30 and 31 declare the properties (`.terms p{orphans:1;widows:1}`), and only inside multicol, which does not read them. If one of these moves, a `.terms p` reached a line path: stop, and either name it at Chrome's split in §8 or fix it.
- **`tailwind.css` and the 7 corpus documents** declare none, so all must stay byte-identical.
- **Sweeps**: `out/scratch/f30c/sweep.py` (4616 cases) and `out/scratch/f183v5` (6209). Only cases that declare the properties may change (the `t7` / `t3` ones, `u_col*_o4_*`), and each must move toward Chrome.
- **Fork `fulgur-vrt`**: `.actual.pdf` identical before and after.
- **`bench.py --check`**.

## 6. Steps

1. **Chrome probes** (≤ ½ day; `scripts/reference/render.mjs`):
   - the numbers for tests 4, 7, 8, 9 and 10 and for fixture 38;
   - how Chrome relaxes large values;
   - whether `orphans:1` with room for 1 line splits.

   Fix each expectation. A result that contradicts §4 goes to §8 with the change it forces.
2. **Ours, red**: `orphans_widows.rs`, fixture 38 with its reference and README; commit with the red list.
3. **Fork, reader**: `LineCount`, parsing and its tests. There is no consumer yet, so the bytes cannot move; run the fork lib tests.
4. **Fork, computed value**: `LineRules`, `LineRulesScope`, `line_rules`, the three engine sites, the inheritance tests (anonymous / pseudo first).
5. **Fork, consumers**: `LineSplit.rules`, the signatures, `line_break_at` and the relaxation, the comments, the ADR-0004 line. Fork lib, `render_smoke`, clippy `-D warnings`, fmt.
6. **Ours, green**: bump `Cargo.lock`; `cargo test --release --locked`; byte gates; sweeps. Write a postmortem and stop if a fixture moves outside §5, the page count changes, or a sweep case moves away from Chrome.
7. **Finish**:
   - `render-fixtures.sh`, `compare.py`, look at 38's side-by-side, set its threshold;
   - `vrt-validator` (no `pdf-reviewer`: layout only);
   - findings file: F30c-o done, F30c's bullet closed, the F183 §7.6 note answered;
   - `docs/upstream.md`;
   - §8.

## 7. Risks and non-goals

**Risks**

- **Reader coverage.** These limits apply to F90 / F91 and the `break-*` reader as well; in each case the value falls back to 2, so there is no regression from today.
  - `<link>` stylesheets are not read (fulgur-s5ro).
  - Selectors Stylo parses but the reader drops (F90).
  - Doctype-less class matching (F91).
  - `calc()`.
- **Inheritance outside Stylo.** If Blitz leaves `parent` unset on anonymous or pseudo nodes, they fall back to 2. Step 4 tests this first.
- **Thread-local scope.** It is correct only while pagination runs on the render thread. `AVOID_NODES` already depends on this; the 1- and 4-thread gates check it.
- **Relaxation vs. Chrome for extreme values.** Measured in step 1; the code never overflows more than today.
- **Performance.** No cost without a declaration. With one, a parent walk per `LineSplit` build. Measure `b200-items` with `body{orphans:3}` added (scratch only, reported in §8).
- **Determinism.** `BTreeMap` only; the cascade order is `column_css`'s.

**Non-goals**

- **Multicol columns** (F30d-b): even the initial 2 is not applied there.
- **F30c-w**: Chrome drops `widows` for short paragraphs; the length rule `n >= orphans + widows` stays.
- **F30c-n**: nested paragraphs shorter than a page move whole.
- **F30c-f**: floats.
- **`break-after: avoid`** (F87c).
- **A Stylo fork.**
- **The 1010 px capacity** (F30f-c).

## 8. Evidence

Done 2026-11-02. Fork `subapavel/fulgur` `htmltopdf/srgb-colours` `55154081` (on top of F209's `0feb944f`). Ours: branch `f30c-o-orphans-widows`, rebased onto main `3001f66` (F209 / PR #81 merged, fixture 37 present).

### 8.1 Chrome probes (step 1)

Chrome m141 through `out/scratch/f30co/{gen,run,sweep}.py` (A4, 15 mm, `14px/21px`, a page of 48 lines). Every expectation of §5 is from these; the 15 tests of `orphans_widows.rs` and the fixture test are green.

- Test 1, 2: 9 lines, room for 2 lines, `orphans: 3` / `4` (`widows` equal or default): 0 / 9, `AFTER` page 2 at 186.3 pt; room 3 with `orphans: 4`: 0 / 9; guards as §1 (`or3` k0, k1: 0 / 9, k3, k4, k5: 3 / 6, 4 / 5, 5 / 4; `or4` k4, k5: 4 / 5, 5 / 4).
- Test 4: `widows: 3`, 50 lines: 47 / 3 (49: 46 / 3, 51: 48 / 3); `widows: 4`: 46 / 4 (49: 45 / 4); `widows: 1`, 49 lines: 48 / 1. Ours before: 48 / 2, 47 / 2, 48 / 3, 48 / 2, 47 / 2, 47 / 2.
- Test 5: all of `body{orphans:3}`, `html{orphans:3}`, `style=`, `@media print`, `@layer`, `inherit`, `unset`, `revert`, `!important` against a plain `style`, and `style` `!important` against the sheet's give 0 / 9; `initial` and a later `.x{orphans:2}` give 2 / 7. `calc(3)` is valid in Chrome (0 / 9).
- Test 7: 30 lines, `orphans: 1`: room for 1 line 1 / 29 (`AFTER` 501.3 pt), room for 2 lines 2 / 28 (485.6 pt), none 0 / 30 (517.1 pt), with or without `widows: 1`.
- Test 8, how Chrome relaxes (sweep `sw_*`: 52 and 100 lines, `orphans` and `widows` in 1, 2, 10, 30, 47, 48, 49, 60, at the page top and after a 300 px spacer = 33 lines of room, 256 pages). With `R` lines left and `cap` lines on the page:
  - `orphans > cap`: orphans and widows are both given up (100 lines, `orphans: 60`: 48 / 48 / 4; after a spacer 0 / 48 / 48 / 4: the paragraph moves to a page of its own first);
  - otherwise the break is at `max(R - widows, orphans)`: when the widows cannot be had without breaking the orphans, **Chrome keeps the orphans and gives up the widows** (100 lines, 30 / 30: 48 / 30 / 22; 2 / 30: 48 / 22 / 30; after the spacer 33 / 37 / 30 for both);
  - the exceptions are `widows` larger than the page (49, 60 against 48 lines): see F30c-o2.
- Test 9: `td{orphans:4;widows:4}`, `img60` + 150 words, spacer 996..1032 step 4, 1 px and 2 px borders: 0 | 150 for all 20. `orphans: 2`: 48 | 102 up to 1012, 32 | 118 from 1016 (ours with a 2 px border one word off: 47 | 103, 31 | 119, as before).
- Test 10: 60 lines `orphans: 3` in a `<section>`: room 1 and 2 give 0 / 48 / 12 (`AFTER` page 3, 233.6 pt), room 3 gives 3 / 48 / 9; `orphans: 2`, room 2: 2 / 48 / 10. Padded `<p>` (30 lines, 9 px above the first line): `orphans: 3` with room for 2 or 3 lines 0 / 30 (530.6 pt), `orphans: 2` with room 3: 2 / 28 (492.3 pt).
- Fixture 38: with `@media print { p { orphans: 3; widows: 3 } }` page 1 ends with clause 25, page 2 starts with "26." (`AFTER`-like "50." at 249.3 pt, "Převzal:" at 295.8 pt); the same document without the declaration puts two lines of clause 26 at the bottom of page 1, as ours did.

### 8.2 The fork (steps 3-5)

Fork commit `55154081`: `column_css.rs` (`LineCount`, `ColumnStyleProps::{orphans, widows}`, `parse_line_count_value`), `pagination_layout.rs` (`LineRules`, `LineRulesScope`, `line_rules`, `line_break_at`, `keep_orphans_break`, and `LineSplit::rules`, `inline_root_lines(doc, ..)`, `abspos_line_split(doc, ..)`, `plan_line_breaks`, `starts_in_place`, `widow_safe_break`, `fragment_inline_root` with a `rules` argument), `engine.rs` (three scope sites), `render.rs` and `abspos_flow.rs` (call sites, comment). Fork tests: lib 2465 passed (new: reader values, keywords, invalid, `!important`, cascade; `line_rules` without a scope, inherited per property, keywords, td to p, anonymous block and `::before` box; `starts_in_place`, `widow_safe_break`, `line_break_at`, `keep_orphans_break`, `fragment_inline_root` on author values and on values a page cannot keep, `plan_line_breaks`, `LineSplit::leading`), one `render_smoke` case; `cargo clippy -p fulgur --all-targets -- -D warnings` and `cargo fmt` clean.

**Deviations from §4:**
- *Relaxation.* §4 relaxed to the initial rules. The probes show Chrome keeps the orphans and gives up the widows before that (100 lines, 30 / 30: 48 / 30 / 22, where the initial rules would give 48 / 48 / 4). `fragment_inline_root` therefore has three steps: `line_break_at` with the author's rules, `keep_orphans_break` (only for values other than 2 / 2, only when pulling back would leave the fragment some lines but fewer than `orphans`), `line_break_at` with `INITIAL`, then today's accumulate. With 2 / 2 the first step is today's decision and the other two are empty: byte-identical.
- `plan_line_breaks` (nested paragraphs) only uses the rules for the pull-back; it has no `keep_orphans_break` (F30c-o2).
- `Node::parent` is set on anonymous blocks and `::before` boxes (tests), so no `layout_parent` fallback is needed.
- The length rule `lines >= orphans + widows` stays (F30c-w, F30c-o2).
- Fixture 38 has nine items and two terms paragraphs of 15 and 16 lines (§5 said about 20); the room on page 1 is about 2.3 lines.
- The plan's "37" for the fixture was already 38 in the text.

### 8.3 Gates

Baseline binary: main's after F209 (`htmltopdf-main-f209`, fork `0feb944f`; `Cargo.lock` differs only in the fork rev).
- Byte gates, `RAYON_NUM_THREADS` 1 and 4, run again after the rebase: fixtures 01-37 and the 7 corpus documents byte-identical to the baseline (44 of 44 each; fixtures 24, 25, 30, 31 declare the properties inside multicol and did not move); fixture 38 differs, as it should.
- Sweeps: 11,975 cases (the f30c sweep 4,616, f183v5 6,209 and the `f30cv` probes) rendered with both binaries: 78 differ, all 78 declare `orphans` / `widows` (the `td{orphans:4}` cells, `p_img_l3_o4_*`, `u_sep*_o4_*`, `u_col*_o4_940..956`), and in all 78 the words per page equal Chrome's (50 of them with Chrome PDFs made for this check); none moved away.
- Fork `fulgur-vrt`: the 70 `.actual.pdf` of the failing goldens (this host's fonts differ) are byte-identical to the F209 run's (`f209-vrt-new`; assumed to be at `0feb944f`, not re-run at that rev).
- `compare.py` fixture 38: 5.4 / 2.9 % (baseline binary 5.9 / 5.6 %); threshold 6.4 %. Side-by-side looked at: page 1 ends with clause 25 and blank space below, page 2 starts with clause 26, like Chrome.
- Tests: `cargo test --release --locked` on the tree before the rebase: 81 test binaries, 937 passed, 0 failed (fixture 37 and F209's tests were not in that tree); after the rebase `fixtures` (18) and `orphans_widows` (15) were run again, green. `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings` clean.
- `scripts/render-fixtures.sh` and `compare.py --threshold-check`: all 38 fixtures ok, page counts equal Chrome's.
- `bench.py --check` ok: `b200-items` 1218 ms (accepted exception 1250), `b200-text` 776 ms, `b200-mixed` 897 ms, fixture 38 16.4 ms.
- Performance with a declaration (scratch): `b200-items` with `body{orphans:3;widows:3}` added, median of 7 runs of the CLI: 1287 ms against 1277 ms without (+0.8 %, within noise).

### 8.4 Not done

- `vrt-validator` and `pdf-reviewer` (the latter not required: layout only) were not run by the implementing agent; left to the merge policy's step 2.
- The 100-line probes at other page heights, Chrome versions other than m141, `columns` with `orphans` (pinned as unchanged), `float`, and `widows` larger than a page (F30c-o2).
- Fork `fulgur-vrt` was compared with the earlier F209 run, not re-run at `0feb944f`.
