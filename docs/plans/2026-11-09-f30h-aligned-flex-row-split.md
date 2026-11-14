# F30h: a flex / grid row whose items are not aligned to its top loses the split item's lines silently

Date: 2026-11-09. Finding: F30h in `2026-10-15-m0-findings.md` (medium). The 2026-11-09 re-check puts it on the *Pilot bar*, and it is the first item there with no work in progress. Measured on main 0cbe50f against Chrome m153. Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, through the existing `[patch.crates-io]`. htmltopdf changes only tests, fixture 50, docs and `Cargo.lock`. No upstream PR or issue. F183c is landing in the same fork, so find the code below by function name, not by line number.

## 1. Problem

A flex row holds a short label and an item taller than the room left, with `align-items` other than `stretch` / `flex-start`. The row loses most of the tall item's text and gives no warning. The probes are in scratch `pilotcheck/cases/f30h_*.html` (Tailwind, `@page{size:A4;margin:15mm}`, 1202 words). Compare with `run.py` and `wd.py`.

| Probe | Shape | Ours (main) | Chrome m153 |
|---|---|---|---|
| `f30h_b` | `flex items-center gap-4`: `border p-2` label, then a `flex:1 1 0%` item holding a long `<p>` | 630 / 1202 words; page 1 empty; page 2 starts with a line cut by the sheet's top edge; no warning | 3 pages, all words; the label is on page 2, beside the `§12` line |
| `f30h_c` | the same row with `items-end` | 26 / 1202 words, no warning | the label is on page 3, level with the last line |
| `f30h_e` | default stretch, container `p-2 border` | 4 pages, page 1 blank | 3 pages |
| `f30h_a` | stretch, a card with a background | words and pages equal Chrome; the card's background is missing on pages 2–3 | the background continues |
| `f30h_d`, `f30h_f` | grid row; `items-start` row | equal to Chrome (guards) | |

F213f §7.1 found the same loss at a page bottom: `l10`, `items-end`, room 199 drops `line1`–`line7` silently. It also found that a stretched label beside a 10-line item moves the row whole at rooms 40–199, where Chrome starts the row in place. F30b §7 P2 is the same move: a later grid row whose first item is a stretched label goes to page 2.

## 2. Spec and Chrome

Cited from memory and **unverified**. Step 1 has `css-spec-researcher` check every item.

- **css-flexbox-1 §12** (normative): a flex container may break between lines and inside items. A continued container gives its items the space left after the earlier fragments. §12 does not say where a cross-axis-aligned item goes once the row breaks.
- **css-flexbox-1 §12.1** (informative sample algorithm, single-line row): lay the row out "treating any `align-self` other than `flex-start` or `baseline` as `flex-start`". Then move the container only if an unbreakable item does not fit and the container is not at the top of the page. Then fragment each item from the row's top.
- **css-break-3 §4.1 / §4.4**: monolithic content that does not fit is pushed to the next fragmentainer. The unforced-break rules (orphans / widows, ADR-0010) choose among break points; none of them allows content to be lost. **§5.4** (`box-decoration-break: slice`) applies to F96e only.
- **Chrome m153** (probes b and c) keeps each item's offset from the unfragmented layout. The centred label lands beside the same text line it sits beside unfragmented, and the end-aligned label ends with the last line. Taffy's `final_layout.location.y` is that same offset: ours prints the label beside the same `§12` line as Chrome.
- **Decision.** Keep Taffy's aligned offset, as Chrome does. Reasons: the normative text is silent; §12.1 is a non-binding sample; Chrome is the reference our pixel gates use; and the flex-start reading would need a re-layout, which fulgur's design rules out ("no re-layout after splitting"). This is recorded here and in the finding. If the researcher finds normative text that requires flex-start, stop and write an ADR before step 4.

## 3. Where it belongs: cause and design

The fault is in fulgur's fragmenter: `crates/fulgur/src/pagination_layout.rs`, `fragment_block_subtree`, in the flex / grid row branch (`allow_same_row_rebase`). Taffy's offsets are right (see §2), and Blitz and Parley are not involved.

**Today.** The first item in DOM order opens the row. `RowState` snapshots the walk state there, and `row_bottom` grows by overlap. F28's `first_in_row` lets only that first item move the row. A later item is restored to the row's start only when an earlier one crossed by recursion (`crossed_by_recursion`).

**Hypotheses, ranked:**

1. **H1, the row is anchored at its first item, not at its top. Fits every number.** In b and c the first item is the label, at `location.y = T > 0`: T is about half the row in b and almost all of it in c. Its `child_page_y` lies below the strip, so the strip-overflow cut moves it to the next page and sets `page_taffy_origin = this_top_in_parent - band`, which is the label's top. The leaf or line-split arm of `first_break_h` fires; step 1's dump names which. The tall item (`location.y = 0`) is then not restored: `crossed_by_recursion` is false, or the re-snapshot already holds the label-anchored origin. So it gets `child_page_y = band - T < 0`. Its first T px of lines go above the strip: part into the top margin, the rest off the sheet. What follows from this:
   - The loss is about T, so it is larger for `end` than for `center` (26 against 630 words).
   - Page 1 is empty.
   - The line at the top of page 2 is cut by the sheet edge.
   - Nothing warns, because `render_v2` checks only the bottom edge (`PaintExtent::note_bottom`, F111).
   - The pending-origin rebase that starts a row after a split row also uses `this_top_in_parent`, so a centred row after a split row has the same fault.
2. **H2, the overlap grouping into rows is wrong.** Rejected for the probes: 0 < T + h. It can still split one row in two when item margins or `baseline` make the extents not overlap, so it is a sweep axis.
3. **H3, Taffy's offset is wrong.** Rejected: ours and Chrome put the label beside the same line.
4. **E1 (probe e, F30b P2, F213f's 10-line move): a stretched label decides the row by its stretched height.** The leaf arm of `first_break_h` uses `child_h + leaf_tail`. Its exemption, `row_mate_can_split`, recognises table rows only. In `f30h_a` the label sits flush at the strip top, so the cut's guard `child_page_y > page_start_y + band` saves it. In `f30h_e` the container's `p-2 border` puts the label 9 px down, and the row moves to page 2.

**Decision rule** (flex / grid rows only; tables and their `SplitRow` path are untouched, so F35 stays separate):

- **R1, row band.** When a row's first item is reached, compute the band: the smallest top and largest bottom of the margin boxes over the run of consecutive items whose extents overlap. Reason: the row's top is the cross-start of its line, not the top of whichever item comes first in the DOM.
- **R2, everything is anchored at the row top.** That means the `RowState` snapshot, the pending-origin rebase that starts a row, and the strip-overflow cut. Every item is placed from the row's start at `item_top - row_top`. Reason: an origin anchored at an item's top gives every item above it a negative y, and that is the loss.
- **R3, only items at the row top may move the row.** These are items with offset ≤ 0.5 px; among them the first in DOM order decides, which is F28 unchanged. They are walked first, then the other items in DOM order. Reason: the decision has to come before anything in the row is placed, and in a `flex-start` or stretched row every item is at the top, so their order is unchanged.
- **R4, a lower item does not move the row.** An item below the row top is placed where the row's flow reaches its offset, walking the strips; it never moves the row. If it cannot break and straddles a strip bottom, it moves alone to the next strip top (css-break-3 monolithic push). Reason: this is Chrome's placement in b and c, it needs no new map, and it is deterministic.
- **R5, stretch is not content.** A stretched flex or grid item beside a mate that can split leads with its content, not its stretched height. In code, `row_mate_can_split` is generalised from table rows to flex / grid rows. Reason: this fixes e and P2, and it is what table cells already get.

Signatures (new code only):

```rust
/// html-to-pdf F30h: block extent (parent coordinates) of the flex / grid row opened by `walk_children[i]`.
fn row_band(doc: &BaseDocument, walk_children: &[usize], i: usize) -> (f32, f32);
/// html-to-pdf F30h: page and page-local y that a row starting at (`page`, `y`) reaches `offset` px below its top.
fn place_in_row(strips: PageStrips, page: u32, y: f32, band: f32, offset: f32, h: f32, breakable: bool) -> (u32, f32);
```

`RowState::_row_top` becomes a used `row_top`.

**Warning (the safety net).** Add `PaintExtent::note_top` / `top()`, called where `paragraph.rs` already calls `note_bottom`. Add the matching check in `render_v2`: "content extends N pt above the top page edge on page P and is not printed". Reason: silent loss is the worst class, and a check on the top edge catches any top-side loss that remains, including ones from other causes (`position: relative; top: -…`). It writes only to the log, so it leaves the bytes alone.

## 4. Acceptance

**Fixture `50-centred-note-row-page-break`** (Czech invoice; 49 is reserved for F213e-b):

- **Content.** Fixture 43's skeleton (header, parties, N item rows, total). Then `<div class="flex items-center gap-4 mt-6 text-sm"><div class="w-32 shrink-0 font-semibold">Poznámka</div><div><p>…14-line note…</p></div></div>`, then `<p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
- **Tuning.** Pick N in step 1 so that, in both Chrome and ours, the room is more than three lines and less than T minus one line. In that range main loses lines today.
- **Classes.** Check each class in the committed CSS with `rg`; put anything missing in the fixture's `<style>`. No rebuild.
- **The PNG must show:**
  - page 1 ending with the total and the first note lines;
  - page 2 continuing the note, with "Poznámka" beside the line where Chrome has it;
  - then the thank-you line;
  - nothing in the top margin.
- **Asserts (`fixtures.rs`):**
  - each note word, `Poznámka` and `Děkujeme` once;
  - Chrome's page count (2), with `Poznámka` on Chrome's page;
  - on page 2, every `yMin` at or below the content top minus 1 pt.
- **Compare.** Record main's numbers; the threshold is the worst page after the fix + 1 point.

**Red tests: new `crates/htmltopdf/tests/flex_row_alignment_split.rs`.** Use numbered lines (`Line0001`…), as `nested_paragraph_split.rs` does. Every expected page is Chrome's, taken from step 1. Each test asserts every line once, page 1 holding `Line0001`, the label on Chrome's page, and no word above the content top.

- Rows taller than a page: `items-center`, `items-end`, `items-baseline`; `grid items-center`; `grid-row: span 2`; a centred row after a split row (the pending-rebase case).
- A `p-2 border` flex container (probe e: 3 pages); F30b P2 (`grid-cols-2`, row 2 starts in place).
- A centred / end row near the bottom with a 10-line item at rooms 40, 120 and 199 (F213f `l10`).
- `page_bottom_warning.rs`: the shapes above join "must not warn". New: `text_above_the_top_page_edge_is_reported` (`@page{margin:0}`, `<p style="position:relative;top:-100px">`).

A shape step 1 finds already green stays as a guard.

**Guards: green before and after.**

- `flex-start` and stretched rows: `a_paragraph_beside_a_square_in_a_flex_row_paginates_like_one_in_a_div`, `a_flex_or_grid_row_of_two_tall_paragraphs_ends_with_the_taller_one`, and probes a, d and f as tests.
- F96e's `guard_flex_row_item_keeps_its_closed_frame`, unchanged.
- F213f's shapes: the K5 `<img>` row moves; K4 moves whole (ADR-0010); the multicol C4 shape is unchanged.
- F35's ignored `a_short_cell_beside_a_split_cell_stays_on_the_first_page_when_centred` stays ignored: tables are out of scope.

**Fork, red first:**

- unit tests for `row_band` and `place_in_row` (one per arm);
- `no_flex_row_item_is_placed_above_its_strip` (every fragment y ≥ strip top − 0.5);
- `a_stretched_flex_item_beside_a_splittable_mate_does_not_move_the_row`;
- `PaintExtent` top tests;
- `render_smoke` cases for a centred row and an end-aligned row.

**Sweep: `out/scratch/f30h/sweep.py`**, rendering the same HTML with the branch, main and Chrome m153. About 1150 cases:

| Axis | Values |
|---|---|
| container | flex; `grid-cols-[auto_1fr]`; flex `p-2 border`; inside a `border p-4` card after a `<p>` |
| align | start, center, end, baseline, stretch, label-only `self-center` |
| order | label first; tall item first |
| tall item | 120-line `<p>`, 30-line `div > p`, 10 lines |
| label | one line; bordered with 3 lines |
| room | 40, 120, 199, row at the page top |

Per case, record: words lost, words per page, pages, the label's page, text above the content top, and warnings.

**Pass:**

- no case loses a word;
- no case is farther from Chrome than main;
- every `start` and `stretch` case is **byte-identical to main** (R2 and R3 change nothing when every offset is 0);
- the label is on Chrome's page, except within one line of a page boundary; list those cases.

**Byte gates** (1 and 4 rayon threads):

- Fixtures 01–49 that exist and the 7 corpus documents are byte-identical; only 50 is new. Reason: F213f §7.4's scan found no flex row crossing a page except 08's start-aligned grid, and the warning only writes to the log.
- Fork `fulgur-vrt`: the same failing set, identical `.actual.pdf` files, the 21 examples identical.
- `bench.py --check`.

Any other change stops the work, and a postmortem is written.

## 5. Steps (each at most a day)

1. **Measure (no engine code).**
   - Copy the probes to `out/scratch/f30h/` and add baseline, grid-centre, row-span and P2 probes.
   - Write an uncommitted fork dump test that prints per item `location.y`, the `RowState` snapshot, `child_page_y` and the arm taken.
   - Confirm H1 and E1, have `css-spec-researcher` verify §2, and size fixture 50.
   - Record the results in §7. Stop and re-plan if the tall item's `child_page_y` is not negative in b. Write an ADR first if Chrome treats any measured alignment as `flex-start`.
2. **Red tests and fixture.** `flex_row_alignment_split.rs`, the warning entries, and fixture 50 with its `render.mjs` reference and README. Commit with the guards green.
3. **Fork: warning.** `note_top`, the `render_v2` check, unit tests, smoke test. Byte gate identical. The loss is now warned before it is fixed.
4. **Fork: R1–R3.** `row_band`; anchoring at the row top in the snapshot, the pending rebase and the cut; walk order. Unit tests; byte gate identical for 01–49.
5. **Fork: R4–R5.** `place_in_row` for lower items; `row_mate_can_split` for flex / grid. Unit tests, smoke test, clippy, fmt.
6. **Bump and gate.** `cargo update -p fulgur --precise <sha>`; full tests, byte gates, sweep.
7. **Finish.**
   - `render-fixtures.sh` and `compare.py`; look at 50's side-by-side and at probes b, c and e.
   - Run `vrt-validator`. No `pdf-reviewer`: layout and log only.
   - Update the findings (F30h done; residuals filed), the roadmap and `docs/upstream.md`; write §7.

## 6. Risks, size, non-goals

- **Size.** Medium, about 6 days. Most of the risk is R3's walk order and R4's placement of lower items.
- **Walk order (R3)** changes the order of fragment pushes inside a row. Per-node vectors and the `BTreeMap` geometry keep this deterministic. Counters and `string-set` inside a centred label still need a sweep case.
- **Drift.** R4 walks the strips, not the split item's lines, so a lower item can be up to a line's slack per page away from Chrome's y. Accepted; the sweep lists any case that changes page. Exact line-anchored placement is a follow-up only if a fixture needs it.
- **F28 / F213f.** "First item decides" becomes "first item at the row top decides", which is identical for start and stretch rows; the byte gate and the sweep's byte-identity column pin this. F213f's C4 (multicol item) is not touched.
- **Warning false positives.** Content deliberately placed above the sheet (negative `top`) is not printed, so the warning is correct there. Step 3 greps every fixture log for it and expects none.
- **Frame / background (probe a) goes to F96e and stays out of scope.** Reasons:
  - No text is lost.
  - It needs one fragment per page for a stretched leaf, plus renderer slicing (F96's `box_slices`), which is a different mechanism.
  - One finding per PR keeps the byte-gate exception to fixture 50.
  - F96e's guard pins today's frame, so F30h cannot change it by accident.
  - Pilot-bar item 3 counts text loss only, so F96e does not block the pilot. Add the re-check note ("a stretched card's background is missing on pages 2–3") to F96e. The `p-2 border` container's own frame on continuation pages also goes there.
- **Non-goals:**
  - F35 (table cells, where Chrome puts a middle-aligned cell at the top);
  - `flex-direction: column` / `-reverse` and `order`;
  - `flex-wrap` lines after the first (R1 handles them, but they are not swept);
  - F213f's multicol shape;
  - F96e and F96 container frames;
  - `box-decoration-break: clone`.

## 7. Evidence

(filled in by step 1 and step 7)
