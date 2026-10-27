# F180 — an SVG `background-image` is drawn at 4/3 scale (and F23, the same defect for rasters)

Date: 2026-10-27. Findings: F180, F23 (`2026-10-15-m0-findings.md`). Fix on `subapavel/fulgur` `htmltopdf/srgb-colours`;
tests are ours. F23 is folded in (as F110 was into F103): it is the same field and the same missing conversion.

## 1. Problem

Probe `out/scratch/f152/shape.html` + `shape.svg` (300 x 60, `viewBox="0 0 300 60"`, red and blue 40 px squares at
x 10 and 200) as `<img>` and as `background` of a 300 x 60 `div`. The `<img>` is correct; the background draws at 4/3:
the red square spans x ≈ 13.3–66.7 px, the blue one starts at ≈ 266.7 px and is clipped by the box. Cause:
`convert/style/background.rs:75-82` stores `tree.size()` (usvg user units = CSS px), and `:54-63` the decoded raster pixel
size, both as bare `f32` in `BackgroundLayer::intrinsic_width/height` (`draw_primitives.rs:1101-1102`). The draw path is
in pt: `background.rs` `resolve_size` (`:1875-1903`) returns `(iw, ih)` for `BgSize::Auto`, and `draw_background_layer`
(`:834-851`) hands that as a pt `Size` to `krilla_svg::draw_svg`, which scales the tree to it (krilla-svg 0.7.0
`lib.rs:63-73`): 300 px becomes 300 pt = 400 px. F23 (a 40 px PNG tiles at a 53.3 px pitch) is the same fault in the
raster arm. `list-style-image` is correct (`convert/list_marker.rs:39-40`, `:96-97` already use `.as_px().in_pt()`), and
so is SVG `<img>` (sized by Blitz layout).

## 2. Spec (from memory, unverified; step 0 checks Chrome)

- css-backgrounds-3 §3.9 `background-size: auto`: the image's natural size, via the default sizing algorithm
  (css-images-3 §5.3). §3.6 `background-position`: a percentage aligns point p of the image with point p of the area,
  offset = (area − image) × p. §3.4 `background-repeat`: tiles repeat at the image size.
- css-images-3 §5.1: an SVG's natural size comes from the outer `<svg>` `width` / `height` in user units = CSS px (SVG 2
  §8.2); a raster's natural size at the default 1 dppx is 1 image pixel = 1 CSS px (css-images-4 §6.1).
- css-values-4 §6.2: 1px = 1/96 in, 1pt = 1/72 in; px → pt is × 0.75, applied only through fulgur's `units`
  (AGENTS.md invariant).

## 3. Where it belongs

Engine behaviour, so fulgur, at its only construction site; the field is typed `Pt` so the draw side never sees px.
Converting inside `resolve_size` instead was rejected: it keeps a bare-`f32` field whose unit must be remembered — the
trap this bug fell into.

| Part | Where | Change |
|---|---|---|
| Field | `draw_primitives.rs` `BackgroundLayer` | `pub intrinsic_width: Pt`, `pub intrinsic_height: Pt` (doc: natural size, already in pt). |
| Raster arm | `convert/style/background.rs` `apply_to` | `(iw as f32).as_px().in_pt()`, same for `ih`. |
| SVG arm | same | `svg_size.width().as_px().in_pt()`, same for height. |
| Gradients | same, `resolve_{linear,radial,conic}_gradient` | Return `Option<(BgImageContent, Pt, Pt)>` with `Pt::ZERO` (no intrinsic size; output-neutral). |
| Consumer | `background.rs` `resolve_size` | `let iw = layer.intrinsic_width.to_f32();` at the f32 sink; logic unchanged. |
| Tests | `background.rs` `make_layer`, `test_svg_layer_resolve_size_contain` | Take `Pt`. |

Rows (F19) reuse `BlockStyle.background_layers` and are fixed by the same change. No Blitz / Taffy / Parley change.

## 4. Acceptance

New `crates/htmltopdf/tests/svg_background_scale.rs`: pages rasterised with `pdftoppm` at 96 dpi (1 device px = 1 CSS
px; harness from `svg_image_robustness.rs`); each case a block at a known offset, body margin 0; the bounding box of red
/ blue pixels (colour-distance threshold) inside the box within 1 px of the value from the spec, confirmed in Chrome in
step 0. Assets: `shape.svg`; a 12 x 12 SVG marker; `data/red16.png`.

1. `auto`, `no-repeat`, 300 x 60 box: red x 10–50 / y 10–50, blue x 200–240; equal to an `<img style="display:block">`
   copy within 1 px. Red today.
2. `repeat` (default), 700 x 60 box: red at 10, 310, 610; blue at 200, 500 (300 px pitch). Red today.
3. Position, 400 x 60 box, `no-repeat`: `100% 0` → image at x 100 (red 110, blue 300); `20px 5px` → red x 30 / y 15.
   Red today.
4. Explicit sizes: `150px 30px` and `150px auto` → red x 5–25, blue x 100–120. Green today (guard).
5. `contain` / `cover`, 600 x 60 box: contain → red 10–50; cover → red x 20–100, clipped at y 60. Green today (guard).
6. `list-style-image: url(marker.svg)`: marker ink 12 x 12 px, left of the item text; size equals Chrome's within 1 px
   (position not asserted). Green today (guard).
7. Raster (F23): `red16.png` `no-repeat` covers x 0–16; with `100% 0` in a 100 px box, x 84–100. Red today.

Fork tests: unit test that `build_drawables_for_testing_no_gcpm` gives a 300 x 60 SVG layer `intrinsic_width == 225 pt`;
`render_smoke.rs` SVG and raster background cases; fulgur VRT unchanged (its background fixtures are all gradients).
Fixtures 01-25 use no `url()` background and no `list-style-image` (grep incl. Tailwind `bg-[url(...)]`):
byte-identical, pixel-diff change 0, page counts unchanged. No new invoice fixture (none in scope uses the feature): the
integration test is the fixture, with the Chrome numbers in §7 (precedent F152, F177).

## 5. Steps

0. Chrome reference (Playwright `import { chromium } from "/opt/node-tools/node_modules/playwright/index.mjs"`,
   `printBackground: true`, A4) for cases 1-7, rasterised with `pdftoppm -r 96`; bounding boxes into §7. If Chrome
   differs from §4 by more than 1 px, take Chrome's number and cite the reason.
1. Ours: `svg_background_scale.rs`; cases 1-3 and 7 red, 4-6 green.
2. fulgur: field type, both arms, gradient `Pt::ZERO`, the `resolve_size` sink, unit + smoke tests; `cargo test -p
   fulgur`; `cargo test -p fulgur-vrt` unchanged.
3. Ours: lock bump; all green; `scripts/render-fixtures.sh` + `compare.py` (unchanged); `vrt-validator`; side-by-side of
   `shape.html` before / after for the report. `pdf-reviewer` not required (no writer, font or metadata change).
4. Docs: findings (F180, F23 done; F180b, F180c filed); `docs/upstream.md` fork row.

## 6. Risks and non-goals

- Documents with raster or SVG backgrounds change size — to the correct one (matches Chrome); anyone who tuned around
  the 4/3 sees a move.
- Smaller tiles mean 16/9 more tiles per area: a tiny tiled background reaches the existing `MAX_TILES` cap sooner; the
  cap's behaviour does not change.
- Gradients unaffected (intrinsic size 0; sized through `resolve_gradient_size`).
- Non-goals: F180b (an SVG with no natural size — `viewBox` only or percentage `width` — usvg reports the `viewBox` size
  or 100 x 100, §5.3 says contain-fit); F180c (list markers clamped to the line height; Chrome draws natural size);
  raster density (pHYs, EXIF; F9); `space` / `round` edge cases beyond the tile size.

## 7. Evidence

### Step 0 — Chrome reference (2026-10-27)

Playwright Chromium, `emulateMedia print`, `preferCSSPageSize`, `printBackground: true`, `@page { size: A4; margin: 0 }`,
page 1 rasterised with `pdftoppm -r 96`; ink = red (`r > 200, g < 80, b < 80`) / blue pixels inside the block, as
`[x0, x1) x [y0, y1)` relative to the block's corner (the block sits at (30, 20) px). Same measurement code as the test
(`ink`), same HTML; the Chrome script and its assets were `out/scratch/f180/` (not committed).

| Case | Chrome red | Chrome blue | Plan §4 |
|---|---|---|---|
| 1 `auto no-repeat`, 300 x 60 | [10,50) x [10,50) | [200,240) x [10,50) | same |
| 1 `<img style="display:block">` | [10,50) x [10,50) | [200,240) x [10,50) | same |
| 2 `repeat`, 700 x 60 | [10,49) [311,350) [611,650), y [9,48) | [200,240) [501,540), y [9,49) | same within 1 px |
| 3 `100% 0`, 400 x 60 | [110,150) x [10,50) | [300,340) x [10,50) | same |
| 3 `20px 5px` | [30,70) x [15,55) | [220,260) x [15,55) | same |
| 4 `150px 30px` and `150px auto` | [5,25) x [5,25) | [100,120) x [5,25) | same |
| 5 `contain`, 600 x 60 | [10,50) x [10,50) | [200,240) x [10,50) | same |
| 5 `cover` | [20,100) x [20,60) | [400,480) x [20,60) | same |
| 6 `list-style-image` 12 x 12 SVG | marker [41,53) x [3,15) (12 x 12) | | same size |
| 7 `red16.png no-repeat` | [0,16) x [0,16) | | same |
| 7 `red16.png no-repeat 100% 0`, 100 x 40 | [83,100) x [0,16) | | 84–100 within 1 px |

In case 2 Chrome's repeated tiles land up to 1 px off the ideal grid (red starts 10 / 311 / 611, ends 49 / 350 / 650, y
9–48: the tile is rasterised separately), the 300 px pitch is intact. Every number is within the plan's 1 px tolerance,
so the tests assert the plan's values.

(Gate results go here.)
