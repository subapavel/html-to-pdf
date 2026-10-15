# ADR-0002: Ship a built-in last-resort font

Date: 2026-10-15 · Status: accepted · Plan: `docs/plans/2026-10-15-f3-font-fallback.md`

## Context

Fonts are bundled bytes and `system_fonts(false)` stays (determinism). Until now every face had to be supplied by
the caller and fetched by `scripts/fetch-fonts.sh`. When no face in an element's `font-family` list matched, the
text disappeared from the PDF without any message. Fixture 01 lost its account number, IBAN and variable symbol
that way: Tailwind's `font-mono` is `"JetBrains Mono", ui-monospace, monospace` and nothing in that list was bundled.
On a tax document that is data loss.

## Decision

`crates/htmltopdf` embeds JetBrains Mono Regular (SIL OFL 1.1, `crates/htmltopdf/fonts/`, licence text alongside)
with `include_bytes!` and registers it in `RendererBuilder::build()` after the caller's fonts. The caller's faces
always win; the built-in face is only reached when nothing else matches.

Why committed to git and not fetched: an always-present face must be in the crate, otherwise offline and
deterministic builds break and library users get no guarantee. This goes against the "fonts are fetched" convention
for Inter, which stays fetched because it is a fixture font, not a guarantee of the library.

Why JetBrains Mono: the fixture CSS already names it first, it is OFL 1.1 and it covers Latin Extended-A (Czech).

## Consequences

- Public behaviour: text in `"JetBrains Mono"` (and, with the generic mapping, `monospace`) renders without the
  caller doing anything. Removing the face later would be a breaking change, hence this ADR.
- Binary and repo grow by about 270 KB. Registered but unused faces are not embedded in the PDF (verified), so
  output size is unaffected.
- Only the Regular weight is bundled; bold mono text is Regular or synthesised.
- OFL 1.1 requires the licence text to ship with the font; `fonts/OFL.txt` is part of the crate.
