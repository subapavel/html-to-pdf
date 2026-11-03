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
- **Chrome (version of the committed references), predicted; measured in section 7** (step 1 measured with `scripts/reference/render.mjs` plus `pdftotext -bbox`; words per page, y of `START`, `BOXTOP`, `TAIL`, `AFTER`, page count):
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

### 7.1 Step 1: probes, Chrome and main

Measured 2026-11-03. Chrome is the Playwright Chromium of this VM, the version of the committed references (`Skia/PDF m153`), with
`emulateMedia(print)` and `preferCSSPageSize`, like `scripts/reference/render.mjs`. Ours is main 58f8992 (`Cargo.lock` pins fork
`7a79be15`), the prebuilt CLI binary, bundled Inter. Words and y (yMin in pt from the page top) from `pdftotext -bbox`; "lost" means the
word is on no page. The probes are in the scratchpad `f213/probes.py`, the bare HTML in `f213/p/`.

| Probe | Chrome | main |
|---|---|---|
| P1 | p1 empty; p2 `START` `BOXTOP` `TAIL` 41.7, `AFTER` 267.4 | p1 empty; p2 `AFTER` 43.6; `START` `BOXTOP` `TAIL` lost; warns 32.4 pt |
| P1e (empty box) | p2 `START` `TAIL` 256.2, `AFTER` 271.2 | p2 `START` `TAIL` 257.3, `AFTER` 270.1; no warning |
| P1t (`vertical-align:top`) | p2 `START` `TAIL` 40.9, `BOXTOP` 41.7, `AFTER` 267.4 | as P1; warns 32.4 pt |
| P2 (box 1200, after `S`) | p2 `START` `BOXTOP` `TAIL` 41.7; p3 `AFTER` 184.9 (box sliced) | p2 `AFTER` 43.6; the rest lost; warns 707.4 pt |
| P2a (box 1200, no `S`) | p1 `START` `BOXTOP` `TAIL` 41.7; p2 `AFTER` 184.9 | p2 `AFTER` 43.6; warns 100.6 pt |
| P3 (`<img>` 100x300) | p2 `START` `TAIL` 254.7, `AFTER` 269.7 | p2 `START` `TAIL` 257.3, `AFTER` 272.3 |
| P3s (inline `<svg>`) | as P3 | p2 `START` `TAIL` 257.3, `AFTER` 272.3 |
| P4 | p1 `L1` 647.7 `L2` 662.7; p2 `START` `BOXTOP` `TAIL` 41.7, `L4` 267.4, `L5` 282.4, `AFTER` 297.4 | p1 `L1` `L2`; p2 the rest (`START` 258.8, `L4` 271.6): same pages |
| P4b | p1 `L1` `L2`; p2 `START` `BOXTOP` `TAIL` 41.7, `AFTER` 267.4 (2 / 1) | p2 everything (`L1` 43.6, `START` 287.3): moves whole (ADR-0010) |
| P5 | p2 all four words (`START` 41.7) | as P1; warns 32.4 pt |
| P5b | p1 `x` 647.7; p2 all four words | p1 `x` 650.3; as P1; warns 47.4 pt |

Page counts: P1, P1e, P1t, P3, P3s, P5, P5b 2 pages in both; P2 3 in Chrome (2 in ours); P2a 2 in both; P4, P4b 2 in both.

**What differs from the prediction in section 2.**

- P1 / P1t / P5: Chrome's y is **not** "the line at the top of page 2 with the box below it". For a box with in-flow content Chrome puts the
  box's baseline on its last line box (CSS 2.1 10.8.1), so `START` is level with `BOXTOP` at the top of the box (41.7 pt) and the line is 300 px
  *below* the baseline. Ours seats every inline box bottom-on-baseline (F183c), so `START` is at the box's bottom (257.3 pt on page 2 after the fix)
  and, for a box of **fixed height with block content**, the box itself is painted hanging below that baseline: Q0, a fixed-height box with a block
  child, no `S`, on one page: Chrome `START` 41.7, `AFTER` 267.4; main `START` 257.3 (`BOXTOP` 257.3, the box's top edge also at 257), `AFTER` 270.1. An
  auto-height box (the fixture's) is placed like Chrome's within 5.6 pt (probe R_nofixh: label/`BOXTOP` 47.3 vs 41.7, `START` 287.3 vs 281.7). So the test "START within 1 pt
  of Chrome" cannot hold for P1; it asserts pages only. The y comparison is kept for P1e, P3 and P3s (within 1.1, 2.6 and 2.6 pt).
  This misplaced fixed-height box is **pre-existing and outside F213** (a box that fits, or a line that is alone, shows it the same way); to be filed with
  F183c.
- P1e and P3 / P3s are green on main, as predicted.
- P4 is green, as predicted. P4b moves whole, as ADR-0010 says; Chrome splits it 2 / 1.
- P3 first drew no image on ours ("not a bundled image": a `data:` URI is not a bundled image). The layout is the same; the red test registers `red60.png`.
- The plan's fixture assertion "`QR` appears once" is wrong: `QR Platba` is upper-cased and the sentence says `QR kódem`, so `QR` is printed twice.

### 7.2 Which hypothesis holds: H1 (with the box cut, a case of H1b)

Evidence (no fork test; the PDF is read back):

1. P1, P5: `qpdf --qdf` of page 1 shows the carrier's text painted **on page 1**, below the media box: `START` at y = 874.27 pt (page height 841.89,
   content bottom 799.37, so 32.4 pt past the edge, the figure of the warning), then `BOXTOP`, then `TAIL`. Page 2 holds only `AFTER`. So the
   carrier is not moved and its line is painted: nothing is "measured away" (H1b as the plan describes it does not occur for a box with one child).
2. Widening each page's MediaBox downwards and running `pdftotext -bbox` lists the words below the edge (`geo.py`): P1 p1 `START` `BOXTOP` `TAIL` at 864.1.
3. **The room threshold matches H1's F28 test.** Box with one block child, `S` leaving `room` px (fixed 300 px box, first row `BOXTOP`): `room` 10 and 15 px
   move the line to page 2 (Chrome: page 2 for all); 19, 21, 22, 25, 40, 200 px lose the words and warn. The box's own first row (the border and
   one 20 px line) is about 17 to 19 px; once it fits, `leading_block_size_with` reports "fits" and the carrier is entered in place.
4. **The leaf case differs as predicted.** A box with only an `<svg display:block>` child (first row = the whole 300 px) moves; a box whose child is text
   (an inline root, `D_text`) moves; an empty box (P1e) moves. Only a box whose first row is a *block child shorter than the room* loses its line.
5. **A box with several blocks is cut inside** (the H1b shape). Auto-height box = label + 300 px block `<svg>` + amount (`D_blksvg`, and fixture 43's QR box):
   page 1 gets the label (`BOXTOP` at 649.6 pt), the cut moves the `<svg>` and the amount to page 2 (`AFTER` at 268.6 pt on page 2, after the `<svg>`'s 225 pt), and the line
   is still painted on page 1 below the edge (`START` at 879.1 pt, widened). Fixture 43 on main: the label `PLATBA` stays at 740.3 pt on page 1, `Zaplaťte … 0300.` at 856.6 pt
   (lost), the amount `CZK` and `Děkujeme` on page 2. So the carrier does get a second fragment here, but the line goes to the first fragment, where its baseline is
   below the edge; it is not "painted nowhere". The remedy is the same: do not enter the line.
6. Reading the fork at `7a79be15`: `gate` is in `fragment_block_subtree`'s caller (`pagination_layout.rs`, `let gate = |doc, available_strip, keeps_whole|`, about line 3872 at that commit,
   not `:3879`); `would_split_block_subtree` (about 4496), `leading_block_size_with` (about 4638: the first-row walk that the F28 move at about 3913 uses, with the F30 /
   `abspos_line_split` shortcut only for an inline root of two lines or more) and `fragment_block_subtree` (about 5213), `fragment_inline_root` (about 9039) and
   `record_carrier_slices_after` (about 6762). The plan's line numbers were off by 10 to 300; its function names are right.

Not tested without a fork test: the exact `fragments` / `line_slices` of the carrier (the plan's dump). Evidence 1 to 5 settle which path runs, so the dump is only needed in step 3 as a
lib test. H2 (P2) is as predicted: lost with a warning (707.4 pt, 100.6 pt past the edge). H3 (P5, P5b) is the same as P1: lost with a warning.

### 7.3 Step 2: tests and fixture 43

Run on main 58f8992 with `CARGO_TARGET_DIR=/home/user/html-to-pdf/target cargo test --release --locked --offline --no-fail-fast -p htmltopdf --test tall_inline_box_line --test fixtures --test page_bottom_warning`.

| Test | Result on main |
|---|---|
| `tall_inline_box_line::p1_a_line_with_an_inline_block_taller_than_the_room_moves_to_the_next_page` | **red** (`START` printed 0 times) |
| `tall_inline_box_line::p1_a_box_of_several_blocks_moves_with_its_line` | **red** (`START` 0 times; the box is cut: `BOXTOP` p1, `AMOUNT` p2) |
| `tall_inline_box_line::p1t_the_same_with_vertical_align_top` | **red** |
| `tall_inline_box_line::p5_the_same_nested_in_a_section` | **red** (so H3 holds: the nested gate has the same hole) |
| `tall_inline_box_line::p5b_the_same_after_a_paragraph_in_a_section` | **red** |
| `tall_inline_box_line::g1_an_empty_box_moves_with_its_line` (P1e) | green |
| `tall_inline_box_line::g2_a_tall_image_moves_with_its_line` (P3) | green |
| `tall_inline_box_line::g3_a_tall_svg_moves_with_its_line` (P3s) | green |
| `tall_inline_box_line::g4_a_tall_box_on_the_third_line_splits_before_its_line` (P4) | green |
| `tall_inline_box_line::g5_a_tall_box_on_the_last_of_three_lines_moves_the_whole_paragraph` (P4b, ADR-0010) | green |
| `page_bottom_warning::warns_about_content_below_the_page_only` | **red** at the new silent entry "F213 P1" (warning 32.4 pt, the CLI's figure). With the two F213 silent entries skipped (temporary edit, not committed) the rest of the test, including the new "must warn" case for a 1200 px box (F213b), is **green** |
| `fixtures::qr_inline_block_invoice_moves_the_whole_payment_line_to_page_two` | **red** (`Zaplaťte` printed 0 times) |
| the other 20 tests of `fixtures.rs` | green (`every_fixture_renders_to_a_pdf_deterministically` includes 43) |

`cargo fmt --all -- --check` and `cargo clippy --release --locked --offline -p htmltopdf --tests -- -D warnings` pass.

**Fixture 43, main (`scripts/render-fixtures.sh` with the prebuilt main binary, then `scripts/compare.py 43-qr-inline-block-page-bottom`):** 2 pages, Chrome 2;
pixel diff **3.8 % (page 1) / 1.3 % (page 2), worst 3.8 %**. The number is low because the lost line is small and page 2 is nearly empty; the words are what the test checks.
The renderer warns "content extends 24.9 pt past the bottom page edge on page 1 and is not printed". The side-by-side shows page 1 with the stranded `QR PLATBA` label in a
box without a bottom, page 2 with the QR and amount but not `Zaplaťte QR kódem` / `nebo převodem na účet 123456789/0300.`. No threshold is set in `compare.py` yet; the fix PR sets
it to the worst page after the fix + 1 point (the default of 10 % holds until then).

Sizing: sixteen rows of 41's table. In Chrome the line with a 16 px stand-in for the box starts at 727.2 pt (about 100 px of room), in ours at 729.8 pt (about 97 px); fifteen rows give
about 60 px more room, seventeen about 37 px less (below the 33 px leading of the box, so the label would not fit and main would already move the line).
