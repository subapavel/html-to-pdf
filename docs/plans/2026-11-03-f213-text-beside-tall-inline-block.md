# F213: words beside an inline-block taller than the room left on the page are not printed

Date: 2026-11-03. Finding: F213 in `2026-10-15-m0-findings.md` (medium-high: words lost, warned; pre-existing, the same on main). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`. `Cargo.lock` pins `7a79be15`. The line numbers below were read from the fork's working tree on 2026-11-03, while F96 was being committed on the same branch. Re-read them at the commit you build on, which is F96's head once it is pushed.

## 1. Problem

A paragraph has text, an `inline-block` taller than the room left below it on the page, and more text. Its words (`START`, `TAIL`) are printed on no page. The renderer warns "content extends N pt past the bottom page edge on page 1 and is not printed". Chrome prints them. `vertical-align: top` loses them as well, because in Parley 0.6 every inline box sits bottom-on-baseline (F183c). The validator's files (`out/scratch/f209v/pgv.html`, `pgf.html`) are lost. Rebuild them under `out/scratch/f213/` (git-ignored).

Common head for every probe: `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}`. The content box is 1009.13 px. `S` = `<div style="height:809px"></div>`, which leaves 200 px of room. `B(h, extra)` = `<span style="display:inline-block;width:150px;height:{h}px;border:1px solid #000;{extra}"><span style="display:block">BOXTOP</span></span>`.

| Probe | Body | What it isolates |
|---|---|---|
| P1 | `S<p>START B(300) TAIL</p><p>AFTER</p>` | the finding (a box with block content) |
| P1e | P1 with an empty box (no `BOXTOP` span) | the leaf case (expected green on main, §3) |
| P1t | P1 with `vertical-align:top` | the second validator file |
| P2 / P2a | `B(1200)`, after `S` / with no `S` | a line taller than a page |
| P3 / P3s | `<img src="tall.png" style="width:100px;height:300px">` (1×1 PNG) / inline `<svg width="100" height="300">` instead of the box | an atomic replaced element |
| P4 | `S<p>L1<br>L2<br>START B(300) TAIL<br>L4<br>L5</p>` | the tall atomic on line 3, 2 / 3 split allowed |
| P4b | `S<p>L1<br>L2<br>START B(300) TAIL</p>` | the same as the last line (ADR-0010: moves whole) |
| P5 / P5b | P1's `<p>` in `<section>` / in `<section><p>x</p>…</section>` | nested: wrapper F28 / nested cut |

## 2. Spec and Chrome

The spec is cited by the planner from memory (css-spec-researcher not run). The wording and section numbers are **unverified**; confirm them before step 2.

- **css-break-3 §4.1**: Class A break points lie between sibling block boxes, Class B between line boxes, Class C between a content edge and its child. A line box has no break point inside. The same section lets a UA treat `inline-block` / `inline-table` (and other inline-level boxes that establish a formatting context) as monolithic. ADR-0004 relies on the same text. We already treat an inline-block that shares its line as monolithic (ADR-0004, F30f-b).
- **css-break-3 §4.4**: orphans/widows (Rule 3), relaxed only when the allowed breaks cannot avoid overflow (as quoted in ADR-0010). For P1 the Class A break before the paragraph avoids overflow, so it is the break to take.
- **CSS 2.1 §13.3.3**: page breaks occur between block boxes and between line boxes, never inside a line box.
- **Monolithic content taller than the fragmentainer**: css-break-3 lets paged media slice it, as quoted in ADR-0004. The brief places this in "§5.x"; the section is unverified. It applies to P2 only.
- **Chrome m141, predicted** (step 1 measures with `scripts/reference/render.mjs` plus `pdftotext -bbox`; words per page, y of `START`, `BOXTOP`, `TAIL`, `AFTER`, page count):
  - P1, P1e, P1t, P3, P5: the whole line goes to the top of page 2.
  - P4: 2 / 3.
  - P4b: Chrome may split it 2 / 1 (ADR-0010: we move it whole).
  - P2: the line moves to page 2 and the box is sliced; `TAIL` ends up on the page where the box's bottom lands.

## 3. Where it belongs: cause, ranked from reading the fork, and design

This is fulgur fragmenter behaviour (`crates/fulgur/src/pagination_layout.rs`). Blitz and Parley lay the line out correctly. The fix goes in the fork, carried by the existing `[patch.crates-io]`. htmltopdf only bumps `Cargo.lock` and records the change in `docs/upstream.md`. No upstream PR or issue.

**Hypotheses.** A one-line carrier never takes the line path: `line_metrics.len() > 1` fails at `:3564` and `:3620`, so it reaches the block path.

1. **H1, most likely: the gate enters the one-line carrier, and nothing moves it.**
   - `gate` (`:3879`) asks `would_split_block_subtree` (`:4503`). That function walks the carrier's `layout_children`, finds the inline-block (300 px > 200 px room) and returns true.
   - F28 (`:3937-3965`) moves the carrier only if `leading_block_size_with` (`:4645`) does not fit. For an inline root with one line, `abspos_line_split` is `None`. The walk then descends into the box and returns the box's first row (`BOXTOP`, about 20 px), which fits.
   - So `fragment_block_subtree(carrier)` runs in place. The box is the carrier's flush first child, which the cut cannot move (comment at `:6125-6129`). The carrier closes as one fragment on page 1.
   - The line's baseline (at the box's bottom) is then painted below the page edge. `paragraph.rs:844-851` records it in the paint extent, and `render.rs:420-424` warns. This matches the observed warning.
   - P1e (empty box) should be green on main: with no children, `leading` is the box's own height, so F28 moves the carrier.
2. **H1b: the carrier is split and its line is measured away.**
   - If the walk splits the box, the carrier gets two or more fragments with no `line_slices`.
   - `record_carrier_slices_after` (`:6944`) only covers an inline-block `alone_on_its_line`.
   - `render.rs` `paragraph_lines_for_page` (`:4898-4977`) then finds the line taller than every fragment and paints it nowhere (F30f plan, cause 6).
3. **H2, P2 only: a line taller than a page.**
   - F28 skips the move, because `leading > page_height_px`.
   - The gate enters the carrier (`h > page_h_px`), or the strip slicer (`:4079`) cuts it.
   - Either way the line goes the way of H1 or H1b.
4. **H3: the same holes in the nested walk.** `fragment_block_subtree`'s `gate` (`:6005`) and the F28 leading of a wrapper (P5) have the same shape.
5. **Not expected to be broken: P4.** It takes `fragment_inline_root` (`:9221`). Parley's line metrics include the inline box (the same reading as `starts_in_place`), and `choose_line_break_with` breaks before line 3.

Step 1 picks the hypothesis with a fork lib test that dumps the carrier's and the box's `fragments` / `line_slices` for P1, P1e, P2 and P5.

**Design.**

```rust
/// F213: an inline root of exactly one line whose line holds an atomic inline box (inline-block / -table / -flex /
/// -grid, `<img>`, `<svg>`) and no inline-block that is alone on its line (ADR-0004 keeps those on the F30f path),
/// and whose line box fits a fresh page (`<= fresh + 1.0`, the strip slicer's slack). Such a line is monolithic
/// (css-break-3 §4.1): the block walk must not enter it.
fn monolithic_line_carrier(doc: &BaseDocument, node: &blitz_dom::Node, fresh: f32) -> bool;
```

- **Both gates** (`:3879`, `:6005`) add `&& !monolithic_line_carrier(..)`. Reason: only then does the existing move-whole code take the Class A break before the paragraph:
  - `:4023-4028` at body level;
  - the nested cut for a child that has content above it.
- **`leading_block_size_with`** returns `own_h` for such a node. Reason: a wrapper whose first child is the carrier (P5) must be moved by F28. Inside the wrapper the carrier is a flush first child the cut cannot move, so without this the wrapper is entered in place and the line overflows there.
- **No renderer change.** The carrier is one fragment. Its `InlineBox` arm paints the box on the carrier's page.
- **Implementation constraints:**
  - Read the Parley lines without allocating (no `collect_inline_line_metrics`). The check runs for every inline-root child that reaches a gate.
  - No new map, so the output stays deterministic.
- **Left alone on purpose:**
  - A line taller than a page (H2) keeps main's path: slicing a shared line is a new ADR-0004 decision (F213b).
  - Multi-line inline roots keep their paths (F213c if step 1 finds one).

## 4. Acceptance

**Fixture `43-qr-inline-block-page-bottom`** (Czech invoice; classes checked in the committed `tailwind.css`).
- **Page 1:** header and party grid as in 03; an items table with N rows; totals.
- **Then** `<section class="mt-6 text-sm"><p>Zaplaťte QR kódem <span class="inline-block border border-gray-300 rounded-lg p-4"><span class="block text-xs font-semibold uppercase tracking-wider text-gray-500 mb-1">QR Platba</span><svg class="block w-28 h-28" viewBox="0 0 37 37">(path of 03)</svg><span class="block text-xs font-mono mt-1">48 400,00 CZK</span></span> nebo převodem na účet 123456789/0300.</p></section><p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
  - One line, about 530 px wide. The box is about 186 px tall.
- **Tuning N:** pick N in step 1 so that, in both Chrome and ours, the paragraph starts 80-120 px above the content bottom. That room is above the box's leading (about 33 px) and below its height.
- **No `align-top` in the fixture.** It would add F183c's misplaced text (at the box's bottom instead of its top) to the diff. The red tests cover `align-top`.
- **The PNG must show** page 1 ending after the totals, with a gap below. Page 2 starts with the whole line (text, box with QR and amount), then the thank-you line.
- **Asserts** (`fixtures.rs`): `Zaplaťte`, `QR`, `převodem`, `0300.` and `CZK` each appear once, all on page 2, with `yMax` at most the content bottom; page count equals Chrome's (2).
- **Compare:** record main's numbers (page count, per-page %) in §7. The fixture is new, so there is no "before" threshold; set it to worst page + 1 point.

**Ours: new `crates/htmltopdf/tests/tall_inline_box_line.rs`.** Probes are from §1. Every expected number is Chrome's from step 1, and each test names it.

1. `a_line_with_an_inline_block_taller_than_the_room_moves_to_the_next_page` (P1): `START`, `BOXTOP`, `TAIL` on page 2, `START` within 1 pt of Chrome; `AFTER` after them. Red on main.
2. `…_with_vertical_align_top` (P1t): the same pages. y is not asserted against Chrome (F183c).
3. `…_nested_in_a_section` (P5, P5b). Red on main if H3 holds.
4. Guards (green on main; step 1 confirms, else they become red tests): P1e, P3, P3s, P4 (2 / 3), P4b (moves whole, naming ADR-0010).
5. `page_bottom_warning.rs`: P1 and P5 join the "must not warn" list. P2 joins "must warn", with a comment naming F213b.

**Fork** (red first):
- Lib tests:
  - `monolithic_line_carrier`: true for text plus a box; false for a box alone on its line, for two lines, for a line with no box, for a line taller than a page;
  - `a_one_line_carrier_taller_than_the_room_moves_whole`: the carrier is one fragment on page index 1, and the box's fragment is on the same page;
  - `…_in_a_wrapper`;
  - the F30f carrier test unchanged.
- A `render_smoke` case.

**Byte gates** (1 and 4 rayon threads; a change in any of them stops the work and a postmortem is written):
- every fixture on main plus 43: byte-identical except 43;
- the 7 corpus documents;
- fork `fulgur-vrt`: the same failing set, and identical `.actual.pdf` files;
- `bench.py --check`.

**Sweep** `out/scratch/f213/sweep.py`:
- grid: room {40, 80, 120, 199, 201} × box height {100, 300, 1000, 1200} × content {empty, block text, img, svg} × align {baseline, top} × nesting {body, section, section after `p`} × lines {1, 3 (box on 3), 5 (box on 3)};
- every changed case must put each word on Chrome's page;
- any case that moves further from Chrome stops the work.

## 5. Steps

1. **Rebuild and measure (no code).** Rebuild the §1 probes; measure Chrome and main, including words, y and warnings; write the geometry-dump fork test; record which hypothesis holds in §7. If neither H1 nor H1b holds, stop and re-plan.
2. **Red tests and fixture.** Our red tests and guards, fixture 43 with its Chrome reference (`render.mjs`) and README; commit.
3. **Fork, gates.** `monolithic_line_carrier` with its unit tests; the two gates. Fork lib tests, clippy, fmt.
4. **Fork, wrapper.** `leading_block_size_with` (P5); the wrapper lib test; `render_smoke`.
5. **Bump and gate.** Bump `Cargo.lock`; our tests, byte gates, sweep.
6. **Finish.**
   - `render-fixtures.sh` and `compare.py`; look at 43's side-by-side.
   - Run `vrt-validator`. No `pdf-reviewer`: this is layout only.
   - Update the findings file (F213 done; F213b and F213c filed) and `docs/upstream.md`; write §7.

## 6. Risks and non-goals

- **ADR-0004.**
  - An inline-block alone on its line must still be entered. Otherwise F30f's split, fixture 16 and `wrapper_line_split.rs` move. So the check excludes it.
  - A shared-line box taller than a page stays unsplit, as ADR-0004 decides.
- **Page-bottom gaps.** Moving the line leaves a gap at the page bottom, as in Chrome. Documents where main overflowed may gain the page Chrome has.
- **Wrapper moves.** The leading change can move a wrapper (and F215 keep chains) whose first child is such a line. That is intended; the sweep watches it.
- **Capacity.** The 1010 px capacity (F30f-c) and exact fits use the existing `+1.0` slack, with no new tolerance.
- **Non-goals:** vertical-align (F183c); the strut (F183b); `inline-flex` sharing its line (F30f-b); images taller than a page (F32b, F183g).
- **To file:**
  - **F213b**: a one-line carrier taller than a page loses its baseline words; Chrome slices it. This needs an ADR-0004 amendment.
  - **F213c**: a multi-line inline root that a split barrier (flex item, table cell) keeps off the line path and that the gate enters. File it only if step 1 or the sweep shows one.

## 7. Evidence

(to be filled in by the implementer)
