# F184 — `html` / `body` backgrounds are painted on their boxes, not propagated to the canvas

Date: 2026-10-27. Finding: F184 (`2026-10-15-m0-findings.md`). Fix on `subapavel/fulgur` `htmltopdf/srgb-colours`; tests
are ours.

## 1. Problem

Repro harness from the F180 vrt-validator (`scratchpad/f180v/cases/07*.html`, `@page { size: A4; margin: 0 }`; Chrome
PNGs in `out/07*-*.chrome-*.png`). `07-body` (`body { margin: 20px; background: url(quad80x40.svg) no-repeat 0 0 }`)
draws the image at the body box (20, 30), Chrome at (0, 0). `07b-html` (`html { background: url(...) repeat }`) tiles
only the html box (y 0–360 of 1123), Chrome the whole page. `07c` (body `repeat`, 3 pages) restarts the tile phase at
each page top and at x 20; Chrome is one continuous canvas (page 2 opens with a cut 17.5 px band). `07d` (body
`no-repeat 50% 50%`, html 2560 px tall) centres the image on page 1 (y ≈ 571); Chrome draws it once, on page 2 at
(357, 137). `07e`: the root fill stops at 594.75 pt on a 595.28 pt page (0.53 pt strip unpainted). Cause: `render.rs`
`render_v2` (:288-340) paints `<html>` and `<body>` as ordinary boxes in a per-page pre-pass (`paint_root_block_v2`,
:3599) — html at `(margin.left, margin.top)` with its layout size, body at its own offset with the content-area height;
nothing implements the canvas background. Colours have the same fault (a body `background-color` with the default 8 px
margin leaves white strips Chrome fills; F37's page-0 overshoot is the same pre-pass).

## 2. Spec (Backgrounds 3 from memory; the paged-media wording unverified — step 0 checks Chrome)

- css-backgrounds-3 §2.11.1: the root element's background becomes the canvas background; its painting area covers the
  whole canvas, but images are sized and positioned against the root element as if painted for it alone (positioning
  area from the root's `background-origin`); the root paints no background of its own; `background-clip` does not
  limit the canvas.
- §2.11.2: if the root is HTML `html` with `background-image: none` and a transparent `background-color`, the computed
  background of its first `body` child is used instead, as if specified on the root; body's own used background is then
  the initial value.
- css-page-3 §3.3 (unverified): the canvas is painted within each page's page area; the page box's own background
  (`@page`) is separate and painted under it. Evidence: F37 measured Chrome's propagated body fill ending at 1065 px = the
  bottom of the A4 / 15 mm page area.
- css-break-3 §3: the root box is one fragmented box; page k shows root-y `[k·H, (k+1)·H)` (H = page-area height), so a
  repeating canvas background is continuous across pages (Chrome `07c`, `07d`).

## 3. Where it belongs

Engine painting: the fulgur fork. Stylo computes but does not propagate backgrounds (that is paint time, in every
engine), and fulgur paints html / body itself, so neither Blitz nor Stylo is involved. All rects in pt; the page area
from `resolved_content_area` (already pt); root-y offsets from px strip heights via `.as_px().in_pt()`, the only
conversion.

| Part | Where | Change |
|---|---|---|
| Source | `render.rs`, new `fn canvas_background_source(d: &Drawables) -> Option<NodeId>` | Root if its colour has alpha > 0 or it has a layer, else body if body does (§2.11.2), else `None`. Pure; unit-tested. |
| Painter | `background.rs`, new `pub fn draw_canvas_background(canvas, style: &BlockStyle, positioning: Rect, painting: Rect)` | Colour fills `painting` (no radius); each layer resolves size / position against the origin box of `positioning`, clipped to `painting` (layer clip and radii ignored, §2.11.1). |
| Layer core | `background.rs` `draw_background_layer` (:565) | Split into `draw_background_layer_in(canvas, style, layer, origin: (f32,f32,f32,f32), clip: ClipShape)`; the old fn wraps it unchanged (byte-identical). |
| Per page | `render.rs` `render_v2` pre-pass | Paint the canvas first: painting = the page area; positioning = the root's box at `(margin.left, margin.top − k·H)` with the root's layout size. |
| Phase | `pagination_layout.rs` `PageStrips` | `pub(crate) fn canvas_top(self, page: u32) -> f32` (= `page · page_h`, px), carried to `render_v2` so the phase follows the strips the content uses (F109). |
| Boxes | `paint_root_block_v2(..., paint_background: bool)` | html / body keep borders and shadows, but skip the background when they are the canvas source. |

## 4. Acceptance

New `crates/htmltopdf/tests/canvas_background.rs` (harness from `svg_background_scale.rs`: `pdftoppm -r 96`, ink
bounding boxes). Assets `quad80x40.svg`, `quad40x20.png` copied to `tests/data/`. Numbers are Chrome's (§7); the
approximate values here are read from the validator PNGs.

1. A, `07-body`: image at [0,80)×[0,40). Red today ([20,100)×[30,70)).
2. B, `07b-html`: html tiles cover rows 0–1122 of page 1. Red today (stop at 360). Body's own image not asserted (F37).
3. C, `07c`: on page 2 the tile rows continue the page-1 grid (offset 1122.5 mod 40); on every page x starts at 0. Red.
4. D, `07d`: one image, on page 2 at [357,437)×[137,177); pages 1 and 3 have no ink. Red today.
5. E, colour with `@page { margin: 15mm }`, `body { margin: 40px; background: #ffd }`: the page area [57,737)×[57,1066)
   tinted, the margin area white (or tinted, if step 0 says so). Red today (body box only).
6. F, the strip: `html { background: #00f }`, margin 0, rasterised at 300 dpi: the last 2 columns are blue. Red today.
7. G, gradient: `body { background: linear-gradient(#f00, #00f) }` on a 200 px document: stripes repeat every 200 px down
   the whole page (sized to the root box, painted over the canvas). Red today.
8. Guards (green before and after): html and body both coloured → html fills the page area, body only its own box;
   `body_inline_split::s6`; `svg_background_scale::small_raster_tile_on_body_covers_the_whole_page`; a running footer
   stays visible over a body colour.

Fork tests: `canvas_background_source` unit tests (root wins, body fallback, neither); a `draw_canvas_background`
origin / clip unit test; a `render_smoke.rs` image canvas background over 3 pages. fulgur VRT: 16 fixtures with
`body` / `html { background: white }` change PDF bytes (the white rect grows) but not pixels — verify with `pdftocairo`
that the pixel diff is 0; do not re-bless goldens in the fork (its VRT already has 26 environment-dependent diffs here).
Fixtures 01-25 and `bench/corpus` set no html / body background (no Tailwind `bg-*` on `<body>`, none in preflight):
byte-identical, pixel diff 0, page counts unchanged. No new invoice fixture; the integration test is the fixture
(precedent F180, F152, F177).

## 5. Steps

0. Chrome reference (`scratchpad/f180v/chrome.mjs`, `printBackground: true`, `preferCSSPageSize`) for cases 1-8, each
   with margin 0 and 15 mm; record in §7 the painted area (page area or whole page), the origin on page 1 and the page-2
   phase. Where Chrome differs from §4 by more than 1 px, take Chrome's number.
1. Ours: `canvas_background.rs`, cases 1-7 red, 8 green.
2. Fork: `canvas_background_source`, the `draw_background_layer_in` split (byte-identical), unit tests.
3. Fork: `draw_canvas_background`, `PageStrips::canvas_top`, the `render_v2` wiring and `paint_background`;
   `cargo test -p fulgur`; the VRT pixel check.
4. Ours: lock bump; all green; `render-fixtures.sh` + `compare.py` (unchanged); `bench.py --check`; `vrt-validator`;
   side-by-side of `07b` / `07d` for the report. `pdf-reviewer` not required (no writer, font or metadata change).
5. Docs: findings (F184 done; F184b, F184c filed), `docs/upstream.md` fork row, §7.

## 6. Risks and non-goals

- Visible changes that match Chrome: a body colour fills the whole page area; a letterhead / watermark on `body` moves to
  the root origin; a gradient on a short document's body repeats every root-height.
- The canvas fill must not feed `PaintExtent` (F111 overflow warnings): it is clipped to the page area and must not call
  `note_painted_box`.
- Root opacity and transforms: the canvas keeps today's `draw_with_opacity(root.opacity)` wrapper; Chrome's behaviour
  there is not measured.
- Non-goals: F37 (body's own box when it is not the source: page-0 overshoot, content-area height); F122 (overflow
  propagation); F185(f) (tile phase on ordinary split boxes); F185(b) (`background-attachment: fixed`); `@page {
  background }` (not parsed); html `margin` ≠ 0 as the positioning origin; F184b (Blitz lays out at a whole-px viewport,
  793 vs 793.7 px, so `100%` positions sit ≤ 1 px left); F184c (forced breaks and `break-inside: avoid` gaps make
  Chrome's stitched root taller than our layout height, so `%` positions differ in such documents).

## 7. Evidence

(Step 0 Chrome table, gates, before / after numbers.)
