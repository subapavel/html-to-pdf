# F187 — an absolutely positioned child of `<body>` is placed in the wrong frame (and F92, F40)

Date: 2026-10-27. Finding: F187 (`2026-10-15-m0-findings.md`). F187 is the same defect as F92 ("the body-direct abspos
window path shifts boxes by the body's offset", F30g step 1), so this plan closes both; it also folds in F40 (fixed boxes
`body_offset` too high on pages 2+), because the fixed pass makes the same subtraction (§3) — precedent F23 in F180. Fix
on `subapavel/fulgur` `htmltopdf/srgb-colours`; tests and fixture 26 are ours.

## 1. Problem

`body { margin: 30px; border: 4px solid #c00 }` with `<div style="position:absolute; top:50px; left:50px; width:100px;
height:100px">` (repro `scratchpad/f37v/cases/abs-m0.html`, `@page { margin: 0 }`): the box is at x 20..119 in our output,
50..149 in Chrome; y is right on page 1 (50). Cause: `append_position_absolute_body_direct_fragments` resolves the box
against the initial containing block correctly (`resolve_viewport_cb_location`), but `record_subtree_fragments_at_offset`
stores `x = icb_x - body_offset.0` and `y = icb_y - k*stride - body_offset.1` on every page (`pagination_layout.rs:7232`,
`:7380-7382`). The renderer uses the in-flow fragmenter's frame: `x = margin.left + frag.x` (no body x offset; in-flow
fragments already hold `body_x + child.x`, `:2030`) and `y = margin.top + body_offset.y + frag.y` on page 0 only
(`render.rs:348-352`, `draw_v2_page`). So x is short by `body_offset.x` on every page and y by `body_offset.y` on every
page after the first. `append_position_fixed_fragments` (`:6808`, `record_fixed_subtree_descendants`) does the same
subtraction (F40). Two more errors in the same root resolution: an axis with both insets `auto` falls back to Taffy's
static position, which is body-relative and misses the body origin (`left:50px` alone after a 100 px block: y 104 here,
134 in Chrome); a `position: relative` body is never the containing block (the window pass always uses the ICB, and
`abspos_flow::containing_block` looks only below `<body>`). Boxes F30g fragments (`abspos_x`, `locate_icb`) already use
the right frame.

## 2. Spec (from memory; step 0 checks Chrome)

- CSS 2.1 §10.1 (4): the containing block of `position: absolute` is the padding box of the nearest ancestor whose
  `position` is not `static`, else the initial containing block; §10.1 (1) / §13.2: in paged media the ICB is the page
  area (css-page-3 page model). With a static body its margin, border and padding do not move the box.
- CSS 2.1 §10.3.7 / §10.6.4: `left` / `top` (or `right` / `bottom`) are offsets from the containing block's padding
  edges, `%` of its width / height; with both insets `auto` on an axis the box keeps its static position (which includes
  body's offset).
- css-break-3: an absolutely positioned box is placed as if not fragmented, then sliced: `top: 2000px` lands on the page
  holding y 2000 of the stitched ICB.
- CSS 2.1 §9.6.1: in paged media `position: fixed` boxes repeat on every page relative to the page area (F40).

## 3. Where it belongs

fulgur's own frame mismatch between the geometry passes and `render_v2`. Taffy / Blitz are not involved (Taffy lays an
abspos box out against its parent — a sizing problem, F94 — and fulgur overrides the position). All CSS px in
`pagination_layout`; the renderer keeps its single `Px::in_pt()`; `render.rs` code does not change, only comments.

| Part | Where | Change |
|---|---|---|
| Frame rule | `pagination_layout.rs`, new `pub(crate) fn icb_to_fragment(page_index: u32, x_icb: f32, y_on_page: f32, body_offset_y: f32) -> (f32, f32)` | Pure: `(x_icb, y_on_page - if page_index == 0 { body_offset_y } else { 0.0 })` — the frame the in-flow fragmenter writes and `render_v2` reads (the F29 rule). Every pass writing ICB-placed geometry uses it. |
| Root position | new `fn body_direct_root_icb(doc, child, body, viewport_w_px, viewport_h_px) -> (f32, f32)` used by `append_position_absolute_body_direct_fragments` | Static body: an explicit inset axis from `resolve_viewport_cb_location` (unchanged); an `auto` axis = `body_origin + final_layout.location`. Positioned body: `body_origin + final_layout.location` on both axes (Taffy already resolves against body's padding box, insets and `%` included). |
| Storage | `record_subtree_fragments_at_offset::walk` | `stored_x` no longer reduced by `body_offset.0`; `stored_y` through `icb_to_fragment`, incl. the monolithic-continuation branch (0, pages ≥ 1). Nested abspos and CB anchors unchanged. |
| Positioned body in the flow path | `pagination_layout/abspos_flow.rs` `containing_block` | `Some(body_id)` when body's `position` is not `static`, so a long body-direct paragraph F30g flows stays in the window frame. |
| F40 | `append_position_fixed_fragments`, `record_fixed_subtree_descendants` | Pass the ICB `(x, y)` and apply `icb_to_fragment` per page instead of subtracting `body_offset` once. |
| Comments | `render.rs:110-113`, `:342-347`; `drawables.rs:498-504`; `body_origin_in_px` doc | x is page-area-relative; y page-area-relative minus body's y offset on page 0 only. |

## 4. Acceptance

New `crates/htmltopdf/tests/abspos_body.rs` (harness of `canvas_background.rs`; 96 dpi ink box of a `#0a0` 100x100 box;
1 px tolerance; A4; unless stated `body { margin: 30px; border: 4px solid #c00 }` at m0 and m15 (page area x 57..737,
y 57..1066, H 1009.17)). Nominal numbers; step 0 replaces them with Chrome's.

1. A1 `top:50; left:50`: m0 x 50..150, y 50..150; m15 x 107..207, y 107..207. Red today (x 20 / 77).
2. A2 `right:50; bottom:50`: m0 x 644..744, y 973..1073; m15 x 587..687, y 916..1016. Red (x 30 px short).
3. A3 `left:10%; top:10%`: m0 x 79..179, y 112..212; m15 x 125..225, y 158..258. Red on x.
4. A4 static position: a 100 px block, then the box with `left:50`, `top:auto`: y 134..234 at m0. Red (104).
5. A5 `body { position: relative }` with a 300 px in-flow block (body is the CB): `top:50; left:50` → 84..184 both axes;
   `right:50; bottom:50` → x 610..710, y 184..284. Red.
6. A6 nested positioned ancestor `<div style="position:relative; margin:40px 0 0 20px; height:200px">`: box at x 104,
   y 124. Guard (green).
7. A7 box under a static wrapper with `margin-left:20px` (CB = ICB): x 50, y 50. Guard (green; F30g `abspos_x`); if red,
   step 2 applies the same rule there.
8. A8 page 2 at m0: 150 one-line 20 px blocks, box `top:2000px; left:50`: page 2, x 50..150, y 877..977. Red (x 20, y 847).
9. A9 page 2 at m15: `top:1800px`: page 2, x 107..207, y 848..948. Red.
10. A10 y offset only: `body { margin: 0 }`, first child `margin-top: 40px` (collapses through body), `top:2000px` at m0:
    page 2 at y 877. Red (837).
11. A11 fixed (F40): body margin 30, `position:fixed; top:50; left:50`, 3 pages at m0: x 50..150, y 50..150 on every
    page. Red (x 20; y 20 on pages 2-3).
12. A12 guards (green before and after): A1 / A8 shapes with `body { margin: 0 }`; `abspos_flow.rs` (incl.
    `an_abspos_paragraph_with_a_body_margin_stays_in_the_page_area`); `body_offset.rs`; `canvas_background.rs`;
    `body_box_background.rs`.

Fork tests: unit tests for `icb_to_fragment` (page 0, page 2, negative y) and `body_direct_root_icb` (explicit, `auto`,
mixed axes; positioned body); update `position_fixed_top_zero_compensates_for_body_offset` (page 0 unchanged, plus a
2-page variant: `y == 0` on page 1, `x == icb_x`); a `render_smoke.rs` body-direct box in a margined body.

New fixture `fixtures/invoices/26-stamp-page-margin` (an invoice written for a browser): `@page { size: A4; margin: 0 }`,
`body { margin: 15mm }`, a body-direct `Zaplaceno` stamp `position:absolute; top:20mm; right:20mm` (inline style; the
compiled Tailwind has no position utilities); README; Chrome reference from `scripts/reference/render.mjs`. The PNG shows
the stamp 20 mm from the right page edge (today 57 px further left, over the header); its compare number falls from
main's binary to the after number; threshold set as for other new fixtures (worst page + 1 point).

Byte predictions: fixtures 01-25 and `bench/corpus` byte-identical (no body-direct abspos and no fixed box: the stamps
in 15 and `b200-mixed` sit in a `position:relative` wrapper; preflight gives body margin 0); `compare.py` and page counts
unchanged. fulgur VRT byte-identical (the only abspos fixture, `paint/opacity.html`, has margin 0). fulgur-wpt
`css-page` (`fixedpos-*`, `page-name-abspos-*`): no PASS → FAIL; a FAIL → PASS gets its expectation updated.

## 5. Steps

0. Chrome reference (Playwright `import { chromium } from "/opt/node-tools/node_modules/playwright/index.mjs"`,
   `printBackground`, `preferCSSPageSize`) for A1-A11 at m0 and m15; record each ink box and page in §7; take Chrome's
   number where it differs by more than 1 px. Also record Chrome's `left:50px; margin-left:10px` (input for F187b).
1. Ours: `abspos_body.rs` (red cases red, guards green); fixture 26 + reference + main's compare number on it.
2. Fork: `icb_to_fragment`, `body_direct_root_icb`, the storage change, unit tests; `cargo test -p fulgur`; VRT check.
3. Fork: positioned body as the containing block (root position + `containing_block`); a lib test with a flowed long
   paragraph in a relative body.
4. Fork (F40): the fixed pass on `icb_to_fragment`; update its test; run fulgur-wpt `css-page`.
5. Ours: lock bump; all green; `render-fixtures.sh` + `compare.py` (01-25 unchanged, 26 down); `bench.py --check`;
   `vrt-validator`; side-by-side A1 / A8 / 26. `pdf-reviewer` not needed.
6. Docs: findings (F187, F92, F40 done; F187b filed), `docs/upstream.md` fork row, §7.

## 6. Risks and non-goals

- `window_cuts_lines` and Keep / Flow: the choice reads window y, which moves by `body_offset.y` on pages ≥ 1, so a
  body-direct root in a margined body could switch between Keep and Flow; `abspos_flow.rs` and A8-A10 cover it, the
  validator sweeps it.
- Links and tagging: named destinations and link rectangles of abspos and fixed content move with the box (intended).
- WPT: `fixedpos-*` tests run with the UA body margin; a test and its reference moving together stay equal; any change
  is explained, not re-blessed blindly.
- Byte stability: with body offset (0, 0) the helper is the identity (A12, 01-25).
- Non-goals: F94 (Taffy sizes an abspos box against its parent: `left:0; right:0` in a margined body is 68 px narrower);
  F119; F93; F95; F187b (new: `resolve_viewport_cb_location` drops `margin-left` / `margin-top` for `left` / `top` while
  `abspos_x` adds it; shared with the fixed and nested passes, needs its own WPT check); `<html>` with a margin or
  `position`; relative offsets of body itself; RTL static position.

## 7. Evidence

(Step 0 Chrome table, before / after numbers, fixture 26 compare, WPT and VRT results, validator summary.)
