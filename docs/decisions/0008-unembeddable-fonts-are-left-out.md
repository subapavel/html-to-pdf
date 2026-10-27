# ADR-0008: A bundled font file the PDF writer cannot embed is left out whole; strict refuses it

Date: 2026-10-27 · Status: accepted · First use: F178 (`docs/plans/2026-10-27-f178-krilla-rejected-faces.md`)

## Context

Text is shaped by Parley (fontique / skrifa) and embedded by krilla, which parse a font separately. A face Parley
accepts but krilla does not (no `head` table) had its runs skipped at draw time without a word, even with
`strict_fonts`; a face with no glyph outlines was embedded as a Type 3 font with empty glyphs — text that extracts but
is invisible. Bytes that were not a font at all were passed through with a log line. css-fonts-4 §4.3 treats an invalid
font resource as one that failed to load, and §5 then matches the remaining faces: text is drawn in a fallback face,
never dropped. Chrome (OTS) rejects both broken files and draws the text in a fallback.

## Decision

- Every bundled font file is checked when it is registered (fulgur `font_check::check_font`): each face must be
  accepted by krilla's own `Font::new` and have glyph outlines (`glyf`, `CFF `, `CFF2`) or colour / bitmap glyphs
  (`COLR`, `SVG `, `CBDT`, `sbix`); bytes that are not OpenType are rejected.
- A file with any bad face is left out whole (a collection too): its families are not registered and it can never be
  the default family, so its text falls back to the other bundled faces (weight 700 with only 400 left matches 400; we
  do not synthesise bold, ADR-0002).
- Non-strict: one warning per file naming it, the face and the reason. With `strict_fonts(true)` rendering fails with
  `Error::RejectedFonts` (at render time: builder calls come in any order and `build()` cannot fail).
- Safety net: a run krilla still cannot draw (fonts pushed into fulgur's `AssetBundle::fonts` directly) is reported as
  dropped text (`TextWithoutFont` under strict, a warning otherwise), never skipped silently.

## Consequences

- A document with a damaged font file renders its text in a fallback face instead of losing it; strict templates fail
  loudly. WOFF1, a WOFF2 decode failure and an I/O error still fail the builder at once, as before.
- fulgur fork behaviour change: bytes with unknown magic are now an error (they never drew anything).
- The check costs microseconds per face (§7 of the plan: 0.054 ms for five Inter faces + JetBrains Mono, 0.015 ms for a
  17 MB collection).
- Not covered: leaving out only the bad face of a collection (F178b); deeper sanitising of tables Parley and krilla both
  accept (broken `loca` / `cmap`, all-empty outlines; F178c).
