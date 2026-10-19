# ADR-0005: The document's `@page` decides the page; the caller overrides only what it sets

Date: 2026-10-19 · Status: accepted · First use: F6, `@page` half (`docs/plans/2026-10-19-f6-page-size-margins.md`)

## Context

`Renderer::render` used to call fulgur's `page_size` and `margin` for every document. In fulgur both calls are
overrides, and an override beats every `@page` rule, so a document's own `@page { size: A5; margin: 0 }` was
ignored: every page was A4 portrait with 15 mm margins, and the CLI's `--margin-mm` (default 15) and `--landscape`
did the same. Chrome honours `@page` (our references use `preferCSSPageSize`). css-page-3 ("Page context", "Page
size: the `size` property") makes the page size and margins properties of the page context set by the author's
`@page` rules; the UA only supplies a default. Invoice templates set `@page` as a matter of course (A4 landscape for
wide tables, A5 for receipts, `margin: 0` for full-bleed headers). The library is unpublished and has no bindings,
so the public type could still change freely; once bindings exist it cannot.

## Decision

Per property (the size, and each margin side), the first of these applies:

1. what the caller set in `RenderOptions::page` (`Page { size: Option<PageSize>, margin: Option<Margins> }`; the
   CLI's `--page-size`, `--margin-mm`, `--landscape`);
2. the document's `@page :first` / `:left` / `:right` rule that matches the page;
3. the document's plain `@page` rule;
4. the default: A4 portrait and 15 mm on every side (`DEFAULT_MARGIN_MM`).

A caller's size is final, orientation included. A caller's margin replaces all four sides. A property the caller
leaves `None` is the document's.

- **Default margin 15 mm** (css-page-3 leaves it to the UA): it is the product's behaviour so far, about 30 tests
  assume it for documents without `@page`, and it keeps a document that sets only `size` or only `margin-top`
  looking the way it did. Chrome's default is about 10 mm and WeasyPrint's 75 px (both from memory); none is more right.
- **Default size A4**: fulgur's own default, the page box of `@page { size: A4 }`.
- Implemented with the fulgur fork's non-override setter `default_margin` (the "Config defaults" layer of fulgur's
  own priority model), not with a UA stylesheet injected into the bundle, which would depend on the merge order of
  bundle, link and inline rules.
- fulgur's constants for A4, A3 and letter are kept (595.28 x 841.89 pt), so `@page { size: A4 }` is 595.28 pt wide
  while a caller who forces `PageSize::A4` (210 x 297 mm) gets 595.2756. The 0.0044 pt is accepted; making the
  constants exact would re-bless about 40 fork goldens.

## Consequences

- `Page`, `PageSize`, `Margins` are new public types; `Page::margin_mm`, `Page::A4` and `Page::LETTER` are gone.
- `htmltopdf render` without page flags lets the document decide. `--margin-mm` has no default; `--landscape` alone
  means A4 landscape, as before.
- Documents whose `@page` margin differs from 15 mm now print with their own (fixtures 02 and 05 move; the rest of
  01-17 declare `size: A4; margin: 15mm` and render as before).
- Chrome references of documents without `@page` would use Playwright's defaults, so every fixture declares `@page`.
- Fragmentation is still done against page 1's content height (F109): a `:first` rule with a smaller margin than the
  other pages pushes their content into the bottom margin. Not covered by this decision.
- Named pages (`page:`), `%` / `em` / `auto` margins and `@page` inside `@layer` / `@media` are not supported.
