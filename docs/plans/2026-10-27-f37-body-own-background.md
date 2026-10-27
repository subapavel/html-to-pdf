# F37 — `<body>`'s own box is painted as tall as a page on every page, not as the fragments of its box

Date: 2026-10-27. Finding: F37 (`2026-10-15-m0-findings.md`, incl. the 2026-10-27 note). Fix on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after F184); tests are ours.

## 1. Problem

`html { background: #eee } body { background: #fff; margin: 40px }` with a 300 px `<div>` (the grey-page / white-card
invoice pattern, F184 vrt-validator): Chrome draws a white card from y 40 to 340 and grey below; we draw white from 40 to
the page bottom. Cause: `render.rs` `render_v2`, body pre-pass (:342-373), calls `paint_root_block_v2(body, margin.left +
body_offset.x, y, Some(content_area_h), ..)` on every page. On page 0 `y = margin.top + body_offset.y`, so the box ends
`body_offset` below the page area (the original F37 probe: `body { margin: 40px; border: 3px solid }` + 130 `<p>` at
15 mm closes the frame at 1106 px; Chrome leaves it open at 1066); on pages 1+ it paints a closed frame over the whole
page area; on the last page it ignores where body ends. `draw_v2_page` (:963-973) skips body's block paint, so the
pre-pass is body's only painter. Geometry: the block walk (`pagination_layout.rs:2074-2084`) records one body fragment
(page 0, y 0, height = layout height); the F111 inline-root path (:2031-2037) one per page with `line_split_insets`;
neither says where body's box ends on each page. When body is the canvas source (F184) its background is right but its
border and shadow have the same fault.

## 2. Spec (from memory; step 0 checks Chrome)

- css-backgrounds-3 §2.11.2: when the root has its own background, body's background is not propagated; it is painted on
  body's own box like any element's (painting area from `background-clip`, initially the border box, §3.7).
- css-break-3 §5.4 `box-decoration-break: slice` (initial): the box is rendered as if not fragmented, then sliced at the
  breaks; borders, padding and `border-radius` appear only at the real start (page 1) and real end (last page);
  backgrounds and shadows are one continuous image over the stitched box (`%` positions and tile phase refer to it).
- css-break-3 (box model for breaking): a box that breaks has its content box extended to fill the rest of the
  fragmentainer, so body reaches the page-area bottom on every page but its last.
- CSS 2.1 §10.6.3 / §8.1: an auto-height body ends at its last in-flow child plus padding and border; margins are not
  painted. css-break-3 §5.2: margins are truncated at breaks (body's `margin-top` applies on page 1 only).

## 3. Where it belongs

Engine painting and fragmentation, the fork: Blitz / Taffy give body the right layout box; the fragmenter and the html /
body pre-pass are fulgur's. Units: slices are px, as the strips; render converts with `Px::in_pt()` only.

| Part | Where | Change |
|---|---|---|
| Record | `pagination_layout.rs` `PaginationGeometry` | `pub box_slices: Vec<Fragment>`: body's border box on each page it spans, in strip coordinates; empty for everything else (struct literals get `..Default::default()`). |
| Slices | new `pub(crate) fn body_box_slices(strips: PageStrips, x: f32, w: f32, body_h: f32, end: (u32, f32)) -> Vec<Fragment>` | Pure. One page: `[0, body_h)`; otherwise page 0 `[0, strips.bottom(0))`, middle pages `[0, fresh())`, last `[0, min(end.1, bottom(last)))`. |
| Block walk | `fragment_pagination_root` | Track page and cursor of the last emitted child (not `page_index`: a trailing `break-after` must not add a slice); `end.1` = cursor + (`body_h` − `prev_bottom_y_in_body`) (trailing padding, border, inner margin). `fragments` unchanged (counters, dispatch, page counts untouched). |
| Inline root | `fragment_body_inline_root` | Slices from body's own fragments: every fragment but the last runs to `bottom(k)`, the last is `frame_fragment`'s height. |
| Paint | `render.rs`, new `fn paint_body_box_v2(canvas, entry, slices, page, x_pt, top_pt, page_size, paint_background)` | No slice on this page: nothing. One slice: `paint_root_block_v2` at its rect, no clip. Several: paint the unbroken box at `top − Σ earlier heights`, height `Σ heights`, clipped to the page width × the slice's y span (the clip opens to the page edge at the real top and bottom so shadows survive). |
| Helper | `fn slice_on_page(slices, page) -> Option<(f32 before, f32 total, &Fragment)>` | Pure; unit-tested. |
| Wiring | `render_v2` body pre-pass (:356-373) | Call `paint_body_box_v2`; `top_pt` = the same `body_top_pt` `draw_v2_page` gets; no `note_painted_box` (F111 safety net); no clip push when body paints nothing (fixture bytes). |

## 4. Acceptance

New `crates/htmltopdf/tests/body_box_background.rs` (harness of `canvas_background.rs`: `raster`, `is_colour`,
`first_red_run`, `tests/data/quad80x40.svg`). A4, `@page { margin: 0 }` unless stated; 20 px blocks. Nominal numbers;
step 0 replaces them with Chrome's.

1. K1 card: the problem CSS + `padding: 20px`, `<div style="height:300px">`: white x 40..754, y 40..380; grey at
   (400, 390) and (400, 600). Red today ((400, 600) white).
2. K2 three pages: html `#eee`; body `margin: 40px; border: 4px solid #c00; border-radius: 16px; background: #fff`;
   150 blocks. Page 1: top border rows 40..44, side borders to row 1122, no bottom border. Page 2: sides only, white at
   (400, 2) and (400, 1120). Page 3: bottom border at the content end (≈ 820..824), rounded bottom corners, grey below.
   Red today (closed frames on pages 2 and 3).
3. K3 overshoot at 15 mm: `body { margin: 40px; border: 3px solid #00f }`, 130 blocks: page 1 has no blue in rows
   1067..1122, sides end at or above 1066. Red today (frame closes at 1103..1106).
4. K4 image against body's box: K1 + `url(quad80x40.svg) no-repeat 100% 100%`: ink x 674..754, y 340..380. Red today.
5. K5 phase over the slices: html `#eee`; body `margin: 0 40px; background: url(quad80x40.svg) repeat`; 3000 px of
   content: the first red run on page 2 continues page 1's grid (offset 1122.56 mod 40, within 2 px — F184 rounding
   note). Red today (grid restarts at 0).
6. K6 body as canvas source with a border: `body { margin: 40px; border: 3px solid #00f; background: #ffd }`, 300 px:
   tint fills the page (F184), blue bottom border at rows 343..346. Red today (bottom border off the page).
7. K7 inline-root body (F111 shape): html `#eee`; body `margin: 40px; background: #fff`; 200 `LineNNN<br>` at 20 px:
   white to the page bottom except on the last page, where it stops at the last line; all 200 lines printed. Red today
   on the last page.
8. Guards, green before and after: `canvas_background.rs` (h2's "F37" comment points here), `body_inline_split.rs`,
   `body_offset.rs` (incl. `j`: body padding + border, 3 pages), `svg_background_scale.rs`.

Fork tests: unit tests for `body_box_slices` (one page, three pages, trailing padding, a trailing `break-after` adds no
slice, the clamp) and `slice_on_page`; a fragmenter test (body `box_slices` heights `[bottom(0), fresh(), end]`); a
`render_smoke.rs` bordered body with an image over 3 pages. Fixtures 01-25 and `bench/corpus` give body no background,
border or shadow (Tailwind preflight `border: 0 solid`, no `bg-*` on `<body>`): byte-identical, `compare.py` and page
counts unchanged. The fulgur VRT fixtures with a body background have body as the canvas source and no border: expected
byte-identical (`cmp`); any change must show a `pdftocairo` pixel diff of 0. Do not re-bless goldens.

## 5. Steps

0. Chrome reference (Playwright `import { chromium } from "/opt/node-tools/node_modules/playwright/index.mjs"`,
   `printBackground: true`, `preferCSSPageSize`) for K1-K7 at margin 0 and 15 mm; record in §7 body's top / bottom per
   page, which borders show, corner shapes, the K5 phase. Take Chrome's number where it differs by more than 1 px.
1. Ours: `body_box_background.rs`, K1-K7 red, guards green.
2. Fork: `box_slices`, `body_box_slices`, both body paths, unit tests — no output change (render does not read it yet;
   VRT byte-identical).
3. Fork: `slice_on_page`, `paint_body_box_v2`, wiring, smoke test; `cargo test -p fulgur`; VRT check.
4. Ours: lock bump; all green; `render-fixtures.sh` + `compare.py` (unchanged); `bench.py --check`; `vrt-validator`;
   side-by-side K1 / K2. `pdf-reviewer` not needed (no writer, font or metadata change).
5. Docs: findings (F37 done; F37b, F37c filed), `docs/upstream.md` row, §7.

## 6. Risks and non-goals

- Visible changes that match Chrome: a card body is as tall as its content; a body frame is open at page breaks; body
  images and gradients size against the stitched box (Σ slices) — the slice model, which also matches Chrome where
  `break-inside: avoid` gaps lengthen the box.
- Byte stability: the clip and the extra paint must stay off when body paints nothing, or every fixture's bytes change.
- `fragments` must not change (counters, `string-set`, tagging and `page_node_ids` read it; page counts come from it).
- Non-goals: F96 (slicing other split boxes); F37b (html's own border and shadow are still painted at layout size on
  every page); F37c (a body taller than its content's pages, `min-h-screen` with a margin, is clamped at the last content
  page where Chrome may add a page); `box-decoration-break: clone`; F38 (an offset ≥ the page height); F109 (`@page
  :first` sizes); Chrome's whole-px phase drift (F184).

## 7. Evidence

(Step 0 Chrome table, before / after numbers and the gates.)
