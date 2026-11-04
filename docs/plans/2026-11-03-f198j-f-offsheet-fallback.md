# F198j-f: the off-sheet fallback of F198j bakes only the boxes it can cut

Date: 2026-11-03. Finding: F198j-f in `2026-10-15-m0-findings.md` (medium, warned, small). Follows F198j (`2026-11-02-f198j-multicol-shifted-children-and-tall-lines.md`, §8.6 round 1 of the review). Fork: `subapavel/fulgur`, branch `htmltopdf/srgb-colours` (with F213d `b81c6782` and F213e `eade038a` on top). Line numbers are from that head; re-read them before editing. No `css-spec-researcher` output exists: §2 is from memory and unverified.

## 1. Problem

F198j paints a relative offset inside a cut box as a shift of its pieces. Round 1 of the review added a fallback: when any shifted atom's painted extent leaves the sheet (`fill_multicol_columns` sets `McFill::lost`), the group is planned again with every shifted box replaced by its baked description (`McContainer::baked`, from `mc_box_baked`: the offsets are left in the flow, as main described them). When `mc_box_baked` returns `None`, the fallback removes the box from `boxes` (`fragment_multicol_container`, `pagination_layout.rs:2128-2156`, `alt.remove(node)`). The box is then one atom placed whole: everything below the first page edge is lost, with a warning. `mc_box_baked` returns `None` for exactly the shapes main refused. 34 of the validator's 286 probes hit this. Words missing (main / round 1 / round 2): `@page{margin:0}` with every paragraph `top:-8px`, 2058 / 0 / 2058; A5 with `top:-40px`, 2316 / 0 / 2316; `top:-20px` with 5 mm margins, 2094 / 0 / 2094; `top:60px` on every 5th paragraph, 2140 / 0 / 2140; a `top:20px` wrapper around a tall line, 1210 / 5 / 1210. No committed fixture shows the problem. Fixture 40's single `top:2px` nudge stays in the margin and never triggers the fallback. The repro shapes are rebuilt as tests in §4.

## 2. Spec (unverified)

- **CSS 2.1 §9.4.3, css-position-3 (relative positioning; section number unverified).** A relatively positioned box is laid out in normal flow and then shifted. The shift does not affect the position of the boxes after it. So the static layout is the one the planner must break. The shift is applied to the pieces afterwards, and this holds whatever the sign or uniformity of the offsets.
- **css-break-3 §2 / §4.1 (as cited in ADR-0004).** Line boxes are monolithic. Monolithic content that does not fit is not split; content taller than the fragmentainer may be sliced. ADR-0004 decides that we never slice a line. A line is printed whole or not at all.
- **css-break-3, fragmentation and relative positioning (section unverified).** The offset applies to each fragment after fragmentation. Chrome matches this (F198j §8.1, `x_relshift`).
- **css-page-3 (page box and sheet; section unverified).** Nothing outside the sheet is printed. A line shifted off the sheet is lost in Chrome too. The F198j round-1 validator measured Chrome dropping 34 / 52 / 100 words of the clip card at 100 / 150 / 300 px.

Consequence: the spec's answer for every probe is "static fragmentation plus painted shift". The fallback is there only to keep main's word count where main's in-flow description prints more words than painting does. It must never print fewer words than painting.

## 3. Where it belongs, and the rule

The fix is in the fulgur fork only (multicol fragmenter, our code). Blitz and Taffy are correct. No upstream issue or PR (standing decision).

**Why `mc_box_baked` refuses these shapes.** It runs `mc_box_at` with `rel = [0, 0]` on Taffy's shifted positions. Two checks there refuse:

- **The first child is above its box.** A negative shift larger than the box's top border and padding puts the first child above the box (`cy < -0.5`). This covers shapes 1 to 3: the card has a 1 px border.
- **A sibling is above its predecessor's last line.** A downward shift larger than the shifted child's last line (about 17.3 px) puts its next sibling above that last unit (`above_previous`). This covers shapes 4 and 5.

Step 1 confirms both with a dump.

**Rule.** The fallback is taken box by box, only for the boxes whose baked description exists:

1. A box with a non-zero `slack` gets a baked entry only when `mc_box_baked` describes it.
2. When the group's plan is `lost`, every box of the group that has a baked entry is swapped in and the group is planned again. A box without a baked entry keeps its painted plan: it loses only the lines that leave the sheet, as in round 1.
3. If the final plan is still `lost`, the engine warns that a line is painted past the edge of the page.

Why per box: the atoms of different boxes do not share frames. A baked box prints every word (it has no shift, so it cannot be `lost`). A box that has to stay painted is no worse off for its neighbour being baked. An all-or-nothing rule ("fall back only if every box bakes") would bring back round 1's 85 to 495 missing words for the boxes that bake fine.

Why this cannot print fewer words than main:

- `mc_box_baked` accepts a superset of what main's `mc_box` accepted, because F198j §4.1 and §4.2 only relaxed checks. So a box without a baked entry was placed whole on main. Whole placement loses everything below the first page edge of a box taller than a page.
- The painted plan loses only the lines whose shift carries them off the sheet: at most `|shift|` px of content per column edge.
- The sweep (§4.4) checks this probe by probe.

**Code (fork, `crates/fulgur/src/pagination_layout.rs`):**

```rust
struct McContainer<'a> { /* … */ baked: BTreeMap<usize, McBox> } // was BTreeMap<usize, Option<McBox>>
fn flow_holds(flow: &[crate::multicol_layout::FlowItem], node: usize) -> bool; // a Block node or a Lines source
```

- `multicol_container` (`:1167-1198`): insert into `baked` only `Some(..)` from `mc_box_baked`. Update the field's doc comment, which currently says "`None`: it was left whole".
- `fragment_multicol_container`, `Group` arm (`:2116-2157`):
  - Keep `(plan, lost)` mutable.
  - If `lost` and some `baked` node is `flow_holds(&group.flow, ..)`, swap only those nodes, warn as today ("… the offset is part of the flow there"), and re-plan with `plan_multicol_pages_checked`, which updates `lost`.
  - Then, if `lost`, warn: "pagination: a relative offset inside a multi-column container paints a line past the edge of the page; what is outside the page is cut or not printed".
  - Delete the `None => alt.remove(node)` arm.
  - The `flow_holds` filter keeps the "part of the flow" warning honest when the baked box sits in another group (only possible with spanners). It does not change any plan.

**Byte reasoning:**

- A group whose plan is not `lost` is untouched.
- A `lost` group whose shifted boxes all bake is planned exactly as in round 2. After the swap no atom has a shift, so `lost` is false and no new warning fires.
- Only a `lost` group that holds a box with no baked description changes: that box keeps its painted pieces instead of being placed whole.

## 4. Acceptance

### 4.1 Ours: `crates/htmltopdf/tests/multicol_boxes.rs` (red first)

Add `fn render_page(page_css: &str, body: &str) -> Vec<u8>` (`document` fixes `@page{size:A4;margin:15mm}`). Change `shape_offset_card(top: i32, clip: bool, every: usize)` so that only paragraphs with `i % every == every / 2` (§7: `every - 1` does not reach the fallback) get `position:relative;top:<top>px`; the existing callers pass `every = 1`. Step 1 records main's, round 2's and Chrome m153's numbers for each test in its doc comment.

- **R1** `a_negative_offset_at_margin_zero_keeps_its_shift_and_every_word`: `@page{size:A4;margin:0}`, `shape_offset_card(-8, false, 1)`.
- **R2** `a_large_negative_offset_on_a5_keeps_its_shift_and_every_word`: `@page{size:A5;margin:10mm}` (§7: at 15 mm head is green), `shape_offset_card(-40, false, 1)`.
- **R3** `a_negative_offset_with_narrow_margins_keeps_its_shift_and_every_word`: `@page{size:A4;margin:5mm}`, `shape_offset_card(-20, false, 1)`.
- **R4** `an_offset_on_every_fifth_paragraph_keeps_its_shift_and_every_word`: A4 15 mm, `shape_offset_card(60, false, 5)`.
- **R5** `a_shifted_wrapper_around_a_tall_line_keeps_its_shift_and_the_rest_flows_on`: A4 15 mm, a `CARD` holding `<div style='position:relative;top:20px'>` with 300 words, one 60 x 1200 `red60.png` and 300 words, then a `<p>` of 600 words.

Asserts:

- **R1 to R4:** every word printed once. Round 1 printed all of these, so the expected count is the step-1 number of the round-1 binary, 0 missing.
- **R5:** `assert_words_once_but_tall_lines`, at most 5 words missing, all on the tall line's baseline.
- **All:** Chrome m153's page count, and the shift oracle (`assert_shift_oracle` against the same card without offsets, ±0.01 pt) for every word printed in both.
- **On round 2 (fork head) these are red:** the step-1 numbers, about 1200 to 2300 missing.

Guards, green today:

- **G1** `guard_a_uniform_offset_that_leaves_the_sheet_is_still_planned_in_the_flow`: `shape_offset_card(100, false, 1)`. Every word once. The first word of page 1, column 2 is round 2's number (baked), not the flat card's: today's path stays. The existing `an_offset_that_would_paint_a_line_off_the_sheet_keeps_every_word` stays as it is.
- **G2** `in_one_flow_a_card_main_could_cut_is_baked_and_one_it_could_not_keeps_its_shift`: one multicol container with two tall cards, `shape_offset_card(100, false, 1)`'s card, then R4's card (words numbered on). Every word of both is printed once. This is the test that pins "per box": an all-or-nothing rule loses about 85 words of card 1, and round 2 places card 2 whole.
- **G3:** `guard_a_child_pulled_above_the_previous_line_keeps_the_old_path` (418, 2 pages) and `guard_position_relative_without_an_offset_changes_nothing` are unchanged. Margins are not offsets: no slack, no fallback.

### 4.2 Ours: `multicol_shifted_warning.rs`

The logger is global to the process. Install it once (`std::sync::Once`) and serialise the tests (`static SERIAL: Mutex<()>`). Add `an_offset_off_the_sheet_is_warned_about_on_both_paths`:

- R4's shape: exactly one "past the edge of the page" warning, no "taller than a page", no "part of the flow". On round 2 it warns "taller than a page" and "part of the flow" (red).
- `shape_offset_card(100, false, 1)`: one "part of the flow" warning and no "past the edge".
- `top:20px`: no warning.
- The existing test keeps its one "taller than a page" for `margin-top:-40px`.

So no shape that warned before goes silent.

### 4.3 Fork

- **Unit test** `multicol_container_keeps_an_in_flow_plan_only_for_a_box_it_can_cut`, on a Blitz document with a 2-column container and a tall card:
  - `top:-8px` on every paragraph of a 1 px border card: no `baked` entry;
  - `top:8px` on every paragraph: an entry;
  - `top:60px` on every 5th paragraph: no entry.
- **`render_smoke`** `render_v2_smoke_multicol_card_with_offsets_main_could_not_cut_keeps_its_shift`: the R2 and R4 cards, every word printed (`extract_pdf_text`). Red on the fork head. This covers the coverage gotcha in fulgur's `CLAUDE.md`.
- `render_v2_smoke_multicol_card_with_offsets_that_leave_the_sheet_keeps_every_word` and `plan_reports_an_offset_that_paints_a_line_off_the_sheet` stay green.

### 4.4 No new fixture

None:

- **No committed fixture moves**, so there is no golden to update. Fixture 40 (`top:2px`) never sets `lost`. Fixtures 15 and 26 use `position` outside any multicol container.
- **The shapes are stress shapes, not invoices.** Every paragraph shifted -8 px on a margin-free page is not something an invoice does. A fixture tuned to hit the sheet edge would be brittle against any line-height change.
- **R1 to R5, G2 and the Chrome-numbered asserts already pin the behaviour,** in the same way F198j round 1 was pinned (§8.6, no fixture).

### 4.5 Sweep and byte gates

**Sweep.** Rebuild the validator's 286 probes in `out/scratch/f198jf/` (generator committed only as the test shapes above):

- pages: A4 15 mm, A4 5 mm, A4 margin 0, A5 15 mm;
- cards: plain border, clip card, `CARD`, strip-first card;
- offsets: uniform `top` in {-40, -20, -12, -8, -2, 2, 8, 20, 60, 100, 150, 300} px, and every 2nd / 5th paragraph `top:20px` / `60px`;
- wrapper around a tall line: `±20px`;
- 2 and 3 columns.

Binaries: "main" is pre-F198j (fork `329b4e43`, the old path); "round 1" is `58c78ae2`; "head" is today's fork; "new" is this change. Chrome m153 for each probe. Per probe, record: words missing (of `wNNNN`), words printed twice (Chrome), pages, the first word of every column, the warnings, and from a scratch dump (not committed) each box's `slack`, baked yes/no and `lost`.

Pass criteria:

1. `new_missing <= main_missing` and `new_missing <= head_missing` on all 286 probes.
2. The 34 affected probes are exactly the ones with `lost` and a box with no baked entry, and `new_missing == round1_missing` on them (0 to 224).
3. Every other probe is byte-identical to head.
4. Every probe that warned on head still warns.
5. On no probe are the page-count difference to Chrome or the summed first-word distance larger than head's. Report the per-probe table in the PR.

**Byte gates** (1 and 4 rayon threads, `out/scratch/f30fl/gate.sh`): every committed fixture (01–44 today; 45 if it lands first), the 7 corpus documents, and the F198j sweep (498 files) identical to main, except probes that are both `lost` and hold a box with no baked entry. Reason: §3 byte reasoning. `compare.py`: no number moves, no threshold changes. Fork VRT: `.actual.pdf` identical before and after; do not re-bless on this host.

## 5. Steps (each at most a day)

1. **Measure (½ day).** Generator, the four binaries, Chrome m153, the scratch dump. **Proceed** only if every one of the 34 has a box without a baked entry and `lost`. If a probe has a baked entry and still loses words, the cause is elsewhere: stop, file it, and keep the rest of the plan.
2. **Red tests (½ day).** R1 to R5, G1, G2, the warning test, the fork unit and smoke tests, with step 1's numbers. Commit red.
3. **Fork change (½ day).** §3 code. Then fork `cargo fmt`, `cargo clippy --all-targets -D warnings`, `cargo test -p fulgur`, fork VRT actuals.
4. **Ours (½ day).**
   - `Cargo.lock` bump; byte gates; sweep table; `compare.py`; `bench.py --check`.
   - Docs: F198j-f done in the findings file; `docs/upstream.md`; a line in the F198j plan §8.6 pointing here.
   - `vrt-validator`. `pdf-reviewer` is not needed: no writer path, fonts or metadata are touched.

Size: small, about 30 lines of fork code. Tests: about 150 lines here and 80 in the fork.

## 6. Risks and non-goals

- **Lines past the sheet stay lost for boxes main could not cut**, and are now warned. Chrome loses such lines too (§2). The spec gives no way to print a line that lies outside the sheet.
- **`lost` is conservative.** It tests the line box's extent, not its glyphs. A line 8 px above a margin-free sheet still prints its baseline (R1: 0 missing) but is warned. The warning text says "cut or not printed".
- **The bake is still group-wide among the boxes that can bake.** A box whose own shift stays on the sheet is baked because a sibling box's shift leaves it. This costs position only (7 to 20 pt off Chrome), not words, as in round 2. Non-goal: a per-box `lost` set.
- **A shifted line taller than the sheet always reports `lost`**, whatever the shift (`top + h` already exceeds the sheet). The outcome is the same words either way (the baseline words of the tall line, F198l). Non-goal; file it if the sweep shows a position cost.
- **Very large shifts.** In principle a painted plan could lose more than whole placement, for example a shift of more than a column on every line. Criterion 1 of the sweep catches it. If it happens, compare the atom loss of both plans and take the smaller; do not widen the rule beforehand.
- **Non-goals:** F198j-d (shapes still placed whole), F198j-e (rounding of offsets), F198j-g (first-page margins in the sheet check), F198l (slicing a tall line), offsets of a top-level column child (F30d-b).

## 7. Measurements (step 1, 2026-11-03)

Binaries, all `htmltopdf` release builds of this repo's main c625feb with only the fork pin changed in a scratch copy (never committed): **main** = fork `329b4e43` (pre-F198j), **F198j** = `58c78ae2` (painted shift, no fallback; the plan's "round 1"), **head** = `b81c6782` (F198j round 2, F213d; byte-identical to the pre-built `htmltopdf-main-f213d`). Chrome 153.0.8010.12 (Playwright, print media, `preferCSSPageSize`). Words by `pdftotext -bbox`. Inter-Regular bundled, 1 rayon thread. All 286 + 347 probes rendered with exit code 0 on every binary.

**Probe sets.** (a) The validator's 286 probes (its 292 html files without the six `perf_*`), re-run. (b) The §4.5 matrix rebuilt: pages `a4m15`, `a4m5`, `a4m0`, `a5m15` x cards plain / clip / `CARD` / strip-first x the 16 offsets x 2 columns, plain card in 3 columns, the wrapper around a tall line at +-20 px, plus the test shapes: 347 probes. The generator is scratch (not committed); the test shapes are `crates/htmltopdf/tests/multicol_boxes.rs` and were dumped through `dump_chrome_inputs` to Chrome, whose numbers agree with the matrix probes.

**The 34.** Head warned "part of the flow" on 75 of the 286 probes and "part of the flow" plus "taller than a page" on 40. A throwaway build of the fork at `b81c6782` in a scratch clone (one `eprintln!` of each box's `slack`, `baked` entry and `lost` before the fallback; the fork itself untouched; its PDFs are byte-identical to head's on all 633 probes) shows: **35 probes have `lost` and a box with `slack` and no baked entry**; on 34 of them head loses more words than F198j (the plan's 34); on the 35th (`e_huge_shift`, a shift of thousands of px) every binary prints nothing. The other 5 of the 40 (`j_tall_shift`, `l_m0_tall_*`, `t_inrel_*`) have a baked entry: their 5 to 7 missing words are the baseline of a line taller than a page (F198l), same on head and F198j. Words missing of the 34 (Chrome clips lines it moves off the sheet, so it loses some too):

| probe | Chrome | main | F198j | head | pages Chrome / main / F198j / head |
|---|---|---|---|---|---|
| `j_a5u_40` | 16 | 2316 | 0 | 2316 | 8 / 2 / 8 / 2 |
| `j_a5u_50` | 20 | 2312 | 0 | 2312 | 8 / 2 / 8 / 2 |
| `j_a5u_60` | 24 | 2316 | 64 | 2316 | 8 / 2 / 8 / 2 |
| `j_clipstrip_100` | 0 | 1519 | 0 | 1519 | 4 / 2 / 4 / 2 |
| `j_landu_60` | 54 | 2091 | 0 | 2091 | 4 / 2 / 4 / 2 |
| `j_m0u_20` | 14 | 2058 | 44 | 2058 | 3 / 2 / 3 / 2 |
| `j_m0u_40` | 28 | 2058 | 86 | 2058 | 3 / 2 / 3 / 2 |
| `j_m0u_8` | 0 | 2058 | 0 | 2058 | 3 / 2 / 3 / 2 |
| `j_m0u_80` | 84 | 2058 | 173 | 2058 | 3 / 2 / 3 / 2 |
| `j_small1b_20` | 12 | 2094 | 129 | 2094 | 5 / 2 / 4 / 2 |
| `j_small1b_40` | 25 | 2086 | 121 | 2086 | 5 / 2 / 4 / 2 |
| `j_small1b_80` | 51 | 2088 | 126 | 2088 | 5 / 2 / 4 / 2 |
| `k_dn300_cl` | 100 | 2200 | 85 | 2200 | 4 / 2 / 4 / 2 |
| `k_dn300_pl` | 0 | 2200 | 85 | 2200 | 4 / 2 / 4 / 2 |
| `k_dn60_cl` | 16 | 2158 | 0 | 2158 | 4 / 2 / 4 / 2 |
| `k_dn60_pl` | 0 | 2158 | 0 | 2158 | 4 / 2 / 4 / 2 |
| `k_up300_cl` | 58 | 2100 | 215 | 2100 | 4 / 2 / 4 / 2 |
| `k_up300_pl` | 58 | 2100 | 215 | 2100 | 4 / 2 / 4 / 2 |
| `k_up60_cl` | 18 | 2116 | 0 | 2116 | 4 / 2 / 4 / 2 |
| `k_up60_pl` | 18 | 2116 | 0 | 2116 | 4 / 2 / 4 / 2 |
| `u_up100` | 66 | 2116 | 96 | 2116 | 4 / 2 / 4 / 2 |
| `u_up100_cl` | 66 | 2116 | 96 | 2116 | 4 / 2 / 4 / 2 |
| `u_up100_m5` | 64 | 2088 | 224 | 2088 | 4 / 2 / 4 / 2 |
| `u_up20_m5` | 12 | 2094 | 0 | 2094 | 4 / 2 / 4 / 2 |
| `u_up40_m5` | 25 | 2086 | 53 | 2086 | 4 / 2 / 4 / 2 |
| `u_up60` | 36 | 2116 | 0 | 2116 | 4 / 2 / 4 / 2 |
| `u_up60_cl` | 36 | 2116 | 0 | 2116 | 4 / 2 / 4 / 2 |
| `u_up60_m5` | 38 | 2087 | 105 | 2087 | 4 / 2 / 4 / 2 |
| `x_mix_100` | 0 | 2140 | 19 | 2140 | 4 / 2 / 4 / 2 |
| `x_mix_60` | 0 | 2140 | 0 | 2140 | 4 / 2 / 4 / 2 |
| `x_mixcl_100` | 0 | 2140 | 19 | 2140 | 4 / 2 / 4 / 2 |
| `x_mixcl_60` | 0 | 2140 | 0 | 2140 | 4 / 2 / 4 / 2 |
| `x_mixneg_100` | 0 | 2134 | 19 | 2134 | 4 / 2 / 4 / 2 |
| `z_tall_in_rel_card` | 0 | 1210 | 5 | 1210 | 3 / 2 / 3 / 2 |

Sum over the 34: Chrome 939, main 70925, F198j 1979, head 70925 (head is main's number on every one: the box is placed whole). F198j is never worse than main. No probe prints a word twice on F198j; Chrome prints some twice (312 on these). In the matrix: 76 probes, all with the same signature, main 155789 / F198j 1792 / head 155789 words missing; F198j's worst is 118.

**Stop condition (§5 step 1): not hit for the 34.** Every one has `lost` and a box with no baked entry, and F198j (what the fix reproduces) prints the number above. Probes outside the 34 that have a baked entry and still lose words exist, with known causes that this change does not touch: `j_small1_8/20/40/80` (a first-page margin smaller than the others: F198j-g, "first-page margins in the sheet check"; on `_8` and `_20` head already loses more than main, 180 against 154 and 154 against 148) and the tall-line baselines above (F198l). Nothing new to file for the plan.

**Corrections to §4.1 found by measuring.**

- R2 at A5 with 15 mm margins is green on head: 40 px is 30 pt, below the 42.5 pt margin, so `lost` is false. The validator's `j_a5u_*` use 10 mm (28.3 pt); R2 uses `size:A5;margin:10mm`.
- R4 with the shifted paragraph the last of each five (`i % 5 == 4`) is green on head: the shifted lines stay on the sheet (`e5t60p4` in the matrix). The validator's `x_mix_60` shifts `i % 5 == 2`, which is what reaches the fallback (2140 missing on head). `offset_card` shifts `i % every == every / 2`.
- R5 as built (`CARD` holding the wrapper and the 600-word paragraph): main and head 902 of 1200, F198j 4, Chrome 0, 3 pages. At `a4m0` F198j loses 12 on the `-20px` wrapper, so "at most 5" is the A4 15 mm number only.
- The page counts F198j prints equal Chrome's on R1 to R5 (3, 8, 4, 4, 3). The oracle holds to 0.01 pt on F198j for R1 to R4 (the test passes on a build of `58c78ae2`).
- G1 (`top:100px` on every paragraph): main and head 0 missing, F198j 85, Chrome 0; column 2 of page 1 starts with `w0306` on main and head (offsets in the flow), `w0342` on F198j and in Chrome. G2: main and head 2140 missing, F198j 95 (all in the first card), Chrome 0; pages main 6, F198j 8, head 6, Chrome 8.

**Predicted §4.5 criteria, from F198j's numbers (the fix itself does not exist yet).** Criterion 1 (`new <= main` and `new <= head`) holds on all 347 matrix probes and on 284 of 286 validator probes; `j_small1_8` and `j_small1_20` fail it already on head (F198j-g). Criterion 5: on the 34 and the 76 the page-count distance to Chrome is not larger than head's on any probe. Criteria 2 to 4 need the fixed fork.

**Step 2 (red tests), state on this commit.** On head: R1 to R5, G2 and `an_offset_off_the_sheet_is_warned_about_on_both_paths` fail; G1 and the existing tests pass. On a build of `58c78ae2` R1 to R5 pass; G1 and G2 fail there by design (85 and 95 words). G2, the warning test and the two fork tests (`fork-tests.patch`, kept outside both repos) are not seen green: they need the fixed fork. R1 to R5, G2 and the warning test are `#[ignore = "F198j-f: red until the fork fix"]`.

### Steps 3 and 4: the fix and its gates (2026-11-03)

Fork `d0780431` (on F213e's `ccaa0207`, pushed append-only): `McContainer::baked` is `BTreeMap<usize, McBox>`, `multicol_container` inserts only the boxes `mc_box_baked` describes, `fragment_multicol_container` swaps the baked boxes that `flow_holds` and plans again with `plan_multicol_pages_checked`, warns "paints a line past the edge of the page" when the plan is still lost, and `plan_multicol_pages` is `#[cfg(test)]`. The two fork tests of `fork-tests.patch` were adapted (the helper trait went: `baked.contains_key`, because "no entry" is what the fix changes) and compiled red on `ccaa0207` (the unit test fails at `top: -8px`; the `render_smoke` case prints 115 of 2500 words at A5 / `top:-40px`), green after. Fork gates: `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (workspace), `cargo test -p fulgur --no-fail-fast` 2556 lib tests and all 44 other test binaries green, fork VRT: the same 35 goldens differ on this host as before (pinned fonts missing) and the 35 `.actual.pdf` are byte-identical to `ccaa0207`'s.

Ours: main a7fdf92 and `origin/f213e-first-block-below-top-margin` (PR #93 was still open) merged, `Cargo.lock` pinned to `d0780431`, R1 to R5, G2 and the warning test un-ignored. Baseline binary `htmltopdf-e3` (main + F213e).

- **Byte gates** (fixtures 01 to 45 and the 7 corpus documents, 52 PDFs, 1 and 4 rayon threads): identical to `htmltopdf-e3` at both, and 1 thread equals 4. No fixture moved, so there is no golden to update.
- **Sweep, validator's 286 probes** (new = `htmltopdf-f198jf`): 251 byte-identical to e3; the 35 that differ are the 35 with `lost` and an unbaked box (the 34 and `e_huge_shift`; same words there). Criterion 2: on the 34, new equals F198j word for word (1979 missing in all, worst 224; e3 70925). Criterion 3: every other probe identical. Criterion 4: no probe that warned on e3 is silent. Criterion 5: pages are never farther from Chrome's than e3's, and the summed first-word distance of the columns (an empty column counts as its Chrome word number) falls from 1,198,803 to 866,821 and is not larger on any probe. Criterion 1 (`new <= main` and `new <= e3`): fails on `j_small1_8` and `j_small1_20` only, as on e3 (F198j-g); new equals e3 there. 2 rayon thread counts give identical bytes on all probes.
- **Sweep, 347-probe matrix**: 271 identical to e3, the 76 that differ are exactly the 76 affected (criterion 3), all of criteria 1, 4, 5 hold with no exception. On the 76 new equals F198j except `a4m15_g2_two_cards` (G2: F198j 95 missing, new 0): 1697 words missing in all against 155789 on e3 (F198j 1792), worst 118.
- **`compare.py --threshold-check`**: all 45 fixtures ok, no number moved (`out/compare/` re-rendered with the new binary), thresholds untouched.
- **Warnings**: R4's shape now warns once "paints a line past the edge of the page" and nothing else; the all-baked shape keeps its "part of the flow" warning; `top:20px` is silent.
- **Tests and lints here**: `cargo test --release --locked --no-fail-fast` 89 test binaries, 1078 passed, 0 failed, 14 ignored (none of them is a F198j-f test: R1 to R5, G2 and the warning test run and pass, G1 stays green); `cargo fmt --check` and `cargo clippy --release --locked --all-targets -- -D warnings` clean in both repos. `bench.py --check` ok (b200-text 755 ms and b200-mixed 840 ms under 1000 ms; b200-items 1321 ms is over the 1250 ms exception on this machine, as it was for F198j and F213 on the same host; scaling r = 4.03, gate 6.0; the change touches a path no fixture or corpus document takes: no fixture sets `lost`).
- **Not verified**: Chrome was not re-run for the fixed binary beyond the sweep numbers above; the fork VRT goldens were not re-blessed (no pinned fonts on this host); `markdownlint` was not run (no npm).

