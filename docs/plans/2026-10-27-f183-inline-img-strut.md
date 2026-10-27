# F183 — an inline `<img>` sits on the floor of its line box (no strut below the baseline)

Date: 2026-10-27. Finding: F183 (`2026-10-15-m0-findings.md`). Status: **active** — the owner created `subapavel/parley` on 2026-10-27; fix branch `htmltopdf/inline-box-strut` on the
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

(Step 0 Chrome table; after the fix.)
