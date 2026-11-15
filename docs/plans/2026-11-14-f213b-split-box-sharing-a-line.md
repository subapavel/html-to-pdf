# F213b: a box taller than a page that shares its line is split, and the line is painted where its baseline is

Date: 2026-11-14 · Finding: F213b (with F213d-b; F222 and backlog B23 measured, not targeted) · Unblocks: F217 (PR #95, §7.9 case 2)
Base: html-to-pdf main `b8174c6`; fulgur fork `htmltopdf/srgb-colours`, main's lock `7dad143b`. The local branch head is now
`7626f400` (F213e-c's ADR-0012 revert of F217's `99a8401e`, not pushed; `origin` is still at `99a8401e`). The function
names below are from that tree; line numbers move (the checkout is shared), so none are given.

## 1. Problem

An atomic inline box (`inline-block`, `inline-table`, `inline-flex`, `inline-grid`) that is taller than a page and shares
its line with text loses words. The text on the line sits at the line's baseline. For an `inline-block` that is the
baseline of the box's last line box (CSS 2.1 §10.8.1), so it is usually on the box's last page. We paint the line once,
on the page where the box starts (`record_carrier_slices_after`, `render::carrier_line_page`), so the baseline lands
below that page's edge and is printed nowhere. A one-line carrier at least has its box fragmented: the walk enters it and
`box_continues_on` paints the later pages. A paragraph of several lines has nothing: `record_split_inline_root_descendants`
records the box with one `record_descendant` on the page of its line, so everything below that page is lost too.
Today's repro (main and the F217 head are byte-identical here, F217 plan §7.9):
`<div style="width:300px">Lead words before <span style="display:inline-block;width:100px"> 130 × <span style="display:block">Tnnn</span> </span> trailing words after the box in text.</div>`
prints `T000`..`T052` and none of "Lead words before … trailing". With F217's correct flex-item sizing, the same shape
loses words in six `rvw/sh` documents (`flex110`, `flexwrap`, `minw0`, `flexalign` `__tallcontent`,
`inlflexc__tallcontent`, `flexalign__tallh`). Chrome fragments the box over the pages and prints the line's text with
the slice that holds the baseline.

## 2. Spec

### 2.1 References

Not re-checked against the published text: unverified. Step 1 asks `css-spec-researcher` to quote them exactly.

- **css-break-3 §4.1 (monolithic).** Line boxes have no break point inside them. Inline-blocks and other inline-level
  boxes that establish an independent formatting context may be monolithic, "that is, in the cases where a single line
  box is too large to fit within its fragmentainer even by itself and the UA chooses to split the line box, it may
  fragment such boxes." This licenses F213b: the line may be split and the box fragmented. The same section lets
  paged media slice monolithic content that is taller than the fragmentainer (Chrome's choice, ADR-0004 Context).
- **css-break-3 §3.3 / §4.4.** `orphans` / `widows` and the Class B breaks inside the box (ADR-0004, ADR-0010).
- **css-break-3 §5.4.** `box-decoration-break: slice` for the box's frame (F96, as for an alone box).
- **CSS 2.1 §10.8.1.** The baseline of an `inline-block` is that of its last in-flow line box, or its bottom margin edge
  when it has none or `overflow` is not `visible`.
- **css-inline-3 `baseline-source`, css-align-3 §9.1.** `auto` means the last baseline for `inline-block` and the
  first baseline otherwise. So an `inline-flex` / `inline-grid` puts the line's text on the page of its first baseline.
- **css-overflow-3 §2.** Overflow towards the block-start side of the page is not reachable. Chrome cuts
  `flexalign__tallh` that way, and F217 §7.9 records that our PR prints exactly Chrome's 7 words.

### 2.2 Chrome m153

**Known.** All A4, 15 mm margins, Inter 14/20.

- F213b probe `S<p>START B(1200) TAIL</p><p>AFTER</p>`: `START` `BOXTOP` `TAIL` on page 2, the rest of the box on page
  3, 3 pages. Without `S`: page 1, 2 pages. This was recorded before F183c.
- F213d-b, the same box alone: `BOXTOP` at the top of page 1, the box sliced over pages 1 and 2, `AFTER` at 184.9 pt on
  page 2.
- F217 §7.9, Chrome word counts: 171, 143, 143 (`flex110`, `flexwrap`, `minw0` `__tallcontent`), 131 (`flexalign`),
  144 (`inlflexc`), 7 (`flexalign__tallh`).

**To measure in step 1.** Use Playwright with `/opt/pw-browsers/chromium` (record `--version`), Inter via `@font-face`
from `fixtures/fonts`, and `pdftotext -bbox` for each word's page and y.

| Probe | Shape | Record |
|---|---|---|
| P1 | `<p>START <ib 130 block children> TAIL</p><p>AFTER</p>`, at a page top and after `S` | pages of START / T000 / T129 / TAIL / AFTER; START and TAIL y against T129's baseline; T words printed twice |
| P2 | the same with `height:1200px` and one child `BOXTOP`, sharing the line and alone (F213b, F213d-b) | page and y of START, BOXTOP, AFTER |
| P3 | the 300 px `div` of §1, at a page top and after 3 paragraphs; the tall line 1st and 3rd of 4 | the page each line starts on; y of the line after the box |
| P4 | `<ib>L01<br>…L80</ib>` sharing its line | duplicated lines at the page edges; START page |
| P5 | `inline-flex` (column, 130 items) sharing its line | START on the first page (first baseline)? |
| P6 | a short badge with `vertical-align: top` / `middle` on the tall line | page and y of the badge |
| P7 | F222 `later_short_first_30`, B23 `four_staggered` (record only) | pages of every box |
| P8 | the P3 shape in a `border p-4` card | frame on each page; pages |

## 3. Where it belongs

Fulgur fork, pagination and render. No Blitz, Parley or Taffy change: the box sizes and the Parley line (baseline,
box position) are already right since F183c and F217. Nothing changes in `crates/htmltopdf` except tests and the fixture.

### 3.1 Design choice

- **A. A multi-page line fragment.** The line stays one unit in `plan_line_breaks`. Its range spans pages and its slice
  sits on the last page at a negative y, and the box is painted from that one placement.
- **B. The tall line is a split point (chosen).** The paragraph is planned in segments: the lines before the tall line,
  then the tall line, then the lines after it.
  - The boxes on the tall line are fragmented by the block walk (`fragment_block_subtree`): between their children,
    and (§3.2 step 5) between line boxes.
  - The line's glyphs and the short items aligned to its baseline are painted once, on the page that holds the
    baseline.
  - The next segment starts below the line's end, on the box's last page.

Why B:

- The page where a box ends depends on the pushes inside it. Pushes are known only after the walk has fragmented it,
  and A would make `plan_line_breaks` predict them from heights. That is the double derivation F78 removed ("deriving
  the range a second time from float heights drifts").
- B reuses the alone-box machinery as it is: the F30f split, `box_continues_on`, and the frame closed per page (F96).
  ADR-0004's "one place decides line ranges" stays true.
- B prints every word once. A, painted per page and clipped, is Chrome's slicing, which ADR-0004 rejects (a line cut
  in half and printed twice).

The engineer's "negative y" becomes a **lead on the slice**, not a negative `Fragment.y`. `draw_shaped_lines` starts each
line's top at the `y` it is given, so the line's top must start above the fragment. Fragments also drive frames, link
rectangles, tag boxes, the F111 extent and the F213d ledger. A negative fragment would leak into all of them; a slice
field touches only the line painter.

### 3.2 Changes (signatures only)

- **`LineSlice`.** Add `pub lead: Px` (derive `Default`): how far above the fragment's line origin the slice's first
  line starts. It is 0 except on the slice that holds a tall line.
  - `fn line_slices_tile(slices, n)` (or a sibling `line_slices_tile_with_gaps`) accepts empty slices `k..k` on pages a
    tall line crosses.
  - `fn paragraph_line_range_for_page(..) -> Option<(Range<usize>, Pt, Pt)>` also returns the lead. The paint site
    passes `frag.y + inset - lead` to `draw_shaped_lines`. `rebase_lines` is unchanged.
- **`fn box_continues_on(geometry, drawables, ib_id, page)`** becomes "has a fragment on `page`, and `page` is not the
  carrier's line page (`carrier_line_page`, falling back to the box's first page)". Today a box on a page before the
  line page is painted by no one.
- **`fn baseline_on_page(geometry, doc, box_id, baseline_in_box: f32) -> Option<(u32, f32)>`.** Walks down
  `layout_children` to the deepest recorded node whose box-local span contains the baseline. It returns that node's page
  and page-local y (body-relative px), going through its line slices when it was split. When the baseline is past the
  last child (§10.8.1, bottom margin edge), it uses the box's last fragment bottom.
- **One-line carriers.** `fn record_carrier_slices(geometry, carrier_id, line_page: u32, lead: Px)`.
  `record_carrier_slices_after` takes `line_page` from `baseline_on_page` of the box that sets the line's baseline (the
  tallest box on the line), not "the earliest first page".
- **Multi-line carriers.**
  - `fn tall_line_boxes(doc, carrier, line: usize, fresh: f32) -> Vec<usize>`: the atomic inline containers on a line
    taller than `fresh`.
  - `fn fragment_tall_line(geometry, doc, env: McEnv, carrier_id, line, page_in, y_in, strips, depth) -> TallLineEnd`,
    with `struct TallLineEnd { baseline_page: u32, lead: f32, end_page: u32, end_y: f32 }`. Each tall box starts at the
    line's own start, so boxes are not stacked.
  - `fragment_nested_inline_root` gains `env: McEnv` (its four callers have it) and loops over the segments.
    `plan_line_breaks` keeps its contract for one segment. A tall line that is not first in its segment starts a fresh
    strip, which is what the greedy planner already does and what Chrome does (P3). As the first line of its paragraph
    it starts in place, as ADR-0004 does for an alone box.
  - `record_split_inline_root_descendants` skips the boxes `fragment_tall_line` recorded. Other items on the tall line
    are placed relative to the baseline page.
- **Barrier (step 5).** `box_barrier`: an `inline-block` or `inline-table` that shares its line and is taller than a
  fresh page is no `SharesItsLine` barrier, so its paragraphs are split between their lines. `inline-flex` and
  `inline-grid` keep the barrier main gives them: that is 99a8401e's hunk (§3.4).

### 3.3 ADR-0004 amendment (text to add)

> **Amended 2026-11-xx by F213b: a box taller than a page that shares its line is split, and the line is painted where its
> baseline is.** css-break-3 §4.1 lets a UA that splits a line box too tall for a fragmentainer fragment the inline-level
> boxes on it. An atomic inline that shares its line and is taller than a fresh page is fragmented like a box alone on its
> line: between its block children, and for an `inline-block` / `inline-table` between its line boxes (orphans and widows
> as above, each line once). The rest of the line is painted once, on the page that holds the line's baseline. That is the
> box's last in-flow line box for an `inline-block` (CSS 2.1 §10.8.1) and its first baseline for an `inline-flex` /
> `inline-grid` (css-inline-3 `baseline-source`). Lines of the paragraph after it start below the line's end. Chrome m153
> puts the text on the same page (plan §7, P1–P5). A tall line that follows other lines starts a new page, as in Chrome. A
> tall line that is the first of its paragraph starts where it is (as A3 above), where Chrome moves it to the next page.
> The sentence "an `inline-block` that shares its line … are not split (F30f-b)" now covers only boxes that fit a fresh
> page, which F213 moves whole. F213d-b's box (taller than a page, a short child) is now split, not lost. Replaced
> elements taller than a page (F198l, F29b) are not covered.

### 3.4 Sequencing with the fork (ADR-0012)

- **State.** 99a8401e (F217, unmerged PR #95) is already reverted on the local branch by F213e-c (`7626f400`, unpushed).
  F213b's fork commits start **after F213e-c merges**. Starting earlier would make F213b revert F213e-c's unmerged
  commits as well (ADR-0012 item 1).
- **Default: F213b is its own PR on main's tree.** F217 then re-applies 99a8401e on F213b's head (ADR-0012 item 3) and
  re-measures its six documents. The reasons:
  - Merge-policy item 4 and ADR-0012 ("B cannot take A's engine change") forbid the alternative without an amendment.
  - 99a8401e has been gated only together with F217's Blitz commits (§7.9), never alone.
  - F213b's own acceptance does not need it: the six documents need F217's Blitz change anyway, so they are measured on
    a combined local build either way (§4).
- **Exception (F213b's PR carries the re-apply of 99a8401e).** Only when both of these hold:
  - **M1:** a build of main's lock with only 99a8401e added (Blitz at main's `2fd60f3f`) has, against main, identical
    bytes for fixtures 01–48 and the 7 corpus documents, and 0 documents with fewer words over `rvw/sh`, `rvw/sw`,
    `rv/var`, `rv/var2` and the F213d / F213e families;
  - **M2:** in a scratch clone, `git revert 7626f400` on top of F213b's last fork commit has a textual conflict (F213b
    edits `box_barrier`, as 99a8401e does).

  Then F213b's PR re-applies it as its first fork commit, records it in `docs/upstream.md` as "F217's 99a8401e,
  re-applied by F213b (M1, M2)", amends ADR-0012 with that rule, and F217's plan drops the commit from its scope. If M2
  is clean, the default holds whatever M1 says. ADR-0004's text conflicts with F217's amendment as well; F217 resolves
  that when it re-lands.

## 4. Acceptance

**Red tests first** (`crates/htmltopdf/tests/tall_shared_line.rs`). Each test names its probe and Chrome's numbers.

- `t1` (P1, one-line carrier): every word once. START and TAIL are on T129's page, with baselines within 0.5 pt of
  T129's. AFTER comes after it.
- `t2` (P3, multi-line, tall line first): "Lead words before" and "trailing" are level with T129 (Chrome keeps them on
  the box's line, §7 P3), and AFTER is below the div's second line, on the same page.
- `t3` (P3, tall line 3rd of 4): it starts on a fresh page, as Chrome does.
- `t4` (P2, F213d-b): START, BOXTOP and TAIL are on the box's first page, and AFTER is after the box's end. If step 1
  finds main already prints these words, `t4` stays as a guard.
- `t5` (P4, text lines inside the box): every L once (step 5).
- `t6` (P5b, `inline-flex` sharing its line, one item at the bottom of a 1200 px box): START and TAIL on the page of the
  item (the first baseline). The column of 130 items (P5) holds on main already and is the guard `g_t6_…` (§7).
- **Guards:** the F213 shape (a box that fits, moved whole), the F30f alone box and the F183c two-box cases in
  `alone_inline_block_line.rs` stay byte-identical.
- **`page_bottom_warning.rs`:** the F213b and F213d-b "must warn" entries move to "must not warn".
- **Ignored tests:** `f213e_c12_…` is un-ignored if green; otherwise it stays ignored, its comment naming only F29b.

**fulgur lib tests:** the segment planner on `(top, bottom)` lists with a tall line; `line_slices_tile` with gaps;
`box_continues_on` for a box whose fragments come before the line page; `baseline_on_page` through a pushed child and
through a split descendant; a smoke test in `render_smoke.rs`.

**Fixture 51** (`fixtures/invoices/51-terms-box-beside-label/`, Czech content):

- **Content.** An invoice whose "Obchodní podmínky" paragraph has two lines of text. Its third line holds `Souhlasím:`,
  an `inline-block` of about 70 numbered terms (block children, bordered, about 1450 px) and `Podpis odběratele ____`.
  A closing line and a footer follow.
- **The PNGs must show:**
  - page 1: the invoice and the two intro lines;
  - page 2: the box's top frame and the first terms, the frame open at the bottom;
  - page 3: the remaining terms and the bottom frame, with `Souhlasím:` and `Podpis` level with the last term, then the
    closing line and the footer.
- **Against Chrome.** Chrome has the same 3 pages (P3) and differs only by the term it prints twice at each page edge.
  Its word count is Chrome's minus those duplicates. The threshold is the measured value plus 1 point. Commit the Chrome
  reference.

**Gates:**

- **Byte gate.** Fixtures 01–48 and the 7 corpus documents, `RAYON_NUM_THREADS` 1 and 4: identical to main. Nothing in
  them has a line taller than a page, so the expected pixel-diff change is 0.
- **No word lost.** Against main, over `rvw/sh` (455), `rvw/sw` (48), `rv/var` (66), `rv/var2` (38) and the F213d (p/dv)
  and F213e (p/ev) families: **0 documents with fewer words.** The criterion is the multiset of `pdftotext` words, with a
  word main prints twice counted once (the F183c §7.7 ghost words).
- **Combined local build** (F213b + F217's Blitz `f9b6bc04` + 99a8401e, path patches, not committed): the six F217
  documents have at least main's words. Target: Chrome's 171 / 143 / 143 / 131 / 144. `flexalign__tallh` has Chrome's 7
  words (negative overflow, §2.1), the one listed exception.
- **Other checks:** `cargo test`, fmt and clippy (here and in the fork); fulgur lib tests; the same failing set in
  `fulgur-vrt`; `bench.py --check`; `vrt-validator`. `pdf-reviewer` is not needed, because the writer path is untouched.

## 5. Steps (each ≤ 1 day, each green on its own)

0. **Preconditions.** Confirm F213e-c has merged and branch from its head. Run M1 (in parallel, any time) and record the
   §3.4 decision.
1. **Probes and red tests.** Run P1–P8 and the spec quotes into §7. Re-measure the F213b / F213d-b probes on `b8174c6`
   (pre-F183c notes). Write the red tests. No engine change.
2. **Render groundwork.** `LineSlice::lead`, the tile check with gaps, the lead in `paragraph_line_range_for_page` and
   the paint site, and the new `box_continues_on`. Every lead is 0, so the byte gate must be identical. Lib tests.
3. **One-line carriers.** `baseline_on_page`, and `record_carrier_slices` on the baseline page with its lead. Turns
   `t1`, `t4` and `t6` green.
4. **Multi-line carriers, in two parts.**
   - 4a: the segment planner and `TallLineEnd`, pure functions with lib tests.
   - 4b: `fragment_tall_line`, `McEnv` threaded into `fragment_nested_inline_root`, and
     `record_split_inline_root_descendants` skipping the tall boxes. Turns `t2` and `t3` green.
5. **Barrier.** No `SharesItsLine` for a tall `inline-block` or `inline-table`. Turns `t5` green. Run M2 here.
6. **Warnings.** Update `page_bottom_warning.rs`, the F30f-b barrier text and the F213d ledger. Check that no tall line
   triggers the "no matching line ranges" warning.
7. **Fixture 51.** Chrome reference, `compare.py`, the threshold; look at the PNGs.
8. **Gates and docs.** All §4 gates and the combined measurement. Docs: ADR-0004 amendment, findings (F213b and
   F213d-b done; F222 / B23 as measured), `docs/upstream.md`, §7.

Size: medium-large, about 8 engineer days.

## 6. Risks and non-goals

**Risks:**

- **The baseline lookup.** It relies on the recorded descendant geometry. A baseline inside a nested paragraph that is
  split by lines, or under a clip, needs the slice offset. Covered by lib tests.
- **Painting a fragment twice or not at all.** `check_continuations_painted` catches a missing one in debug builds.
  Add the matching check for a box painted twice on its line page.
- **The lead and line-relative items.** It must move them too: inline boxes' `computed_y` is line-relative, and so are
  link and tag rectangles, clip bands and the F111 note. `t1` and `t2` check the y values.
- **The moving shared checkout and the F217 re-apply conflicts.** Expected in `box_barrier` and ADR-0004; handled in
  §3.4.

**Non-goals:**

- Chrome's move of a first-line tall line to the next page (ADR-0004 A3).
- Slicing a line in two.
- Short items with `vertical-align: top` on a tall line: placed from the baseline page, warned if they fall above it;
  P6 records Chrome.
- F222 and B23: one-line carriers keep the F183c walk, which stacks the boxes. They are measured and left open unless
  they fall out of the work. Moving one-line carriers onto the segment path is a later PR.
- Replaced elements taller than a page (F198l, F29b).
- F213e-b (fixture 49), F30h (fixture 50), F217 case 3 (`height: 50%`), F213g (`strict_layout`).

## 7. Evidence

Filled in by the implementing agent on 2026-11-14 and 2026-11-15 (steps 0 to 3 on the 14th, 7.5 is step 3; steps 4 to 8 on the 15th, 7.6 to 7.10). Scratch files: the session scratchpad `f213b/`
(`probes/` for the probes, `main-b` and `m1-99a` for the binaries, `words2.py` and `bytegate.py` for the gates).

### 7.1 Step 0: preconditions and the M1 decision

- F213e-c has merged (PR #101). The branch is main `75e6b60` merged in; main's fulgur pin is `4be3de79` on
  `htmltopdf/srgb-colours` (the fork head, it contains `7626f400`). F213b's fork commits are appended to it.
- **Baseline binary.** `main-b` is a release build of this tree's lock. Its PDFs are byte-identical to the F213e-c binary
  that the earlier gates used (`rv/f213ec-new`) on the 47 fixture directories 01-48 and the 7 corpus documents (`bench/corpus`: 54 documents in all), at `RAYON_NUM_THREADS` 1 and 4.
- **M1.** `m1-99a` is main's lock (Blitz `2fd60f3f`, Parley `21d3c72e`, Taffy `7bbedd5b`) with the fulgur fork head
  `4be3de79` plus `99a8401e` cherry-picked (clean) in a scratch clone, built with a `--config` path patch; nothing
  committed. Results against `main-b`:
  - fixtures 01-48 plus the corpus (54 documents), RAYON 1 and 4: **0 documents differ** (byte-identical);
  - `rvw/sh` (455): 454 identical, **1 with fewer words**: `inlflexc__tallcontent` loses 77 distinct words (`T053` to
    `T129`, 3 pages become 2). 99a8401e splits an `inline-flex` column between its lines, and without F217's Blitz change
    its tall content is lost; this is the "gated only together with F217's Blitz commits" caveat of section 3.4;
  - `rvw/sw` (48): 43 identical, 5 with more words, 0 fewer; `rv/var` (66) and `rv/var2` (38): all identical;
  - F213d family (`f213dv/p`, 5893 documents), F213e family (`f213ev/p`, 2901; `f213e/p`, 74; `f213e/sw`, 256): all
    identical.
- **Decision (section 3.4): the default holds.** M1's criterion "0 documents with fewer words" fails on
  `inlflexc__tallcontent`, so F213b's PR is its own PR on main's tree and does not carry 99a8401e; F217 re-applies it on
  F213b's head and re-measures. M2 is moot because M1 failed (the exception needs both); step 5 does not need to run it.

### 7.2 Step 1: Chrome measurements

Chrome for Testing 153.0.8010.12 (`/opt/pw-browsers/chromium`), Playwright with `emulateMedia print`, `preferCSSPageSize`,
Inter from `fixtures/fonts` through `@font-face`, `pdftotext -bbox` (page, yMin in pt). A4, 15 mm margins, `body` 14px/20px,
`p` margin 0. Box = `inline-block`, 100 px wide, 1 px border, 130 block children `T000`..`T129` (2602 px). `S` = `<p>S</p>`.
Main = `main-b`. A word listed as "lost" is printed nowhere. Chrome ends a sliced-box document with a blank page (P1: 4 pages,
page 4 empty); the page counts below include it.

| Probe | Chrome | Main |
|---|---|---|
| P1 `<p>START box TAIL</p><p>AFTER</p>` page top | 4 pages. T000 p1 44.6; T129 p3 464.6; START and TAIL p3 464.6 (T129's baseline); AFTER p3 480.3; T050 printed twice | 3 pages; every T once (T000 p1 44.3, T129 p3 478.6); START, TAIL lost; AFTER p3 493.6 |
| P1 after `S` | S p1; the box and the line move to p2: T000 p2 44.6, START/TAIL p4 464.6, AFTER p4 480.3; 5 pages | S p1; T000 p1 59.3 (starts in place); START, TAIL lost; AFTER p3 508.6 |
| P1 box alone | T000 p1, T129 p3 464.6, AFTER p3 480.3, T050 twice | every T once, AFTER p3 493.6 |
| P2 `START box(height 1200, BOXTOP) TAIL`, AFTER, page top | START, BOXTOP, TAIL p1 44.6; AFTER p2 187.8; 2 pages | the three words p1 44.3 (F213d-b is not lost on main since F183c); **AFTER p2 43.6** (over the rest of the box) |
| P2 after `S` | S p1; START, BOXTOP, TAIL p2 44.6; AFTER p3 187.8; 3 pages (the plan's pre-F183c note holds) | the three words p1 59.3; AFTER p2 43.6 |
| P2 box alone, page top / after `S` | BOXTOP p1 44.6, AFTER p2 187.8 / BOXTOP p2, AFTER p3 187.8 | BOXTOP p1 44.3, AFTER p2 43.6 / BOXTOP p1, AFTER p2 43.6 |
| P2b same box, one child `BOXLOW` with `margin-top:1100px` (baseline on p2) | START, BOXLOW, TAIL p2 112.1, AFTER p2 187.8 (the margin is kept at the page edge) | BOXLOW p2 43.6, AFTER p2 58.6; START, TAIL lost |
| P3 `div(300px){Lead words before box trailing words after the box in text.}`, page top | T000 p1; `Lead` and `trailing` p3 464.6 (the same line as T129); line 2 480.3; AFTER p3 495.3; 3 pages + blank | T000..T052 p1; the rest of the box and every word of the div lost; AFTER p2 43.6 |
| P3 after `S1..S3` paragraphs | T000 p2 44.6; `Lead` p4 464.6; AFTER p4 495.3 (the tall line starts a fresh page) | T000..T052 on p2; the div's words lost |
| P3 tall line 3rd of 4 (`X1<br>X2<br>Lead..box..trailing<br>X4`) | X1 p1 43.8, X2 p1 58.8; T000 p2 44.6; `Lead`, `trailing` p4 464.6; X4 p4 495.3; AFTER p4 510.3 | X1, X2 p1; T000..T052 p2; `Lead`, `trailing`, X4 lost |
| P3 tall line 1st of 4 | T000 p1; `Lead` p3 464.6; X2 p3 495.3, X4 525.3; AFTER 540.3 | T000..T052; the rest lost; X2, X4 p2 43.6 / 73.6 |
| P4 `START box(80 text lines L01..L80) TAIL` page top | L01 p1 44.6; L51 on p1 and p2; L80 p2 472.1; START, TAIL p2 472.1; AFTER p2 487.8; 3 pages incl. blank | L01..L53 p1; L54..L80, START, TAIL lost; AFTER p2 43.6 |
| P4 after `S` | S p1; L01 p2; START, TAIL p3 472.1; AFTER p3 487.8; 4 pages | L01..L52 p1 (starts in place); the rest lost |
| P4 box alone | L01 p1, L51 twice, L80 p2 472.1, AFTER p2 487.8 | every L once, L80 p2 478.6, AFTER p2 494.3 |
| P5 `inline-flex` column of 130 items sharing the line, page top | START, TAIL, T000 p1 44.6 (first baseline); AFTER p3 480.3; T050 twice | START, TAIL p1 44.3; every T once; AFTER p3 493.6: **already right** (guard `g_t6_…`) |
| P5 after `S` | START, TAIL p2 44.6 (moved); AFTER p4 480.3 | START, TAIL p1 59.3 (in place) |
| P5b `inline-flex`, `align-items:flex-end`, height 1200, one item `FLEXITEM` | START, FLEXITEM, TAIL p2 172.1; AFTER p2 187.8 | FLEXITEM p2 43.6; START, TAIL lost; AFTER p2 58.6 |
| P5c the same with `flex-start` | START, FLEXITEM, TAIL p1 44.6; AFTER p2 187.8 | the three words p1 44.3; AFTER p2 43.6 |
| P6 badge (`inline-block` `BADGE`) after the box on the P1 line, `vertical-align` baseline / bottom / middle | BADGE p3 464.6 / 464.6 / 465.3, with START and TAIL | START, TAIL lost; BADGE p3 494.3 |
| P6 `vertical-align: top` | **BADGE p1 44.6** (the line's top), START and TAIL p3 464.6 | START, TAIL lost; BADGE p3 494.3 |
| P7 F222 `later_short_first_30` | A00..A50 p2, B00..B10 p3, A50..A100 p3, B11..B29 p4, A101..A119 and AFTER p4 (337.1); 4 pages | A words lost; AFTER p2 501.8 |
| P7 B23 `four_staggered` | p2 A00..A50 B00..B10; p3 A50..A100 B10..B60 C00..C40 D00; p4 A101.. B61.. C41.. D01..; AFTER p4 337.1 | 6 pages: A p1..p3, B p3..p4, C p5..p6, D p6; AFTER p6 501.8 |
| P8 the P3 shape in `border 1px; padding 16px` | 4 pages: p1 the empty open frame, p2-p3 the frame sides and the box, p4 closes the frame with `Lead`..`trailing` at 464.6, AFTER 495.3, END 523.1 | 2 pages, T000..T051 on p1, the rest lost |
| P8 after 3 paragraphs | 5 pages, T000 on p3, `Lead` p5 464.6 | 2 pages; 49 T words |

What the probes change in the plan:

- **P1, P2b, P5b** confirm the design: the text of the line sits on the page of the baseline (the last line box of an
  `inline-block`, the first baseline of an `inline-flex`), level with the baseline's own line (within 0.1 pt in
  Chrome). The tests compare our words with each other, because Chrome slices the box and prints a line twice at each
  page edge (ADR-0004) and keeps a margin at the page edge where we truncate it.
- **P3**: `trailing` is on the box's line, not on the line below it (the plan's t2 said "below", section 4 is corrected).
  The line after the tall line starts 15.7 pt below the baseline (T129 to the next line of the div).
- **P2**: F213d-b is not a lost-word bug any more (F183c fixed it); what is left on main is AFTER over the rest of the box
  (p2 43.6 pt, Chrome 187.8 pt). `t4` is red for that reason, not for START / BOXTOP / TAIL.
- **P5**: the `inline-flex` column already holds on main. The red `inline-flex` case needs the first baseline off the
  first page: `align-items: flex-end` (P5b). The column stays as the guard.
- **P6**: a `vertical-align: top` item on the tall line is on the box's first page in Chrome (the line's top), where the
  baseline page is the last one; the plan's non-goal stands (we place it from the baseline page, warned in step 6). Not
  tested.
- **P7**: main loses the A words of `later_short_first_30` (F222) and spreads `four_staggered` over 6 pages (B23);
  recorded, not targeted.
- **P8**: Chrome's card frame is sliced on every page it touches, the first page holding only the empty open frame;
  fixture 51's numbers (section 4) are measured on this shape in step 7.

### 7.3 Spec quotes (fetched 2026-11-14 from w3.org; the `css-spec-researcher` role was not available in this run)

- css-break-3 section 4.1 (Possible Break Points): "Since line boxes contain no possible break points, inline-block and
  inline-table boxes (and other inline-level display types that establish an independent formatting context) may also be
  considered monolithic: that is, in the cases where a single line box is too large to fit within its fragmentainer
  even by itself and the UA chooses to split the line box, it may fragment such boxes or it may treat them as
  monolithic." Section 2.1's paraphrase ended at "it may fragment such boxes"; the UA may equally treat them as
  monolithic, which is what ADR-0004's non-split did. Both are licensed, so the choice is ours.
- CSS 2.1 section 10.8.1: "The baseline of an 'inline-block' is the baseline of its last line box in the normal flow,
  unless it has either no in-flow line boxes or if its 'overflow' property has a computed value other than 'visible', in
  which case the baseline is the bottom margin edge."
- css-inline-3 section 4.2.1 (`baseline-source`): "auto: Specifies last-baseline alignment for inline-block,
  first-baseline alignment for everything else." The css-align-3 section 9.1 text (how the baselines of a flex or grid
  container are found) and css-overflow-3 section 2 (negative overflow) were not re-read.

### 7.4 The red tests (`crates/htmltopdf/tests/tall_shared_line.rs`)

Run with `cargo test --release -p htmltopdf --test tall_shared_line -- --include-ignored` on `main-b`'s tree; the red tests
carry `#[ignore]` with the step that turns them green. Reasons for failing on main:

| Test | Probe | Fails because |
|---|---|---|
| `r_t1_one_line_carrier_…` | P1 | `START` printed 0 times (every T is printed once) |
| `r_t1_the_baseline_is_where_…` | P2b | `START` printed 0 times, BOXLOW is on page 2 |
| `r_t4_f213d_b_…` | P2 | AFTER at 43.6 pt, Chrome 187.8 pt (the three words are printed already) |
| `r_t6_an_inline_flex_box_…` | P5b | `START` printed 0 times |
| `g_t6_an_inline_flex_column_…` | P5 | green on main (guard) |
| `r_t2_…` | P3 | `T053` printed 0 times |
| `r_t3_…` | P3 3rd of 4 | `T053` printed 0 times |
| `r_t5_…` | P4 | `L54` printed 0 times |

### 7.5 Step 3: one-line carriers

Fork commit `1defad04` (message "WIP", made by the session hook; on `c4ecd092`, itself on `4be3de79`; appended, no revert, no force). In `record_carrier_slices_after`
the line of a one-line carrier whose line has text (or an `<img>`) is painted on the page of the baseline of the tallest box
on it (`baseline_line_placement`: Parley's baseline of the line and top of the box give the box's own baseline,
`baseline_on_page` walks the box's `layout_children` and places the point in the deepest node that holds it, through its
line slices when it was split, else its fragments; a point below the last child is the bottom of the last fragment), with
`lead` = the carrier's content origin on that page less the line's top. A line that holds only boxes and white space stays
where F183c put it (no text to print), and so does a line whose baseline is on the first page of its boxes: both are
byte-identical to before.

- **Red tests now green:** `r_t1_one_line_carrier_text_sits_on_the_baseline_page_of_the_box` (P1, page top and after a
  paragraph: START and TAIL on page 3 at 478.6 pt, T129's y exactly; AFTER 493.6 pt), `r_t1_the_baseline_is_where_the_last_…`
  (P2b: START, TAIL, BOXLOW level on page 2), `r_t6_an_inline_flex_box_…` (P5b: START, TAIL, FLEXITEM level on page 2).
  Rendered page 3 of P1 and page 2 of P5b next to Chrome's: START and TAIL level with the last line, frame closed.
  P6 on the new binary: a `vertical-align: top` badge goes with START to the baseline page (Chrome: page 1; the non-goal).
- **t4 became a guard** (`g_t4_…`). Main prints START, BOXTOP and TAIL on page 1 already (F183c); what is wrong there is
  not the baseline but the box: a fixed 1200 px `inline-block` whose only child is short is never entered by the walk
  (`would_split_block_subtree` sees no overflowing child), so it is one oversized leaf that overflows page 1 (warning
  "content extends 102 pt past the bottom page edge"; 193 px of the frame are not drawn) and AFTER is at the top of page
  2 (43.6 pt; Chrome 187.8 pt). The plan assumed words were lost (pre-F183c). Not fixed here (it needs the oversized
  leaf sliced inside the carrier walk, outside section 3.2): recorded as backlog `B29` in `docs/backlog/stress-findings.md`.
- **Byte gate:** fixtures 01-48 and the 7 corpus documents (54), `RAYON_NUM_THREADS` 1 and 4: 0 documents differ from `main-b`.
- **No word lost against `main-b`** (`words2.py`, multiset of `pdftotext` words): `rvw/sh` 455 identical, `rvw/sw` 48
  identical, `rv/var` 66 (65 identical, 1 with more words: `text_around`, +5), `rv/var2` 38 (34 identical, 4 with more:
  `text_between_short` +2, `text_lead_trail` +6, `text_only_after` +3, `text_only_before` +2), F213d family 5893
  identical, F213e family `f213ev/p` 2901 (2897 identical, 4 with more: `fam3/a4_ib_text_40_200` +3, `a4_ib_text_8_200` +3,
  `a4_text_ib_40_200` +1, `a4_text_ib_8_200` +1), `f213e/p` 74 and `f213e/sw` 256 identical. **0 documents with fewer
  words** (so no FEWERWORDS case beyond the known 11 of the F213d family, which are identical to main). `rvw/sh` has no
  change because its shapes need F217's flex-item sizing (section 4, combined build, step 8).
- **Lib tests:** `cargo test -p fulgur --lib` 2569 passed (with main's Blitz / Parley / Taffy pins), 5 new here (the carrier
  line is on the baseline page with a lead; a line of boxes and white space stays; `baseline_on_page` through a pushed child,
  below the last child, and through a split inline root's slices), `render_smoke` 343 passed (one new). Fulgur clippy
  (`--all-targets -D warnings`) and fmt clean.

### 7.6 Step 4: multi-line carriers

Fork commit `d3376d76` (on `1defad04`, appended). The function names differ from section 3.2, which was written before the code:

- `fragment_inline_root_with_tall_lines` is the segment planner and `fragment_tall_box` fragments one box of a tall line; there is no
  `fragment_tall_line` / `TallLineEnd` (the end of the line is a local in the planner) and no separate pure planner: the lines before
  and after a tall line go through `plan_line_breaks` unchanged. `tall_lines(doc, carrier, fresh)` lists the lines that hold an atomic
  inline container taller than a fresh page (+ 1 px). `fragment_nested_inline_root` takes a `McEnv` and calls the planner first (its
  five callers: the multicol span, the body, the body-level paragraph with insets, the block walk, the abspos flow). The body-level
  paragraph without insets does not go through it (`fragment_inline_root` plans it from the line heights): a `tall_line_here`
  condition in front of that branch sends a paragraph with a tall line to `fragment_nested_inline_root` instead.
- A tall line that follows other lines of its paragraph moves to a fresh page (t3); the first line of a paragraph starts in place (t2).
  The slices are one per page: an empty slice (`k..k`) on the pages the line crosses before its baseline page, the line (and the lines
  after it when it ends on the same page) on the baseline page with its lead, an empty slice on the pages after it up to the page the line
  ends on, and the lines after the tall line on that page (when the baseline is on an earlier page, their slice has a negative lead: they
  start below the tail of the line, not at the top of the page). `record_split_inline_root_descendants` takes the boxes to skip.
- `render::carrier_line_page` finds the page of a box's line through the slice that holds the line (only for a carrier whose slices have
  an empty one or a lead, so every other document takes the old arm).
- **Red tests now green:** `r_t2_…` (P3, the line first: `Lead` / `trailing` level with T129, AFTER below the second line) and `r_t3_…`
  (the tall line 3rd of 4: T000 at the top of page 2, `Lead` on page 4 level with T129, X4 below). Numbers against Chrome (probes P3 in
  `probes/h`, ours / Chrome, pt): `Lead` 478.6 / 464.6, AFTER 509.3 / 495.3, the third-of-four X4 509.3 / 495.3, AFTER 524.3 / 510.3: the
  same 14 pt offset everywhere (Chrome slices the box and truncates a margin that we keep, section 7.2), and the same distances between
  the lines. Rendered pages 1 to 3 and 1 to 4 of P3 and the card (P8) look right: the line level with the box's last term, the card's
  frame sliced over the three pages (3 pages, Chrome 4 with an empty first page).
- B29 (the oversized leaf) did not get in the way: the tall boxes of all tests have block children that overflow, so the walk enters
  them.

### 7.7 Step 5: the barrier

Same commit. `box_barrier` (and `split_barrier`, `split_barrier_with`, `line_split_candidate`, which pass the new `fresh` down; every
caller has the page height) no longer makes an `inline-block` / `inline-table` taller than `fresh + 1` that shares its line a
`SharesItsLine` barrier; `inline-flex` / `inline-grid` keep theirs. A box that is an inline root (text lines) is then split between
its lines by `block_walk_line_split`, both in the walk of a one-line carrier (P4 after `S` and at the top) and in `fragment_tall_box`.
`r_t5_…` is green (P4: L01 to L50 on page 1, L51 to L80 on page 2, START and TAIL at 478.6 pt with L80, AFTER 494.3 pt; Chrome 44.6 /
472.1 / 487.8 pt, L51 twice). M2 was not run (moot, section 7.1).

Tests the barrier changed, all of them about a box that shares its line and is taller than a page (they asserted main's loss, "the lines
below the page are lost"): the three fulgur lib tests of F30f-b (`an_inline_block_sharing_its_line_keeps_its_paragraph_whole`, the
inline-table one and `unprinted_inline_lines` now use a box that fits a page; one new test pins the 3-fragment split of the 600 px box),
and in this crate `wrapper_line_split.rs` (A5: 212 words on one page before, 4000 words once now), `table_wrapper_split.rs` (B8: the
text of an `inline-table` sits on the page of its FIRST row: CSS 2.1 section 10.8.1, "the baseline of an `inline-table` is the baseline of the first row of the table") and
`unsplit_paragraph_warning.rs` (no "placed whole" warning for them).

### 7.8 Step 6: warnings

`page_bottom_warning.rs`: four shapes move to "must not warn" (a column of 130 children sharing its line, the same in a paragraph of two
lines, the tall line after two other lines, a box of 80 text lines). The F213b / F213d-b "must warn" entry stays, with its comment
rewritten: a fixed 1200 px box with one short child is not entered by the walk (B29), so its overflow is real and still warned. A new
warning says when a box that fits a page sits on a tall line and starts above the page that holds the baseline (B30: `rv/var2/two_lines_wrap`,
the second box of each line, 14 words lost, warned: "a box of 611px on a line taller than a page starts 201px above the page that holds the line's
baseline"); a test for it is in `page_bottom_warning.rs`. No tall line triggers the "no matching line ranges" warning or the
"painted on no page" ledger in any test or probe (P1 to P6, P8, fixture 51). `LineBarrier::SharesItsLine`'s text is kept (it still names the box
that fits a page).

### 7.9 Step 7: fixture 51

`fixtures/invoices/51-terms-box-beside-label/` (the numbers 49 and 50 are taken by F213e-b and F30h): 80 terms (the plan said about 70; 80 on 18 px
lines make 1440 px) in a 340 px box, the paragraph 640 px wide, `Souhlasím:` / `Podpis odběratele` on its third line. Chrome m153 reference
committed with `scripts/reference/render.mjs`: 3 pages, `Obchodní` at 328.1 pt on page 1, term 1 at 47.8 pt on page 2 (the tall line starts a
page), page 3: `Souhlasím:` and `Podpis` at 354.3 pt, `80.` at 356.8 pt, `Děkujeme` 373.8 pt. Pixel diff (`scripts/compare.py`), before (main-b,
fork `4be3de79`) / after: 1.9 / 9.3 / 3.3 % (318 pt of the box past the bottom edge of page 2, warned) to 1.8 / 4.5 / 4.9 % (page 3 grew
3.3 to 4.9 because terms 56 to 80 and the line are printed there now; the worst page fell 9.3 to 4.9); threshold 0.059, in `compare.py` with the
cause. Looked at (`out/compare/51-terms-box-beside-label-p2.png`, `-p3.png`): page 1 the invoice and the two intro lines, page 2 terms 1 to 55, page 3
terms 56 to 80 with `Souhlasím:` and `Podpis` level with `80.`, then the closing line and the footer. NOT as the plan said: the frame is closed at the
bottom of page 2 and again at the top of page 3 (Chrome slices it open; ADR-0004 consequence, F96), and our page 2 holds term 55 as its last where Chrome
has 56 and repeats it. Test `terms_box_beside_label_is_cut_and_its_line_is_on_the_last_page` (`fixtures.rs`). The other 47 fixtures have identical
diffs (`compare.py` before and after).

### 7.10 Step 8: gates

- **Byte gate** (`bytegate.py`): fixtures 01 to 48 and the 7 corpus documents, `RAYON_NUM_THREADS` 1 and 4, against `main-b`: 0 documents differ
  (fixture 51 differs, it is new). The tree has no fixtures 49 and 50 (reserved by other findings).
- **Word gate** (`words2.py`, multiset of `pdftotext` words) against `main-b`: `rvw/sh` 455 (443 identical, 12 differ), `rvw/sw` 48 (identical),
  `rv/var` 66 (62 / 4), `rv/var2` 38 (32 / 6), `f213dv/p` 5893 (identical), `f213ev/p` 2901 (2897 / 4), `f213e/p` 74 and `f213e/sw` 256 (identical):
  **0 documents with fewer words**, 25 of the 26 that differ print more (`flex110__tallcontent` +80, `two_lines_wrap` +233, ...; each checked
  against the words of its HTML: none prints a word twice). Still short: `flexalign__tallcontent` (84 of 142, F217's sizing), `two_lines_wrap*`
  (14 words, B30), `nestedflex` / `tdflex` (1 to 2 words, the tokens `IN` `SIDE` and `TD2`).
- **Tests** (real lock, fork `64f48b93`, no path patch): `cargo test --release` over the 86 integration test files, the lib and the CLI crate:
  1082 passed, 0 failed, 15 ignored (18 before; t2, t3, t5 are on). fulgur (main's Blitz / Parley / Taffy pins): `--lib` 2575 passed, the 43
  integration test binaries 579 passed (`render_smoke` 345). `cargo fmt --check` and `cargo clippy --all-targets -D warnings` clean in both repositories.
- **`bench.py --check`**: ok (fixtures 14 to 26 ms, `b200-text` 737 ms, `b200-mixed` 710 ms, scaling r 3.88; `b200-items` 1274 ms against the
  1250 ms of the accepted table exception, `NOTE pages 210 outside 195-205`, within its 1552 ms budget; not compared with main on this machine).
- **Combined local build** (fork `64f48b93` + `99a8401e` cherry-picked clean in a scratch clone, Blitz `f9b6bc04`, path patches in `--config`, nothing
  committed): words printed by main / F213b alone / combined (Chrome, F217 section 7.9): `flex110__tallcontent` 90 / 170 / 170 (171), `flexwrap` 63 / 143 / 143 (143),
  `minw0` 62 / 142 / 142 (143), `flexalign__tallcontent` 65 / 84 / 139 (131), `inlflexc__tallcontent` 135 / 135 / 143 (144), `flexalign__tallh` 13 / 13 / 10 (7, the
  listed exception). Chrome prints a word twice at each page edge, we once, so 1 below it is the same text. Over `rvw/sh` and `rvw/sw` the combined build has 2 documents
  with fewer words than main: `flexalign__tallh` (the exception) and `flex110__nowrap` (a tokenization artefact: `hereLead` and `longSIDE` are glued, no word is lost).
  So F217 can re-apply `99a8401e` on this head and re-measure.
- **Not run:** `vrt-validator`, `fulgur-vrt`, `pdf-reviewer` (the writer path is untouched); Chrome comparison of the P3 / P4 probes by pixels (numbers above are `pdftotext`
  positions); the M2 revert test; ADR-0012's `docs/upstream.md` pin of F217 is unchanged.

