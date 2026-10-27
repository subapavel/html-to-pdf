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

### Step 0: Chrome reference (2026-10-27)

Chromium via Playwright (`printBackground`, `preferCSSPageSize`, `@page { size: A4; margin: 0 }`, body margin 0), the
PDF rasterised with `pdftoppm -r 96` (794 x 1123, 1 px = 1 CSS px), the same way as the tests. Script and raw output:
`out/scratch/f186/chrome.mjs` (not committed). Palette: T `#c00` = (204, 0, 0), R `#0a0` = (0, 170, 0),
B `#00c` = (0, 0, 204), L `#fa0` = (255, 170, 0).

| Case | HTML | Chrome |
|---|---|---|
| 1 | `div { margin: 40px 40px 0; height: 100px; border-bottom: 4px solid #c00 }` | rows y 140..143 are (204, 0, 0) at every x; rows 139 and 144 white. No black. |
| 2 | `div { margin: 40px; width: 200px; height: 100px; border: 8px solid; border-color: #c00 #0a0 #00c #fa0 }` | box x 40..256, y 40..156; (148, 44) = (204, 0, 0), (252, 98) = (0, 170, 0), (148, 152) = (0, 0, 204), (44, 98) = (255, 170, 0); inside white. |
| 3 | same with `width: 88px; height: 48px; border-width: 16px` | box x 40..160, y 40..120. In every 16 x 16 corner square, with u / v the distance in px from the outer corner along the horizontal / vertical side: v < u is the horizontal side's colour, v > u the vertical side's, v == u one anti-aliased pixel (a mitre from the outer to the inner corner). E.g. top-left row y=45 is `LLLLL.TTTTTTTTTT`. |
| 3b | `border-width: 10px 4px 16px 8px`, `width: 88px; height: 48px` | box x 40..140, y 40..114. Boundary top-left runs (40, 40) to (48, 50): row y=45 is `LLLL.TTT`, row y=49 `LLLLLLL.TTTT`; bottom-left runs (40, 114) to (48, 98): row y=105 is `LLLL.BBBBBBB`. Same line rule: boundary at u = (v + 0.5) * w_vertical / w_horizontal. |
| 4 | `td { height: 29px; padding: 0; border-bottom: 1px solid #e5e7eb }`, 3 rows, `width: 300px`, `margin: 40px`, `border-spacing: 0` | `separate` and `collapse` alike: one pixel row of (229, 231, 235) at y 69, 99, 129; nothing else down x=200. |
| 5 | `div { margin: 40px; width: 200px; height: 100px; border-left: 4px solid #2563eb; border-radius: 8px }` | (42, 90) = (37, 99, 235); (39, 90), (140, 41), (237, 90), (140, 138), (100, 90) white. The band tapers along the curve at the top-left and bottom-left corners (rows y 40..51 have blue only in x 40..45). Without the radius: the same band, straight, full height y 40..139. |
| 3c | 3b plus `border-radius: 12px` (box x 40..252, y 40..166) | each side keeps its colour; the colour boundaries run through the curved corners, e.g. top-left row y=45 is `WW.L.TTTTTTTTT` (anti-aliased boundary pixels `.` between the sides). |
| g1 | `border: 6px solid #c00` | along y=98 red x 40..46 and 246..252; down x=140 y 40..46 and 146..152. |
| g2 | g1 plus `border-radius: 12px` | (40, 40) and (41, 41) white; sides red; same runs along y=98. |
| g3 | `border: 4px dashed #00c` | 18 dashes of 9 px with 3 px gaps along the top (fulgur: 9 dashes of 12 px, pre-existing, unchanged). |
| g4 | `border: 4px dotted #00c` | 27 dots along the top. |
| g5 | `border: 9px double #c00` | along y=98: red x 40..43, 46..49, 249..252, 255..258. |
| g6 | `border: 2px solid #c00; border-bottom-width: 8px` | down x=140: red y 40..42 and 142..150. |

Findings from the step-0 run on the unchanged engine (tests `border_colors.rs`): cases 1, 2, 3, 3b, 3c, 4 separate and 5
are red (top colour used for every side: black bottom band, red right side, black left accent, black row separators);
case 4 collapse is already green (the collapsed-border painter carries its own per-edge colour from the Blitz fork's
`collapsed_borders`), and guards g1-g6 are green.

### Steps 2-3: results (2026-10-27, fork `091fad98`)

- `border_colors.rs`: 14 / 14 green (cases 1, 2, 3, 3b, 3c, 4, 4 collapse, 5 and guards g1-g6). Pixels differing from Chrome by more
  than 40/255 in a whole A4 page (old -> new): c1 2856 -> 4, c2 3400 -> 8, c3 3728 -> 16, c3b 2240 -> 8, c4 separate
  900 -> 0, c5 414 -> 0 (left accent with `border-radius: 8px`: the band now tapers along the curve like Chrome),
  c6r (3b with `border-radius: 12px`) 4710 -> 6; g1, g2, g5, g6 and collapsed c4 unchanged (0 / 0 / 0 / 1 / 0).
- Uniform-colour borders take the old code paths: PDFs of g1-g6 and the collapsed table are byte-identical to the previous
  binary; the seven `bench/corpus` documents and fixtures 01-20, 22, 23, 25 are byte-identical.
- Two fixtures change, both borders that were black (the top colour is `currentColor`) and now have their own colour:
  21-display-contents (`border-bottom: 1px solid #d1d5db` row rules; compare 3.7 -> 3.2 %) and 24-columns-span
  (`.terms .span { border-bottom: 1px solid #d1d5db }`; pages 8.6 / 6.7 -> 8.6 / 6.6 %).
- Collapsed table borders (F22) and `column-rule` never read the top-colour shortcut: the collapsed edges carry their own
  colour from the Blitz fork and `column-rule` has its own parser. Mixed colours with `border-radius` are painted as wedges
  clipped to the border ring when every drawn side is solid; with a dashed / dotted / double / 3D side they stay straight
  per-side lines (not filed: no invoice need).
- fulgur-vrt in this environment: 35 of 64 fixtures differ before and after (same set). Seven of the failing fixtures'
  actual PDFs change, each a side border that now has its own colour (`bugs/cover-page-break-after` p2 `border-bottom:
  3pt solid #e94560`, `layout/review_card_inline_block` `border-left: 4pt solid #e94560`, `gcpm/running-element` and
  `gcpm/element-policy-{first,first-except,last,start}` `border-bottom: 1pt solid #999`); their goldens still hold the black
  borders and need re-blessing in a clean environment.
