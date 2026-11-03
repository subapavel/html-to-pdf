# F213e: the first block on page 1 is not moved when page 1 starts below the content top

Date: 2026-11-03. Finding: F213e in `2026-10-15-m0-findings.md` (medium: content below the content box, or lost; pre-existing, main and F213 byte-identical). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, carried by the existing `[patch.crates-io]`; build on F213d's head once it is pushed. Line numbers were read from the fork's working tree on 2026-11-03 while F213d was being committed (not from `cca17235`); re-read them at the build commit.

## 1. Problem

The fragmenter works in body coordinates. `y = 0` is the top of the body's content, and the cursor starts there (`pagination_layout.rs` `fragment_pagination_root`, `:3380`). F29 handles the body's own offset on page 0 (its margin, a first-child margin collapsed through it, html's padding and border: `body_origin_in_px`, `:10958`) only by moving page 0's bottom up (`PageStrips::bottom(0) = page_h - first_offset`, `:2783-2833`). But every rule that moves a box whole to the next page asks whether something precedes it with `cursor_y > 0.0` or `content_above` (`cursor_y - gap > 0.5`). For the first child on page 0 both are false whatever the offset. So a first block that does not fit `bottom(0)` but fits a fresh page stays where it is. It hangs up to `first_offset` px below the content box, or below the page edge, where it is lost with the "content extends N pt past the bottom page edge" warning.

Body `padding-top` / `border-top` work because they are inside the body: the first child's `location.y` is greater than 0, so the gap at `:3607-3609` makes `cursor_y > 0`. The validator's files are lost. Its numbers (`margin-top: 100px` → line at 832.6 pt, 33 pt below the content box, no warning; `200px` → lost, warning 63 pt) fit a one-line carrier about 950 px tall that is the first child. They do not fit the brief's 300 px box, which fits the 909 px of room. Step 1 rebuilds both shapes.

Today's shapes:

| Shape | Our result | Chrome |
|---|---|---|
| Q1 `body{margin-top:Mpx}` + `<p>START B(950) TAIL</p><p>AFTER</p>`, M = 100 / 200 | hangs / lost | page 2 |
| Q1b M = 809 + `B(300)` (room 200, as F213 P1) | lost | page 2 (predicted) |
| C4 `<img src=red60.png style="display:block;width:100px;height:950px">` first, M = 100 | hangs 41 px below | page 2 (predicted) |
| C5 `break-inside:avoid` card, 45 lines × 20 px, first, M = 200 | entered and split | moved whole (predicted) |

So the defect is not specific to inline-block carriers.

## 2. Spec

css-spec-researcher was not run. The wording and section numbers below are cited from memory and are **unverified**; confirm them in step 1.

- **css-break-3 §4.1.** Class A break points lie between siblings, Class B between line boxes, Class C between a content edge and its child content only if there is a non-zero gap. A margin collapsed through `<body>` is not such a gap, so read strictly there is no break point before the first box.
- **css-break-3 §4.4.** When the rules leave no break that avoids overflow, they are relaxed in order, and in the end the UA "may break anywhere" so that content is not lost off the fragmentainer's edge. We take the break in front of the first box under that rule. It is also Chrome's behaviour: Blink allows a break before a child when there is "container separation", meaning a non-zero offset from the fragmentainer start (from memory).
- **css-break-3 §5.2.** Margins adjoining an unforced break truncate to zero. A margin at the start of the fragmentation context adjoins no break and is kept. So page 1 keeps the offset (as F29 does), and the moved box starts page 2 at the content top with its collapsed margin truncated. That is exactly our existing move (`cursor_y = 0` on page ≥ 1).
- **CSS 2.1 §13.3.3.** The allowed breaks are listed there; rules A, B and D are dropped when the list gives too few break points. An unforced break sets the adjoining margins to 0.
- **CSS 2.1 §8.3.1.** The body's top margin collapses with its first in-flow child's when the body has no border, padding or clearance. The root's margins do not collapse. So `first_offset` may be partly the child's own margin.
- **css-page-3, page model.** Each page's page area is that page's fragmentainer, and `@page :first` margins shrink only page 1's. This is the basis of F109: we paginate every page against page 1's box (`engine.rs:258-305`; warning at `render.rs:199-266`).

**Chrome probes** (step 1; `render.mjs` settings; `pdftotext -bbox`: page of each word, yMin, page count):

- Q1 and Q1b.
- C3: `@page :first{margin-top:100mm}` with `B(300)` after a spacer, and with `B(900)` first.
- C4, C5.
- C6: C5 with no body margin, but the card has `margin-top:200px` collapsing through body.
- C7: M = 100 with `break-before:page` on the first child. Does Chrome print a blank page?
- C8: `body{margin:8px}` with a first `<img>` 1001 / 1002 / 1003 px tall (Chrome's 1010 px fragmentainer, F30f-b).
- C9: body `border-top:5px` plus M = 100 plus C4: is the body frame on page 1?
- C10: `<section>` around Q1's `<p>`.
- C11: `html{padding-top:100px}` plus C4.
- C12: M = 100 plus a first `<img>` 1200 px tall (taller than a page).

## 3. Where it belongs: cause and design

This is fulgur fragmenter behaviour, so the fix goes in the fork (`pagination_layout.rs` only). htmltopdf only bumps `Cargo.lock` and updates `docs/upstream.md`. No upstream PR or issue.

**Hypotheses, ranked:**

1. **H1 (read from the code): the move rules ignore the offset.** There are five body-level rules that move a box whole, all gated on `cursor_y > 0.0`:
   - multicol F28, `:3731`;
   - the leading move of a split paragraph, `:3860`;
   - the paragraph move-whole, `:3978`;
   - the F28 wrapper move, `:4226`;
   - the plain advance, `:4312` (carriers since F213, images, kept-whole avoid boxes).

   In addition, `content_above` (`:4116`) feeds `keeps_whole_at` (`:4143` → `avoid_keeps_whole` `:1753`, `above = false` → the avoid card is not kept and the gate enters it) and the F28 leading filters (`:4236`, `:4241`).
2. **H2 (`:first`): F109, not the guard.** `page_h` is page 1's content height for every page. A first block taller than page 1's box but within page 2's counts as "taller than a page" (`monolithic_line_carrier` needs ≤ fresh + 1, and F28 needs `leading <= fresh`), so it is entered in place and lost (the F213b shape). Treating `:first` as an offset is not a guard fix: it changes the height of every later page, the render strip phase (`render.rs:317`), the F184 canvas and the F109 warning. It moves to F109.
3. **H3: `first_offset` is 0** (clamped, or not read). Step 1 checks with a lib dump.

**Design.** Page 0 with an offset behaves like a page with content above, except for a box that would not fit a fresh page either.

```rust
impl PageStrips {
    /// html-to-pdf F213e: page 0 starts `first_offset` below the page area's top (body margin, collapsed child margin,
    /// html padding/border), so something precedes the first box there (css-break-3 §5.2 keeps that margin).
    pub(crate) fn offset_above(self, page: u32) -> bool;
    /// Whether the class A break in front of a box of `size` at `cursor` may be taken to move it whole: `cursor > 0`
    /// (unchanged), or `offset_above(page)` and `size <= fresh() + 1.0` (on a fresh page it fits; otherwise moving
    /// only leaves page 0 empty).
    pub(crate) fn may_move(self, page: u32, cursor: f32, size: f32) -> bool;
}
```

- **The five rules** call `may_move` instead of `cursor_y > 0.0`. They pass the size each rule already compares with the room: `leading`, `para_total_h.max(keep_size)` or `child_h.max(keep_size)`. Reason: with no offset, `may_move` is `cursor > 0`, which keeps every document without an offset byte-identical.
- **`keeps_whole_at` and the F28 leading filters** take `content_above || offset_above(page_index)`. Reason: the avoid card has to be kept whole and moved by `:4312`, and a wrapper has to be counted with its leading avoid box or keep chain.
- **Unchanged on purpose:**
  - the forced-break guards `:3566` and `:3659`. Reason: we must not add a blank page before a `break-before:page` first child. C7 decides; if Chrome differs, file it as F213e-b.
  - `starts_with_relaxed_avoid` (`:4117`, avoid boxes taller than a page);
  - `keep_size` (`:3703`) and `member` (`:3950`), which are F215;
  - the `content_above` that is passed to `fragment_block_subtree` (`:4273`). Reason: nested forced breaks (`:6159-6178`, F83b Z1_234). The body-level F28 already moves a wrapper whose first row does not fit.
- **Determinism:** no new map and no change to float arithmetic. A moved box gets `cursor_y = 0.0` as on the existing paths.

## 4. Acceptance

**Fixture `45-letterhead-keep-together`** (Czech invoice for pre-printed letterhead paper).

- **Page and body:** `@page { size: A4; margin: 15mm }` and `body { margin-top: 70mm }` in the fixture's `<style>` (as 26 sets its body margin). Page 1 has 744.6 px of room.
- **First block:** `<main class="break-inside-avoid">` with:
  - a header as in 13 (`flex items-start justify-between border-b-2 border-gray-900 pb-3 mb-6`);
  - parties (`grid grid-cols-2 gap-8`);
  - an items table (`w-full border-collapse text-sm`, rows `border-b border-gray-200`, cells `py-2`, amounts `text-right tabular-nums`);
  - totals (`flex justify-end`, `w-72`).
- **After it:** `<p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
- **Classes:** every one was checked in the committed `tailwind.css`. There are no `mt-[…]` classes, which is why the margin is in `<style>`.
- **Sizing:** in step 2, tune the row count so that `main` is 820–900 px tall in both Chrome and ours (between 744.6 and 1009).
- **The PNG must show:** page 1 empty, as in Chrome; page 2 the whole invoice from the content top, then the thank-you line. Today page 1 holds the header and the first rows, because the avoid box is split.
- **Asserts** (`fixtures.rs`):
  - 2 pages;
  - no word on page 1;
  - `Faktura`, the first and last row, `Celkem` and `Děkujeme` once each, all on page 2;
  - `Faktura` yMin within 2 pt of Chrome's;
  - no yMax past the content bottom.
- **Compare:** record main's per-page numbers in §7 (page 1 is expected to be large). Set the threshold to the worst page after the fix plus 1 point.

**Ours, red first.** Each test names Chrome's number from step 1.

- `tall_inline_box_line.rs`:
  - `f213e_q1_a_first_line_under_a_body_margin_moves_to_page_two`, M = 100 and 200;
  - `…_q1b` (M = 809, `B(300)`);
  - `…_c10_in_a_section`.
- `body_offset.rs`:
  - `a_first_tall_image_under_a_body_margin_moves_to_page_two` (C4) and its `html{padding-top}` twin (C11);
  - `a_first_avoid_card_under_a_body_margin_moves_whole` (C5);
  - `…_when_the_offset_is_its_own_collapsed_margin` (C6).
- Guards, green on main:
  - a first block that fits stays on page 1;
  - a block taller than a page is not moved (C12);
  - `break-before:page` on the first child gives no blank page (C7);
  - the C8 sizes;
  - body padding unchanged.
- `#[ignore = "F109"]`: C3 with `B(900)`.
- `page_bottom_warning.rs`: Q1 with M = 200 joins the "must not warn" list.

**Fork:**

- Lib tests with `run_pass` and `fragments_of`:
  - `page_strips_offset_above_and_may_move`;
  - `a_first_monolithic_block_under_a_body_margin_moves_to_page_one`;
  - `a_first_avoid_box_under_a_body_margin_moves_whole`;
  - `a_first_wrapper_whose_first_row_does_not_fit_moves`;
  - `a_first_box_taller_than_a_page_stays_on_page_zero`;
  - `a_forced_break_before_the_first_child_stays_collapsed`.
- One `render_smoke` case.

**Byte gates** (`RAYON_NUM_THREADS` 1 and 4; any unexpected change stops the work and a postmortem is written):

- Fixtures 01–43 plus 45: byte-identical except 45.
  - Tailwind preflight gives every fixture a body margin of 0. Only 07 (`h1.mt-8`) and 26 (`body{margin:15mm}`) have an offset, and their first child is small.
  - Step 1 confirms which fixtures have an offset (including 15, 27 and 37).
- The 7 corpus documents.
- `cargo test -p fulgur` and fulgur-vrt: the same failing set and identical `.actual.pdf` files, except cases listed and checked against Chrome.
- The F213 1440-case sweep: identical.
- `bench.py --check`.

**Sweep** (`out/scratch/f213e/sweep.py`):

- grid: offset source {body margin, collapsed child margin, html padding, margin + body border} × offset {8, 100, 200, 809} × first block {carrier 300/950, img 950, avoid card, 40-line `<p>`, 40-row table, section + carrier, img 1200} × before it {nothing, zero-height `counter-reset` div};
- every changed case must be on Chrome's pages, and none may move farther from Chrome.

## 5. Steps

1. **Rebuild and measure (no code).** Build the §2 probes in Chrome and on main, plus a throwaway fork lib dump of `first_offset` for Q1 and the fixtures. Record which hypothesis holds in §7. If Q1 does not reproduce, stop and re-plan.
2. **Red tests and fixture.** Our red tests and guards; fixture 45 with its `render.mjs` reference and README; commit.
3. **Fork, rules.** `offset_above` / `may_move` with their unit test; the five rules and the `keeps_whole` / leading `above`; lib tests; clippy, fmt.
4. **Fork, full suite.** `cargo test -p fulgur` and fulgur-vrt. Re-bless only the goldens Chrome confirms, and name each; `render_smoke`.
5. **Bump and gate.** Bump `Cargo.lock`; our tests; byte gates; sweep; bench.
6. **Finish.**
   - `render-fixtures.sh` and `compare.py`; look at 45's side-by-side.
   - Run `vrt-validator`. No `pdf-reviewer`: this is layout only.
   - Findings: F213e done, its `:first` bullet moved to F109 with C3, follow-ups filed. Update `docs/upstream.md`; write §7.

## 6. Risks and non-goals

- **fulgur's UA `body{margin:8px}`.** Every fork test and golden has `first_offset = 8`. A first block between `bottom(0)` and `fresh + 1` tall now moves and leaves page 0 blank. Example: the `gcpm_snapshot` 294 px spacer on a 293.33 px page, if it is body's first child. C8 tells whether Chrome agrees. If Chrome keeps such blocks in place, compare against `chrome_bottom(0)` in the offset-only branch, and do not loosen the slack.
- **A blank first page** is Chrome's output, and it is better than lost content. Fixture 45's README says so. Margin boxes, `counter(page)`, link and bookmark targets move to page 2 as in Chrome. Check with C9 that the body's frame and background on the blank page match Chrome (F37 slices).
- **A small offset** (07, 26) can now move a first block that it pushes past the page bottom. That is intended.
- **Non-goals:**
  - `@page :first` / `:left` / `:right` page heights (F109; sketch: `PageStrips{first_h, rest_h}`);
  - body as the inline root (F111 `fragment_body_inline_root`);
  - first blocks taller than a page (F213b, F29b);
  - keep-with-next of a first heading (F215, `keep_size`);
  - relaxed avoid boxes;
  - nested first-child moves inside a wrapper that is not moved;
  - forced breaks at the start (C7 → F213e-b if Chrome differs);
  - flex rows (F213f);
  - `vertical-align` (F183c).

## 7. Evidence

### 7.1 Steps 1 and 2: measurements (2026-11-03)

**Method.** Chrome 153.0.8010.12 (m153), Playwright `page.pdf({ preferCSSPageSize: true, printBackground: true })` with print media (the settings of
`scripts/reference/render.mjs`); ours is main `efdd02a` (the F198j binary). `pdftotext -bbox` for words (yMin in pt, page); `pdftoppm -r 72` red pixels for
the image. Document: `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}`. `B(h)` is the F213 150 px wide `inline-block` of height `h`
with a block child `BOXTOP`; `IMG(h)` is a block `<img>` of the 60 px red square stretched to 100 x h px; a card is `break-inside:avoid;border:1px solid #000`
around 45 lines of 20 px (900 px). Probe scripts, HTML and PDFs are in the author's scratch directory (not committed); the committed form of each case is a test.
`first_offset` was not dumped from the fork (no fork edit this round, no scratch build): it is read from the output instead (below).

**Hypothesis.** H1 holds; H2 is F109 as planned; H3 is refuted. For each offset source (body margin, margin collapsed through body, html padding, html border, UA
`body{margin:8px}`) a 60-line paragraph puts exactly the lines on page 1 that Chrome does (40 lines for 200 px, 50 for 8 px; `f213e_g_a_first_paragraph…`),
so `first_offset` is in `bottom(0)` for every source. The same offset does not reach the move rules: a monolithic first block hangs for all four real
sources, and `body` padding or border (inside the body, `location.y > 0`) already moves it like Chrome (C9, identical page 1 including the body's border and background).

**Chrome / main (efdd02a), page of each thing and yMin:**

| Probe | Chrome | main |
|---|---|---|
| Q1 M=100: `<p>START B(950) TAIL</p><p>AFTER</p>` | p1 empty; START, BOXTOP, TAIL 41.7 and AFTER 754.9 on p2 | START at 819.8 on p1 (ends 832.6, the validator's number); AFTER p2. No warning |
| Q1 M=200 | the same as M=100 | lost, warning "63.1 pt past the bottom page edge"; AFTER p2 |
| Q1b M=809, B(300) | p1 empty; START 41.7, AFTER 267.4 on p2 | lost, 32.4 pt |
| Q1 M=0 / M=100 with B(300) (fits) | one page, START 41.7 / 116.7 | one page (F183c y offset) |
| C4 M=100, IMG(950) first | p1 empty; image 42 pt on p2; AFTER 753.4 | image on p1 from 117 to 830 pt (box ends 800) |
| C11 `html{padding-top:100px}` / `html{border-top:100px solid}` + C4 | the same as C4 | the same hang |
| C6 image with `margin-top:100px` collapsing through body | the same as C4 | image on p1 from 192 pt to 841 |
| C5 M=200, card of 45 `<br>` lines first | **not moved**: lines 1-40 on p1 (first at 191.7), 41-45 + AFTER on p2 | lines 1-43 on p1, 42 and 43 below the content box, 44 and 45 lost (22.6 pt) |
| C6 card with `margin-top:200px`, no body margin | the same 40 / 5 split | the same hang |
| card of 45 block children (`div` or `p`), bare or in `<section>`, M=200 | 40 / 5 split | 40 / 5 split (same) |
| card as `display:flex` / `grid` / `<table>` first | 40 / 5 split | 40 / 5 split (same) |
| card as `inline-block;width:100%` first | monolithic: p1 empty, all 45 lines on p2 | split in place 40 / 5 (enters it) |
| a zero-height `div` before the card, M=200 | **moved whole**: p1 empty, all 45 lines on p2 | hang as C5 (`cursor_y` stays 0) |
| a 2 px `div` before the card | moved whole | moved whole (same) |
| C7 M=100, `break-before:page` on the first `<p>` | one page, START 115.9, no blank page | one page, START 118.6 (same) |
| C8 `body{margin:8px}`, IMG 1001 / 1002 px | stays on p1 (image 48 pt), AFTER p2 40.9 | same |
| C8 IMG 1002.5 / 1003 / 1004 / 1008 px | p1 empty, image on p2, AFTER on p3 (3 pages) | image stays on p1 hanging 2-7 px below the content box, AFTER p2 |
| C9 body `margin-top:95px;border-top:5px`, IMG(950) | p1 empty but for the body's blue border and background; image p2; AFTER 753.4 | the same (already moves) |
| C10 `<section>` around Q1, M=100 / 200 | as Q1 | START 819.8 on p1, warning "ends 46px below the content box" / lost |
| C12 M=100 IMG(1200), and B(1200) M=100 | p1 empty; the box starts at the top of p2 and runs to p3 (3 pages) | image on p1 and p2 (clipped), 2 pages; B(1200) lost (warning) |
| C12 M=0 IMG(1200) | starts on p1, continues on p2 | on p1 and p2 (clipped) |
| C3b `@page:first{margin-top:100mm}`, `<p>START B(900) TAIL</p>` | **not moved**: START 282.4 on p1, the box runs on to p2, AFTER 201.4 on p2 | lost (116.6 pt past the edge) |
| C3a `@page:first{margin-top:100mm}`, 300 px spacer, B(300) | fits: START 507.4 on p1 | START 723.3 (ignores `:first`, F109) |
| 45 lines paragraph M=200 / 120 lines M=100 / 40 one-row table M=200 | split 40+5 / 45+50+25 / 32 rows + 8 | the same |

**What contradicts the plan.**

1. **C5 and C6 (the avoid card).** The plan predicted "moved whole". Chrome splits a first avoid box in place. css-break-3 §4.1: a Class A break point lies between
   siblings; with nothing before the box there is none, so the `avoid` has nothing to act on. With any sibling before it (even a zero-height one) Chrome honours the avoid
   and moves the box. A monolithic box (`<img>`, `inline-block`) is moved because it cannot be split. So the `keeps_whole_at` / F28-leading change in section 3
   (`content_above || offset_above`) is wrong for a box with nothing before it: it would send block cards, which we already split like Chrome (40 / 5), to page 2.
   The `offset_above` gate belongs on **monolithic** boxes only (the plain advance `:4312` for carriers and images). What is red for cards is narrower: an avoid box of
   *inline text only* (a paragraph-like box, `:3978` kept-whole path) hangs and loses lines, where Chrome splits it 40 / 5; and a zero-height sibling before a card
   (Chrome: moved, ours hangs because `cursor_y` is 0).
2. **Fixture 45 as planned.** `<main class="break-inside-avoid">` as the first block is not moved by Chrome (207 words stay on page 1, split at the 14th row) and ours splits it
   the same way, so it would be green. Fixture 45 therefore wraps the invoice in `inline-block w-full` (plus `break-inside-avoid`), which is monolithic: Chrome page 1 empty.
3. **C12 is not a guard.** A first image (or box) taller than a page under an offset is moved by Chrome (page 1 empty, then sliced on pages 2 and 3). Leaving it in place
   (`size <= fresh() + 1.0` in `may_move`) is the F213b / F29b non-goal, not Chrome's behaviour. Recorded as an `#[ignore]` test with Chrome's numbers.
4. **C3 (`@page :first`).** Chrome does not move the box (no content above it) and slices it across pages 1 and 2; it is not an "offset moves it" case at all. Ignored test
   `f213e_c3_…` (F109, with the F213b slicing).
5. **C8, the decisive probe.** Chrome keeps a first block in place while its bottom is at most 1010 px (8 + 1002), i.e. `chrome_bottom(0)` = `ceil(bottom(0))` of F30f-b,
   and moves it from 8 + 1002.5. It does not keep blocks that overflow only because of the UA 8 px margin: a 1003 px image moves and leaves page 1 empty. So the move test for
   the offset-only branch is `cursor + size > chrome_bottom(0)` (not `bottom(0)`, which would move the 1002 px image). Guard `f213e_g_c8_…` pins this; `f213e_c8_…` (1003, 1004, 1008) is red.
   The risk in section 6 (a first block between `bottom(0)` and `fresh + 1` leaving page 0 blank) is therefore what Chrome does, for monolithic blocks.
6. **C7** matches the plan: Chrome ignores a forced break with nothing before it, no blank page; ours is the same. Guard.
7. **Q1 shape.** The validator's numbers reproduce exactly (819.8 / 832.6 pt, and the 63 pt warning) with the 950 px carrier; the C10 `<section>` shape hangs rather than loses at M=100 and
   warns "ends 46px below the content box".

**Which fixtures have a non-zero first offset.** Measured in Chrome (body `getBoundingClientRect().top` in print media, 01-45): only **07** (32 px: first child `h1.mt-8` collapsed through body)
and **26** (56.7 px: `body{margin:15mm}`). 15, 27 and 37 are 0 (preflight or an explicit `margin:0`); 19's first child is inline text, offset 0. Fixture 45 has 264.6 px (70 mm).
So the byte gate of section 4 holds as planned: 01-43 should be identical, 07 and 26 can only change if their first block does not fit, which it does.

### 7.2 Step 2: tests and fixture 45 on main efdd02a

Fixture `fixtures/invoices/45-letterhead-keep-together/`: `main.inline-block.w-full.break-inside-avoid` of 843 px (header, parties, 14 rows, totals) under `body{margin-top:70mm}`, then the thank-you line;
Chrome reference by `render.mjs` (2 pages, page 1 empty). **Before: 2 pages, pixel diff 4.3 % / 5.5 % (worst page 5.5 %)**: ours splits the box (page 1: header and 13 rows, "Faktura" at 241.8 pt;
page 2: the 14th row and the totals). `compare.py` threshold set to 6.5 % (before + 1 point), to be lowered when the fix lands.

Red on main (the fix turns them green): `body_offset.rs` `f213e_c4_…`, `f213e_c11_…`, `f213e_c6_a_first_tall_image_…`, `f213e_c8_…`, `f213e_c5_…`, `f213e_c6_a_first_avoid_card_…`,
`f213e_c5z_…`; `tall_inline_box_line.rs` `f213e_q1_…`, `f213e_q1b_…`, `f213e_c10_…`; `page_bottom_warning.rs` `warns_about_content_below_the_page_only` (the new Q1 M=200 entry warns 63.1 pt);
`fixtures.rs` `letterhead_invoice_moves_whole_to_page_two` (page 1 prints 198 words).
Green on main (guards): `body_offset.rs` `f213e_g_a_first_image_that_fits_…`, `f213e_g_c5_a_card_after_a_2px_box_…`, `f213e_g_c5_a_first_avoid_card_of_blocks_…`, `f213e_g_a_first_paragraph_taller_…`,
`f213e_g_c8_…`, `f213e_g_body_padding_and_border_…`; `tall_inline_box_line.rs` `f213e_g1_…` (fits), `f213e_g2_…` (C7), `f213e_g3_…` (no offset).
Ignored (`#[ignore]`, red when run): `f213e_c12_…` (F213b / F29b), `f213e_c3_…` (F109).
