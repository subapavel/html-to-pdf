# F30c-o3: author `orphans` / `widows` — how far to relax for a short paragraph, keep-the-orphans in the nested splitter, `orphans` larger than a page

Date: 2026-11-02. Finding: F30c-o3 (MEDIUM, vrt-validator on the F30c-o PR), plus the parts of F30c-o2 that come out of it. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`. The work goes **on top of F30c-n** (`a699c0c5`, `e9765f05`, `cd9fb62e` after `55154081`). Line numbers below were read at `cd9fb62e`. Before editing, run `git -C fulgur log --oneline 55154081..HEAD`, then find each site again with `rg -n` on the function names in §3. Those names did not change in F30c-n.


*Coordinator note (2026-11-02), step 0 is done:* a `css-spec-researcher` pass checked the quotes marked \* in §2 and confirmed the default reading. The decision is recorded as **ADR-0010** (`docs/decisions/0010-orphans-widows-follow-the-spec-not-chrome.md`): Rule 3 is not dropped while the Class A break before the paragraph is allowed, and the §3.3 "fewer lines … kept together" sentence is normative. Chrome's split fails Blink's and WebKit's own `printing/page-break-orphans-and-widows` test. Implement the **default**, not Variant B; Variant B stays below only as a record. Two corrections to §2:

- css-break-3 drops Rule 3 (orphans / widows) **first** and Rules 1, 2 and 4 (`avoid`) after it. CSS 2.1 uses the reverse order; the CSSWG resolved for the css-break-3 order in September 2015.
- So there is one more shape where the spec allows the split: the break before the paragraph is forbidden (`break-after: avoid` on the heading before it, `break-before: avoid`, or an ancestor's `break-inside: avoid`). Add a test for it (`a_short_paragraph_after_an_avoid_break_keeps_its_orphans`: a heading with `break-after: avoid`, then a 5-line `<p>` with `orphans: 3; widows: 3` and room for the heading plus 3 lines). Measure Chrome, and record what the engine does today. If the result is not the css-break-3 one (keep the orphans), fix it here when the fix is small, otherwise file it.

Test 4 names ADR-0010 in its comment.

## 1. Problem

Shared setup for all probes: A4, 15 mm margins, `14px/21px` Inter, `p{margin:0}`, lines are `w0000<br>…`, 48 lines fit a page, and a spacer of `1008-21k` px leaves room for k lines. Probes are in `scratchpad/f30cov/`. "Ours" means the F30c-o binary.

1. **Short paragraphs** (`grid/`, 2,940 cases, body level). Ours matches Chrome in 2,746. All 194 misses have the same shape: ours moves the paragraph whole, and Chrome splits it. Chrome's page-1 line count follows one rule in every case:
   - n, if the paragraph fits (room ≥ n);
   - 0, if room < orphans;
   - otherwise min(room, max(orphans, n − widows)).

   Examples:

   - `g_n5_o3_w3_k3`: Chrome 3 / 2, ours 0 / 5.
   - `g_n4_o1_w5_k2`: Chrome 1 / 3 (widows larger than the paragraph), ours 0 / 4.
   - `g_n7_o3_w5_k6`: Chrome 3 / 4.
   - `li` with 4 lines, `orphans:3; widows:3`, room 3 (`int/P_li2_k3`): Chrome 3 / 1.

   Cause: `has_allowed_break` requires n ≥ orphans + widows.

2. **The same rule for long paragraphs** (`g2/`, measured on Chrome's PDFs):
   - `body_n60_o30_w47_k40` and `_w50_`: Chrome 30 / 30, ours 0 / 30 / 30. The 60-line paragraph has no break that honours 30 + 47, so it moves whole and still has to break later.
   - `nest|li_n60_o30_w47_k40`: Chrome 30 / 30, ours 40 / 20.
   - `nest|li|td_n100_o30_w30_k0`: Chrome 0 / 48 / 30 / 22, ours 0 / 48 / 48 / 4. `pad_` gives Chrome 0 / 47 / 30 / 23 and ours 0 / 47 / 48 / 5.

   Cause: the nested splitter `plan_line_breaks` does not have the keep-the-orphans step that `fragment_inline_root` has.
3. **`orphans` larger than a page** (nested, `td`, `li` and padded paragraphs):
   - `ov/nest_o50_k3`: Chrome 0 / 48 / 48 / 4, ours 3 / 48 / ….
   - `ov/td_o50_k3`: Chrome leaves page 1 empty.
   - `tbl/T6_wideorph_k0` (`td{orphans:60}`, room of about 1 px): ours prints `w0000` below the content box.

   Cause: every cut gate drops the move when `leading > fresh` (`first_break_h = 0`), so the paragraph starts where it is. `plan_line_breaks` then puts at least one line on a 1 px strip.

No committed fixture shows any of these. Fixtures 01-39 and the corpus declare no values that reach these paths (§5).

## 2. Spec (cited by the planner; css-spec-researcher not run; * = wording from memory, unverified)

- **css-break-3 §3.3\* / CSS 2.1 §13.3.2\***: the minimum number of line boxes before a break (`orphans`) and after it (`widows`). The value is `<integer [1,∞]>` and the initial value is 2. CSS 2.1 adds\*: "If a block contains fewer lines than the value of 'widows' or 'orphans', the rule simply becomes that all lines in the block must be kept together."
- **css-break-3 §4.1**: the break before a box (Class A) is a break point.
- **css-break-3 §4.4\* / CSS 2.1 §13.3.3\***: a Class B break needs ≥ orphans lines before it and ≥ widows after it. These rules are relaxed **only** when the allowed breaks cannot keep the content from overflowing the fragmentainer. Neither spec says whether orphans or widows is given up first once relaxation is allowed. That choice is the UA's.

**Decision (default):**

- **A paragraph that fits a page** always has a Class A break before it (or it starts at the top of a page, where it fits), and that break avoids overflow. So §4.4 does not relax the rule, and the CSS 2.1 sentence keeps the paragraph together. Our "move whole" is the conforming result and Chrome's 3 / 2 and 1 / 3 are the deviation. AGENTS.md puts the spec first ("Chrome is the reference, not the truth"), so the 194 grid cases stay as they are and are pinned with the spec citation.
- **A paragraph taller than a page, and every strip after the first**, cannot avoid relaxing wherever it starts. Here the spec leaves the choice to the UA, so we take Chrome's: start in place when the orphans fit, keep the orphans, then fall back to the initial 2 / 2. Items 2 and 3 of §1 are this case, and so is the tall part of item 1.

Step 0 checks the three \* quotes. If css-break-3 does not keep a short block together, or allows relaxing to avoid a Class A break, switch to Variant B (§4). The same reasoning applies to F30c-w (Chrome 2 / 1 with the initial values). It gets a one-line note in its finding; its code is not touched here.

## 3. Where it belongs (fork `crates/fulgur/src/pagination_layout.rs`, lines at `cd9fb62e`)

This is fulgur's own pagination code, so the change goes in the fulgur fork. Stylo, Blitz and Parley only supply the values and the line metrics. htmltopdf bumps `Cargo.lock` and adds the commit to `docs/upstream.md`. No upstream PR (standing decision).

- **Length rule:** `has_allowed_break(n, rules)` 8464 is the one shared predicate. It is called by the body-level `starts_in_place` 8455 (call site 3583, inside `start_here` 3574-3595) and by `LineSplit::has_inner_break` 6984. `has_inner_break` is used by `LineSplit::starts_in_place` 6992, which `block_walk_line_split` 7951-7953 calls, and by `may_start_in_place` 7452. `leading_block_size` uses it through `may_start_in_place` at 4602.
- **Break choice:** `widow_safe_break` 8409, `line_break_at` 8418 and `keep_orphans_break` 8439 are used only by `fragment_inline_root` 8518 (decision at 8552-8567). `plan_line_breaks` 6874 has its own greedy pull-back (6903-6908) and is called by `fragment_nested_inline_root` 8145.
- **Start gate for the nested paths:** `LineSplit::leading` 6973 is used in four places:
  - the body-level padded branch 3509-3516 (`leading <= fresh`);
  - `leading_block_size` 4560, 4591 and 4610 (`split.leading().min(..)`);
  - `fragment_block_subtree` `first_break_h` 5832-5839 (`leading <= fresh`, otherwise 0), with the cut at 5886-5888;
  - `row_mate_can_split` 8093.

## 4. Fix

```rust
/// Chrome's choice at line `i` (the first that does not fit), for author rules: author rules → keep the orphans → initial 2 / 2.
/// `None`: no break honours even the initial rules.
fn choose_line_break(start: usize, i: usize, total: usize, rules: LineRules) -> Option<usize>;
fn has_allowed_break(n: usize, taller_than_a_page: bool, rules: LineRules) -> bool;
impl LineSplit { fn leading_within(&self, fresh: f32) -> f32; }
fn starts_in_place(lines: &[(f32, f32)], top: f32, bottom: f32, fresh: f32, rules: LineRules) -> bool;
```

1. **One break chooser for both splitters.** Move the chain at 8552-8560 into `choose_line_break`.
   - `fragment_inline_root` calls it (a refactor; no behaviour change).
   - `plan_line_breaks` replaces 6903-6908 with `if end < n { end = choose_line_break(start, end, n, rules).unwrap_or(end) }`. We must keep the `unwrap_or(end)` because the nested splitter never overflows today and must not start to.
   - With `INITIAL` the result equals today's pull-back. Proof: when fewer than 2 lines precede the break, the old `pulled >= start + 2` test fails as well. A fork test checks this exhaustively.
   - What changes: nested, `li`, `td` and padded paragraphs get keep-the-orphans (48 / 30 / 22).
2. **Length rule, tall arm.** `has_allowed_break` returns `orphans >= 1 && (n >= orphans + widows || taller_than_a_page && rules != INITIAL && n > orphans)`.
   - The body-level `starts_in_place` gets a `fresh` parameter and passes `span > fresh + 1.0`. This is the slack of `nested_split_needed`.
   - `has_inner_break` passes `false`: its callers handle only paragraphs that fit a page, and `nested_split_needed` always splits taller ones.
   - We must keep `n > orphans`: `starts_in_place` indexes `lines[orphans - 1]`.
   - The initial-value guard keeps the bytes of every document without author values. A 3-line paragraph taller than a page is F183g, not this finding.
   - Why `fragment_inline_root` always finds a break on the first strip (update the comment at 8447-8454 and the "fewer than four lines" comment at 3567-3568): i ≥ orphans and n − i ≥ 1. Then `line_break_at` gives i or n − widows, and otherwise `keep_orphans_break` gives orphans, because 0 < n − widows < orphans when n > widows.
   - Leave the n ≤ widows case of `keep_orphans_break` alone: in the default it only occurs past a page, which is F30c-o2.
3. **`orphans` larger than a page.** `leading_within(fresh)` returns `fresh` when `rules != INITIAL && leading() > fresh`, and `leading()` otherwise.
   - Use it at 3509, 4560, 4591, 4610 and 5832. The gates compare with `<= fresh`, so the paragraph now moves whenever something precedes it on the strip, which is Chrome's "0 if room < orphans".
   - On the next page, item 1's chooser relaxes to 2 / 2, giving 48 / 48 / 4.
   - At a strip top nothing moves, because the existing guards hold: `child_page_y > page_start_y + band` at 5886 and `cursor_y > 0.0` at 3510.
   - `row_mate_can_split` (8093) keeps the raw `leading()`. Row machinery is F30c-t's.
4. **Comments:** update the docs of `has_allowed_break`, `plan_line_breaks`, `fragment_inline_root` and `LineSplit::leading`. Replace the "F30c-o3 changes it here" notes.

**Variant B (Chrome model, only if step 0 overturns §2):**

- In `has_allowed_break`, the author arm becomes `n > orphans` (no tall condition).
- `keep_orphans_break` adds `start == 0 && total <= widows → Some(orphans)` (g_n4_o1_w5 → 1 / 3). Do not apply it to later fragments: Chrome keeps the natural break there.
- Add fixture 41 (below). The grid then expects 2,940 of 2,940 at Chrome.

## 5. Acceptance

**Ours:** new tests in `crates/htmltopdf/tests/orphans_widows.rs`, using that file's helpers (`per_page`, `assert_inside` at 799.37 pt). Every number comes from Chrome m141.

1. `a_paragraph_taller_than_a_page_starts_where_its_orphans_fit_when_no_break_keeps_the_widows`: `body_n60_o30_w47_k40` and `_w50_` (spacer 168 px) print 30 / 30 on 2 pages. Red (0 / 30 / 30).
2. `a_long_nested_paragraph_keeps_its_orphans`:
   - `nest|li|td_n100_o30_w30_k0`: 0 / 48 / 30 / 22;
   - `pad_`: 0 / 47 / 30 / 23;
   - `nest|li_n60_o30_w47_k40`: 30 / 30.

   Red.
3. `orphans_larger_than_a_page_move_the_paragraph_and_keep_every_line_inside`:
   - `ov/{nest,td}_o{49,50,52}_k{0,3,10}` plus the step-1 `li` and `pad` twins;
   - page 1 has 0 words, the pages read 48 / 48 / 4, and no word ends below 799.37 pt;
   - `T6_wideorph_k0`: the same, and `w0000` is not on page 1.

   Red. `o44`-`o48` are guards and green.
4. `a_short_paragraph_that_fits_a_page_keeps_its_lines_together` pins the deliberate deviation. The comment cites §2 and gives Chrome's numbers:
   - `g_n5_o3_w3_k3`: 0 / 5;
   - `g_n4_o1_w5_k2`: 0 / 4;
   - `g_n7_o3_w5_k6`: 0 / 7;
   - `P_li2_k3`: the first `li` on page 2.

   Green. In Variant B these flip to Chrome's numbers and turn red first.

**Fork:**

- `choose_line_break` equals the old chain for every start < i < total ≤ 60.
- `plan_line_breaks(INITIAL)` equals a test-local copy of the old function for n ≤ 120 and rooms of 1-60 lines.
- `plan_line_breaks(rules(30,30), 100 lines, 48-line rooms)` gives `[0..48, 48..78, 78..100]`.
- `has_allowed_break` truth table: (60, tall, 30/47) is true; (60, not tall) is false; (3, tall, INITIAL) is false.
- `starts_in_place(60 lines, room 840, fresh 1009, 30/47)` is true.
- `leading_within`: `rules(50,2)` gives `fresh`; `INITIAL` with a leading larger than a page is unchanged.
- `leading_block_size` of a `<section>` holding an `orphans:50` paragraph gives `fresh`.
- A first-in-body paragraph with `orphans:60` on page 0 with a body offset stays on page 0.
- Existing tests keep their assertions.
- One `render_smoke` case.

**Fixture: none, in the default.** In the default, pixels move only for values of about a third of a page or more (≥ 15 at 14 px / 21 px), or for orphans larger than a page. No invoice declares those; fixture 38 uses 3 / 3. The tests above pin every shape against Chrome, and the byte gate shows no committed fixture moves. Variant B needs **41-terms-short-clauses**:

- a Czech invoice like 38 with `@media print{.terms p{orphans:3;widows:3}}`;
- a 5-line clause starting with about 3.5 lines of room (clear of the F30f-c 1 px window);
- Chrome 3 / 2, ours 0 / 5;
- `fixtures.rs` asserts the first word of page 2;
- threshold = worst page + 1 point; both pages should drop.

**Byte gates** (1 and 4 rayon threads, against the F30c-n binary):

- Fixtures 01-39 (40 if it has landed) and the 7 corpus documents must be byte-identical. 24, 25, 30 and 31 declare 1 / 1 in multicol, which does not read the values; 38's 3 / 3 cannot reach the tall arm or keep-the-orphans at text-xs. A moved fixture or a page-count change stops the work.
- The F30c sweep (4616 cases), the F30c-n sweep (756) and `f183v5` (6209) must be identical; their `_o4_` cell cases stay at Chrome's 0 | 150.
- `grid/` (2,940) must be identical (it has no tall paragraphs).
- `g2/` (1,320): changes are allowed only in the shapes of tests 1-2, and each must be at Chrome's numbers. Known misses that stay: `widows` ≥ 47 at later strips (F30c-o2).
- `ov/`: `o49`-`o52` move to Chrome's numbers; `o44`-`o48` stay identical.
- `fulgur-vrt` must be identical; `bench.py --check` must pass.

## 6. Steps

0. **css-spec-researcher** (≤ 1 h): quote §3.3, §4.4 and CSS 2.1 §13.3.2-3. Choose the default or Variant B and record it in §8.
1. **Chrome** (≤ ½ day): read the `ov/` PDFs with `pdftotext -bbox`; render `li_` and `pad_` twins of `ov/` with `scripts/reference/render.mjs`; re-run `grid/`, `g2/` and `ov/` on the F30c-n binary as the new baseline (the validator's "new" was the F30c-o binary).
2. **Ours, red:** tests 1-4. Commit with the red list.
3. **Fork:** extract `choose_line_break` and wire it into `plan_line_breaks`, with the equivalence tests. Run the fork lib tests; fixtures must be byte-identical.
4. **Fork:** the tall arm of `has_allowed_break` and the `fresh` parameter, with tests.
5. **Fork:** `leading_within` at the five sites, with tests. Run clippy `-D warnings` and fmt.
6. **Ours, green:** bump `Cargo.lock`; run `cargo test --release --locked --no-fail-fast`, the byte gates, the sweeps and the bench. On any gate failure, write a postmortem and stop.
7. **Finish:**
   - run `compare.py` and look at 36 and 38;
   - run `vrt-validator`;
   - findings: F30c-o3 done; F30c-o2 narrowed to widows larger than a page, `calc()` and multicol; a note under F30c-w;
   - update `docs/upstream.md` and §8.

## 7. Risks and non-goals

**Risks:**

- **The spec call itself.** If step 0 overturns it, switch to Variant B, which is small and specified in §4.
- **The chooser in `plan_line_breaks`** changes nested output for author values on later strips. The sweeps check each change against Chrome.
- **Padded short paragraphs** still start in place (pre-existing, F30c-w). They now break where Chrome does: a 5-line, 3 / 3 paragraph with room for 4 goes 3 / 2 instead of 4 / 1.
- **The `leading_within` clamp** relies on the strip-top guards. A fork test covers page 0 with a body offset.
- **Line numbers** may move with F30c-n review fixes. Re-locate sites by name.

**Non-goals:**

- F30c-o4 (reader items).
- F30c-o2: widows larger than a page's lines (`body_n60_o3_w50_k3`: Chrome 3 / 48 / 9, ours 3 / 7 / 48 / 2), `calc()` and multicol.
- F30c-w's code.
- F30c-t, F30c-g and `row_mate_can_split`.
- F30f-c (the 1 px capacity window).

## 8. Evidence

To be filled after implementation.
