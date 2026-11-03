# F213d: an inline-block alone on its line, with a short block child, disappears at a page bottom without a warning

Date: 2026-11-03. Finding: F213d in `2026-10-15-m0-findings.md` (medium: content lost silently; pre-existing, main and F213 byte-identical). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, carried by the existing `[patch.crates-io]`; htmltopdf bumps `Cargo.lock` and records the change in `docs/upstream.md`. No upstream PR or issue. Line numbers were read from the fork's **working tree** on 2026-11-03 (F198j was being committed; `git show 329b4e43:` was not available to the planner). Re-read them at the commit you build on.

## 1. Problem

A `<p>` whose only content is a fixed-height `inline-block` with a short block child is printed nowhere when the room left on the page is smaller than the box but larger than its first row. Ours draws no `BOXTOP`, no border, and gives no warning. Chrome puts the box on page 2. The validator's file (`f213v/c/alone_r120.html`) is lost. Rebuild it under `out/scratch/f213d/` (git-ignored):

- Head: `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}`. The content box is 1009.13 px.
- `S(R)` = `<div style="height:{1009-R}px"></div>`, which leaves R px of room.
- `A(h, extra)` = `<p><span style="display:inline-block;width:150px;height:{h}px;border:1px solid #000;{extra}"><span style="display:block">BOXTOP</span></span></p><p>AFTER</p>`.
- `S(60|120|199) A(300)`: lost, silent.
- `S(10) A(300)`: moves whole and prints.
- An auto-height box of 15 lines prints, split between its lines (ADR-0004).
- `inline-flex` alone moves whole (F213).

## 2. Spec and Chrome

Cited from memory (`css-spec-researcher` not run): **unverified**. Confirm before step 4.

- **css-break-3 §4.1.** Break points are Class A (between siblings), Class B (between line boxes) and Class C (content edge / first child). There is no break point inside a line box. A UA may treat `inline-block` / `inline-table` (inline-level boxes that establish a formatting context) as monolithic.
- **css-break-3 §2 / §4.4 (monolithic content).**
  - Monolithic content that does not fit the room is pushed to the next fragmentainer, using the Class A break in front of the line.
  - Only monolithic content taller than the fragmentainer may be sliced in paged media.
  - §4.4 Rule 3 (orphans/widows) does not apply inside a monolithic box.
- **CSS 2.1 §13.3.3.** Page breaks occur between block boxes and between line boxes, never inside a line box.
- **CSS 2.1 §10.8.1.** The baseline of an `inline-block` with in-flow line boxes is its last line box. This is relevant to the y position only (F183c note).
- **ADR-0004.** It splits an alone box between its lines, so that a box *taller than a page* prints every line once instead of Chrome's slice. That reason does not apply to a box that fits a fresh page: moving it whole also prints every line once, at Chrome's position. §3 amends the ADR accordingly.

**Chrome probes.** Step 1, Playwright Chromium of the committed references, as `scripts/reference/render.mjs`. For each probe, record per word the page and y (`pdftotext -bbox`), the page count, and whether Chrome slices. Record ours (main) the same way, plus its warnings. Predicted: every box that fits a page moves whole to page 2; a taller one moves to page 2 and is sliced.

| Id | Probe | Question |
|---|---|---|
| Q1 | `A(300)` with R ∈ {5, 10, 15, 20, 25, 60, 120, 199, 250, 299, 303, 310} | room sweep; where main switches from move to loss |
| Q2 | `A(h)` with R = 120, h ∈ {100, 600, 1000, 1009, 1012, 1200, 2500} | above / below a page |
| Q3 | R = 120: auto height with 15 lines; `min-height:300px`; auto with `padding-bottom:250px`; `height:300px` filled with 15 lines; `height:300px` with 40 lines overflowing visibly | auto vs fixed; overflowing content |
| Q4 | fixed 196 px: caption + 72 px `<svg>` + name; R inside (150, 190) and R below the content height | several block children |
| Q5 | `<p>START A TAIL</p>` (F213 guard); `<p>Úvod<br>A</p>` (the box on line 2 of 2) | text before / after |
| Q6 | Q1 at R = 120 in `<section>`, in a `border p-4 rounded-lg` card, in an `overflow-hidden` card, in `<section><p>x</p>…</section>`, in a `td`, in a flex item | nesting |
| Q7 | `display:inline-table` (fixed / auto); `inline-flex` (guard) | other atomics |
| Q8 | `vertical-align:top` / `middle` | F183c interference (pages only) |
| Q9 | `text-align:right` / `center` | x on page 2 |
| Q10 | no `S`; `A(1200)` fixed with one short child, R = 120 | residual: box taller than a page |

## 3. Where it belongs: cause, design, safety net

This is fulgur behaviour: the fragmenter in `pagination_layout.rs` and the renderer in `render.rs` / `paragraph.rs`. Blitz and Parley lay the line out correctly. Our crate changes only tests, fixture 44 and docs.

**Hypotheses, ranked from reading the fork.**

1. **H1, most likely: F213's P1 path, silent because the line has no glyphs of its own.**
   - The body-level gate (`:4060-4087`) enters the carrier `<p>`: `would_split_block_subtree` (`:4687`) sees the 302 px box.
   - The `monolithic_line_carrier` exclusion does not stop it, because it returns `false` for a box `alone_on_its_line` (`:8432-8434`).
   - F28 (`:4121-4139`) asks `leading_block_size_with` (`:4829`). That returns the box's first row: border plus `BOXTOP`, about 21 px. This fits, so the carrier is entered in place. R = 10 fails that test and moves, which matches the finding.
   - Inside the carrier the box is not entered: `would_split_block_subtree(box)` is false because `BOXTOP` fits. The box cannot be cut either: it is the flush first child, so `child_page_y > page_start_y + band` (`:6463`) is false. The carrier closes as one fragment on page 1.
   - The line (baseline at the box's bottom) and the box (hanging below it, F183c note) are painted below the page edge.
   - The F111 warning (`render.rs:417-425`) stays silent for two reasons. The line holds only whitespace. The box's content is painted under `canvas.paint_extent.enter_local()` (`paragraph.rs:1071`), and `PaintExtent` records nothing while local (`draw_primitives.rs:804-808`).
   - The 15-line auto box is entered and split by F30f, so its lines are recorded per page. `inline-flex` is not `is_inline_block_or_table`, so F213 covers it.
   - Test: widen page 1's MediaBox (F213 §7.2 `geo.py`). `BOXTOP` appears below the edge.
2. **H2: the carrier is split and has no slices (F30f cause 6).**
   - `record_carrier_slices_after` (`:7136-7167`) records slices only when the box spans pages (`last > first`, `:7160`). `record_carrier_slices` also returns silently when the carrier has no fragment on the box's first page (`:7106-7112`).
   - A carrier with two or more fragments and a box with one fragment is then measured by `paragraph_lines_for_page` (`render.rs:4933-4965`). Its one line is taller than every fragment, so it gets `None` on every page.
   - The box is painted only by its carrier's `InlineBox` arm.
   - Test: the widened MediaBox shows nothing. The geometry dump shows carrier fragments ≥ 2 and empty `line_slices`.
3. **H3: the line is drawn on a page where the box has no fragment.**
   - The `InlineBox` arm finds no fragment (`paragraph.rs:1027-1034`, `find(page_index)`) and skips silently.
   - `inline_box_continues_on` (`render.rs:1390`) is false on the box's own first page.

Step 1 settles the hypothesis with the MediaBox read-back and a fork lib test that dumps `fragments` / `line_slices` of the carrier and the box for Q1 R = 120. The fix D1 below removes the shape for boxes that fit a page under any of the three; D2 depends on which holds.

**D1, the fix (fragmenter).** An `inline-block` / `inline-table` alone on its line is monolithic when its carrier fits a fresh page and its content ends inside it. It is moved whole, exactly as F213 moves a shared line.

```rust
/// F213d: whether the in-flow content of the inline box `ib` ends inside its padding box in the block axis
/// (the walk of `clip_hides_nothing`, extracted; output-neutral for its present caller).
fn content_ends_inside(doc: &BaseDocument, ib: &blitz_dom::Node) -> bool;
```

- In `monolithic_line_carrier` (`:8408`), the alone exclusion becomes `alone_on_its_line(..) && !content_ends_inside(..)`. The height test (`<= fresh + 1.0`) already keeps a box taller than a page on the ADR-0004 / F30f path.
- No new call site is needed. Both gates (`:4086`, `:6211`) and `leading_block_size_with` (`:4853`) already ask this predicate. So:
  - the carrier is not entered;
  - the move-whole code takes the Class A break;
  - a wrapper whose first child is the carrier moves with it (F28).
- Reason for content inside: a 300 px box whose 40 lines overflow it visibly would otherwise move to page 2 and still lose the overflow. F30f splits that one today.
- Reason for "fits a fresh page" rather than "fixed height" or "a split would leave a fragment with no line":
  - it is Chrome's rule (Q3 confirms);
  - it is the rule F213 already uses, so alone and shared lines behave alike;
  - it needs no split simulation.
- **ADR-0004 amendment, in the same PR:** "an alone box that fits a fresh page is monolithic (F213d); the split applies to boxes taller than a page".

**D2 (only if step 1 shows H2 for Q10 or any residual).** In `record_carrier_slices_after`, drop the `last > first` condition. A carrier of two or more fragments whose one line holds an alone box gets its slices on the page of the box's first fragment. Reason: that closes cause 6 for any box with one fragment. Limit: 1 day. If it needs more, file F213g-b and ship D1 plus D3.

**D3, the safety net (renderer).** It writes nothing to the PDF, so the bytes are identical.

- **(a) Box content below the edge.** In the `InlineBox` arm (`paragraph.rs:1027`), before `enter_local`, record `oy_page + content_frag.height` as the painted bottom. This only applies when the canvas is not already local. Then the existing "content extends N pt past the bottom page edge" warning fires for a box and its text. Reason: H1 is silent today only because of `enter_local`.
- **(b) Content on no page.** A per-render ledger, `thread_local` like `CONTINUATIONS_PAINTED` (`render.rs:1313`), cleared at the start of `render_v2`.
  - It records the line range of each paragraph painted at the single call site of `paragraph_lines_for_page` (`render.rs:4864`).
  - It records each inline-box root painted by the carrier arm or by `paint_inline_box_continuation`.
  - After the page loop it warns, in node-id order and capped like `MAX_DROPPED_TEXT_WARNINGS`:
    - "N of M lines of paragraph "…" are painted on no page and are not printed";
    - "an inline box "…" is painted on no page and is not printed".
  - Exclusions, each with its reason:
    - `!entry.visible` (F209);
    - the hidden-tail slice of a clip box (ADR-0009);
    - nodes without geometry (running elements; F95 b stays out).
  - Repeats (`position: fixed`, `thead`) only need to be painted at least once.

```rust
fn paragraph_line_range_for_page(lines: &[ShapedLine], fragments: &[Fragment], line_slices: &[LineSlice],
    page_index: u32, is_split: bool) -> Option<std::ops::Range<usize>>; // paragraph_lines_for_page wraps it
fn report_unpainted(geometry: &PaginationGeometryTable, drawables: &Drawables);
```

- **Strict mode: warning only, no error.**
  - Strict switches are per input class: `strict_fonts` maps to `strict_text`, `Error::TextWithoutFont`; `strict_images` maps to `Error::SvgImages`.
  - Pagination losses (the F111 bottom-edge and F30 "placed whole" warnings) are warnings under strict mode today. `page_bottom_warning.rs` renders strict and expects `Ok` with F213b's warning.
  - `TextWithoutFont` would name the wrong cause.
  - A `strict_layout` switch is new public API: file it as **F213g**.

## 4. Acceptance

**Fixture `44-signature-box-page-bottom`** (Czech invoice).

- **Layout:** the 43 skeleton (header, parties, N rows of 43's items, "Celkem k úhradě"), then `<p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
- **The signature box comes last:** `<p class="mt-8 text-right"><span class="podpis inline-block w-72 border border-gray-300 rounded-lg p-4 text-left">`, holding:
  - `<span class="block text-xs font-semibold uppercase tracking-wider text-gray-500">Razítko a podpis dodavatele</span>`;
  - an `<svg class="block mt-2" width="72" height="72" viewBox="0 0 100 100">` stamp: two circles and a check path, no SVG text;
  - `<span class="block text-xs text-gray-600 mt-2 pt-2 border-t border-gray-300">Ing. Jana Nováková, jednatelka</span>`;
  - then `</span></p>`.
- **Classes:** every class is checked in the committed `tailwind.css`. It has no usable height (`h-12`, `h-28`, `h-full` only), so the fixture's `<style>` (where 43 sets `@page`) adds `.podpis{height:52mm}`, about 196.5 px; the content is about 146 px. No npm, no rebuild.
- **Box last:** the thank-you line comes before the box because ours paints a fixed-height box about 55 px lower than Chrome's (F183c note). Placed after the box, the next line would be overlapped.
- **Tuning N:** pick N in step 2 so that in Chrome and in ours the room is 160-185 px: the content fits, the box does not.
- **PNG, main:** page 1 ends after the thank-you line; the box is absent on both pages.
- **PNG, after:** page 2 holds the whole box (border, caption, stamp, name), about 41 pt lower than Chrome's.
- **Asserts** (`fixtures.rs`):
  - `Razítko`, `Nováková` and `Děkujeme` each appear once;
  - the first two are on page 2, `Děkujeme` on page 1;
  - every `yMax` is at most the content bottom;
  - page count is Chrome's (predicted 2).
- **Compare:** record main's numbers. The fixture is new, so the threshold is the worst page after the fix + 1 point.

**Ours, `crates/htmltopdf/tests/alone_inline_block_line.rs`** (every expected page is Chrome's from step 1):

1. `a_fixed_height_box_alone_on_its_line_moves_to_the_next_page` (Q1 at R 60, 120, 199): `BOXTOP` on page 2, `AFTER` after it. **Red.**
2. `…_with_several_block_children` (Q4). **Red.**
3. `…_in_a_section` / `…_in_a_card` (Q6). **Red.**
4. `an_inline_table_alone…` (Q7). **Red** if Chrome moves it.
5. `an_auto_height_box_that_fits_a_page_moves_whole` (Q3, 15 lines). **Red** on pages: main splits it.
6. Guards, green on main:
   - R = 10;
   - Q2 at 1200 (every word once, ADR-0004);
   - `inline-flex` alone;
   - Q5 (F213);
   - Q3 overflowing (F30f path, every word once).

**`page_bottom_warning.rs`** (one logger per binary; the filter gains "painted on no page"):

- Q1 to Q4 and Q6 join "must not warn".
- Q10 joins "must warn" if step 1 shows it is still lost (comment names F213b).
- **Red at step 3:** the Q1 probe warns (D3 lands before D1) and is recorded in §7.

**New `crates/htmltopdf/tests/no_unpainted_content.rs`:** every fixture renders with neither "painted on no page" nor a bottom-edge warning. The allow-list is predicted empty; any entry names its finding.

**Fork tests** (red first):

- Lib:
  - `monolithic_line_carrier` cases: the "alone box" case becomes true when it fits and false when taller or overflowing;
  - `an_alone_inline_block_that_fits_a_page_moves_whole` (200 px pages: 150 px box, short child, room 60): the carrier is one fragment on page 1, and so is the box;
  - `…_in_a_wrapper`;
  - `content_ends_inside` units;
  - the F30f carrier tests (`:18883-18954`, 30 lines on 200 px pages) unchanged.
- Render units:
  - `paragraph_line_range_for_page_matches_paragraph_lines_for_page`;
  - `a_split_carrier_without_slices_is_reported`;
  - `an_inline_box_painted_below_the_edge_extends_the_paint_extent`.
- `render_smoke`: `test_render_fixed_height_inline_block_alone_moves_whole_smoke`.

**Byte gates** (1 and 4 rayon threads; any change outside the list stops the work and a postmortem is written):

- **After step 3:** every PDF is byte-identical (fixtures 01-43, the 7 corpus documents, the 21 fork examples, `fulgur-vrt` `.actual.pdf`).
- **After step 4:** byte-identical except fixture 44. Fixtures 03, 27, 35, 37 and 43 have inline-blocks, but none is alone on a page-crossing line: step 1 greps and confirms.
- Fixture 16 and `wrapper_line_split.rs` (4000 / 1500-word boxes, all taller than a page) must not move.
- `bench.py --check`.

**Sweep** (`out/scratch/f213d/sweep.py`):

- Grid: R {10, 20, 60, 120, 199, 201, 400} × h {100, 300, 900, 1000, 1200} × mode {auto, fixed, min-height} × content {1 block, 3 blocks, 15 lines, svg + caption} × nesting {body, section, card, section after p} × {inline-block, inline-table}.
- Every changed case must be on Chrome's pages, and none farther.
- Zero "no page" warnings except a named residual list.

## 5. Steps

1. **Measure (no code).**
   - Rebuild the Q probes and render them in Chrome and on main.
   - Run the MediaBox read-back and the dump lib test (not committed).
   - Record in §7 which hypothesis holds.
   - Grep the fixtures for alone boxes.
   - Size N for fixture 44.
   - Stop and re-plan if none of H1-H3 holds.
2. **Red tests and fixture.** Our tests, fixture 44 with its Chrome reference (`render.mjs`) and README; commit.
3. **Fork, D3.**
   - The (a) extent note, the ledger, `paragraph_line_range_for_page`, `report_unpainted`, the units.
   - Byte gate: everything identical.
   - Our warning tests show Q1 warns.
4. **Fork, D1.** `content_ends_inside`, the predicate change, lib tests, `render_smoke`, clippy, fmt.
5. **Fork, D2 (conditional, ≤ 1 day).** Otherwise file F213g-b.
6. **Bump and gate.** `cargo update -p fulgur --precise <sha>`; our full tests; byte gates; sweep.
7. **Finish.**
   - `render-fixtures.sh`, `compare.py`; look at 44's side-by-side.
   - Run `vrt-validator` and `pdf-reviewer` (merge policy 5: `render.rs` is on the writer path).
   - Update the findings (F213d done; F213g filed), the ADR-0004 amendment and `docs/upstream.md`; write §7.

## 6. Risks and non-goals

- **Pages move toward Chrome.** Alone boxes that fit a page but were split are now moved whole, leaving a gap at the page bottom, as Chrome does. A document may gain Chrome's page. The sweep and fixture gates watch this.
- **ADR-0004 scope.** Boxes taller than a page keep the split. If the fixture 16 / `wrapper_line_split.rs` bytes move, that is a regression and work stops.
- **Safety-net false positives** (repeats, multicol, list items, clipped tails, margin boxes). Each must be fixed by an exclusion with a reason, never by muting. Step 3 runs every fixture, the corpus, the fork VRT, the examples and the F213 sweep.
- **(a) newly warns** where a fixed-height box hangs below the edge (F183c). Those are true losses. List them in §7; they are not fixed here.
- **Thread-local ledger.** It assumes per-document single-threaded paint, as `CONTINUATIONS_PAINTED` already does. It is reset at the start of every render.
- **Rebase.** The F30f / F213 predicates and the render walks conflict on a fulgur bump.
- **Non-goals:**
  - the fixed-height box's y (F183c note);
  - a shared line taller than a page (F213b);
  - F213e, F213f;
  - slicing;
  - abspos under a clip with no geometry (F95 b);
  - a strict-layout switch (F213g).

## 7. Evidence

### 7.1 Step 1: probes, Chrome and main

Measured 2026-11-03. Chrome is the Playwright Chromium of this VM, the version of the committed references (`Skia/PDF m153`), with `emulateMedia(print)` and
`preferCSSPageSize`, like `scripts/reference/render.mjs`. Ours is main 5c04d20 (`Cargo.lock` pins fork `329b4e43`), the prebuilt CLI binary, bundled Inter.
Words and y (yMin, pt from the page top) from `pdftotext -bbox`; "lost" means the word is on no page. "Wide" is the MediaBox read-back of F213 section 7.2:
every page's MediaBox widened downwards by 3000 pt, then `pdftotext -bbox`. The probe scripts are in the scratchpad `f213d/` (`q.py`, `q11.py` to `q15.py`); `S(R)`, `A(h)` are as in section 1.
The CLI prints no warning for any case below that is called "silent".

| Id | Chrome | main |
|---|---|---|
| Q1 `A(300)`, R 5, 10, 15, 20 | p2 `BOXTOP` 41.7, `AFTER` 267.4 | same pages (p2), moved whole; the threshold is the box's first row, 21 px (1 px border + 20 px line): R 19 and 20 move, R 21 and up do not |
| Q1 R 25, 60, 120, 199 | the same | p2 `AFTER` 43.6 only. **`BOXTOP` is lost**: wide shows it on p1 at 996.8 (R 25), 970.6 (60), 925.6 (120), 866.3 (199). Silent. The box (border too) is painted below the page edge: page 1 is empty in the PNG |
| Q1 R 250, 299 | the same (the box is 302 px) | `BOXTOP` on p1 at 828.1 / 791.3, `AFTER` p2: the box stays on page 1 hanging into the margin; nothing is lost but the box's bottom |
| Q1 R 303, 310 | box and `AFTER` fit: p1 `BOXTOP` 571.2 / 565.9, p2 `AFTER` 40.9 | p1 `BOXTOP` 788.3 / 783.1, p2 `AFTER` 43.6: same pages |
| Q2 `A(h)`, R 120, h 100 | box fits (102 px): p1 `BOXTOP` 708.4, p2 `AFTER` | same pages |
| Q2 h 600 | p2 `BOXTOP` 41.7, `AFTER` 492.4 | `BOXTOP` lost (wide p1 1150.6), silent |
| Q2 h 1000, 1009, 1012, 1200, 2500 | p2 `BOXTOP`; `AFTER` on p3 (p4 for 2500); the box is sliced from 1012 up | `BOXTOP` lost, silent (wide p1 1450 to 2575); `AFTER` on p2, so 2 pages instead of 3 (4) |
| Q3 auto height, 15 **block** children, R 120 | **p2 L01 to L15 and `AFTER`: moved whole** | split: L01 to L05 p1 (711.1 to 771.1), L06 to L15 p2. Same at fixed 300 px with 15 block children |
| Q3 auto height, 15 lines of text (`L01<br>L02...`), R 10, 120, 250 | p2, whole | p2, whole already (the box is itself the inline formatting context; also 30 and 45 lines): this shape is **green on main** |
| Q3 `min-height:300px`; auto with `padding-bottom:250px` | p2 `BOXTOP` 41.7 | `BOXTOP` lost (wide p1 925.6 / 903.1), silent |
| Q3 fixed 300 px, 40 block children (800 px of overflow) | p2 all 40 lines, whole | split: L01 to L05 p1, L06 to L40 p2: every word once |
| Q3 fixed 300 px, 40 **text** lines (`L01<br>...`) | p2 all 40 lines | **L01 to L22 are on no page, even at the top of page 1 with no `S`**: L23 is at y 3.8 on p2, so the first 22 lines sit above the page top. Silent. A bug of the F183c family (the overflow of a fixed box is seated above it), not this finding |
| Q4 fixed 196 px: caption, 72 px svg, name; R 40, 100 | p2 CAPTION 41.7, NAME 122.7 | cut: CAPTION p1 (771 / 726), NAME p2 |
| Q4 R 130 | the same | CAPTION p1 759.1; NAME painted at 840.1, past the page edge: lost (wide only) |
| Q4 R 150, 170, 190, 195 | the same (R 200 fits, Chrome keeps it on p1) | the whole box on p1, its words 5 to 50 pt below the content box, inside the page margin: printed, in the wrong place |
| Q5 `START A TAIL`, `A TAIL`, `START A`, `Úvod<br>A` | p2 for all | p2 for all (F213): green |
| Q6 section, card, `overflow:hidden` card, `section p x`, table cell, flex item | p2 `BOXTOP` (`x` stays on p1, 707.7) | `BOXTOP` lost in every one (wide p1 925.6 to 940.6), silent |
| Q7 inline-table of table-row spans, fixed 300 px | p2 | p2: green |
| Q7 inline-table, auto, 15 rows | p2 all rows | split 5 / 10, like Q3 blocks |
| Q7 `inline-flex`, R 10 and 120 | p2 | p2: green (F213) |
| Q8 `vertical-align` top, middle, bottom; Q9 `text-align` right, center | p2 | `BOXTOP` lost for all five (wide p1 925.6), silent |
| Q10 no `S`, `A(1200)` | p1 `BOXTOP` 41.7 (sliced), p2 `AFTER` 184.9 | `BOXTOP` **lost at the top of page 1**: wide p1 933.8; `AFTER` p2 43.6; silent |
| Q10 R 120, `A(1200)` | p2 `BOXTOP`, p3 `AFTER` | lost (wide p1 1600.6), 2 pages |

Differences from the plan:

- **Q3, the question of the brief.** Chrome moves an auto-height box of 15 lines that fits a page whole, as the plan predicts (css-break-3 section 4.4). What differs is main:
  only a box with **block children** is split (5 / 10); a box of plain text lines is already moved whole. The red test uses block children; the text-lines shape is a guard.
- Guard "Q3 overflowing, every word once" holds for block children only; with text lines main loses L01 to L22 (above). The guard uses block children.
- Q2 at h 1200 is not a guard: `BOXTOP` is lost on main (silent). Q10 is lost at the top of the page with no `S`, so it is a residual of any alone box taller than a page whose child is short;
  Chrome slices it. The guard "every word printed once, ADR-0004" uses an auto-height box of 70 block lines (1402 px).
- The loss is not only for a box with the first row fitting but a **short child**: any box whose first row fits is entered in place; whether words are lost depends on how far the box
  is painted below its line (F183c seats a fixed-height box on the baseline of its last line, so it hangs below by about its free space). A fixed 196 px box with a 129 px content
  (the plan's fixture) is never lost, only misplaced into the page margin (Q4 R 150 to 195); the fixture needs a taller box (section 7.3).
- A real `<table style="display:inline-table">` cannot be alone in a `<p>` (the parser closes the paragraph); in a `<div>` it moves on main as the span-built one does.

### 7.2 Which hypothesis holds: H1

1. Q1 R 120: page 1 is empty in the PNG (not even the box's border is on it), page 2 has `AFTER` only; the widened MediaBox shows `BOXTOP` on **page 1**, 925.6 pt, below the
   page edge (841.89 pt). The carrier is therefore one fragment on page 1 and its content is painted there: H1. H2 (a split carrier with no slices) and H3 (a line drawn on a page without
   the box's fragment) predict that nothing is painted anywhere, widened or not: refuted for Q1, Q3, Q6, Q7, Q8, Q9 and Q10, every case above whose word shows in wide.
2. The room threshold is F28's first-row test exactly: R 20 moves, R 21 stays (the first row is the border plus one 20 px line, 21 px). H1 predicted it.
3. The silence is as H1 predicted: the fork at 329b4e43 paints the box's content under `canvas.paint_extent.enter_local()` (`paragraph.rs:1071`), and `PaintExtent` records nothing while local
   (`draw_primitives.rs:804-808`); the line itself holds only whitespace. Function names checked at `git show 329b4e43:`: `monolithic_line_carrier` (line 8227), `alone_on_its_line` (8170),
   `leading_block_size_with` (4648), `record_carrier_slices_after` (6955), `clip_hides_nothing` (7506). `alone_on_its_line` returns false for a box that shares the line, true
   for ours; the exclusion in `monolithic_line_carrier` therefore stops the F213 move for it, as the plan says.
4. No fork dump test was run (the fork is not edited in this round); the read-back separates H1 from H2 and H3 without it.
5. The F213 mechanism is the same one with text beside the box; the difference is that here nothing else on the line has glyphs, so even the F111 text check has nothing to see.

### 7.3 Step 2: tests, fixture 44, fixtures with an alone inline-block

**Grep.** `inline-block`, `inline-flex`, `inline-table`, `inline-grid` in `fixtures/invoices/*/index.html`: 03 (two pills in one paragraph: they share the line), 27 (`.meta div`
twice at 49 % each, `.pay dt` next to its `dd`), 35 (`.dot::before` before text), 37 (hidden `.stamp` beside the title; `.meta div`, `.meta dt`), 43 (the QR box beside text). None is alone on its
line, and none is on a line that crosses a page. `bench/corpus` has none. D1 should leave every PDF but 44 byte-identical; the byte gate of step 4 confirms it.

**Fixture 44** (`fixtures/invoices/44-signature-box-page-bottom`). Thirteen rows of 41's table, the total, the thank-you line, then the signature box last. `.podpis{height:70mm}` (about 265 px) in the
fixture's `<style>`; every class is in the committed `tailwind.css`. The plan's `52mm` loses nothing at any row count: with 9 to 12 rows the whole box is on page 1 (the name 50 to 133 pt above the
edge), with 13 the name is in the page margin (820 pt), with 14 to 16 the box is cut (caption p1, name p2), with 17 it moves. A 60 mm box loses the name by 0.1 pt at 13 rows only. 70 mm: 12 rows lose the name
(843 pt, 1 pt past the edge), 13 rows lose it by 29 pt (871 pt) with the layout room 160 px, above the 147 px height of the content so the box is not cut. 13 rows chosen.
In Chrome the thank-you line is at 646 pt and the box is on page 2 (`RAZÍTKO` 55.8, name 140.5). In ours the thank-you line is at 646 pt, `RAZÍTKO` at 786 pt on page 1, the name at 871 pt (wide).
After the fix the box is expected on page 2 below Chrome's by about the free space below the name (F183c): the README says so, to be measured in step 6.

**Fixture 44 on main 5c04d20** (`scripts/render-fixtures.sh`, `scripts/compare.py 44-signature-box-page-bottom`): **1 page, Chrome 2**; pixel diff **3.5 % (page 1), 0.4 % (page 2, ours absent)**, worst 3.5 %,
page-count mismatch (FAIL: no `PAGE_COUNT_MAY_DIFFER` entry and no threshold; none is added in this round). No warning. The PNG shows page 1 ending with the thank-you line and, below a gap, the top of the box
(border, caption, the top of the stamp) cut by the page edge; page 2 does not exist.

**Tests**, run on main with `CARGO_TARGET_DIR=/home/user/html-to-pdf/target cargo test --release --locked --offline --no-fail-fast -p htmltopdf --test alone_inline_block_line --test fixtures --test page_bottom_warning --test no_unpainted_content`:

| Test | Result on main |
|---|---|
| `alone_inline_block_line::r_a_fixed_height_box_alone_on_its_line_moves_to_the_next_page` (Q1 R 60, 120, 199) | **red** (`BOXTOP` printed 0 times) |
| `…::r_the_same_with_vertical_align_and_text_align` (Q8, Q9) | **red** (same) |
| `…::r_a_box_with_several_block_children_moves_whole` (Q4, R 40 to 190) | **red** (`CAPTION` on page 1) |
| `…::r_the_same_in_a_section_a_card_a_cell_and_a_flex_item` (Q6) | **red** (`BOXTOP` 0 times) |
| `…::r_an_inline_table_alone_that_fits_a_page_moves_whole` (Q7 auto, 15 rows) | **red** (`L01` on page 1) |
| `…::r_an_auto_height_box_that_fits_a_page_moves_whole` (Q3: 15 blocks, fixed 300 with 15 blocks, `min-height`, `padding-bottom`) | **red** (`L01` on page 1) |
| `…::g_a_box_whose_first_row_does_not_fit_the_room_already_moves` (Q1 R 10, 20) | green |
| `…::g_a_box_that_fits_the_room_stays_on_page_one` (Q2 h 100) | green |
| `…::g_an_inline_flex_box_alone_moves_whole` (Q7) | green |
| `…::g_a_fixed_height_inline_table_alone_moves_whole_already` (Q7) | green |
| `…::g_a_box_that_shares_its_line_keeps_f213` (Q5, four shapes) | green |
| `…::g_a_box_taller_than_a_page_with_block_lines_prints_every_line_once` (70 blocks, ADR-0004) | green |
| `…::g_a_box_whose_content_overflows_it_visibly_prints_every_line_once` (Q3, 40 blocks) | green |
| `…::g_a_box_of_text_lines_moves_whole_on_main_already` (Q3, 15 text lines) | green |
| `page_bottom_warning::warns_about_content_below_the_page_only` | **red** at the new "must warn" case, Q10 (a 1200 px box alone, with 120 px of room): no warning. The 7 new "must not warn" entries (Q1 to Q4, Q6) are green on main: they are guards for step 3, where the net will see the box |
| `no_unpainted_content::no_fixture_has_content_that_is_painted_on_no_page` | **red** at its sentinel (a 1200 px box alone: the net is silent). The loop over the 43 fixtures passes on main (no warning), so the allow-list is empty |
| `fixtures::signature_box_invoice_moves_the_whole_box_to_page_two` | **red** (1 page, Chrome 2) |
| the other 22 tests of `fixtures.rs` | green |

`cargo fmt --all -- --check` and `cargo clippy --release --locked --offline -p htmltopdf --tests -- -D warnings` pass.

### 7.4 Steps 3 to 7: the fix, the net and their gates

Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `7d620b3a` (step 3, the net; on `cca17235`, which has F198j) and `62216d9a` (step 4, the fix). Re-located by name at `cca17235`: `monolithic_line_carrier`, `alone_on_its_line`,
`leading_block_size_with`, `clip_hides_nothing`, `paragraph_lines_for_page`, `draw_paragraph_inner_paint`. Differences from the plan:

- **D3 (a)**, `paragraph.rs` `InlineBox` arm: the box's bottom (`oy + content_frag.height`) is noted before `enter_local`, and the box is recorded in the ledger.
- **D3 (b)**, `render.rs`: `paragraph_line_range_for_page` returns `(range, rebase)`; `paragraph_lines_for_page` is the copy of it (test-only now). `draw_paragraph_inner_paint` and `draw_paragraph_v2` /
  `draw_block_with_inner_content` take the node id and record the range at the one call site. `paint_inline_box_continuation` records its box. `report_unpainted` runs after the page loop. `unpainted_report` skips: a paragraph
  that is not visible (F209), has opacity 0 or sits in a block with opacity 0 (`draw_with_opacity` skips it), an inline box with opacity 0 and its subtree, the hidden tail of a clip box (ADR-0009), a paragraph the multicol plan
  paints (`paragraph_slices`, F198b), a node without geometry or without a fragment; an inline box whose carrier is not planned, not visible or hides a tail. Cap 20 plus one count line. Warnings only, no strict error (F213g).
- **D1**: `content_ends_inside` is the body of `clip_hides_nothing`, which calls it (output-neutral). In `monolithic_line_carrier` the alone exclusion became `alone_on_its_line(..) && !content_ends_inside(..)`. Both gates
  and `leading_block_size_with` already ask the predicate, so no new call site.
- **D2** was not needed. Q10 (a box taller than a page) is not a split-carrier-without-slices case: its carrier is one fragment and the box is painted below the edge (H1), so D2 would not change it. Filed F213d-b; D3 warns.
- **The F183c-b above-the-top check** was not done (needs one more fork commit and a rerun of every gate; filed in F183c-b).

**Fork tests.** Red first (5 of 6 red with `content_ends_inside` a stub returning `false`): `an_alone_inline_block_that_fits_a_page_moves_whole` (the carrier was one fragment on page 0 at y 140),
`..._in_a_wrapper`, `an_alone_inline_block_of_auto_height_moves_whole_where_it_fits_a_page`, `content_ends_inside_reads_the_block_axis_of_an_inline_box` (7 cases), the `monolithic_line_carrier` cases (now 13),
`an_alone_inline_block_taller_than_a_page_is_still_entered` (green on both); net: `paragraph_line_range_for_page_matches_paragraph_lines_for_page`, `a_split_carrier_without_slices_is_reported`,
`the_unpainted_report_leaves_out_what_is_not_painted_by_design`, `an_unpainted_inline_box_is_reported_once_painted_it_is_not`, `an_inline_box_painted_below_the_edge_extends_the_paint_extent` (red with the note turned off:
"painted down to 0 pt"); `render_smoke::test_render_fixed_height_inline_block_alone_moves_whole_smoke` (red with the check off: `BOXTOP` not printed). Fork: lib 2542 and every integration test pass (`render_smoke` 339),
`cargo clippy -p fulgur --all-targets -- -D warnings` and `cargo fmt --check` clean.

**Step 3 byte gate** (base = fork `cca17235` binary; RAYON 1 and 4): 44 fixtures and 7 corpus documents identical (51 PDFs; 1 thread equals 4 threads); the only warning is fixture 44's ("content extends 129.9 pt past
the bottom page edge on page 1": the net sees the box). Fork `fulgur-vrt` (35 `.actual.pdf`) and the 21 examples (`fulgur render`, 4 threads): identical to the base. Ours with D3 only: the Q1 probe of `page_bottom_warning.rs`
warns ("content extends 307.6 pt", the red the plan expects at step 3) and `no_unpainted_content.rs` is red at fixture 44.

**Step 4 byte gate** (fork `62216d9a`, base `cca17235`, after merging main with F198j and fixture 40): 44 fixtures and 7 corpus documents, 51 PDFs, RAYON 1 and 4: identical **except fixture 44**; fixtures 16 and 40 and
the corpus did not move; no net warning anywhere. Fork `fulgur-vrt` 35 `.actual.pdf` and the 21 examples: identical.

**Ours, on main + fork `62216d9a`:** `cargo test --release --locked --no-fail-fast`: **1049 passed, 0 failed, 9 ignored** after one test fix: `page_bottom_warning.rs` "Q2, a 600 px box, room 120" was in "must not warn" and warns on page 2
(the moved fixed box hangs 90.9 pt below the edge: F183c, a true loss of the box's bottom), so the silent entry is now a 400 px box and the 600 px box is a "must warn" case naming F183c. `cargo fmt --all -- --check` and
`cargo clippy --release --locked --all-targets -- -D warnings` clean. The six red tests of `alone_inline_block_line.rs`, the fixture-44 test, Q10 of `page_bottom_warning.rs` and the sentinel of `no_unpainted_content.rs` are green.

**Sweep** (`f213d/sweep.py`, 2464 cases: R {10, 20, 60, 120, 199, 201, 400} x mode {auto; fixed and min-height at h 100, 300, 900, 1000, 1200} x content {1 block, 3 blocks, 15 lines, caption + svg + name} x nesting {body, section,
card, section after a paragraph} x {inline-block, inline-table}; Chrome rendered from the same HTML; "on Chrome's pages" = every (word, page) Chrome prints is printed on that page):

- **824 cases changed**: **742 are exactly on Chrome's pages now and closer than before**, 52 are no closer (the same distance), 30 are farther. The 82 changed cases that are not exactly on Chrome's pages are
  one family: `card` (a `border p-4 rounded-lg` card, 17 px of top inset) with h = 1000, a box of 1002 px that fits a fresh page but not a fresh page inside the card. Chrome leaves the card's frame empty on page 2 and puts the box on page 3; ours
  moves the box to page 2 (before: split or lost). In 30 of them one more word-page differs from Chrome than before (3 to 4, 15 to 16, 2 to 3; page counts unchanged at 3 against Chrome's 4). **This is a case that moves farther, named and
  not fixed**: F213d-c (the fits-a-page test uses the page height, not the room the ancestors' insets leave).
- Words lost on no page: 465 on main, **181** now: 168 are the box of 1200 px (taller than a page, F213d-b), 13 are the fixed 100 px box with 15 text lines (F183c-b, silent). No case loses more words than before.
- Net warnings: 0 on main (silent), 1216 now. All are true partial losses of the box: a box of 900 px and more moved to page 2 hangs below the edge (F183c, 196 cases per height), a box of 1200 px is lost (F213d-b), and 40 cases that
  Chrome keeps on page 1 and that fit its room (R 400, h 300; R 120, h 100: the box is painted below its line, 8 to 113 pt past the edge, F183c, unchanged by F213d). 432 of the warned cases have a box shorter than 1000 px and lose no word. No warning fires on a case
  that is fully on Chrome's pages and loses nothing, except the F183c hang.
- Pages: 546 cases had a different page count from Chrome on main, 376 now.

**Fixture 44** (`render-fixtures.sh` with the fork `62216d9a` binary, `compare.py`): **2 pages like Chrome, 3.3 % / 0.8 %** (before 1 page, 3.5 % / 0.4 %); threshold set to 4.3 % (worst page + 1 point). I looked at both pages
of the side-by-side: page 1 ends with "Děkujeme za spolupráci." and a gap; page 2 holds the whole box (border, caption, stamp, name, free space), 91.3 pt (121.7 px) lower than Chrome's (F183c). Every other fixture's pixel diff is unchanged
(their PDFs are byte-identical), all `ok`.

**bench.py --check**: ok. b200-items 1480 ms (over the 1250 ms exception on this machine, as the unchanged binary: 1365 / 1404 ms base against 1395 / 1467 ms new in two alternating runs of seven; b200-text 811 / 819 ms
against 890 / 840 ms: the ledger's cost is inside the noise of this machine), scaling r = 4.18.

**Not run**: `vrt-validator` and `pdf-reviewer` (the coordinator's), the Ruby / Python bindings of the fork, the fork's wasm demo.
