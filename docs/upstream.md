# Upstream tracking

What we depend on, what we have reported or patched, and what blocks invoices.

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
| `border-collapse: collapse` ignored, shared borders painted twice | Blitz — [#386](https://github.com/DioxusLabs/blitz/issues/386) | open | 03 |
| Inline-level boxes get no background/border/shadow | fulgur draw path | open (documented in fulgur `docs/css-support.md`) | 03 (badges) |
| Nested tables do not fragment | fulgur / Taffy | open | — |
| A text run whose `font-family` list resolves to no face is dropped without any warning | fulgur / Blitz / Parley | **to file** (F3); we guard with a static check in `htmltopdf` | 01-04 (before F3) |
| With `system_fonts(false)` the fontique generic-family and fallback maps are empty, so `monospace`, `serif`, `sans-serif`, `system-ui` (and the default `serif`) match nothing; Parley should fall back to any registered face rather than drop the run | Parley / fontique | **to file** (F3) | `fixtures/probe.html` row G |
| fulgur 0.40.0 exposes no hook for generic-family mapping (`FontContext` is built privately in `blitz_adapter::parse_inner`) | fulgur | **to file / PR** (F3 step 4, blocked, see plan) | — |
| Determinism depends on host fonts for SVG `<text>` — renders with host FreeSerif even with `system_fonts(false)` | Blitz `fontdb` global | open; breaks our no-host-fonts invariant for SVG text | 03 (QR has no text, so unaffected) |
| Media queries evaluated as `screen`; `@media print` rules ignored — `blitz-dom` `make_device` hardcodes `MediaType::screen()`, fulgur has no override | Blitz + fulgur (see fulgur's Blitz PR #390) | **to file**; polyfilled in `htmltopdf` (F7, `media.rs` swaps `print`/`screen`) | 05 (`print:` variants) |
| `<style media="…">` attribute ignored: the sheet applies whatever the media | Blitz | **to file** — not polyfilled (F7 plan §5) | — |
| Generic families (`monospace`, `sans-serif`) alone cannot be mapped to a bundled face | fulgur / Parley font fallback | **to file**; mapped in `htmltopdf` by rewriting the keywords (F3b) | probe |
| `@property` ignored: Stylo 0.8.0 gates the at-rule parser (`stylesheets/rule_parser.rs:752`) on `layout.css.properties-and-values.enabled`, absent from `stylo_static_prefs` 0.8.0 → false; the cascade side is compiled in for servo. Upstream stylo main has it `true`. | Stylo via Blitz | **to file** on Blitz (bump Stylo / enable pref) — F2b; we polyfill bundled CSS in `htmltopdf` (F2) | all (Tailwind borders/shadows) |
| `linear-gradient(<dir> in oklab, …)` (colour-interpolation method) paints nothing | Stylo or Blitz paint — unverified | **to file** — F8 | 05 (Tailwind gradients) |
| `usvg` rejects `oklch()` in inline SVG fills, falls back to black | resvg/usvg | open — part of F1 | 05 (icon) |

| Font descriptors carry the SmallCap flag (ISO 32000-1 Table 123, bit 18) on every font | Krilla | open — harmless, report | all |
| `Producer` says `fulgur`; set ours via `EngineBuilder::producer` | ours (htmltopdf) | todo, trivial | all |

## Our patches / PRs

_(none yet)_
