# F1 — Convert non-sRGB colours to sRGB before painting (`oklch()`, `lab()`, `color-mix()`, inline SVG)

Date: 2026-10-15. Finding: F1 in `2026-10-15-m0-findings.md`. Status: **done** — carried as fork branches via
`[patch.crates-io]` (the forks were created by the owner after `docs/postmortems/2026-10-15-f1-fork-creation-denied.md`).

## 1. Problem

Stylo keeps an `AbsoluteColor` in the colour space it was written in. Two places read its components as if they
were sRGB:

1. **fulgur** `src/convert/style/mod.rs::absolute_to_rgba` — the single funnel for text, text-decoration, border,
   background, gradient-stop and box-shadow colours. `oklch(21% 0.034 264.665)` → `0.21 0.034 1 rg`.
2. **blitz-dom** `src/node/node.rs::write_outer_html` — replaces `currentColor` in inline `<svg>` attributes with
   `color.to_css_string()`, i.e. `oklch(…)`, which usvg cannot parse ("Fallback to black").

Tailwind v4's whole palette is `oklch()` and its opacity modifiers are `color-mix(in oklab, …)`, so every Tailwind
colour is wrong (purple/magenta text, pink cards in 05) and `fill="currentColor"` icons are black.

## 2. Spec

css-color-4 §4 / §9 (`lab()`, `oklch()` …): the colour is defined in its own space and is converted for display;
§13 (gamut mapping) — CSS Color 4 asks for perceptual gamut mapping of out-of-gamut colours, Chrome clips.
We clip (clamp per channel), matching Chrome. css-color-5 §2 (`color-mix()`): result is in the interpolation space.

## 3. Where it belongs

Upstream, both one-liners (Stylo already has the conversion: `AbsoluteColor::into_srgb_legacy`):

```diff
--- fulgur-0.40.0/src/convert/style/mod.rs
@@ pub(super) fn absolute_to_rgba(c: style::color::AbsoluteColor) -> [u8; 4] {
     let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
+    // Stylo keeps a colour in the space it was specified in (`oklch()`, `lab()`,
+    // `color-mix(in oklab, …)` …), so `components` are only R/G/B after
+    // converting to sRGB. Out-of-gamut values are clipped by `q`.
+    let c = c.into_srgb_legacy();
     [
```

```diff
--- blitz-dom-0.2.4/src/node/node.rs
@@ pub fn write_outer_html(&self, writer: &mut String) {
         let current_color = self
             .primary_styles()
             .map(|style| style.clone_color())
-            .map(|color| color.to_css_string());
+            .map(|color| color.into_srgb_legacy().to_css_string());
```

Fork points (from `.cargo_vcs_info.json` of the published crates):

| Crate | Fork | Base commit | Branch |
|---|---|---|---|
| fulgur 0.40.0 (`crates/fulgur`) | `subapavel/fulgur` | `8d8194969329d922233c9258c86d5f28a5aa5221` | `htmltopdf/srgb-colours` |
| blitz-dom 0.2.4 (`packages/blitz-dom`) | `subapavel/blitz` | `0a77560e59440d3c6b5f14b673b9c722a08a3d4c` | `htmltopdf/svg-current-color-srgb` |

Workspace `Cargo.toml`:

```toml
[patch.crates-io]
fulgur = { git = "https://github.com/subapavel/fulgur", branch = "htmltopdf/srgb-colours" }
blitz-dom = { git = "https://github.com/subapavel/blitz", branch = "htmltopdf/svg-current-color-srgb" }
```

Not fixed here, noted for follow-up: fulgur's own `column_css::parse_color` (column-rule colours) has a separate
cssparser path; not used by any fixture.

## 4. Acceptance

- `crates/htmltopdf/tests/colour_spaces.rs` (committed, `#[ignore]` until the patch lands — remove the attributes in
  the same PR): sRGB red written as `rgb`/`oklch`/`oklab`/`lab`/`color-mix` paints red within 4/255, as a
  background and as an inline-SVG `currentColor` fill. Verified: fails on 0.40.0 (`oklch` → `[160, 66, 255]`,
  SVG → black), passes with both patches.
- No `usvg … Failed to parse fill value: 'oklch(…)'` warning when rendering 05.
- `scripts/compare.py`, measured with both patches applied via `cargo --config 'patch.crates-io.<crate>.path=…'`:

| Fixture | Before | After |
|---|---|---|
| 01-basic | 5.3 % | **4.4 %** |
| 02-long (worst page) | 10.6 % | **10.4 %** |
| 03-logo-qr | 10.1 % | **8.9 %** |
| 04-bilingual-vat | 9.9 % | **5.8 %** |
| 05-fancy | 25.4 % | **21.4 %** |

  Lower `THRESHOLDS` in the same PR: 01 → 0.055, 02 → 0.115, 03 → 0.10, 04 → 0.07, 05 → 0.225.
  The finding's "01 below 3 %" was too optimistic: 01's remaining diff is row rules (Blitz #386) and text metrics.
- Page counts unchanged (02 still 4/3, F5).

## 5. Steps once the forks exist

1. Push the two branches above with the diffs from §3; open the upstream PRs (fulgur-rs/fulgur, DioxusLabs/blitz).
2. Add `[patch.crates-io]`, `cargo update -p fulgur -p blitz-dom`, remove the `#[ignore]`s, lower thresholds.
3. `vrt-validator` and `pdf-reviewer` (colour operators in the content stream are the PDF writer path).
4. Record forks and upstream PRs in `docs/upstream.md` under "Our patches / PRs".

## 6. Risks

- The git patch for blitz-dom must stay at version 0.2.4 or cargo ignores it (warning "patch was not used").
- `into_srgb_legacy` drops `none` components to 0 — same as legacy `rgb()` serialisation; harmless here.

## 7. As landed

- `subapavel/fulgur` branch `htmltopdf/srgb-colours` = `8d81949` (0.40.0) + the fulgur hunk.
- `subapavel/blitz` branch `htmltopdf/svg-current-color-srgb` = `0a77560` (blitz-dom 0.2.4) + the blitz-dom hunk + an
  htmltopdf-only commit that takes `blitz-traits`, `stylo_taffy` and `debug_timer` from crates.io. Without it the
  git checkout's own copies of those crates would sit next to the crates.io ones that fulgur and blitz-html use
  (two `blitz-traits` → type mismatches). `Cargo.lock` changes only the sources of `fulgur` and `blitz-dom`.
- Cargo warns "skipping duplicate package `fulgur`": the fulgur repo also contains `crates/fulgur-ruby/ext/fulgur`;
  cargo picks `crates/fulgur`, which is the right one.
- Measured on main + F7/F3b/F8/F10: 01 5.3 → 4.4, 02 10.6 → 10.4, 03 10.1 → 8.9, 04 9.9 → 5.8, 05 32.3 → 21.5 %.
  Thresholds lowered accordingly.

