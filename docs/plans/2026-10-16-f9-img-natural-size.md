# F9 — `<img>` without `width`/`height` attributes is not rendered

Date: 2026-10-16. Finding: F9 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours`, carried via `[patch.crates-io]`. No Blitz change.

## 1. Problem

`<img src="logo.png" width="120" height="36">` renders; `style="height:48px;width:auto"` (Tailwind `h-12 w-auto`) and a
bare `<img src="logo.png">` emit no image XObject. Fixture 03 has no navy "PI" logo top-left; Chrome draws it.

Root cause: the image bytes live only in fulgur's `AssetBundle`; Blitz's layout never sees them.

| Step | Where | What happens |
|---|---|---|
| 1 | `htmltopdf/src/lib.rs:239-256` | Images go into `AssetBundle` (`add_image`); no `base_path`. |
| 2 | blitz `mutator.rs:690-702` | Parsing `<img>` calls `net_provider.fetch(.., ImageHandler)`. |
| 3 | fulgur `net.rs:96-107,134-137` | `FulgurNetProvider` serves only files under `base_path`; never consults `AssetBundle`. |
| 4 | blitz `document.rs:669-682` | Not reached: `special_data` stays `None`. |
| 5 | blitz `layout/mod.rs:259-278` | `inherent_size = Size::ZERO`. |
| 6 | blitz `layout/replaced.rs:55,96-118` | ratio 0/0 = NaN; `height:48px` → 0×48; bare → 0×0; both attributes → 120×36. |
| 7–8 | fulgur `convert/replaced.rs:50,206-216`, `render.rs:2819` | `ImageEntry` 0 wide → no `draw_image`. |

Second defect: `convert/replaced.rs:202` takes only PNG/JPEG/GIF, so a bundled `<img src="x.svg">` never paints.

F23 (background tiles 4/3) has a different cause (missing px→pt in the background path).

## 2. Spec

CSS 2.1 §10.3.2 / §10.6.2 (auto sizes take the natural size; one dimension + ratio), §10.4 (min/max keep the ratio);
css-images-3 §5 (1 image px = 1 CSS px unless the format states a resolution); css-sizing-3 §5.2.2 (compressible
replaced elements); HTML §15.4.3 (`width`/`height` attributes as presentational hints + `aspect-ratio`); SVG as image:
root `width`/`height` give the natural size. Tailwind preflight: `img{display:block}`, `img{max-width:100%;height:auto}`.

## 3. Design (fulgur fork)

| File | Change |
|---|---|
| `blitz_adapter.rs` | `seed_image_natural_sizes(doc, assets)`: for every `<img>` whose `src` resolves via `assets.get_image` (same key as `convert_image`): raster → header-only `decode_dimensions` → `ImageData::Raster(RasterImageData::new(w, h, <shared empty Arc>))`; SVG → `usvg::Tree::from_data` (no host fonts) → `ImageData::Svg`; then clear the node cache and insert damage. |
| `engine.rs` | Call it right after `parse_html_with_local_resources`, before any pass that resolves (caption pass). |
| `convert/replaced.rs` `convert_image` | SVG asset → `SvgEntry`, as `convert_svg` does. |

Rejected: serving bundle bytes through `FulgurNetProvider` (full RGBA decode of every image; Blitz's SVG parse uses
host fonts; key drift). Units: Blitz gets natural px; `size_in_pt` converts once.

## 4. Acceptance

`crates/htmltopdf/tests/img_natural_size.rs` (96 dpi, margin-less, bbox of non-white pixels, ±1 px; W×H read from
`logo.png`'s IHDR): `bare_img`, `height_only_keeps_ratio`, `percent_width`, `width_attr_only`, `table_cell`, `svg_img`,
`both_attrs_unchanged`, `missing_src_renders_nothing`, `fixture_03_logo_matches_chrome` (resolve first why Chrome's
logo looks ≈150×37 px, not 48 px tall). Compare: 03 drops; 01, 02, 04, 05 byte-identical.

## 5. Steps

1. Ours: tests (red where stated), the 03 size question against Chrome 141.
2. Fork: raster seeding + call sites + fulgur unit / render_smoke test.
3. Fork: SVG seeding + `convert_image` SVG arm.
4. Ours: lock bump, compare, threshold, docs, `vrt-validator`.

## 6. Risks and non-goals

Seeding must happen before the first resolve; corrupt headers → no seeding (today's behaviour); viewBox-only SVG
natural size differs from Chrome. Not done: `<img>` in `@page` margin boxes (built with no assets), warning on an
unregistered `src`, JPEG EXIF orientation / density, `srcset`/`<picture>`, `width="50%"` attributes.

## 7. As landed

- Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `545b016` (bundled raster `<img>` get their natural size in layout:
  `seed_image_natural_sizes` feeds Blitz `Resource::Image` with header-only `decode_dimensions` before the first
  resolve), `5b43669` (bundled SVG `<img>` seeded and painted via `insert_svg_entry`). 4 + 1 unit tests, 3 + 1 smoke
  tests; 21 example goldens byte-identical.
- Blitz fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`: `4787ff3` — a block-level replaced element with
  `width: auto` is no longer stretched to the container width by Taffy's block layout (CSS 2.1 §10.3.4; Taffy 0.9.2
  `compute/block.rs` stretches every block child, `TODO: Allow stretch-sizing to be conditional`). Without it, F9 would
  have turned "logo missing" into "logo stretched to page width" for Tailwind's preflight `img{display:block}`.
  Only block-layout calls are affected (flex / grid / absolute pass `vertical_margins_are_collapsible = false`).
- Logo size question: `logo.png` is 240×72 px (no pHYs); `h-12 w-auto` → 160×48 CSS px; Chrome's "150×37" is the
  opaque part inside the PNG's transparent frame.
- `img_natural_size.rs`: 14 tests (bare, height-only ratio, `width:50%`, `width` attr only, table cell, SVG, both
  attrs, missing src, fixture 03 logo within ±2 px of Chrome, block-level not stretched / clamped by `max-width` /
  `width:100%` fills / `margin:0 auto` centres / flex items unchanged). Chrome 141 matches every case within 1 px.
- Compare: 03 6.2 → 5.7 % (threshold 0.072 → 0.067); 01, 02, 04, 05 byte-identical.
- Known: block-level `<svg>`/`<canvas>` with `width:auto` follow the same rule (not separately tested); the
  `@page` margin-box `<img>`, unregistered-`src` warning, EXIF orientation, `srcset` remain non-goals.
