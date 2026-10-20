# F103: margin-box content is not aligned in its box (vertical; folds in F110, horizontal)

Date: 2026-10-20. Findings: F103 (`2026-10-15-m0-findings.md:609-612`) and F110 (`:646-648`), folded in (§3.3). Status: done (§7).

- The fix goes on the fulgur fork `subapavel/fulgur` `htmltopdf/srgb-colours`, after `0779580`.
- Blitz is not changed: the fix uses the F13 cell `vertical-align` and the F48 table height that are already in the fork.
- Line references are to `0779580` as read in `/home/user/fulgur`. I could not confirm the checked-out commit with git.
- I had no `css-spec-researcher` output.

## 1. Problem

Every `@page` margin box paints its content from the top-left corner of its rectangle.

**What the code does:**

- The rectangle is right:
  - top boxes have `y = 0` and `height = margin-top` (`gcpm/margin_box.rs:116-133`, `:321-327`);
  - bottom boxes have `y = page_h - margin-bottom` (`:150-167`, `:328-333`);
  - side boxes get their distributed slot (`:334-345`); corners use `bounding_rect` (`:102-113`, `:136-147`).
- Stage 3 of `MarginBoxRenderer::render_page` then lays the content out as plain block flow in a viewport of that size, from the top: `<body style="margin:0;padding:0;">{html}</body>` (`render.rs:5028-5038`). It is painted at `rect.x, rect.y` (`:5069-5101`).
- Nothing reads a default or an author `vertical-align` or `text-align`.
  - The author's declarations go on a `<div style>` (`:4867-4875`, kept as raw text by `gcpm/parser.rs:592-605`).
  - `vertical-align` on a block has no effect.
  - The `text-align` default is missing, which is F110.
- The margin document is paginated at the box height (`:5040-5044`) and only page-0 fragments are painted (`:5084-5089`). Lines that do not fit in the box are therefore dropped without a warning, as they are today.

**What it looks like** (`out/compare/*.png`, looked at):

- **F103 repro** (`/tmp/claude-0/pdfrev-f99b/docs/own1.html`): `@top-center` text at PDF y 832–842 pt, which is 0–10 pt from the top edge.
- **02-long p2:** footer about 29 px (≈ 21.7 pt) above Chrome's. `Strana 2 / 3` starts at the left of the `@bottom-right` box; Chrome ends it at the right content edge.
- **18-a5-receipt:** footer about 11 px above Chrome's and left-aligned; Chrome centres it.
- **17-running-footer:** the `element()` footer hugs the top of the 15 mm band. Its two flex halves also wrap to two lines each; Chrome's reference uses two string boxes on one line, so the shapes are not the same.

Expected arithmetic, Inter, `line-height: 1.5` from Tailwind preflight in the margin document:

- 18: (28.35 − 12) / 2 ≈ 8.2 pt ≈ 11 px lower. This matches the F6 observation.
- 02: (56.69 − 13.5) / 2 ≈ 21.6 pt lower. This matches the PNG.

Current compare numbers (`out/compare/summary.json`):

| Fixture | Per page | Threshold |
|---|---|---|
| 02 | 4.84 / 4.76 / 4.58 % | 5.9 % |
| 17 | 5.72 / 4.64 % | 6.8 % |
| 18 | 3.10 % | 4.1 % |

## 2. Spec

Everything here is cited from memory. Section numbers and wording are **unverified**.

- **css-page-3, the margin-box table** (§4.2 or its neighbour; "page-margin boxes"). Each of the 16 boxes has a UA default for `text-align` and `vertical-align`:

  | Boxes | `text-align` | `vertical-align` |
  |---|---|---|
  | `top-left-corner`, `bottom-left-corner` | right | middle |
  | `top-left`, `bottom-left` | left | middle |
  | `top-center`, `bottom-center` | center | middle |
  | `top-right`, `bottom-right` | right | middle |
  | `top-right-corner`, `bottom-right-corner` | left | middle |
  | `left-top`, `right-top` | center | top |
  | `left-middle`, `right-middle` | center | middle |
  | `left-bottom`, `right-bottom` | center | bottom |

  These are defaults, so an author declaration wins.
- **css-page-3 §5.3** (margin-box dimensions; §5.3.2 is the variable-dimension rules).
  - The outer height of a top or bottom box is the page margin.
  - The outer width of a side box is the page margin.
  - The box's own padding and border sit inside that size.
- **css-page-3: `vertical-align` in a margin box behaves as for table cells.** CSS 2.1 §17.5.3 then gives:
  - `top`, `middle`, `bottom` align the content in the box;
  - `baseline`, `sub`, `super`, `text-top`, `text-bottom`, `<length>` and `<percentage>` act as `baseline`, which for a lone cell is the top.
  - Content taller than the box: a table-cell row grows, so the content starts at the top.
- **CSS 2.1 §16.2 (`text-align`):** it applies per line box, so multi-line content is centred line by line.
- **Chrome** (margin boxes since about Chrome 131): it centres both ways by default (F6 observation, 02 and 18 PNGs). Two things are not known and are measured in step 0:
  - whether Chrome honours an author `vertical-align: top | bottom`;
  - what Chrome does with overflowing content.

  Where Chrome and the spec disagree we follow the spec and record it (AGENTS.md: Chrome is the reference, not the truth).

## 3. Where it belongs

**fulgur fork.** GCPM margin boxes are fulgur's own layer: Blitz, Stylo and Taffy do not know them. It does not belong in ours, because `htmltopdf` must not reach into `render_page`. No Blitz change is needed for option A.

### 3.1 Options

| | What | Decision |
|---|---|---|
| **A** | Stage 3 wraps the content in a fixed-size table, `display:table;table-layout:fixed;width:W;height:H` > `table-row` > `table-cell`. The cell's style is `text-align:<default>;vertical-align:<default>;<author declarations>`. | **Chosen, behind the step 2 gate.** See below. |
| B | Keep today's layout and read the computed `vertical-align` of the margin-box `<div>` (defaults first in its style attribute). Paint at `rect.y + dy`, where `dy = max(0, H − h) · {0, ½, 1}`. `text-align` comes in the same way as in A. | **Fallback if the A gate fails.** A margin-box background or border is then content-height and moves with the text (a spec deviation; F-new). |
| C | Lay out twice: once to measure, then again with `padding-top: dy`. | Rejected: it doubles the layouts of the 215 margin documents of `b200-items`, the cost F99b removed. |
| D | String-parse `vertical-align` from `rule.declarations` and use a flex column. | Rejected: it re-implements the cascade on raw text (`var()`, `!important`, repeats). |

Why A:

- The cascade lets an author `vertical-align` or `text-align` beat the default with no parsing of our own.
- Blitz already aligns cells (F13).
- The F48 table height is a minimum, so overflowing content grows the row and starts at the top. That is CSS 2.1 §17.5.3, and it is what the overflow case does today.
- A margin-box background or border fills the whole box, as §5.3 says it should.

Rules for the wrapper and why:

- **`div`s, not `<table>`:** the `<table>` element brings the UA `border-spacing: 2px` and Tailwind preflight's `border-collapse: collapse`, and both would move the content.
- **`table-layout: fixed`:** the cell must be exactly `W` wide. Otherwise an unbreakable word widens the table, where today's block lets it overflow (F59 `uses_fixed_algorithm`, which needs the table `width`).
- **An explicit `table-row`:** it keeps us off F50's anonymous-box path.

Why the gate is needed: A sends margin documents through table layout and the table paint path (F19, F22, F50, the fragmenter's row handling). Two things are unproven:

- that a table of exactly the page height `H` (Taffy rounding) stays on page 0 of the margin document;
- that the tagged structure tree gains no Table / TR / TD elements.

### 3.2 Design (signatures only)

**`gcpm/margin_box.rs`:**

```rust
impl MarginBoxPosition {
    pub fn default_text_align(self) -> &'static str;      // the §2 table
    pub fn default_vertical_align(self) -> &'static str;
}
```

**`render.rs`:**

```rust
struct ResolvedBox<'r> { measure_html: String, content_html: String, declarations: &'r str }
fn margin_box_cell_html(pos: MarginBoxPosition, b: &ResolvedBox<'_>, rect: MarginBoxRect) -> String; // A
fn margin_box_content_offset(v_align: &str, box_h: Pt, content_h: Pt) -> Pt;                           // B only, clamped >= 0
```

- **Stages 1a, 1b and 2 are unchanged.**
  - `measure_html` is exactly today's `html` (`:4867-4876`), so the measured widths and heights, and the rectangles, stay byte-identical.
  - The per-page cache counts (F99b: 201 / 215 on `b200-items`) are unchanged.
- **Stage 3 builds `margin_box_cell_html`.**
  - `W` and `H` come from `rect.width.in_px()` and `rect.height.in_px()`. No 72/96 factor appears anywhere.
  - Author declarations go through `escape_attr` as today.
  - The render-cache key (`:5015-5019`) is built on the new HTML. It must include the position's defaults: a `@top-left` and a `@top-right` with the same text and width would otherwise share one entry and one alignment.

### 3.3 Folding F110 in

F110 ("centre box text not centred") is the `text-align` column of the same UA table, applied at the same line of stage 3 to the same element. It moves the same fixtures (02's `Strana`, 18's footer) and the same goldens.

Splitting would mean:

- re-blessing about 12 VRT goldens, about 20 snapshots and 2 examples twice;
- pinning `page_from_css.rs:455-456` (`t11`) twice;
- writing a compare comment for 18 that cannot say which half moved what.

So the split would be artificial. Both findings close in this PR. Everything else from the box table stays out (§6).

## 4. Acceptance

**Step 0 probes (`out/scratch/f103/`).** Each probe is rendered by main's binary and by Chrome; record `pdftotext -bbox` y (top-left origin) for every word in §7.

| # | Shape |
|---|---|
| P1 / P2 | `@top-left/center/right` and `@bottom-left/center/right`, 9 pt, 15 mm |
| P3 | margins 10 / 20 / 30 mm |
| P4 | 7 / 9 / 14 pt |
| P5 | two lines (`"A\A B"` with `white-space: pre`, and a wrapping text in a third-width box) |
| P6 | author `vertical-align: top | bottom | baseline | 4px` on `@top-center` and `@bottom-center` |
| P7 | 5 lines in a 10 mm band (overflow) |
| P8 | `@left-top/middle/bottom` and `@right-*`, 25 mm side margins |
| P9 | the four corners |
| P10 | `element()` footer (fixture 17 shape), plus `@top-center { border-bottom: 1px solid; background: #eee }` |
| P11 | `counter(page)` over 3 pages |
| P12 | author `text-align: left` on `@bottom-center` |

**New `crates/htmltopdf/tests/margin_box_align.rs`** (A4 unless noted; strict; Inter).

- Helpers come from `page_from_css.rs`.
- `CHROME_*` constants hold the step-0 Chrome numbers, so the test needs no Chrome.

| # | Test | Asserts | Before the fix |
|---|---|---|---|
| 1 | `top_boxes_centred` | bbox centre within ±0.5 pt of Chrome's (P1) and within 1 pt of the band centre | red |
| 2 | `bottom_boxes_centred` | as test 1, for P2 | red |
| 3 | `centring_follows_margin_height` | P3 | red |
| 4 | `centring_follows_font_size` | P4 | red |
| 5 | `multi_line_block_centred` | first-line top gap and last-line bottom gap within 1 pt of each other (P5) | red |
| 6 | `author_vertical_align_honoured` | `top`: yMin ≤ band top + half-leading + 0.5. `bottom`: yMax ≥ band bottom − half-leading − 0.5. `baseline` / length: same as `top`. If Chrome differs, assert the spec and note Chrome in a comment. | red |
| 7 | `overflow_starts_at_box_top` | P7: the first line at the box top ±1 pt, and the printed words equal main's list (no new loss) | guard, green |
| 8 | `side_boxes_top_middle_bottom` | P8 | red |
| 9 | `corner_boxes` | P9: middle; `text-align` right (left corners) and left (right corners) | red |
| 10 | `centre_box_centred_horizontally` | F110: alone, and with left and right present; word centre within ±0.5 pt of Chrome's | red |
| 11 | `right_box_ends_at_content_edge` | 02 shape: xMax within ±0.5 pt of Chrome's | red |
| 12 | `author_text_align_wins` | P12 | guard |
| 13 | `running_footer_centred` | P10 `element()`; the link annotation rect still contains its word | red |
| 14 | `counter_footer_same_y_every_page` | P11 | red on y |
| 15 | `margin_box_background_fills_band` | P10, pixel probe at band top and band bottom. With fallback B: `#[ignore = "F-new"]`. | red |
| 16 | `tagged_struct_count_unchanged` | P10 with `tagged(true)` has the same `/StructElem` count as main | guard |

Other tests in ours:

- `page_from_css.rs` `t11`: the x pin at 28.35 becomes "centred on the page ±0.5 pt" (F110).
- `running_elements.rs` and `body_inline_split.rs:319` assert band membership only and must stay green.

**fulgur fork:**

- Unit tests for the 16-row table.
- `margin_box_cell_html` escapes declarations and puts the author's after the defaults.
- `margin_box_content_offset` (B only).
- Two cache-key tests:
  - `top_left_and_top_right_same_text_differ`;
  - the measure cache is unchanged.
- `render_smoke`: a box whose height is not a whole number of px (25.4 mm, 7.5 pt) is still painted (Taffy rounding guard).
- `gcpm_integration` and `link_integration` green.

**Fixtures:**

- 02, 17 and 18 must improve: footers in Chrome's place, `Strana` right-aligned, 18 centred.
  - Look at `out/compare/{02-long-p1..3,17-running-footer-p1..2,18-a5-receipt-p1}.png`.
  - Lower each threshold to the new worst page + 1 point.
  - **Any growth stops the step.** It is not "content that was missing", so the merge policy does not allow it. 17 is the one at risk, because its reference shape differs (§1).
- 01, 03-16, 19, 20 and `b200-text` / `b200-mixed` must be `cmp`-identical to main. They have no margin boxes, so stage 3 never runs.
- `b200-items` changes. Gate: `bench.py --check`, and its median within +3 % of main.

**Fork goldens.** They move. Re-bless each one only after `pdftotext -bbox` before and after shows that **only margin-box words moved**, by the predicted dy (and dx for non-left boxes) within 0.5 pt, and that body words are identical. Any other mover stops the step.

- VRT (12): `gcpm/{counter-page-numbers, element-policy-first, -first-except, -last, -start, leader-toc, margin-box-implicit-href, page-counter-footer-over-body, page-selector, running-element, string-set}`, `bugs/cover-page-break-after`.
- `gcpm_snapshot.rs`: every case with a margin box (about 20; list them in step 0).
- `examples/header-footer`, `header-footer-split`.
- Anything else that `grep '@\(top\|bottom\|left\|right\)-'` finds in step 0.

**Reviews:** `vrt-validator` with no high-severity finding. `pdf-reviewer` only if test 16 or the fork's tagged examples show a structure change.

## 5. Steps (each one day or less)

0. **Measure (half a day).**
   - Run P1-P12 with main's binary and with Chrome (scratch `chrome-pdf.mjs`, as in F36) and fill §7.
   - Grep every fork test and golden that has a margin box.
   - **Stop** and re-plan if Chrome does not centre top or bottom boxes by default.
1. **Red tests (one day).** Write `margin_box_align.rs` with the Chrome constants, and change `t11`. Record which tests are red.
2. **Gate A/B (half a day).**
   - Prototype A in the fork, patched locally.
   - Run P1-P12, the fractional-height boxes, fixture 17, `running_elements` R4 (a table and a flex footer), test 16, and `b200-items` five times.
   - **Use A** if everything paints and test 16 and speed hold. **Otherwise use B** and file the background deviation.
3. **fulgur, pure (half a day).** The defaults table, `margin_box_cell_html` (or `margin_box_content_offset`), and unit tests.
4. **fulgur, wiring (one day).**
   - `ResolvedBox`, stage 3, the cache key.
   - Lib and smoke tests.
   - Ours green against the local fork.
   - The fixture `cmp` matrix.
5. **Goldens (one day).** Run the per-golden bbox audit, re-bless, run fmt, clippy (`-D warnings`), lib, VRT and `examples_determinism` (10 of 11 as before), then push.
6. **Ours (one day).**
   - Lock bump, `cargo test --release --locked`, compare, look at the PNGs, thresholds and comments, `bench.py --check`.
   - Docs: F103 and F110 done in the findings, the `docs/upstream.md` row, §7.
   - Then `vrt-validator`.

## 6. Risks and non-goals

**Risks:**

- **Running elements (F102).** `element()` content keeps its own margins. A table cell is a block formatting context, so a child's `mb-4` counts in the centred height and does not collapse through. Step 0 checks this against Chrome (P10).
- **Inheritance.** The copy still inherits from the margin document, not from the page context (F102 §6). Different `line-height` / `font-size` defaults from Chrome's page context show up as a centring offset. If tests 1-4 miss ±0.5 pt only because of line height, record it and widen the tolerance to the measured value with a note. Do not fix inheritance here.
- **`counter(page)` / `counter(pages)`.** The text changes per page, but the position does not, so the cache keys still multiply exactly as before (one extra string per key). Speed is gated in step 2.
- **Images (F107).** Images are not painted today. When F107 lands, an image's natural size counts in the centred height, so its tests must include a centring case.
- **Author `margin` on a margin box no longer moves it** (table cells have no margins). Anyone who worked around F103 with `margin-top` sees a change; `padding` still works. §5.3.2 margins and `auto` are not implemented either way.
- **Author `display` on a margin box** (for example `flex`) turns the cell into a block. F50 then gives it an anonymous cell, which aligns at the top. Accepted and documented.
- **Rebase.** This touches `render_page` stage 3, which F99b also touched. Keep the change to one wrapper and one key.

**Non-goals:**

- §5.3.2 width resolution with an author `width`;
- margin-box margins;
- overflow handling beyond today's: lines past the box are still dropped without a warning. If step 0 shows that Chrome paints them, file a finding;
- merging `@page` and `@page :first` declarations for one box (today the more specific rule replaces the whole box, `render.rs:4834-4842`; file a finding if confirmed);
- F104, F105, F107, F117, F20 (tagging margin content);
- writing modes.

## 7. Result

Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `9e72c5b` (the change and its tests) and `cc1db38` (re-blessed snapshots, VRT goldens and `examples/header-footer{,-split}`), after `0779580`. No Blitz change. Ours: red tests `4765237`, then the lock pin to `cc1db38`, the compare thresholds, docs.

### Step 0: Chrome (Playwright Chromium, `pdftotext -bbox`, y from the top of the page, pt)

Chrome centres by default, top and bottom, so the stop rule did not fire. Probe files and output: scratch `f103/` (`probes.py`, `step0.txt`). Main = binary of `84f15a4`; "now" = the pinned fulgur `cc1db38`.

| Probe | Chrome | main | now |
|---|---|---|---|
| P1 `@top-left/center/right` 9 pt, 15 mm | y 15.28..26.17; TL x 42.52, TC centre 298.0, TR ends 553.49 | y 1.03; TC at 207, TR ends 403 | y 15.28; TC centre 297.6, TR ends 552.76 |
| P2 bottom, same | y 815.53..826.42 | y 800.40 | y 814.65 |
| P3 10 / 20 / 30 mm (top centre y, bottom centre y) | 13.98 / 28.23 / 42.48, 827.73 / 813.48 / 799.23 | 6.48 (all) | within 0.3 of Chrome |
| P4 7 / 9 / 14 pt | centres 20.71 / 20.73 / 20.41 | 4.95 / 6.48 / 9.91 | 20.71 / 20.73 / 20.41 (identical to Chrome) |
| P5 `"A\A B"` and a wrapped third-width box | A 8.53..19.42, B 22.03..32.92, wrapped lines 8.53 / 22.03, TR5 15.28 | 1.03 | identical to Chrome |
| P6 author `vertical-align` | `top`, `baseline`, `4px`, `super`: top (yMin 1.03); `bottom`: yMax 41.17; `middle`: 15.28. Bottom band: top 801.28, bottom 840.67, middle 815.53 | all top | same as Chrome (bottom band 0.88 higher, below) |
| P7 five lines in 10 mm | centred, overflowing both ends: L3 L4 L5 at 8.53 / 22.03 / 35.53, L1 L2 off the page; bottom M1-M3 at 795.28.. | L1 L2 at 1.03 / 14.53, M1 M2 | same words as main (L1 L2 M1 M2), first line at the top: spec, not Chrome (F139) |
| P8 side boxes, 25 mm | LT/RT 43.78, LM/RM 415.78, LB/RB 787.78 (y min); x centre 35.24 / 559.84 | LT 43.55, LM 295.83, LB 548.12; x 0 / 524.41 | LT 43.55, LM 415.08, LB 786.62; x 30.4 / 554.1 |
| P9 corners | TLC x 53.27..70.49, TRC from 524.41, y 15.28 | all x at the corner edge, y 1.03 | TLC 53.65..70.87, TRC 524.41, y 15.28 |
| P11 `counter(page)`, 3 pages | same y, centre 298.0 every page | x 42.52, y 800.40 | y 814.65, centre 297.6 |
| P12 author `text-align: left` | x 42.52 | same | same |
| P13 `CENTRE13` alone / between left and right | 275.51..320.50 both | 42.52 / 196.27 | 275.14..320.14 both |
| P14 shape of 02 (20 mm bottom) | Faktura x 42.52, `Strana 1 / 1` ends 553.50, y 808.03 | Strana at 303.76, y 786.23 | ends 552.76, y 807.98 |
| P10 `element()` footer | not available in Chrome (it ignores `running()`); a string box with the same rule and padding is 20.25 pt high | text line at 806.40 | 818.40 = main + (42.52 - 18.75) / 2 |
| P10 background and border | not measured in Chrome (no text); pixel probe `t15` | band-height only at the text | whole 15 mm band |

Two Chrome artefacts, not ours, set the tolerances: its 15 mm bottom band (and its side boxes) sit 0.88 pt (0.7 / 1.2 pt for the side ones) lower than the mm geometry, and its content area ends 0.74 pt further right (553.50, not 552.76). The tests allow 1 pt there and 0.5 pt elsewhere.

### Gate A / B (step 2)

**A chosen.** On the prototype: P1-P14 paint and match Chrome; the 16 tests pass (13 were red on main); `t07` prints exactly main's words (the row grows, page 0 keeps the lines that fit); `t16` (tagged, with a link in the footer) has the same two `/StructElem` as main and no Table / TR / TD; fixtures 17, `running_elements` (R4 table and flex footers) and `body_inline_split` are green; a band of 25.4 mm, 7.5 pt, 13.37 pt and 10.3 mm paints (`render_smoke`); `b200-items` +1.4 % (below). `t15` (background and border fill the band) is on, not ignored.

### Red and green

`margin_box_align.rs` on main's binary: `t01`-`t06`, `t08`-`t11`, `t13`-`t15` red (13); `t07` (overflow words), `t12` (author `text-align`), `t16` (tagged) are guards, green before and after. `page_from_css.rs` `t11` (centred on the page, was "starts at the margin edge") red before. All green now. `t17` (the `leader()` page number stays in the box) and `t18` (an author margin does not hide the box) were added after the golden audit found both bugs in the first wrapper (below); both fail on that first wrapper. Fork: 2203 lib tests (4 new: the 16-row table, `ResolvedBox`, `margin_box_cell_html`, the cache keys), 276 `render_smoke` (2 new), every other integration test, `fulgur-vrt` and `gcpm_snapshot` pass.

### Compare, per page, `out/compare/summary.json`, main `84f15a4` -> pinned `cc1db38`

02 4.84 / 4.76 / 4.58 -> 4.61 / 4.53 / 4.35; 17 5.72 / 4.64 -> 5.68 / 4.59; 18 3.10 -> 2.86 %. Thresholds: 02 0.059 -> 0.057, 17 0.068 -> 0.067, 18 0.041 -> 0.039. 01, 03-16, 19 and 20 are byte-identical to main (`cmp` of `htmltopdf.pdf`), so are `b200-text`, `b200-mixed` and the four `slow-*` documents. The PNGs `02-long-p1..3`, `17-running-footer-p1..2`, `18-a5-receipt-p1` were looked at: the footers sit at Chrome's height and are centred in the band; `Strana n / 3` ends at the right content edge; 18's footer is centred. 17's footer is still two wrapped lines of two flex halves against Chrome's one line of two strings (the reference shape, see the fixture README), so it moves 0.05 points only.

### Goldens (fork), `pdftotext -bbox` before / after

- 18 gcpm snapshots (the 19th has no margin box), 12 VRT goldens (exactly the 12 the plan listed), `examples/header-footer` and `header-footer-split`.
- In every file the word lists and page assignments are identical, and no word in the content area moved: only words of margin boxes, top and bottom ones by (band - line) / 2 (+21.00, +23.25, +22.50, +14.25 ... pt) with `-center` and `-right` boxes to the centre and the right edge, side boxes to top / middle / bottom, corners opposite. Examples: `gcpm_multipage_counter` 16 of 1216 words (the footer), `string-set` 6 of 158, `running-element` 14 of 130. `element-policy-*`: the string `first:` stays at y -1.22 (the box is 40 pt, its content taller: starts at the top) and only moves in x.
- The audit caught two bugs in the first wrapper before anything was blessed: `gcpm/leader-toc` lost the page number (`flex: 1; min-width: 0` leader fill of 400 characters stretched a cell that took its width from its content: fixed with `box-sizing: border-box; width: W` on the cell) and `examples/header-footer-split` lost `Page 2 / 2` (`@top-right { margin-top: 12px }`: Blitz puts a cell with a margin-top below its row and the fragmenter moves it off the margin document's page: fixed by forcing the cell's margin to 0).
- `examples/svg` and `examples/wasm-demo` differ from their committed PDFs on this host with and without the change (no margin box in them); not touched.

### Speed (`scripts/bench.py`, this VM: 4x Xeon 2.80 GHz, baseline machine 2.10 GHz, so only hard budgets apply)

`b200-items` median, four alternating rounds of 11 runs, main -> now: 2334 / 2249 / 2314 / 2382 (mean 2320) -> 2355 / 2355 / 2324 / 2373 ms (mean 2352), +1.4 % (gate +3 %). `bench.py --check` ok (b200-text 953, b200-mixed 1100, fixtures 22-55 ms). 210 pages and 604.5 kB as on main. The 2100 words that moved on `b200-items` are the footers, at y 808.0 instead of 786.2; no body word moved. The per-page cache counts of F99b (201 / 215) were not re-counted: `ResolvedBox::measure_html` is what stages 1a, 1b and 2 always measured (unit test), the render cache keys gain the position's defaults.

### Deviations from the plan

1. The cell also carries `box-sizing: border-box; width: <W>px` (the plan had only the table width): without it Blitz sizes the cell from its content (leader fill).
2. The cell's margin is forced to 0 with `!important` after the author's declarations (the plan accepted "author margin no longer moves it"; the real effect was that the box vanished). Blitz honours `!important` in a style attribute (probed).
3. `margin_box_content_offset` (option B) was not written. The "measure cache unchanged" test is a unit test of `ResolvedBox::new`, not a run of `render_page`.
4. Chrome cannot show `element()`, so `t13` asserts main's position plus (band - block height) / 2 (0.75 pt) and that the link rectangle still contains its word. Whether a child's margin in a running element counts in the centred height (plan §6, `mb-4`) was not compared with Chrome.
5. Tolerances: 1 pt for Chrome's 15 mm bottom band, right edges and the middle / bottom side boxes (Chrome's px snapping, above); 0.5 pt otherwise.
6. `vrt-validator` and `pdf-reviewer` were not run (the first is run by the owner's agent, the second was not needed: `t16` shows no structure change).
7. `running_elements.rs` `check` (a) compared the content-area words in pdftotext's reading order; with the footer and header centred in their bands pdftotext lists the first table row of R2 in another order (`Cena` before `Položka`, same positions). It now sorts the content words by page, y, x before the word-by-word comparison (still within 0.5 pt). Found by the full `cargo test`, not by the plan.
8. Not verified: `examples_determinism` of the fork's CLI gives 10 of 11 as before (`committed_svg_matches_rendered` fails with and without the change, no margin box in `examples/svg`); `@page :first` merging (F140) and Chrome's `display: flex` margin box (F141) were not probed; no Windows / macOS; the per-page cache counts of F99b were not re-counted. The fork's `Cargo.lock` was changed by the local Blitz path and not committed. Ours: `cargo test --release --locked` 569 passed, 0 failed, 4 ignored (as before); fmt, clippy `-D warnings` and `bench.py --check` ok.

New findings: F139 (lines past the box are dropped without a warning; Chrome centres the overflow), F140 (`@page :first` replaces the whole box), F141 (author margin / width / height / display on a margin box).
