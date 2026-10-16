# F11 — `none` colour components are mis-converted

Date: 2026-10-15. Finding: F11 in `2026-10-15-m0-findings.md`. Status: **done** — fulgur fork `a5effd6`, Blitz fork
`92b7a92`.

## 1. Problem

`oklch(50% none 0)`, `oklab(50% 0.1 none)`, `lab(50% none 40)`, `color(display-p3 none .5 .5)` painted black;
`lch(50% 40 none)` and `oklch(70% 0.1 none)` lost their chroma. Stylo's `AbsoluteColor::to_color_space` passes a
`none` component to the conversion as NaN (it uses NaN as the "missing" marker for interpolation), and the NaN
survives into the sRGB components; `into_srgb_legacy` only clears the flags afterwards.

## 2. Spec

css-color-4 §4.4 (*"Missing" color components and the `none` keyword*): outside interpolation a missing component
behaves as 0 in the colour's own space. Chromium 141 agrees: `oklch(50% none 0)` and `oklch(50% 0 0)` both paint
`rgb(99 99 99)`; `lch(50% 40 none)` and `lch(50% 40 0)` both `rgb(179 89 121)`.

## 3. Fix

Rebuild the colour with every `none` component as 0 (`c0()/c1()/c2().unwrap_or(0.0)`) before `into_srgb_legacy`,
in both places that convert for output:

| Where | Fork | Commit |
|---|---|---|
| fulgur `convert/style/mod.rs::absolute_to_rgba` (text, borders, backgrounds, gradients, shadows) | `subapavel/fulgur` `htmltopdf/srgb-colours` | `a5effd6` |
| blitz-dom `node/node.rs::write_outer_html` (`currentColor` in inline SVG) | `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` | `92b7a92` |

Alpha is left as stored: Stylo already keeps a `none` alpha as 0.

## 4. Acceptance

- `crates/htmltopdf/tests/colour_spaces.rs::none_components_behave_as_zero`: eight `none` colours (oklch, oklab,
  lab, lch, display-p3, hsl, rgb) paint the same as the colour with 0 written, as a background and as an SVG
  `currentColor` fill. Failed before (`oklch(50% none 0)` → black).
- `none` alpha, all-`none` and `hwb()` cases are in the same test (added after `vrt-validator`).
- Fixtures unchanged (Tailwind never emits `none`): 01 4.4, 02 10.3/11.8/9.6, 03 8.2, 04 5.7, 05 21.6 %; fixture PDFs
  byte-identical to the pre-PR build (`pdf-reviewer`).
- `vrt-validator`: 69 `none` notations match Chrome 141 on background, text, border, text-decoration, box-shadow,
  `currentColor` and SVG `currentColor`. `pdf-reviewer`: no NaN/out-of-range colour operands, colour spaces and
  ExtGState unchanged, deterministic.

## 5. Risks

- `color-mix()` and relative colours resolve `none` inside Stylo, before it reaches us; where they change colour
  space the result still differs from Chrome (F16).
- A `none` gradient stop now counts as 0; css-color-4 §12 would carry the neighbouring stop's component, and fulgur
  interpolates in sRGB whatever `in <space>` says (F16). Better than the old black, not yet spec.
- krilla-svg emits `DeviceGray` for neutral SVG paints (also before this PR); PDF/A needs an OutputIntent or
  `DefaultGray` for that — PDF/A track, not this finding.
- The fulgur branch now carries F1 and F11. Upstream fulgur `main` has F1 (fulgur-xfzo) but the branch can only be
  dropped on a bump once that release also handles `none`.
