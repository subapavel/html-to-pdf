# Upstream tracking

What we depend on, what we patch in our forks, and what blocks invoices. We do not report bugs or send
PRs upstream (owner's decision, 2026-10-15); "known, not reported" marks upstream bugs we only work around.

| Layer | Crate / repo | Pinned | Notes |
|---|---|---|---|
| Glue + pagination + PDF | [fulgur](https://github.com/fulgur-rs/fulgur) | 0.40.0 | ZeroVer; breaking per minor |
| DOM, style, layout | [Blitz](https://github.com/DioxusLabs/blitz) (`blitz-dom`) | via fulgur | |
| CSS engine | Stylo (Servo/Firefox) | via Blitz | |
| Box layout | Taffy | via Blitz | no table algorithm — tables approximated over block/flex/grid |
| Text | Parley + HarfBuzz port | via Blitz | |
| PDF writer | Krilla | via fulgur | |

## Known gaps that hit invoices

| Gap | Where | Status | Fixture |
|---|---|---|---|
| Explicit `<td>/<th>` widths get the cell padding added even under `box-sizing: border-box` (`layout/table.rs:175`, TODO in source): Tailwind tables get wider fixed columns and a narrower auto column | Blitz | **carried** (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`) — F5 | 02 (4 pages instead of 3), 03 |
| `text-transform` not applied: text nodes are pushed to the line builder as written (`layout/construct.rs:944`) | Blitz | **carried** (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`) — F4 | 02–05 (`uppercase` headings) |
| `rewrite_marker_content_url` panics (slice `block_start..i-1` with `i-1 < block_start`) when a block is opened at the very end of a stylesheet (`a{`); also scans `<style>` inside HTML comments, `<script>`, `<textarea>`, `<title>` and attribute values, and treats an escaped `\{` as a block | fulgur (`blitz_adapter.rs:3371`) | **known, not reported** — F10; `htmltopdf` closes open blocks and catches engine panics | — |
| `border-collapse: collapse` ignored, shared borders painted twice | Blitz — [#386](https://github.com/DioxusLabs/blitz/issues/386) | open | 03 |
| Inline-level boxes get no background/border/shadow | fulgur draw path | open (documented in fulgur `docs/css-support.md`) | 03 (badges) |
| Nested tables do not fragment | fulgur / Taffy | open | — |
| A text run whose `font-family` list resolves to no face is dropped without any warning | fulgur / Blitz / Parley | **known, not reported** (F3); we guard with a static check in `htmltopdf` | 01-04 (before F3) |
| With `system_fonts(false)` the fontique generic-family and fallback maps are empty, so `monospace`, `serif`, `sans-serif`, `system-ui` (and the default `serif`) match nothing; Parley should fall back to any registered face rather than drop the run | Parley / fontique | **known, not reported** (F3) | `fixtures/probe.html` row G |
| fulgur 0.40.0 exposes no hook for generic-family mapping (`FontContext` is built privately in `blitz_adapter::parse_inner`) | fulgur | **known, not reported** (F3 step 4, blocked, see plan) | — |
| Determinism depends on host fonts for SVG `<text>` — renders with host FreeSerif even with `system_fonts(false)` | Blitz `fontdb` global | open; breaks our no-host-fonts invariant for SVG text | 03 (QR has no text, so unaffected) |
| Media queries evaluated as `screen`; `@media print` rules ignored — `blitz-dom` `make_device` hardcodes `MediaType::screen()`, fulgur has no override | Blitz + fulgur (see fulgur's Blitz PR #390) | **known, not reported**; polyfilled in `htmltopdf` (F7, `media.rs` swaps `print`/`screen`) | 05 (`print:` variants) |
| `<style media="…">` attribute ignored: the sheet applies whatever the media | Blitz | **known, not reported** — not polyfilled (F7 plan §5) | — |
| Generic families (`monospace`, `sans-serif`) alone cannot be mapped to a bundled face | fulgur / Parley font fallback | **known, not reported**; mapped in `htmltopdf` by rewriting the keywords (F3b) | probe |
| `@property` ignored: Stylo 0.8.0 gates the at-rule parser (`stylesheets/rule_parser.rs:752`) on `layout.css.properties-and-values.enabled`, absent from `stylo_static_prefs` 0.8.0 → false; the cascade side is compiled in for servo. Upstream stylo main has it `true`. | Stylo via Blitz | **known, not reported** (Blitz) (bump Stylo / enable pref) — F2b; we polyfill bundled CSS in `htmltopdf` (F2) | all (Tailwind borders/shadows) |
| `linear-gradient(<dir> in oklab, …)` (colour-interpolation method) paints nothing: Stylo 0.8.0 gates it on `layout.css.gradient-color-interpolation-method.enabled` (`values/specified/image.rs:37`), absent from `stylo_static_prefs` 0.8.0 → false | Stylo via Blitz | **known, not reported** with F2b; polyfilled in `htmltopdf` (F8, `gradient.rs` removes the method) | 05 (Tailwind gradients) |

| Font descriptors carry the SmallCap flag (ISO 32000-1 Table 123, bit 18) on every font | Krilla | open — harmless, report | all |
| `Producer` says `fulgur`; set ours via `EngineBuilder::producer` | ours (htmltopdf) | todo, trivial | all |

## Our patches / PRs

| Crate | Fork branch | Base | Fix | Upstream status |
|---|---|---|---|---|
| fulgur 0.40.0 | `subapavel/fulgur` `htmltopdf/srgb-colours` | `8d81949` | `absolute_to_rgba` converts to sRGB (F1); `none` components count as 0 before converting (F11) | F1 fixed independently on fulgur `main` (fulgur-xfzo, same call); drop the branch only when a release > 0.40.0 has both |
| blitz-dom 0.2.4 | `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` | `0a77560` | `currentColor` serialised as sRGB for usvg (F1); `text-transform` applied (F4); explicit cell widths respect `box-sizing` (F5); SVG `currentColor` treats `none` components as 0 (F11); a mixed `calc()` cell width is `auto` instead of a panic (F14; upstream PR #653 resolves it as a track instead, not per css-values-4); plus an htmltopdf-only commit taking sibling crates from crates.io | carried (F1 still present on Blitz `main`) |

Bumping fulgur or blitz-dom means rebasing these branches onto the new release commit (or dropping them once the
fix is released) and re-running `cargo update -p fulgur -p blitz-dom`.

All blitz-dom fixes go on the one branch `htmltopdf/blitz-dom-0.2.4` (one source per crate in
`[patch.crates-io]`). `htmltopdf/svg-current-color-srgb` (F1 only) is superseded and can be deleted.
