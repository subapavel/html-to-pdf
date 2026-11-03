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

To be filled in by the implementer: probes, the hypothesis that holds, red/green, gates, sweep, compare numbers.
