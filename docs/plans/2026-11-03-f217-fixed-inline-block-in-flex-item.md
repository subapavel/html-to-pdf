# F217: a fixed-size inline-block in a flex item's text is measured without its size

Date: 2026-11-03. Finding: F217 in `2026-10-15-m0-findings.md` (medium: visible overlap; pilot list). Fork: `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`. Line numbers were read on 2026-11-03 from `/home/user/blitz` and from Taffy `7bbedd5` (`subapavel/taffy` `htmltopdf/minimum-contribution`, the commit pinned in `Cargo.lock`). Re-read them at the commit you build on.

## 1. Problem

The flex item does not count a non-replaced `inline-block` with a definite `height` that sits in the item's own inline content. Probe `d.html` (scratchpad `f217/`): `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}` with `<p>BEFORE</p><div style="display:flex;gap:16px"><div style="width:128px;flex-shrink:0">Platba:</div><div>Zaplaťte QR kódem <span style="display:inline-block;width:112px;height:112px;border:1px solid #000"></span> nebo převodem.</div></div><p>AFTER</p>`. Ours puts `AFTER` at 73.6 pt; Chrome m153 at 145.2 pt. The row is one 20 px line, so the next block is painted over the QR box. F213f §7.3 saw the same defect with `min-height`, with the box alone in the item, under `align-items:flex-start` and under `flex:1`. That probe also reports the box *drawn* about 20 px tall, so the finding's "over the 112 px box" needs re-measuring (step 1).

It does not happen in these cases:
- the same line in a block (`c.html`: ours 159.1 pt, Chrome 156.4 pt);
- the carrier wrapped in a `<p>` inside the item;
- a grid item;
- an auto-height box (`b.html`, `padding:50px 0`);
- an `<img>` (F213f K5).

## 2. Spec (none of these were checked against the published text: unverified)

- **css-flexbox-1 §9.2 step 3, 3.E.** The flex base size of an item with `flex-basis: auto` and an auto width is its max-content size. The item's contents keep their own sizes.
- **css-flexbox-1 §9.4 step 7.** The hypothetical cross size comes from "performing layout as if it were an in-flow block-level box with the used main size and the given available space, treating auto as fit-content". Nothing there suspends the sizes of the item's descendants.
- **CSS 2.1 §10.3.9 and §10.6.6.** An `inline-block` non-replaced element uses its specified `width` / `height`; only `auto` falls back to shrink-to-fit or to §10.6.7. **§10.4 and §10.7:** `min-*` / `max-*` apply.
- **CSS 2.1 §10.8 and §10.8.1, css-inline-3 §4 (line box height).** "For 'inline-block' elements, the margin box is used when calculating the height of the line box." The line box spans from the top of the topmost box to the bottom of the lowest one. For a box with no in-flow line boxes, the baseline is the bottom margin edge.
- **css-sizing-3 §5.1.** A box's intrinsic size contribution is its margin box under the given constraint, and that includes its preferred size.

## 3. Where it belongs

### 3.1 Root-cause hypotheses, ranked

**H1 (most likely): Blitz passes the flex item's `SizingMode::ContentSize` to the item's atomic inlines.**
- *The contract.* Taffy's `SizingMode::ContentSize` means "the caller already applied this node's own size, so ignore `size`". `compute_leaf_layout` also ignores `min_size` and `max_size` in this mode (`leaf.rs:37-43`). `compute_block_layout` ignores only `size` (`block.rs:90`); its `min_size` clamp still applies (`:235`).
- *Who sends it.* Taffy's flexbox sends `ContentSize` for every item:
  - the flex base size (`flexbox.rs:765`, `:808`);
  - the hypothetical cross size (`:1398`);
  - baselines (`:1469`);
  - the final layout, `calculate_flex_item` (`:1896`).
- *Who resets it.* Block, grid and table reset the mode for each child: `block.rs:262/377/445`, `grid_item.rs:390/429`, `grid/alignment.rs:209/222`, Blitz `table.rs:431`.
- *Where it leaks.* `BaseDocument::compute_inline_layout` (Blitz `packages/blitz-dom/src/layout/inline.rs:89-94`) builds `child_inputs` for the inline boxes with `..inputs`. The item's `ContentSize` therefore reaches each box (`:105`, and the absolute branch `:271`), and the box's own `height` / `width` are dropped.
- *What it explains:*
  - Block parent, grid item and table cell are fine: the inline root gets `InherentSize` there.
  - `<img>` / `<svg>` are fine: the replaced path, `layout/mod.rs:245-345`, never reads `sizing_mode`.
  - Padding still counts.
  - `min-height` is lost on a box with text: that box is an inline root, so it goes through the leaf path.
  - The box is drawn short: `calculate_flex_item` is `ContentSize` too.
  - Inline-flex boxes (`flexbox.rs:190`), inline-grid boxes (`grid/mod.rs:72`) and inline-table boxes (Blitz `table.rs:481`) should fail the same way.
- *What it predicts:*
  - A fixed **width** is lost too: the box is drawn narrow.
  - `min-height` on an empty box, or on a box holding only blocks, should be green (`block.rs:235`).

**H2: Taffy's cache key ignores `sizing_mode`.** `compute/mod.rs:171-187` keys on known dimensions, available space and run mode only. A box laid out in one mode can be served from the cache in the other. This alone does not explain the block/flex split. It would show as order-dependent sizes when the same box is measured from both paths.

**H3: Blitz's `ComputeSize` branch (`inline.rs:168-191`) measures a clone before the box sizes settle.** Unlikely: the box sizes are written before the clone.

**H4: fulgur records the box from another pass.** Unlikely: F213f saw the box drawn short, so the layout itself is wrong.

### 3.2 Decision

The fix goes in the Blitz fork, `inline.rs`, `compute_inline_layout`: `child_inputs` gets `sizing_mode: SizingMode::InherentSize`.
- **Not Taffy.** Flex must keep `ContentSize` for the item itself, or the item's own `width` would override the flexed size (`flex-shrink` would break). The inline formatting context is Blitz's algorithm, so Blitz must reset the mode for its children, as Taffy's block and grid code does for theirs.
- **Not Parley.** Parley takes `ibox.width/height` as it is given them.
- **Not fulgur.**

Record a new row in `docs/upstream.md`: "known, not reported; not checked against Blitz main".

## 4. Acceptance

### 4.1 Blitz fork unit tests (`inline.rs`, next to `trailing_forced_break_tests`)

Use the same `laid_out` harness: a `section` with `display:flex`, a `div` item, and a `span` box inside the item's text.

| Box (in a flex item) | Assertion |
|---|---|
| empty, `width:112px; height:112px` | box `final_layout.size` = 112 x 112; item height ≥ 112 |
| text, `height:112px` | box height 112 |
| text, `min-height:120px` | box height ≥ 120 |
| `inline-flex`, `height:112px` | box height 112 |
| `width:300px`, text | box width 300 |
| same, `flex-direction:column` | box height 112 |
| `position:absolute` box | size unchanged (guard) |

### 4.2 htmltopdf red tests: `crates/htmltopdf/tests/flex_item_inline_box.rs`

Same `HEAD` and `render`, words read back with `pdftotext -bbox`, as `tall_inline_box_line.rs`. Let `dy = y(AFTER) − y(BEFORE)` and `L = 15 pt + 85.5 pt` (one text line plus the box's 114 px margin box). The spec gives `L` as a lower bound, independent of either engine.

**Red today:**
- `r1` (`d.html`): `dy ≥ L`, and within 2 pt of Chrome (145.2 pt). Today 30 pt.
- `r2`, a box with `QR` text: `dy ≥ L`. No Chrome comparison: Chrome sets the baseline at the inner text line (F183c), which explains its block-path number too.
- `r3`, `min-height:112px` with text: `dy ≥ L`.
- `r4`, `inline-flex`: `dy ≥ L`.
- `r5`, `width:300px` with text: `x(nebo) − x(kódem) ≥ 225 pt`.
- `r6`, a column flex item: `dy ≥ L`.
- `r7`, fixture 46: in both rows, the first word of the block after the row is at least 84 pt below the label word.

**Guards, green before and after, byte-identical:**
- `g1` block (`c.html`);
- `g2` auto box (`b.html`);
- `g3` `<img src=red60.png style="height:112px">` (this answers "does `<img>` work?": predicted yes);
- `g4` a grid item;
- `g5` a table cell;
- `g6` `<p>` inside the item.

The Chrome constants come from the sweep, with a comment for each.

### 4.3 Fixture 46: `fixtures/invoices/46-qr-flex-payment-row/`

Payment rows built from classes that exist in the committed CSS (checked: `.flex`, `.gap-4`, `.w-32`, `.shrink-0`, `.inline-block`, `.w-28`, `.h-28`, `.border-gray-300`, `.rounded-lg`, `.p-4`). `inline-flex`, `min-h-28`, `size-28`, `p-2`, `h-24`, `flex-col` are missing, so variants needing them go in a `<style>`. No rebuild.

- **Row A:** label `Platba:`, then `Zaplaťte QR kódem <span class="inline-block w-28 h-28 border border-gray-300 rounded-lg p-4"><svg width="64" height="64" class="block" …QR…></svg></span> nebo převodem na účet 123456789/0300.` Size the svg below 78 px: at 78 px the auto height equals 112 and the row is green on main.
- **Row B:** label `Razítko:`, then `Převzal <span class="inline-block w-28 h-28 border border-gray-300 rounded-lg"></span> dne …` (empty: the visible overlap).
- A paragraph follows each row. Add a `README.md` and the Chrome `reference/`, made with `scripts/reference/render.mjs`.

**What the PNG must show:** both boxes 112 px square, and no text inside or over them.

**Threshold:** add a new `compare.py` line at the post-fix number plus 1. Record the main number in §7 for the before/after report.

### 4.4 Sweep against Chrome (scratchpad generator, not committed)

The sweep is a cross product:
- **Box:** `inline-block` / `inline-flex` / `inline-grid` / `inline-table`.
- **Content:** empty / text / block child.
- **Size:** `height` / `min-height` / `width`.
- **Context:** flex row item / flex column item / item with `items-start` / `flex:1` / grid item / table cell / block.

Plus `<img>` and `<svg>` with a height. Render with main and with the branch, Chrome m153 as in `render.mjs`.

**Pass when:**
- `dy ≥` the box's margin box plus one line;
- empty and block-child boxes are within 2 pt of Chrome;
- the grid / table-cell / block / replaced cells are byte-identical to main.

### 4.5 Byte gate

Fixtures 01–44 must be identical. None of them has a sized non-replaced inline box inside an inline root that is a flex item:
- 35 `.dot::before` (6 x 6) is in a `<p>` in a block section;
- 27 and 37 (`.meta div`, `dt` widths) are in blocks and table cells;
- 43 and 44 are in a `<p>`;
- 03's badges have no size.

No fixture 45 is in this checkout: check it at branch time. Also run the fulgur fork's `fulgur-vrt` and `examples_determinism` against the bumped Blitz; any golden change must have this shape and be named in the PR.

## 5. Steps (size: small; about 3 days)

1. **Confirm H1 (≤ ½ day).**
   - Write the §4.1 empty-box test; it should be red, with the box about 0 x 0 (or 2 x 2 with a border).
   - Log `inputs.sizing_mode` per `compute_inline_layout` call (not committed).
   - Set `InherentSize` in `child_inputs`; the test turns green.
   - Measure the box drawn in `d.html`, and the H1 predictions: width red; `min-height` on an empty box green.
   - Stop rule: if the flip leaves the test red, test H2 (clear the box's cache between passes), then re-plan.
2. **Red tests and fixture 46 in html-to-pdf (≤ 1 day).**
   - §4.2 tests;
   - fixture 46 with references;
   - run the sweep on main;
   - record the numbers in §7.
3. **Fix in the Blitz fork (≤ ½ day).**
   - Code: `child_inputs` gets `InherentSize`; its comment cites CSS 2.1 §10.6.6 and the Taffy contract.
   - Tests: the §4.1 tests.
   - Ship: push `htmltopdf/blitz-dom-0.2.4`; bump `Cargo.lock`.
4. **Gates (≤ 1 day).**
   - `cargo test`;
   - the byte gate;
   - the fulgur fork VRT and examples;
   - `scripts/compare.py`, then look at the PNGs of 46;
   - `scripts/bench.py --check`;
   - the sweep on the branch;
   - `vrt-validator`.
5. **Record (≤ ½ day).**
   - `docs/upstream.md` row;
   - findings: F217 done, residuals filed (for example H2 if seen);
   - §7 evidence.

## 6. Risks and non-goals

**Risks:**
- **Intended spread.** Every sized atomic inline in a flex item's text changes: widths too (badges, `<input>` / `<textarea>` leaf sizes). Text in such items may re-wrap. That is the spec behaviour; the sweep and byte gate bound it.
- **Percentage sizes.** A percentage `height` / `width` on such a box now resolves against `parent_size = available_space`, as on the block path today. It could differ from Chrome when the flex measure's available space is definite. The sweep has one `height:50%` case to see it; any fix is out of scope.
- **H2.** The cache key ignores `sizing_mode`, so the item itself still mixes modes. Not changed here.
- **Rebase.** One field in one struct literal; cheap on a Blitz bump.

**Non-goals:**
- the baseline of a box with text (F183c);
- multicol items in flex rows (F213f);
- Taffy's cache key;
- percentage heights of inline-blocks.

## 7. Evidence

Measured on 2026-11-03. Binaries: `main` = main c625feb (Blitz fork `3cea708f`), byte-identical to the F213d main binary; `new` = main + Blitz fork `5305fa41` (`4c3af6ae` red tests, `5305fa41` fix). The `Cargo.lock` change is the two `blitz-dom` / `blitz-html` source lines and nothing else (`cargo update -p blitz-dom --precise` also moved `swash`'s `skrifa` from 0.44.0 to 0.42.1, which is a dependency change and was reverted by hand; `--locked` builds).

### 7.1 Step 1: H1 confirmed

- Logging `inputs.sizing_mode` in `compute_inline_layout` (not committed): every flex item that is an inline root (node 4 in the tests, and the inner root of an `inline-flex` box) is laid out in `ContentSize`; the only non-flex case logged, the inline roots of the `position:absolute` test, got `InherentSize` (block, grid and table parents were not logged: the sweep's byte-identity covers them).
- Red before the flip: an empty `width:112px; height:112px` box in a flex item is **0 x 0** (row and column); a box with text and `height:112px` is 20 px; `min-height:120px` with text is 20 px; `inline-flex` `height:112px` is 20 px. As H1 predicts, the width is lost too (0, not 112) and `min-height` on an **empty** box is green (`compute_block_layout` keeps the `min_size` clamp).
- One line, `sizing_mode: SizingMode::InherentSize`, in `child_inputs`: every test is green, `cargo test --release -p blitz-dom --lib -- --test-threads=1` 169 passed (with the default thread count `svg_log_tests::a_rejected_inline_svg_is_a_warning` fails on main too: it reads a process-wide log and Stylo's debug lines of the other tests land in it). The stop rule (a red test after the flip, then H2) did not apply.
- Plan text that did not hold: "the box is drawn about 20 px tall" is true for a box with text; an empty box is 0 x 0. The finding's "over the 112 px box (84 pt)" is right for the empty box.

### 7.2 Step 2: red tests on main

`cargo test --release --locked -p htmltopdf --test flex_item_inline_box` on main: 7 failed (`r1`-`r7`), 6 passed (`g1`-`g6`). `dy` (AFTER minus BEFORE) before / after / Chrome: `d.html` 30.0 / 100.5 / 104.25 pt; box with text 31.5 / 100.5 / 100.5; `min-height` with text 31.5 / 100.5 / 100.5; `inline-flex` 31.5 / 100.5 / 100.5; `width:300px` (`kódem` to `nebo`) 56.0 / 266.0 / 266.0 pt (box 225.75 pt).

The plan's "within 2 pt of Chrome (145.2 pt)" for `r1` was wrong: Chrome's row is 104.25 pt, ours 100.5 pt, the lower bound exactly. The 3.75 pt (5 px) is the strut's descent under an empty `inline-block`'s baseline, F183b; the block path has it on main too (probe `e.html`, the same markup in blocks: Chrome 119.25 pt, ours 115.5 pt). `r1` asserts the bound and a ceiling of Chrome + 2 pt.

### 7.3 Fixture 46 (`compare.py`, 96 dpi)

| | main | new | Chrome |
|---|---|---|---|
| pixel diff, page 1 | 3.0 % | 2.8 % | |
| "Variabilní" (after row A), pt | 454.6 | 465.1 | 469.1 |
| "Děkujeme" (after row B), pt | 508.6 | 588.1 | 595.8 |
| row A box | drawn about 98 px, text over its bottom | 112 px square, nothing over it | 112 px |
| row B box | 2 px sliver | 112 px square | 112 px |

Threshold 3.8 % (2.8 + 1). The number hardly moves because the boxes are small; the product difference is in the PNG (`out/compare/46-qr-flex-payment-row-p1.png`). The remaining 3.75 pt per row is F183b.

### 7.4 Sweep against Chrome (scratchpad generator, 273 documents)

Box (`inline-block`, `inline-flex`, `inline-grid`, `inline-table`) x content (empty, text, block child) x size (`height`, `min-height`, `width`) x context (flex row, flex column, `align-items:flex-start`, `flex:1`, grid item, table cell, block) = 252, plus `<img>` and `<svg>` with a height and one `height:50%` box in each context = 21. Chrome m153, same page and font settings.

- **Byte-identical to main:** all 39 documents of each of the grid, table-cell and block contexts (replaced and `50%` ones included).
- **Flex contexts:** 92 documents change bytes (23 per context); the other 16 per context were already right (`inline-table` rows, `min-height` on an empty box, blocks in `inline-flex`/`inline-grid`, `<img>`, `<svg>`).
- **Against Chrome (flex contexts, 4 x 39 documents, `dy` and `dx` each):** on main 61 `dy` and 52 `dx` are off by more than 2 pt; on the branch **none** of the `dx`, and the only `dy` still off by more than 2 pt are 32 empty boxes (`height` and `min-height`, four box kinds, four contexts): all by the F183b 3.75 pt. No document that was within 2 pt of Chrome on main is outside it now.
- **Not changed, seen in the sweep:** `inline-grid` with `min-height` and text or a block child is 1.5 pt taller than Chrome on main and on the branch (102.0 vs 100.5 pt).
- Percentage height (`height:50%` in a flex item with text): `dy` 31.5 pt before and after, as Chrome (an indefinite basis is `auto`); the width changed to Chrome's 125.0 pt.

### 7.5 Gates

- Byte gate, fixtures 01-44 and the 7 corpus documents, `RAYON_NUM_THREADS` 1 and 4, `main` against `new`: **only fixture 46 differs** (both thread counts); each binary gives the same bytes at 1 and 4 threads. Fixture 45 is on an unmerged branch and was not part of it.
- `cargo test --release --locked --no-fail-fast`: 90 test binaries, 1065 tests passed, 0 failed.
- `cargo fmt --check`: clean. `cargo clippy --release --locked --all-targets -- -D warnings`: clean. Blitz fork: `cargo fmt -p blitz-dom --check` and `cargo clippy --release -p blitz-dom --all-targets -- -D warnings` clean.
- `python3 scripts/compare.py --threshold-check`: 45 fixtures, all within their thresholds, exit 0.
- `bench.py --check` (`HTMLTOPDF_BIN` absolute, `--skip-slow`; machine shared with two other builds, load 4-6): exit 0; the PDFs are byte-identical to main's, and the A/B on `b200-items` / `b200-text` is inside the noise (main 1366 / 750 ms and 1456 / 897 ms, new 1450 / 884 ms and 1490 / 864 ms). On that loaded machine `b200-items` is above the 1250 ms table exception for both binaries.
- Fulgur fork VRT (`fulgur-vrt` of `subapavel/fulgur` `htmltopdf/srgb-colours` `b81c678`, run from a copy of the cargo checkout so that `/home/user/fulgur` was not touched, Blitz as a path patch at `5305fa41`, Taffy and Parley at the pinned revisions): 35 of 64 fixtures differ from the committed goldens, as they do with Blitz `3cea708f` (the goldens are not ours); the 35 `*.actual.pdf` files of the two runs are **byte-identical** and the lists of differing fixtures are identical. `examples_determinism` was not run. `vrt-validator` was not run.

### 7.6 New observations (not part of F217)

- A flex item that is an inline root and has an explicit `width` larger than the container is not shrunk: `<div style="display:flex;width:300px"><div style="width:400px">text</div><div style="width:100px">Tail</div></div>`: Chrome shrinks the first item (Tail at 222.7 pt), ours keeps 400 px (Tail at 342.5 pt, byte-identical on main and on the branch); `flex:0 1 300px` and a block child do shrink. Probably the item's own `ContentSize` / `size` interplay in `compute_leaf_layout`.
- The 3.75 pt F183b gap above (already filed).

### 7.7 Final gates on the merge with main a734840 (2026-11-04)

Binaries: `main` = `htmltopdf-main-f213e` (main code through #93; main a734840 differs from it by docs only), `new` = this branch head after the merge (Blitz fork `5305fa41`, fulgur `ccaa0207`, `skrifa` as on main). `Cargo.lock` differs from main in the two Blitz source lines only; `cargo metadata --locked` passes.

- Byte gate, fixtures 01-46 and the 7 corpus documents (53 PDFs), `RAYON_NUM_THREADS` 1 and 4: **only fixture 46 differs** (both thread counts); each binary gives the same bytes at 1 and 4 threads. This time fixture 45 was part of the gate.
- `cargo test --release --locked --no-fail-fast`: 90 test binaries, 1083 passed, 0 failed.
- `cargo fmt --check`: clean. `cargo clippy --release --locked --all-targets -- -D warnings`: clean.
- `render-fixtures.sh` (`HTMLTOPDF_BIN` absolute) then `python3 scripts/compare.py --threshold-check`: exit 0, 46 fixtures all within their thresholds; fixture 46 2.8 % (threshold 4 %), page count 1/1. `out/compare/46-qr-flex-payment-row-p1.png` looked at: both boxes are 112 px squares, the QR code sits in row A's box, each paragraph stands below its box, as in Chrome (3.75 pt higher, F183b). `pdftotext -bbox` line positions: "Variabilní" 454.6 pt on main, 465.1 pt now (Chrome 469.1); "Děkujeme" 508.6 pt on main, 588.1 pt now (Chrome 595.8).
- `bench.py --check` (`HTMLTOPDF_BIN` absolute, machine shared with another build): exit 0. Fixture 46 median 14.8 ms. `b200-items` 1302 ms (above the 1250 ms table exception on this loaded machine, as in §7.5), `b200-text` 962 ms, `b200-mixed` 953 ms; scaling r = 3.57.
- Not run: `vrt-validator`, the fulgur fork VRT and `examples_determinism` (see §7.5 for the earlier fork VRT run).
