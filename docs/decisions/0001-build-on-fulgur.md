# ADR-0001: Build on fulgur rather than Blitz directly or a new engine

Date: 2026-10-15 · Status: accepted

## Context

We want HTML + Tailwind → PDF with no browser, optimised for precision and speed, starting with invoices.
Options considered:

1. **Own engine** (Stylo + Taffy + Parley + own box tree, fragmentation and PDF writer).
2. **Blitz** (DioxusLabs) + own pagination + own PDF backend.
3. **fulgur** — Blitz + Krilla + a pagination layer, already shipping: `break-*`, orphans/widows,
   repeated `thead`, `@page` margin boxes with `counter(pages)`, GCPM running headers, font subsetting,
   tagged PDF / PDF/UA-1, byte-deterministic output, MiniJinja templates, npm/PyPI/RubyGems bindings.

## Decision

Option 3. `fulgur` is a Cargo dependency pinned to an exact minor (0.x, breaking changes per minor).
Our crate `htmltopdf` wraps it and is the **only** place that imports it.

Engine fixes go upstream (fulgur, then Blitz/Taffy/Parley) as PRs. We carry patches only while a PR is open,
via `[patch.crates-io]` pointing at our fork branch. A hard fork is explicitly *not* chosen now.

## Consequences

- Phases M0–M3 of the original plan (basic render, fonts, fragmentation) are mostly done on day one; our work
  starts at the invoice-specific layer and at upstream gaps (`border-collapse`, inline backgrounds, nested tables).
- We inherit fulgur's architecture: Blitz does layout for the *whole document* as one tall page, fulgur then
  slices it into pages ("Pageable" tree). Anything that needs layout to know about page boundaries
  (true CSS fragmentation, `break-inside` affecting line breaking, page-aware floats) is hard in this model.
  If invoices ever need that, revisit this ADR.
- Single-maintainer risk upstream. Mitigation: MIT/Apache-2.0 licence, pinned versions, the fork option stays open.
- Contributing upstream requires signing fulgur's CLA.

## Review trigger

Revisit if: (a) upstream goes quiet for > 6 months, (b) a required invoice feature needs layout-time
fragmentation, or (c) a Blitz change breaks pagination in a way fulgur cannot absorb.

Correction (2026-10-16): "repeated `thead`" above was wrong for fulgur 0.40.0 — its v2 render path had the repeat
deferred. It is implemented in our fork (F12, `docs/plans/2026-10-16-f12-repeat-thead.md`).
