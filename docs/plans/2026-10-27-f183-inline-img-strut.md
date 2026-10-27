# F183 — an inline `<img>` sits on the floor of its line box (no strut below the baseline)

Date: 2026-10-27. Finding: F183 (`2026-10-15-m0-findings.md`). Status: **done** (Parley fork `1f23226`, Blitz fork `63628c03`; section 7.3) — the owner created `subapavel/parley` on 2026-10-27; fix branch `htmltopdf/inline-box-strut` on the
v0.6.0 release commit `3fe5c239`. Low impact on the product today:
Tailwind's preflight makes every `img` / `svg` `display: block`, so no Tailwind invoice (fixtures 01-26, `bench/corpus`)
has an inline baseline image; hand-written HTML with an inline logo or QR code does.

## 1. Problem

`<body style="margin:0;font:16px Inter"><img src="shape.svg"><div>…</div>` (`out/scratch/f180/shape.html`): Chrome leaves
about 4 px between the image and the next block, ours none. Blitz pushes the `<img>` into Parley as an `InlineBox`
(`construct.rs:1147-1153`, `whitespace.rs:229`). Parley 0.6's line metrics (`layout/line/greedy.rs:480-492`,
`BreakLines::finish`) count a box only as ascent and set `line_height = max(h)` (`compute_line_height` has a `TODO:
account for vertical alignment`, `layout/data.rs:232-235`). A line with no text run has no metrics of the root's font, so
no strut; a line with an image and text is wrong too (60 px image + 16 px text: line 60 tall where Chrome gives 64, the
image starts 2 px above the paragraph top). fulgur paints from Parley's `line_height` / `baseline`
(`convert/inline_root.rs:641-644`, `convert/mod.rs:1218-1221`); Blitz sizes the paragraph from Parley's height
(`layout/inline.rs:290-293`).

## 2. Spec

CSS 2.1 §10.8.1 (every line box starts with a strut — a zero-width inline box with the block's font and line height; a
replaced inline element with `vertical-align: baseline` puts its bottom margin edge on the baseline, so the strut's
descent plus half-leading stays below it); §10.8 (line box from the highest box top to the lowest bottom; a replaced
element's box is its margin box, no leading); §9.4.2 (a line with no in-flow content has zero height); css-inline-3 §2,
css-fonts-4 §5 (first available font for the strut). Quirks Mode Standard §3.3: Chrome drops the gap in quirks mode
(every fixture has a doctype).

## 3. Where it belongs

Parley owns the line geometry Blitz and fulgur both read. Blitz-only versions were rejected: adding the missing height to
the Taffy size breaks F78's rule that line slices tile the paragraph; inflating the box (`h + D`) fixes an image-only line
but puts the text baseline 2-4 px low on a line with text.

| Part | Where | Change |
|---|---|---|
| API | Parley fork `inline_box.rs`, `layout/mod.rs`, `layout/data.rs` | `InlineBox::baseline_aligned: bool`; `pub struct Strut { ascent, descent, line_height }` (layout units); `LayoutData::strut: Option<Strut>`; `Layout::set_strut`. |
| Line metrics | Parley fork `greedy.rs` `BreakLines::finish` | Only on a line holding a `baseline_aligned` box with a strut set: text ascent / descent / line height = max over text runs and the strut, rounded, leading split as today; `above = max(text_above, max box height)`, `line_height = above + text_below`, `baseline = y + above`. Every other line runs the old code (byte-identical). |
| Box flag | Blitz `construct.rs` (:1147-1153, :1200-1201), `whitespace.rs` (`InlineOp::InlineBox`, `replay`) | `InlineOp::InlineBox { id, baseline_aligned }`; `strut_box(node)`: `img` / `svg`, not abspos / fixed, computed `vertical-align: baseline`. |
| Strut | Blitz `construct.rs` `build_inline_layout_into` | `root_strut(font_ctx, style, scale)` only when an op has `baseline_aligned`: first available font, `skrifa` metrics at the root `font-size` (the source of Parley's `push_run`); `line-height: normal` = ascent + descent + line gap. |
| Patch | workspace `Cargo.toml`, the Blitz and fulgur fork workspaces | `[patch.crates-io] parley`, branch `htmltopdf/inline-box-strut` on the v0.6.0 release commit. |

## 4. Acceptance (nominal; step 0 measures Chrome)

New `crates/htmltopdf/tests/inline_img_strut.rs` (`#f00` 60 x 60 PNG with `width` / `height` attributes, next block a
`#00f` 20 px div, `@page { margin: 0 }`, body margin 0): S1 16px Inter → block top 64 (today 60); S2 `line-height:1.5` →
66; S3 14px → 64; S4 `line-height:1` → 62; S5 10 x 10 image → image 5..15, block 19; S6 image + text → image top 0
(today −2), block 64; S7 two wrapped images → 0, 64, block 128; S8 image in a flex item → 64; S9 header logo with padding
and a bottom rule → rule at 96 (today 92); S10 QR in a table cell → cell bottom 121 (today 117); S11 inline `<svg>` as S1.
Guards (green before and after): `display:block` image; `vertical-align: top / middle / bottom`; Tailwind
`inline-block`; an abspos inline image; an `inline-block` badge on a text line; `img_natural_size.rs`. Parley fork unit
tests (image-only line, box + text baseline, no strut → bit-identical, abspos 0 x 0 box → no strut); Blitz lib test
(a `div` holding a 60 px `img` is 64 px tall; `vertical-align: middle` → 60). New fixture `27-inline-logo` (plain CSS, no
preflight: inline header logo and a QR `<img>` in a payment cell) with its Chrome reference. Fixtures 01-26 and
`bench/corpus` byte-identical (preflight makes images block); fulgur VRT goldens with an inline baseline `<img>` move —
list them, check against Chrome.

## 5. Steps

0. Chrome reference for S1-S11 and the guards (+ a quirks-mode probe); check whether Parley `main` already fixed the
   TODO. 1. Ours: red tests, fixture 27. 2. Parley fork (once `subapavel/parley` exists). 3. Blitz fork. 4. Locks,
   gates, `vrt-validator`. 5. Docs (`docs/upstream.md` Parley fork row; F183 done; F183b-d filed).

## 6. Risks and non-goals

- The strut must use the face and metric source Parley's runs use (S6 checks it).
- Line heights still add up to the paragraph height (F78 slices, F30f continuations unaffected).
- Fallback if the owner declines the Parley fork: inflate the box by the strut for text-less lines in Blitz (fixes
  S1-S5, S7-S11, not S6; postmortem).
- Non-goals: F183b (`inline-block` on its last line's baseline); F183c (`vertical-align` other than `baseline` on inline
  boxes); F183d (strut on text-only lines of a small span); F37d (trailing `<br>`, a different code path).

## 7. Evidence

### 7.1 Step 0: Chrome reference (2026-10-27)

Chromium via Playwright (`printBackground`, `preferCSSPageSize`, print media), A4, rasterised with `pdftoppm` at 96 dpi; rows are
`[y0, y1)` of the red image, the blue 20 px block (`.blk`) and black ink, and `getBoundingClientRect` in brackets. Every case:
`@page { margin: 0 }`, `body { margin: 0; font: 16px Inter }`, the image `red60.png` 60 x 60 with `width` / `height`
attributes. Probe: `scratchpad/f183/probe.mjs`; the cases are the ones in `inline_img_strut.rs`.

| Case | Chrome | Nominal in section 4 | Main's binary |
|---|---|---|---|
| S1 16px | image 0..60, block 64..84 | 64 | block 60 |
| S2 `line-height: 1.5` | block 66 | 66 | 60 |
| S3 14px | block **63** (ascent 13.56 -> 14, descent 3.38 -> 3, rounded separately) | 64 | 60 |
| S4 `line-height: 1` | block 62 | 62 | 60 |
| S5 10 x 10 image | image 6..16 (DOM), block **20** | 5..15 / 19 | image 0..11, block 10 |
| S6 image + text | image 0..60, text bottom row 59, block 64 | 0 / 64 | block 60 (image top -2 in the section 1 text; the raster here starts at 0) |
| S7 two wrapped images (`width: 100px`) | images 0..60, 64..124, block 128 | 0, 64, 128 | one run 0..120 |
| S8 image in a flex item | block 64 | 64 | 60 |
| S9 header: `padding: 16px 0`, `border-bottom: 2px` | image 16..76, rule 96..98 | 96 | rule 92 |
| S10 100 px QR in `td` (`padding: 8px`, `border-bottom: 1px`) | image 8..108, border 120..121, cell bottom 121, block 121 | 121 | 117 |
| S11 inline `<svg>` 60 x 60 | as S1 | | as S1 on main |
| S12 `<img style="display: inline-block">` | as S1 | | as S1 on main |
| G1 `display: block` | block 60 | | green |
| G2 `vertical-align: top / middle / bottom` | block 60, 60, 60 | | green |
| G3 Tailwind preflight image | block 60 | | green |
| G3 empty `inline-block` 48 x 48 (F183b) | block **52** (box on the baseline; strut below it) | | 48, unchanged by this finding |
| G4 abspos image (`left: 200px; top: 100px`) | block 0..20, image 100..160 | | green |
| G5 `inline-block` badge on a text line | block 24 | | green |

Differences from section 4's nominal numbers: S3 is 63 and S5 is image 6..16 / block 20, because Chrome rounds the strut's ascent
and descent separately (SimpleFontData: `lroundf(ascent) + lroundf(descent) + lroundf(line gap)` is `line-height: normal`). The
strut therefore takes `line-height: normal` as that sum, not as the unrounded metrics, and Parley's leading rounding (which
mimics Chrome for explicit line heights) then adds nothing.

**Quirks mode.** The same S1-S12 without a doctype: Chrome drops the strut gap (S1 block 60, S2-S4 60, S5 image 0..10 / block 10,
S7 120, S9 rule 92, S10 117, S8 60, S11 60) but S6 stays 64 (text on the line). Ours has no quirks mode (a document without a
doctype renders as in standards mode), so `<img>` without a doctype is 4 px off Chrome. Every fixture has a doctype; not in scope
(filed as F183e, the general quirks-mode question).

**Parley `main`.** `subapavel/parley` `main` is at `dd6fe3a` (2026-10-26, Parley 0.12.0) and does fix this, with a different and
much larger API: `InlineBox { kind, baseline, vertical_align }`, `layout/style_metrics.rs` (the root span box is the strut,
CSS 2 section 10.8; "the strut only applies once the line has content"), and a rewritten `layout/line_break.rs`. Our base is the
0.6.0 release that Blitz 0.2.4 / Stylo are built against; moving to 0.12 would bump Parley, Fontique, Swash and the Blitz glue
and is a dependency bump, so the fork keeps the minimal change of section 3 on v0.6.0. When Blitz moves to a Parley with the
upstream fix, this fork branch is dropped (`docs/upstream.md`).

Fixture 27 (Chrome PNG, 96 dpi): header rule rows 121..123, QR rows 454..566, payment cell top border 440 and bottom border 583.
Main's binary: rule 117, compare **7.6 %** (one page, 1 / 1; main's number for the threshold).

### 7.2 After the fix (2026-10-27)

Forks: `subapavel/parley` `htmltopdf/inline-box-strut` `f551ab4` (v0.6.0 `3fe5c23` + one commit), `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` `e5b7feb8`; `fulgur` is unchanged (`3fa6f44b`, it reads Parley's `line_height` / `baseline`).
`Cargo.lock` points `parley` (and, because the Parley git source holds it, `fontique` 0.6.0, the same release source) at the
fork's git rev.

- `inline_img_strut.rs`: S1-S12, G1-G5 and fixture 27 all match Chrome within 1 px (the QR rows of fixture 27 within 3 px:
  the items table above carries 2 px of its own text drift against Chrome; the distance QR -> cell border is within 1 px of
  Chrome's 17). 18 tests, all red or the guards green before, all green after.
- Parley fork: 8 new unit tests (`tests/test_strut.rs`), 78 + 3 pass (`PARLEY_IGNORE_DECODING_ERRORS=1`: the snapshot PNGs are
  Git LFS pointers in the clone, so the image-snapshot comparisons are skipped; the bit-for-bit claim is covered by the
  new test that compares line metrics with and without a strut / flag).
- Blitz fork: 5 new lib tests; `cargo test -p blitz-dom --lib` 136 pass, 138 with `--features parallel-construct`.
- html-to-pdf: `cargo test --release` 767 passed, 0 failed (70 test binaries); `cargo fmt --all --check`, `cargo clippy
  --release --all-targets -- -D warnings` clean.
- Byte identity with main's binary: fixtures 01-26 and the seven `bench/corpus` documents render to identical bytes (no
  baseline-aligned box on any of their lines, preflight makes the images block). Only 27 differs.
- Compare (worst page): fixture 27 7.6 % on main -> 6.1 % (threshold 7.1 %); 01-26 unchanged (all at their thresholds).
- fulgur: `fulgur-vrt` renders the same 64 fixtures byte-for-byte with and without the change (35 differ from the committed
  goldens in this environment's fonts both before and after; the 35 actual PDFs are identical): no golden moves. `svg/shapes`
  is the only VRT fixture with an inline baseline `svg` and it has nothing below it. `cargo test -p fulgur --lib` (2349) and
  the image / inline-box / svg / smoke / link integration tests pass with the Parley fork patched in.
- Side by side (main / after / Chrome) of S1, S6, S10 and fixture 27: `out/scratch/f183/sbs.png`.

Not verified: the Parley snapshot PNG comparisons (LFS pointers); a variable font's `MVAR`-adjusted ascent for the strut
(`skrifa` is asked without variations, like nothing else in a fixture needs); quirks mode (F183e, section 7.1); the Blitz
fork's CI / clippy (not run); the bench numbers beyond fixture 27 (coordinator's gate).

### 7.3 vrt-validator fixes (2026-10-27)

The validator ran 249 cases (`out/scratch/f183v`, Chrome PDFs beside main's and the branch's) and found three defects in 7.2. Forks now:
Parley `1f23226` (on `f551ab4`), Blitz `fd40643d` and `63628c03` (on `e5b7feb8`).

1. **A short `line-height` pulled the next block up** (high): `line-height: 0` and a 60 px image gave block top 54, main and Chrome 60 (also
   `.25`, `.5`, `10px`, `calc(1px + .5em)`; text and a 30 px image with `line-height: 0`: 24, Chrome 30). The half-leading is negative,
   so `descent + leading_below` went below zero. CSS 2.1 section 10.8: the line box ends at the lowest box bottom, and a baseline image
   ends at the baseline, so the part below it is `max(descent + half-leading, 0)`. Parley `BreakLines::finish`: `below = (descent +
   leading_below).max(0)`; `above` stays at least the tallest flagged box; `min_coord` / `max_coord` are kept inside the line box
   (pagination reads them as the line's top and bottom).
2. **An image in an atomic inline was drawn 4 px low** (high): `<span style="display:inline-block"><img></span>`, a block inside an
   inline-block, an image in a `<button>` (`ib_img_bg`, `ib_img_text`, `ib_nested_div`, `ib_lines2`, `ib_qr_dl`, `img_in_button`, `ib_wrap`):
   the inner line is 64 tall (right) but the outer line puts the bottom of the inline-block on its baseline instead of the baseline of
   its last line, so the image sat 4 px lower than on main (Chrome: image 0..60, block 64). That is **F183b**, out of scope. Until
   F183b Blitz sets no strut when the inline formatting context is inside an atomic inline: `inside_atomic_inline` walks from the
   context's root up and stops at an ancestor with an inline-level outside display and an inside other than `flow` (`inline-block`,
   `inline-flex`, `inline-grid`, `inline-table`) or at an inline-level `<button>` / `<input>` / `<textarea>` (Blitz lays these out as
   atomic inlines although their `display` is `inline`, `construct.rs`). These cases are exactly main's again. F183b must remove this
   check when the outer line uses the last line's baseline.
3. **A flagged image grew a line with an unflagged box** (medium): 80 px `vertical-align: top` image plus a 40 px baseline image: 84,
   main and Chrome 80 (`c_mixed_vtop_base`; 50 px: 54 / 50). Parley draws every box bottom-on-baseline, so the baseline has to be at
   least as low as every box is tall. The strut path now keeps the height of unflagged boxes apart: `above` (strut and text part) uses
   flagged boxes and text only, `baseline = max(above, tallest unflagged)`, `line_height = max(above + below, tallest unflagged)`.
   Main's result whenever an unflagged box dominates (`ib_plus_img`: main's 100, Chrome 104 is F183b), the strut result otherwise.
   A line where an unflagged box is between `above` and `above + below` is up to the strut descent short of Chrome (its strut would
   still sit below the baseline): not a regression against main, and F183c's.

Tests (red before, green after): Parley `short_line_height_does_not_pull_the_next_block_up`,
`unflagged_box_taller_than_the_flagged_ones_keeps_the_old_line` (`flagged_boxes_and_a_smaller_unflagged_box_use_the_strut` as a guard;
it replaces `text_and_unflagged_box_next_to_a_flagged_one`, which asserted the 74 of finding 3); Blitz `an_img_inside_an_inline_block_has_no_strut_until_f183b`;
`inline_img_strut.rs` `v1_*` (two), `v2_*`, `v3_*` (two), with the Chrome numbers above (main's positions for v2 and the dominant
unflagged box).

Results: Parley lib 80 pass (`PARLEY_IGNORE_DECODING_ERRORS=1`); Blitz `blitz-dom --lib` 137, 139 with `parallel-construct`;
fulgur `cargo test -p fulgur --lib` 2349 pass with the forks patched in; html-to-pdf `cargo test --release` 777 passed, 0 failed (70 binaries; `inline_img_strut` 23), `cargo fmt --all --check` and `cargo clippy --release --all-targets -- -D warnings` clean; fixtures 01-26 and the seven `bench/corpus` documents byte-identical to main at 1 and 4
threads (the byte gate renders 27 with Tailwind, where images are block; 27 is judged by compare); compare: fixture 27 6.1 % as in 7.2,
all ok. The validator's 249 cases re-run: 26 changed against the first version of the branch; the red and blue rows of every one are now main's
(F183b: image in an inline-block / button, unflagged box dominant) or Chrome's (the `line-height` cases, `lh0_wrapper_logo`),
none is further from Chrome than main.

Not verified: the Blitz CI / clippy; a `<button>` or `<input>` check has an html-to-pdf test but no Blitz lib test; Parley clippy
(`-D warnings` fails on 7 sites in files this change does not touch, as before it).
