# F215: `break-before` / `break-after: avoid` (keep-with-next)

Date: 2026-11-02. Finding: F215 (medium). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, on top of F30c-o3 (PR #85, fork head `c1b7adba`). Line numbers below are at `c1b7adba`. Re-locate each site with `rg -n` on the function name before editing.

## 1. Problem

F87c parses `avoid`, `avoid-page` and `avoid-column` for `break-before` / `break-after`, but `parse_break_forced` (`column_css.rs:539`) turns them into `BreakBefore::Auto` / `BreakAfter::Auto`. The enums in `draw_primitives.rs:269-278` only have `Auto | Page`, so the fragmenter never sees an avoided break. The result: a heading with `break-after: avoid` stays alone at the bottom of a page whenever its next sibling moves on.

No committed fixture shows this, because none declares the values. The shape is pinned in `crates/htmltopdf/tests/orphans_widows.rs::a_short_paragraph_after_an_avoid_break_keeps_its_orphans`, and the probes are in `out/scratch/f30co3/avoid` (F30c-o3 plan §8). With `h2{break-after:avoid}` followed by a 5-line `p{orphans:3;widows:3}`, ours prints HEAD at the bottom of page 1 and the paragraph on page 2 (`AFTER` at 123.1 pt). Chrome moves the heading with the paragraph (`AFTER` at 139.1 pt).

## 2. Spec

Checked = quoted by the 2026-11-02 `css-spec-researcher` pass (ADR-0010). Everything else is from the planner's memory and must be re-checked in step 0.

- **css-break-3 §3.1** (unverified wording): `break-before` / `break-after` take `auto | avoid | avoid-page | page | left | right | recto | verso | avoid-column | column` (plus region values). `avoid` avoids any break at that point, `avoid-page` avoids a page break, `avoid-column` avoids a column break. So `avoid-column` does not constrain page breaks outside a multicol. §3.1 also propagates the values of a first / last child to the parent's edge (used here only to put that out of scope, F215c).
- **css-break-3 §3.4** (unverified): legacy `page-break-before|after: avoid` aliases `break-*: avoid`. F87c already reads it that way.
- **css-break-3 §4.1**: the point between adjacent in-flow siblings in block flow is a Class A break point.
- **css-break-3 §4.4** (checked):
  - Rule 1: a Class A break is allowed only if the `break-after` / `break-before` values of the boxes meeting there allow it. Any `avoid*` value that applies to the fragmentation type forbids it; a forced value wins over `avoid` (unverified, same as F87c's reading).
  - Rule 3: orphans / widows.
  - Rule 4: an ancestor's `break-inside: avoid`.
  - Relaxation: rules are dropped only if the allowed breaks "doesn't provide enough break points to keep content from overflowing the fragmentainer". Rule 3 is dropped first, then Rules 1, 2 and 4 (CSSWG resolution, September 2015; ADR-0010).
- **CSS 2.1 §13.3.3 Rule A** (unverified): "…properties of all the elements generating boxes that meet at this margin allow it". **Appendix D** (informative) has `h1…h6 { page-break-after: avoid }`. Chrome's UA sheet does not (measured, F30c pre-advance plan §8).
- **css-break-3 §4.5**: guidelines for choosing among the allowed breaks only.

**Reading (default).** The relaxation happens only when no allowed break keeps the content inside the page. A heading that has content above it on the page always has an allowed Class A break before it. If heading + next fits a fresh page, the conforming break is before the heading: the pair moves together, nothing is relaxed, and a short paragraph stays whole (ADR-0010 §3.3). ADR-0010 item 2c ("a paragraph whose preceding break is forbidden…") is read together with its lead-in sentence. It applies only when the chain cannot be moved: the chain is taller than a page. Its own example, an ancestor with `break-inside: avoid`, already behaves this way in both ours and Chrome: the ancestor moves whole.

**Variant B** (only if step 0 overturns the reading): a paragraph right after an avoided break starts in place with the relaxed `choose_line_break` chain, which gives Chrome's 3 / 2.

## 3. Chrome behaviour (m141, A4, 15 mm, Inter 14px/21px)

**Measured** (F30c-o3 §8 `avoid`, 28 probes; F30c pre-advance §8; F87c):

| Shape | Chrome |
|---|---|
| `h2{break-after:avoid}` + 5-line p, o3/w3, room for heading + 0-2 lines | 0 / 5, heading on page 2, `AFTER` 139.1 pt |
| same, room for heading + 3-4 lines | 3 / 2, `AFTER` 76.1 pt (ADR-0010 split, not keep-specific) |
| `p{break-before:avoid}`, and the `<section>` twin | same as the two rows above |
| ancestor `break-inside:avoid` instead | the ancestor moves whole, 0 / 5, 123.3 pt (ours matches) |
| `h2` without author avoid + 30-line p | heading alone at the page bottom with room for 0-1 lines; 2 / 28 and 3 / 27 with room for 2-3 |
| `.a{break-before:page}.b{break-before:avoid-page}` | the later value wins, no break |

**Inferred, to measure in step 1.** Write a throwaway Playwright script in the session scratch, modelled on `scripts/reference/render.mjs` (print media, `@page` respected), over `out/scratch/f215/*.html`, and read the results with `pdftotext -bbox`. Keep every room at least 9 px from a line boundary, away from the F30f-c 1 px window.

- **M1:** `h2` avoid + 30-line p with the initial 2 / 2, room for heading + 0 / 1 / 2 / 3 lines. Expected: the heading moves for 0-1, 2 / 28 and 3 / 27 otherwise.
- **M2:** chain h1 avoid + h2 avoid + p, room for h1 only / h1+h2 / +1 line / +2 lines.
- **M3:** a pair taller than a page: `h2` avoid + a 1000 px `div`, and `h2` avoid + a 60-line p.
- **M3b:** a chain taller than a page: 60 three-line `p{break-after:avoid}` (no allowed break with 2 / 2), and 60 one-line ones. Where does Chrome break: inside a paragraph (Rule 3 first) or between two? Blink's break-appeal order (violating `break-avoid` ranks below violating orphans / widows) is the planner's memory, unverified.
- **M4:** `h2` avoid + a table with `thead` and 10 rows, room for heading / heading + header / heading + header + 1 row; also `table{break-before:avoid}`.
- **M5:** `avoid-page` and `page-break-after:avoid` behave as `avoid`; `avoid-column` gives no page keep.
- **M6:** `h3` avoid at a column end inside `columns:2`, and `h2` avoid before a multicol container.
- **M7:** `h2` avoid + a `break-inside:avoid` card with 6 lines and room for 3.
- **M8:** `h2` avoid + next `break-before:page` (forced wins); `h2{break-before:page;break-after:avoid}`.
- **M9:** fixture 41 and its twin without the print sheet.
- **M10 / M11 (for filing only):** propagation `h2` + `<div><p>`, and `<section>…<h3 avoid></section><p>`.
- **M12 (for filing only):** a breakable head: `p{break-after:avoid}` with 6 lines + a table at the page bottom. Does Chrome pull the last lines over?

## 4. Where it belongs, and the design

This is pagination policy, so it lives in the fork's own fragmenter. Stylo has no longhands for these values in the servo build, which is why the fork's `column_css` reader exists. Nothing goes to Blitz, Taffy or Parley, and no upstream PR or issue is opened (standing decision). htmltopdf only bumps `Cargo.lock`, adds tests and fixture 41, and updates `docs/upstream.md`.

**Approach: lookahead, not rollback.** The walks are greedy and single-pass, and they already predict "what has to fit" (`leading_block_size`, `LineSplit::leading_within`, `first_break_h`). Keep-with-next extends that prediction across an avoided break.

- Rejected: re-running the pass with learned breaks. It is O(pages × passes) on the 200-page documents, it may not converge, and the walk state (rows, origins, bands) cannot be resumed.
- Invariant (we must hold it, otherwise the heading moves and the next box moves again on its own): the extent of the next box comes from **the same predicates the walk uses to move that box**.

**Signatures** (no other code here):

```rust
// draw_primitives.rs
pub enum BreakBefore { Auto, Page, Avoid, AvoidPage, AvoidColumn }   // same for BreakAfter
impl BreakBefore { pub(crate) fn avoids_page_break(self) -> bool }   // Avoid | AvoidPage
// column_css.rs
fn parse_break_value<'i>(input: &mut Parser<'i, '_>, legacy: bool) -> Result<BreakValue, ParseError<'i, ()>>; // replaces parse_break_forced
pub(crate) fn has_avoid_page_break(table: &ColumnStyleTable) -> bool;
// pagination_layout.rs
fn avoid_page_break_between(table: &ColumnStyleTable, names: Option<&UsedPageNameTable>, prev: usize, next: usize) -> bool;
fn body_paragraph_may_start(doc: &BaseDocument, child: &blitz_dom::Node, avoid_inside: bool, is_float: bool, body_block_flow: bool, lines: &[(f32, f32)], fresh: f32) -> bool;
fn unbreakable_extent(doc: &BaseDocument, parent: &blitz_dom::Node, node_id: usize, styles: Option<&ColumnStyleTable>, fresh: f32) -> Option<f32>;
fn first_fragment_extent(doc: &BaseDocument, parent: &blitz_dom::Node, node_id: usize, env: KeepEnv<'_>, fresh: f32) -> f32;
fn keep_with_next_extent(doc: &BaseDocument, parent: &blitz_dom::Node, siblings: &[usize], index: usize, env: KeepEnv<'_>, fresh: f32) -> Option<KeepExtent>;
struct KeepExtent { size: f32, relax_rule3: Option<usize> }
```

1. **Values.**
   - `avoid` and legacy `avoid` map to `Avoid`, `avoid-page` to `AvoidPage`, `avoid-column` to `AvoidColumn`. `column` stays `Auto` (there are no column breaks in the table).
   - Every forced-break test matches `Page` only (`has_forced_page_break`, `subtree_facts.rs:135`, `forced_break_offsets`, the walks), so forced breaks do not change. No match on these enums is exhaustive (checked).
   - `has_avoid_page_break` gates all of the work below. We must gate it, because a document with no avoid value has to take today's path byte for byte.
2. **`avoid_page_break_between`.**
   - False when either side is `Page`, or when the used page names differ: a forced break wins (Rule 1, §4.3).
   - Otherwise true if `prev.break_after` or `next.break_before` `avoids_page_break()`.
   - Direct in-flow siblings only (propagation is F215c).
3. **`unbreakable_extent`.** `Some(height)` when the walk places the box whole on a fresh page:
   - an inline root without an allowed break (`!has_allowed_break(n, tall, rules)`);
   - a leaf or replaced box;
   - a `break-inside: avoid` box that holds (`avoid_inside_holds` on `avoid_measure`);
   - a box whose `leading_block_size` is at least its height − 0.5.

   `None` for anything with an allowed break inside it: a long paragraph, a table of two or more rows, a wrapper.
4. **`first_fragment_extent(next)`.** What the walk will need below the top of `next` so that next starts where it is:
   - a body-level paragraph that passes `body_paragraph_may_start` (the room-independent half of `start_here`, lines 3578-3585, extracted so the walk and the keep share it): `lines[orphans-1].1 - lines[0].0`, the same operands `starts_in_place` uses;
   - a nested paragraph that passes `may_start_in_place`: `LineSplit::leading_within(fresh)`;
   - another paragraph: its whole height;
   - a held avoid box: `avoid_piece_size`;
   - multicol: `mc.top_inset() + mc.first_unit()`;
   - anything else: `leading_block_size(doc, next, styles, 0.0, 0, fresh)`, which is what the F28 cut at 3841 / 5862 uses.
5. **`keep_with_next_extent(head)`.**
   - The head must be unbreakable (step 3).
   - The next in-flow sibling skips whitespace and zero-height boxes. A float, a forced break, a side-by-side sibling (`top(next) < bottom(head) − 0.5`) or the end of the parent ends the chain.
   - While `avoid_page_break_between(cur, next)` holds and `next` is unbreakable, the chain extends. The extent is `top(last) − top(head) + first_fragment_extent(last)`.
   - If the extent would exceed `fresh`, the chain is cut at the last member that fits, so Rules 1 / 2 give way at the latest link. A cut that leaves only the head gives `None` (today's behaviour).
   - Step 7, Rule 3 first: if the member that overflows is a multi-line paragraph without an allowed break, the extent ends at its first line instead (`relax_rule3 = Some(id)`).
6. **Walks.**
   - Body level (`fragment_pagination_root`), plain whole path at 3916: compare `cursor_y + max(child_h, keep.size)`.
   - Body level, short-paragraph move at 3593 (only when `!start_here`): compare `max(para_total_h, keep.size)`.
   - Nested (`fragment_block_subtree`): `first_break_h` (5832-5890), the `keeps_whole` and `child_h + leaf_tail` arms, with `.max(keep.size)`. Only when `block_parent && first_in_row && !parent_is_table && !allow_same_row_rebase`. It uses `walk_children` with an index.
   - Content must precede the head (`cursor_y > 0.0` / `child_page_y > page_start_y + band`). That is already the guard at both sites.
7. **Rule 3 dropped (over-long chain only).**
   - The walk keeps a local `Option<usize>` copied from `relax_rule3`. That paragraph starts in place, and `choose_line_break(…, rule3_dropped: bool)` (8601) adds a last fallback `Some(i)` when `start < i < total`.
   - Thread the same flag through `fragment_inline_root` (8732) and `plan_line_breaks` (6882).
   - Nothing changes when the flag is false; the existing equivalence tests stay.

## 5. Acceptance (red first)

**Ours, new file `crates/htmltopdf/tests/keep_with_next.rs`.** It copies the helpers of `orphans_widows.rs`. Numbers marked M come from step 1.

1. `a_heading_with_break_after_avoid_moves_with_a_long_paragraph` (M1): room for heading + 0 / 1 line gives HEAD on page 2 and 0 / 30 (red). Room for + 2 / 3 lines gives 2 / 28 and 3 / 27 (green). A guard without the avoid leaves HEAD on page 1 (Chrome, measured).
2. `break_before_avoid_on_the_paragraph_keeps_the_heading`: the same numbers. Red.
3. `avoid_page_and_the_legacy_alias_keep_avoid_column_does_not` (M5): red for the first two, a green guard for `avoid-column`.
4. `a_chain_of_two_headings_moves_with_the_paragraph` (M2): red.
5. The pinned test, flipped and renamed `a_short_paragraph_after_an_avoid_break_moves_with_its_heading`. The doc comment keeps the old name so it can be found with grep.
   - For k = 1..5: `[0, 5]`, HEAD on page 2, `AFTER` at 139.1 pt. Red.
   - The comment gives Chrome's 3 / 2 (76.1 pt) for k = 4, 5 and names ADR-0010 and §2 of this plan.
   - The ancestor part stays as it is.
6. `a_heading_before_a_kept_together_card_moves_with_it` (M7): red.
7. `a_heading_before_a_table_moves_with_its_header` (M4): room for the heading only gives heading and `thead` on page 2. Red. Room for heading + header but not row 1 pins whatever the table's own rule does (the F12 header orphan), with that named.
8. `a_forced_break_wins_over_avoid` (M8): green guards.
9. `a_pair_taller_than_a_page_is_not_kept` (M3): the 1000 px box gives today's output (green guard).
10. `the_keep_holds_inside_a_section`: the nested twin of test 1. Red.
11. `grid_items_and_columns_ignore_the_keep`: the PDF bytes equal the twin without the avoid (F30d-b, M6). Green pin.
12. `a_chain_taller_than_a_page_breaks_inside_a_short_paragraph_first` (M3b, step 7): red. Also: every line printed once and nothing below 799.37 pt.

Also update the module doc of `orphans_widows.rs` (line 17).

**Fixture `fixtures/invoices/41-terms-heading-keep/`** (40 is reserved for F198j):

- **Shape.** Czech invoice in the style of 38. The print sheet is `@media print { h2, h3 { break-after: avoid; } }`; this keeps the party-grid `h2`s, which are at the page top, inert. Then nine item rows, "Celkem k úhradě", `<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>`, one `<p class="text-xs text-gray-600 leading-relaxed">` with clauses 1-40 (about 24 lines, initial 2 / 2), and "Převzal: …".
- **Sizing.** Size the items table so the room under the total is the heading block (28 px) plus about 1.5 terms lines (57 ± 3 px).
- **Classes.** Every class is in the compiled sheet (all are used by 38). Do not use `break-after-avoid`: it is not compiled, and Chrome ignores it too.
- **What the PNG must show.** Page 1 ends with the total ("Kč") and page 2 starts with "Obchodní podmínky". Today ours shows the heading alone at the bottom of page 1.
- **README.** Same layout as 38: what it exercises, the twin without the sheet (M9), and the "before" behaviour.
- **Test.** `fixtures.rs::terms_heading_keep_invoice_moves_the_heading_with_its_paragraph` asserts:
  - 2 pages;
  - "Obchodní" is on page 2 and is the first word there, as in Chrome;
  - the last word of page 1 equals Chrome's;
  - "Obchodní" is within 3 pt of Chrome's y;
  - "Převzal:" is on Chrome's page.
- **Threshold.** `compare.py`: worst page after the fix + 1 point. The comment gives the before/after numbers; expect about 5-6 % after (text drift, as in 38 and 39).

**Fork lib tests.**

- Rewrite `avoid_and_column_values_map_to_no_forced_break` (now `Avoid*`, still not forced).
- A truth table each for `has_avoid_page_break` and `avoid_page_break_between` (forced wins, page names).
- Cases for `unbreakable_extent` and `first_fragment_extent`: `h2`, short p, long p, avoid box, table, multicol.
- `keep_with_next_extent`: pair, chain, prefix cut, end of parent, float.
- `choose_line_break(rule3_dropped = false)` equals the old function for every `start < i < total ≤ 60`.
- One `render_smoke` case.

**Byte gates** (1 and 4 threads, against the F30c-o3 binary):

- Must be byte-identical: fixtures 01-39 (40 if it has landed), the corpus documents except `slow-breaks`, the F30c / F30c-n / `f183v5` / `grid` / `g2` sweeps, and `fulgur-vrt`.
- `slow-breaks` (report-only) records its pages before and after, with every `<p>` printed once.
- The `avoid` sweep (28) may change only towards Chrome or towards §2's reading, each change explained.

**Expected pixel-diff change:** 0 on 01-39, and no page count moves. Fixture 41 falls from its before number to text drift.

## 6. Steps (each ≤ 1 day, each testable alone)

0. **`css-spec-researcher`** (≤ 1 h): the §2 quotes and the reading of ADR-0010 2c. Record the default or Variant B in §9.
1. **Chrome:** probes M1-M12; draft fixture 41 and run `node scripts/reference/render.mjs 41-terms-heading-keep`; put the numbers into the tests.
2. **Ours, red:** `keep_with_next.rs`, the flipped test, fixture 41 and its `fixtures.rs` test. Commit with the red list.
3. **Fork:** the values and `has_avoid_page_break`, with unit tests. Byte gate: everything identical (no consumer yet).
4. **Fork:** `avoid_page_break_between`, `body_paragraph_may_start` (extracted; byte gate), `unbreakable_extent`, `first_fragment_extent`, `keep_with_next_extent`, with unit tests. No walk change.
5. **Fork:** the body-level walk sites. Tests 1-9 and 11 go green. Byte gates.
6. **Fork:** the nested walk site. Test 10. Byte gates.
7. **Fork:** Rule 3 dropped in an over-long chain. Test 12. If it breaks a gate, write a postmortem and park it as F215a, keeping its red test with `#[ignore = "F215a"]`.
8. **Ours, green:**
   - bump `Cargo.lock`;
   - run `cargo test --release --locked --no-fail-fast`, the gates and `bench.py --check`, plus a stress probe: 4000 one-line `p{break-after:avoid}` must be ≤ 1.3× its twin without the declaration (if not, add a sliding window to the chain walk);
   - run `compare.py` and look at the 41 PNGs;
   - run `vrt-validator`;
   - findings: F215 done, file F215b-f; update `docs/upstream.md`, the ADR-0010 clarification note and §9.

## 7. Risks

- **The spec reading** (§2). Step 0 decides; Variant B is small.
- **The predicted extent disagreeing with the walk** (the heading moves, then the next box moves again). Mitigation: shared predicates (step 4), plus a check in step 8 that the head's page equals the page of the next box's first fragment for every keep in the probes.
- **Page counts on 01-39.** None declares the values. A moved fixture stops the work.
- **Performance** (`docs/benchmarks.md`: fixtures ≤ 100 ms hard, `b200-*` ≤ 1500 ms, `b200-items` 3000 ms override). The gate makes it zero on documents without the values. The chain walk is bounded by `fresh`.
- **Multicol.** `avoid-column` and avoid inside columns stay unread (F30d-b). Body-level `h2` + multicol container uses the container's first unit (M6).
- **Tables.** The keep follows the table's own move rule, including today's header orphan (F12 step 5).
- **Determinism.** Only f32 arithmetic in walk order, no hash maps. Gated at 1 and 4 threads.
- **Line numbers** move with review fixes; re-locate by name.

## 8. Out of scope (file each one, pin each one)

- **F215b:** a breakable head (a long paragraph, a table, a wrapper) before an avoided break; pulling its last lines or rows over (M12).
- **F215c:** first / last-child propagation (§3.1; M10, M11).
- **F215d:** a relaxed avoid box (taller than a page) after an avoid heading. The F83b advance would separate them again. Pinned as today.
- **F215e:** avoid on table rows, flex / grid items, and inside multicol, including `avoid-column`.
- **F215f:** `break-inside: avoid-column` is read as `avoid` for pages (`column_css.rs:526`).
- Chrome's 3 / 2 short-paragraph split (ADR-0010), Variant B unless step 0 picks it, and the Tailwind `break-after-avoid` utility (needs an npm rebuild of the committed sheet).

## 9. Evidence

To be filled by the engineer: the step 0 result, the M numbers, the red list (done below, steps 1 and 2), the gate results, compare before/after, the bench numbers, and the fork commits.

### Step 1 and 2 result (2026-11-02, base `816fac2` = F30c-o3 PR head, binary of the base)

Chrome is **m153** (Playwright's bundled Chromium; the committed references say `Skia/PDF m153` too), not m141 as in section 3 and in the older tests. Probes and logs: the session scratchpad `f215/` (`lib.mjs`, `m1.mjs` ... `m14.mjs`, `*.log`; not committed). Every probe is the shape of `orphans_widows.rs` (A4, 15 mm, `14px/21px`, spacer for the room, `HEAD` = `h2` of 21 px, no margin). `j` = lines of room under the heading.

| M | Result (Chrome) | Plan said | Ours on the base |
|---|---|---|---|
| M1 | `h2` avoid + 30-line p: `j` 0 / 1 -> `HEAD` page 2 at 44.6 pt, 0 / 30, `AFTER` 532.8 pt; `j` 2 / 3 -> 2 / 28 (485.6), 3 / 27 (469.8). Without the avoid: `HEAD` page 1 (784.8 / 769.1 pt), `AFTER` 517.1 pt. `p{break-before:avoid}` identical | as expected | `HEAD` page 1, `AFTER` 516.8 pt (red for `j` 0 / 1) |
| M2 | chain `h1`+`h2` avoid: room 1, 2, 3 lines -> both on page 2 (`H1` 44.6, `H2` 60.3, `AFTER` 548.6), 4 lines -> 2 / 28 on page 1. `h1` avoid only, `h2` plain: stay | as expected | both on page 1 (red) |
| M3 | pair taller than a page, 1000 px `div` (one line, fixed height): **Chrome splits the box**. `j` 0: `HEAD` + `DIVX` on page 2 (44.6 / 60.3 pt), 12 px of the box on page 3, `AFTER` 52.8 pt. `j` 1: `DIVX` starts on page 1 (784.8 pt), `AFTER` page 2 at 777.3 pt. Same with `break-inside:avoid` on the box. A 1000 px `<img>`: `HEAD` moves to an empty page 2, the image to page 3, `AFTER` page 4 (4 pages). `h2` avoid + 60-line p: same numbers as M1 | "the 1000 px box gives today's output" - true for ours, but Chrome differs for a reason unrelated to the keep (ours does not split a fixed-height box) | `HEAD` p1, `DIVX` p2 whole (44.3), `AFTER` p3 (44.3): pinned |
| M3b | 60 three-line `p{break-after:avoid}` (2 / 2, no allowed break anywhere): Chrome **breaks inside a paragraph** (2 / 1, widows violated): 47 / 48 / 48 / 37 lines, `AFTER` 627.3 pt. 60 one-line: 48 / 12 (nothing to relax). 50 four-line (allowed 2 / 2 break inside): 46 / 48 / 48 / 48 / 10, i.e. Chrome breaks at the 2 / 2 split rather than between paragraphs (ours 48 per page: F215b). `HEAD` + 60 three-line chain with the head near the page bottom: the **head moves to page 2** and the chain breaks 2 / 1 there (47 lines + `HEAD`): step 7 and the "cut the chain" rule agree | Rule 3 first: confirmed | 48 / 48 / 48 / 36, `AFTER` 611.3 pt (red) |
| M4 | `h2` avoid (or `table{break-before:avoid}`) + header + 10 rows of 23 px: room 1 and 2 lines (header needs 23 px) -> all on page 2 (`THEAD` 61.1, `AFTER` 250.1). Room for heading + header only (63 px): Chrome also moves everything (without avoid: `HEAD` alone on page 1, table on page 2). Room for heading + header + one row (84 px): same as without the avoid | as expected | room 1, 2: `HEAD` page 1 (red). Room 3: **header orphan** on page 1, rows on page 2 without a repeated header (F12): pinned |
| M5 | `avoid-page`, `page-break-after:avoid`, `break-before:avoid-page`, `page-break-before:avoid` keep (`j` 0 -> page 2). `avoid-column`, `column` do not. Later of two values wins in both orders | as expected | no keep (red); `avoid-column` green |
| M6 | `h3` avoid at a column end in `columns:2`: Chrome moves it to column 2 (`AFTER` 233.6 vs 217.8 pt), `avoid-column` too. `h2` avoid before a 2-column container of 40 lines: `j` 0 / 1 -> `HEAD` moves (page 2, `AFTER` 375.3), `j` 2 / 3 -> 4 / 36, 6 / 34 on page 1. **Grid items:** Chrome honours the avoid on the `h2` of each item (`j` 0, 1, `HEAD` 44.6 on page 2) | columns "ignore" | columns and grid ignore it, bytes equal to the twin (F215e). Multicol `j` 0: `HEAD` page 1 (red); `j` 1: ours starts the container with 2 lines, unrelated |
| M7 | `h2` avoid + `break-inside:avoid` card of 6 lines: room 1..6 -> `HEAD` page 2 (44.6), `AFTER` 154.8 pt; room 7 -> all on page 1 | as expected | `HEAD` page 1 (red) |
| M8 | forced wins: `break-before:page` / `page-break-before:always` on the next box, `h2{break-after:page}` + `break-before:avoid`: `HEAD` page 1, `AFTER` 123.3 pt. `h2{break-before:page;break-after:avoid}`: page 2, 0 / 30 | as expected | identical (green) |
| M9 | fixture 41 (see below) and the twin without the print sheet: twin has `HEAD` alone at the bottom of page 1 (757.8 pt), "1." at the top of page 2 | as expected | twin identical to Chrome's; fixture 41 red |
| M10 | `h2` avoid + `<div><p>30 lines</p></div>`: the heading moves exactly as in M1 (`j` 0 / 1). `div p{break-before:avoid}` (first-child value) after a plain heading: moves too (propagation) | filing only | ours leaves the heading (F215c) |
| M11 | `<section><p>SECT</p><h3 avoid>HEAD</h3></section><p>30 lines</p>`: `SECT` stays on page 1, `HEAD` moves to page 2 with the paragraph for `j` 0 / 1 (propagation of the last child's avoid to the section's end; the break before the `h3` is allowed) | filing only | `HEAD` stays on page 1 (F215c) |
| M12 | `p.h{break-after:avoid}` 6 lines + table at the page bottom, room for 0, 1, 2 lines: Chrome breaks the **paragraph** 4 / 2 and prints the last two lines with the table on page 2 (also before a 30-line p, room 0 / 1); with room for the header and a row (63 px) nothing moves | filing only | paragraph whole on page 1, table on page 2 (F215b) |
| head at the page top | tall heading (`1008 - 21 j` px) first on page 1, 5-line p with `orphans: 3; widows: 3`, avoid: `j` 0 -> heading alone, 0 / 5; `j` 1 -> 1 / 4 (`AFTER` 107.6), 2 -> 2 / 3 (91.8), 3, 4 -> 3 / 2 (76.1), 5 -> 5 / 0. The same after a `break-before:page` heading on page 2. 30-line p (2 / 2): `j` 1 -> 1 / 29, 2 / 3 as M1. Without the avoid: `j` 1 -> 0 / 5, `j` 3, 4 -> 3 / 2 | relaxes Rule 3 (step 0) | 0 / 5 for every `j` (red) |

**Measurements that differ from the plan or that it does not cover**

1. **Chrome is m153**, not m141 (section 3, section 5 test list, the older test docs).
2. **Head at the page top (gap).** Chrome's relaxation there is real (1 / 4, 2 / 3, 3 / 2), and step 0 asks for a test, which is red now (`a_short_paragraph_after_a_heading_at_the_page_top_relaxes_rule_3`). But sections 4 and 6 contain no step that makes it green: the keep needs `cursor_y > 0` and step 7 is for an over-long chain. It needs a rule "a paragraph right after an avoided break whose head is first on the page starts in place with `rule3_dropped`" (the relaxed `choose_line_break` chain, i.e. a small Variant B limited to this case), to be planned before step 5. Until then this test stays red (or gets `#[ignore = "F215g"]` when the rest goes green).
3. **Chrome also moves the heading in M3's taller-than-a-page pairs** (it splits the fixed-height box, or for an image puts the heading alone on page 2). Section 4 item 5 ("cut the chain, today's behaviour") is the right outcome for ours, but the test cannot be a Chrome match; it pins ours (`a_pair_taller_than_a_page_is_not_kept`).
4. **Grid items honour the avoid in Chrome** (M6); section 8 F215e names flex / grid items, fine, but the pin test now has a grid shape that Chrome does not match.
5. M3b with `HEAD` in front of the chain: Chrome moves the head to page 2 and then breaks inside a paragraph; this is what step 5 (cut) + step 7 would give.
6. M4 at "heading + header, no row": Chrome moves the whole table, ours keeps the header orphan; the plan said "pin whatever the table's own rule does", pinned.
7. Section 1: `parse_break_forced` returns `bool` (step 0 note, not fixed here).
8. Fixture 41 needs **seventeen** item rows, not nine (nine rows leave page 1 half empty; the plan copied the number from 38). Eight rows added; header `pb-3 mb-6` -> `pb-4 mb-8` and table `mb-6` -> `mb-8` to put the room at 29 px (1.5 lines).

**Red list at `2cd0573` (base binary, `cargo test --release --locked`, test binaries run directly)**

Red (new, on the first Chrome assertion): `a_heading_with_break_after_avoid_moves_with_a_long_paragraph`, `break_before_avoid_on_the_paragraph_keeps_the_heading`, `avoid_page_and_the_legacy_alias_keep_avoid_column_does_not`, `a_chain_of_two_headings_moves_with_the_paragraph`, `a_heading_before_a_kept_together_card_moves_with_it`, `a_heading_before_a_table_moves_with_its_header`, `the_keep_holds_inside_a_section`, `a_chain_taller_than_a_page_breaks_inside_a_short_paragraph_first`, `a_heading_before_a_multicol_container_moves_with_it` (added: M6), `a_short_paragraph_after_a_heading_at_the_page_top_relaxes_rule_3` (added: step 0), `orphans_widows::a_short_paragraph_after_an_avoid_break_moves_with_its_heading` (flipped pin), `fixtures::terms_heading_keep_invoice_moves_the_heading_with_its_paragraph`. 12 red.

Green guards: `the_avoid_leaves_the_split_that_already_matches_chrome`, `without_the_avoid_the_heading_stays_at_the_page_bottom`, `a_pair_that_fits_the_room_stays_on_page_one`, `the_later_of_two_break_values_wins`, `a_chain_that_fits_or_ends_at_a_plain_heading_is_left_alone`, `a_heading_before_a_table_keeps_the_table_s_own_header_rule`, `a_forced_break_wins_over_avoid`, `a_pair_taller_than_a_page_is_not_kept`, `grid_items_and_columns_ignore_the_keep`; the other 23 tests of `orphans_widows.rs` and 19 of `fixtures.rs` (including `every_fixture_renders_to_a_pdf_deterministically` with fixture 41). The rest of the workspace suite was not re-run (test-only change; a full `cargo test --release` was started and killed by the 30 min background limit while another agent built in the shared target dir).

The red tests check later numbers (ours `AFTER` after the move, e.g. 532.6 pt) only after their first assertion fails; those values were derived from the base binary's output of the same document without the avoid plus the 15.75 pt of the heading, not observed.

**Fixture 41 on the base binary** (`python3 scripts/compare.py 41-terms-heading-keep`): pages 2 / 2, diff **3.9 % / 13.8 %**, worst 13.8 % (threshold not set yet; default 10 % fails). Page 1 of ours ends with the heading alone at 757.6 pt, page 2 starts with clause 1 at 44.3 pt; Chrome's page 2 starts with "Obchodní" at 43.8 pt, "40." at 402.3 pt, "Převzal:" at 448.8 pt (ours 381.1 / 427.6 pt). Expected after the fix: about 5-6 % on both pages (text drift). `compare.py` gets its entry after the fix.

### Step 0 result (css-spec-researcher, 2026-11-02)

- **Default reading confirmed; Variant B not taken.** In the pinned shape, the Class A break before the heading is allowed. The 5-line paragraph with orphans 3 / widows 3 has no allowed Class B break. Heading + paragraph fits a fresh page. So the §4.4 relaxation never starts, and the heading moves with the whole paragraph. Chrome's 3 / 2 violates Rule 3 while an allowed break exists, so it is non-conforming.
- **Confirmed wording (css-break-3 ED 2025-10-22 = CR 2018-12-04):**
  - §3.1: column values "only have an effect in multi-column contexts", so `avoid-column` does not constrain page breaks.
  - §3.4: legacy `avoid` aliases `avoid`.
  - §4.3: "a forced break value effectively overrides any avoid break value that also applies at that break point".
  - §4.4: Rule 3 is dropped before Rules 1, 2 and 4.
- **Blink's break appeal** (`break_appeal.h`): LastResort < ViolatingBreakAvoid < ViolatingOrphansAndWidows < Perfect. This agrees with §4.4 and with step 7: in an over-long chain, Rule 3 gives way first.
- **WPT (multicol analogues):**
  - `css/css-break/break-between-avoid-000`, `-001`: break at the earlier allowed Class A break.
  - `-004`: violate orphans/widows rather than avoid.
  - `-005`, `-006`: forced trumps avoid.
  - `-009`: propagation.
  - No paged WPT test covers the exact shape.
- **Head first on the page.** When the head is already first on the page, relaxation does start, so Chrome's 3 / 2 is conforming there. Add that as a test: `a_short_paragraph_after_a_heading_at_the_page_top_relaxes_rule_3`.
- **ADR-0010 item 2c.** Amend it with a dated clarification in the F215 PR: "... and no earlier allowed break (e.g. before the chain's head) keeps the content from overflowing".
- **Doc fixes for the F215 PR:**
  - `pagination_layout.rs:8701` and `docs/plans/2026-10-29-f30f-l-sub-line-overflow.md:24` misquote the spec as "fragmented content".
  - §1 of this plan: `parse_break_forced` returns `bool`, not enum values (`column_css.rs:539-552`).

### Coordinator decisions after steps 1 and 2 (2026-11-02)

- **A head at the page top (new step 7b, in scope).**
  - Case: the chain's head is first on its page (nothing in the flow above it on that page), and the chain head + first fragment of the next box does not fit a fresh page.
  - Here no allowed break keeps the content inside the fragmentainer. The break before the head sits at the page top, so it gains nothing. So §4.4's relaxation starts, and Rule 3 is dropped first.
  - Implementation: the walk starts the next paragraph in place with `rule3_dropped = true`. This is the same flag as step 7, set by the same `keep_with_next_extent` when `top(head) == page top`.
  - `choose_line_break`'s existing fallback order (author rules, then keep the orphans, then 2 / 2, then any break) gives Chrome's measured 1/4, 2/3, 3/2, 3/2 for room 1-4. That is conforming here; see the step 0 result, item "head first on the page".
  - The test is `a_short_paragraph_after_a_heading_at_the_page_top_relaxes_rule_3`.
  - This is not Variant B. A head with content above it still moves with the whole short paragraph.
- **Chrome version.** The reference is m153 (the committed references say `Skia/PDF m153`). Read "m141" in §3 as "Chrome, the version of the committed references".
- **M3 (a pair taller than a page).** Ours places the box whole. Chrome slices the fixed-height box. That difference is F96 / box slicing, not F215, so the pin stays.
- **Grid items honour the avoid in Chrome.** That is F215e (out of scope here, ADR-free); the pin stays.
- **Fixture 41 with 17 item rows.** Accepted.
- **Next.** Steps 3+ (fork) wait until PR #85 (F30c-o3) merges. The fork branch is linear, one engineer at a time.
