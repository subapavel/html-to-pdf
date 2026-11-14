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
- `t2` (P3, multi-line, tall line first): "Lead words before" is level with T129. "trailing" is on the next line below
  the box, on the same page.
- `t3` (P3, tall line 3rd of 4): it starts on a fresh page, as Chrome does.
- `t4` (P2, F213d-b): START, BOXTOP and TAIL are on the box's first page, and AFTER is after the box's end. If step 1
  finds main already prints these words, `t4` stays as a guard.
- `t5` (P4, text lines inside the box): every L once (step 5).
- `t6` (P5, `inline-flex` sharing its line): text on the first-baseline page.
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

(Filled in by the implementing agent.)
