# F186 — every border side is drawn in its own colour

Date: 2026-10-27. Finding: F186 (`2026-10-15-m0-findings.md`). Fix on `subapavel/fulgur` `htmltopdf/srgb-colours`;
tests are ours.

## 1. Problem

`fulgur/src/convert/style/border.rs:11-13` reads only `clone_border_top_color()` ("use top border color for all sides
for simplicity") into `BlockStyle::border_color`, and `draw_primitives.rs` `draw_block_border` (:1660-1790) strokes all
four sides with it. So `<div style="border-bottom: 4px solid #c00">` draws a black bottom border (the top colour is
`currentColor`), and `border-color: #c00 #0a0 #00c #fa0` draws every side red (F37 vrt-validator; repro
`out/scratch/f37b/d.html`). Hand-written invoice CSS (`td { border-bottom: 1px solid #e5e7eb }` row separators,
`border-left: 4px solid <brand>` accents) prints dark or wrong-coloured lines. Tailwind fixtures escape it because
preflight and `border-*` utilities set `border-color` on all four sides.

## 2. Spec

css-backgrounds-3 §4.1 `border-top-color` … `border-left-color` (each side has its own colour; initial `currentColor`),
§4.4 the `border-color` shorthand (1-4 values, top / right / bottom / left), §4.5 `border` / `border-<side>` shorthands.
§4.? corner joins: where two sides of different colour meet, the colour boundary runs from the outer to the inner corner
of the border (Chrome draws a diagonal mitre); with `border-radius` it follows the curve.

## 3. Where it belongs

fulgur's style conversion and border painter (Stylo already computes the four colours). Units unaffected.

| Part | Where | Change |
|---|---|---|
| Style | `draw_primitives.rs` `BlockStyle` | Replace `border_color: [u8; 4]` with `border_colors: [[u8; 4]; 4]` (top, right, bottom, left — the order of `border_widths` / `border_styles`); helper `fn uniform_border_color(&self) -> Option<[u8; 4]>`. Update every constructor and test literal. |
| Conversion | `convert/style/border.rs` | Resolve `clone_border_{top,right,bottom,left}_color()` each against `ctx.current_color`. |
| Painter | `draw_block_border` | The rounded uniform path and the rectangular uniform solid / double path run only when the colours are uniform too (so all current output with one colour is byte-identical). Otherwise the per-side path draws each side with its own colour and opacity. Where adjacent sides differ in colour and both are non-zero width, each side is a trapezoid filled with its colour (outer edge to inner edge, mitred at the corners) instead of a stroked line, so the corners split diagonally like Chrome; same-colour joins keep today's lines. With radius and non-uniform colours: draw per-side trapezoids clipped to the rounded border ring (outer rounded rect minus inner rounded rect) — or, if that is out of reach, keep the straight per-side drawing and file it (F186b). |
| Other consumers | multicol `column-rule`, collapsed table borders (F22), outline if any | Check each reads its own colour source; anything that used `border_color` gets the right side. |

## 4. Acceptance

New `crates/htmltopdf/tests/border_colors.rs` (pixel probes at 96 dpi, `@page { size: A4; margin: 0 }`):

1. `border-bottom: 4px solid #c00` on a 100 px div: the bottom band is red (204, 0, 0), no black. Red today.
2. `border: 8px solid; border-color: #c00 #0a0 #00c #fa0`: the middle of each side is its own colour. Red today.
3. Corners: the same box at 16 px sides: the top-left corner pixel near the outer corner is split diagonally (top
   colour above the diagonal, left colour below), within 1 px of Chrome (step 0). Red today.
4. A table with `td { border-bottom: 1px solid #e5e7eb }`, `border-collapse: collapse` and `separate`: separator rows
   are light grey, not black. Red today (separate at least; record collapse).
5. `border-left: 4px solid #2563eb` accent + `border-radius: 8px` (non-uniform widths): the left band is blue, the other
   sides draw nothing. Check today's state; keep or fix.
6. Guards: uniform-colour borders with and without radius, dashed / dotted / double — byte-identical to main (render
   twice: main binary vs new, `cmp`).

Fork tests: unit test of the conversion (four colours, shorthand, `currentColor` per side); `render_smoke` cases for
mixed colours with and without radius. Fixtures 01-25 and `bench/corpus`: byte-identical (every fixture border is
uniform in colour) — if any fixture changes, its pixel diff must move toward Chrome and be named in the PR.

## 5. Steps

0. Chrome reference for cases 1-5 (Playwright `import { chromium } from "/opt/node-tools/node_modules/playwright/index.mjs"`,
   `printBackground: true`, `preferCSSPageSize`); record pixel colours and corner geometry in §7.
1. Ours: `border_colors.rs`, cases 1-4 red, guards green.
2. Fork: `border_colors` field, conversion, painter (uniform paths gated on uniform colour), corner trapezoids; tests;
   `cargo fmt`, clippy, `cargo test -p fulgur`; VRT failing set unchanged (pixel diff 0 where bytes change).
3. Ours: lock bump; all green; fixtures byte check; `compare.py`; `bench.py --check`; `vrt-validator`.
4. Docs: findings (F186 done, F186b if filed), `docs/upstream.md` fork row, §7.

## 6. Risks and non-goals

- Byte stability depends on the uniform-colour gate: every current document with one border colour must take the same
  code path as today.
- Non-goals: `border-image`; `inset` / `outset` / `groove` / `ridge` shading; F186b (radius with mixed colours, if
  deferred).

## 7. Evidence

(Step 0 Chrome table, gates.)
