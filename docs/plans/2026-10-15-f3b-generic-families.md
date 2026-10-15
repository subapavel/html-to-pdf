# F3b — Map generic font families to bundled faces

Date: 2026-10-15. Finding: F3b in `2026-10-15-m0-findings.md` (open part of F3, see `2026-10-15-f3-font-fallback.md`
§7–8).

## 1. Problem

Fonts are bundled and `system_fonts(false)` stays, so fontique's generic-family map is empty: `monospace`, `serif`,
`sans-serif`, `system-ui` … resolve to no face and the text is dropped (probe row G). Stylo parses `ui-*`, `math`,
`emoji`, `fangsong` as plain family names, which match nothing either. Since F3 the static check reports such lists
(an error with `strict_fonts(true)`), but a template that relies on `font-family: monospace` cannot render at all.

## 2. Spec

CSS Fonts 4 §4.2: generic families are keywords; the user agent chooses the face. An unquoted family name is a
sequence of identifiers, so a single escaped identifier can name any family (`JetBrains\000020Mono`).

## 3. Where it belongs

The F3 plan's Step 4 wanted `AssetBundle::set_generic_family` in fulgur (fork + `[patch.crates-io]`). Forks cannot
be created from agent sessions (postmortem on branch `f1-srgb-conversion`), and the result can be had in our crate:
since every generic resolves to nothing today, replacing the keyword with the bundled family it maps to changes
output only where text was being dropped. Same pattern as F2/F7. Keep the upstream item open (Parley should fall
back to any registered face instead of dropping a run).

`crates/htmltopdf/src/fonts.rs`:
- the existing `font-family` / `font` / custom-property scanner records the source range of every entry that is a
  bare generic keyword (`generic_spans`); quoted names are names and are not touched;
- `generic_map(default, overrides)`: `monospace`, `ui-monospace` → built-in JetBrains Mono; every other generic →
  first bundled font (or the built-in face); `RendererBuilder::generic_family(g, family)` overrides (section 3 of the
  F3 plan);
- `map_generics` (bundled CSS, in `build()`) and `map_generics_in_html` (`<style>` and `style=""`, per render) write
  the family as one CSS identifier with six-digit hex escapes — no quotes, spaces or `&`, so it is safe in attributes.

The check uses the same map, so generic-only lists now pass strict mode when the mapped family is registered.

## 4. Acceptance

- `crates/htmltopdf/tests/generic_families.rs`: `monospace`, `sans-serif` after an unknown name, `font: … serif`,
  `ui-monospace` and `font: … system-ui` in `style=""` and `<style>` render in strict mode and their text is in
  `pdftotext`; `monospace` uses JetBrains Mono, the rest Inter; bundled CSS and custom properties are mapped;
  `generic_family` overrides; a quoted `'monospace'` is still an unresolved name.
- Probe row G (`fixtures/probe.html`) is visible.
- Fixtures 01–05 unchanged (their Tailwind theme names Inter / JetBrains Mono first).

## 5. Risks and limits

- In custom properties `math`, `emoji`, `fangsong` are not mapped (`--x: math` may feed `display`). A font stack
  stored in a custom property that is only `math` etc. still drops its text, while the check counts it as mapped.
- `style=""` values containing a character reference are not rewritten; the check treats their generic keywords as
  plain names, so they are reported (error in strict mode) instead of being dropped silently.
- A custom property holding a whole `font` shorthand (`--f: 12px monospace; font: var(--f)`) is not rewritten and
  not checked.
- `@font-face { font-family: monospace }` (invalid CSS) is rewritten like any other declaration; harmless.
- Inline SVG `<text>` does not see the mapping and still uses host fonts (Blitz `fontdb`, already in
  `docs/upstream.md`).
- Found by `vrt-validator` on PR #4 and fixed: the entity-attribute case above (previously counted as mapped), and a
  rule whose selector starts like a declaration (`font:hover { … }`) hiding every later rule from the scan.
- `<link>` stylesheets and `@import`ed sheets are not seen (as for F7).
