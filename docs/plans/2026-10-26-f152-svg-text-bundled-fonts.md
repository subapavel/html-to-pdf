# F152 — SVG text is shaped with the bundled fonts only

Date: 2026-10-26. Finding: F152 (`2026-10-15-m0-findings.md`). Fix on `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`
(a hook) and `subapavel/fulgur` `htmltopdf/srgb-colours` (the font database and face picking); the strict-mode mapping
is ours.

## 1. Problem

The Blitz fork's `packages/blitz-dom/src/util.rs` has `static FONT_DB: LazyLock<Arc<fontdb::Database>>` with
`load_system_fonts()`, used by `parse_svg` for every inline `<svg>` (`layout/construct.rs:193`): the first inline SVG
makes the process scan ~118 host font files, and its `<text>` is shaped with host faces. Probe `out/scratch/f152/`
(release, `main` + F177b), only `Inter-Regular.ttf` bundled, an inline `<svg>` with four `<text>` (no family,
`sans-serif`, `Inter`, `monospace`): `pdffonts` lists bundled Inter-Regular plus host FreeSerif, FreeSans, FreeMono and
a host `Inter-Regular` (CID Type 0C) — even `font-family="Inter"` picks the host's CFF Inter, not the bundled file.

The SVG-as-image sites (`seed_image_natural_sizes`, `convert/style/background.rs:67`, `convert/list_marker.rs:90`, all
via `SvgImageChecks::options` = `offline_options()`) have the opposite fault: `usvg::Options::default()` has an empty
`fontdb` (usvg-0.45.1 `parser/options.rs:120`) and `font_family` "Times New Roman". The same text in `card.svg` as
`<img src>` and `background-image` logs `WARN usvg::text No match for 'sans-serif' font-family` (x3) and draws nothing;
`pdftotext` shows only `Before`. `--strict-fonts` does not catch it (`fonts::scan_html` never sees SVG `font-family`).

Also: usvg `text/colr.rs:188` / `:288` `println!` when a COLRv1 face with a sweep gradient or unsupported blend mode is
used. Together this breaks AGENTS.md goal 3 and the "fonts are bundled bytes" invariant: the PDF depends on the host,
and text is silently lost.

## 2. Spec (from memory, unverified; step 0 checks Chrome)

SVG 2 §11 (Text): `font-family` / `font-weight` / `font-style` are CSS Fonts properties. css-fonts-4 §5 (font
matching): each family in the list in order, generic families map to UA-chosen faces (§2.1.3), and when nothing matches
the UA uses a fallback face — text is never dropped for want of a family. SVG Integration §3 (secure static mode)
forbids external resources (`@font-face` URLs) in an SVG used as an image; the UA's own fonts stay usable, so text in
`<img src="card.svg">` is drawn (Chrome draws it). "Never host fonts" is a project invariant (AGENTS.md, ADR-0002), not
a spec rule: our "UA fonts" are the bundle. In Chrome an inline SVG's text inherits `font-family` / `font-size` from the
HTML; Blitz parses the SVG's `outer_html` alone, so that is a separate gap (F152c).

## 3. Where it belongs

Blitz must stop deciding which fonts an SVG sees; the host (fulgur) supplies them. F177 gave Blitz a per-`<svg>` factory
for the image resolver; widening it to return the whole `usvg::Options` is one hook for one parse, fulgur already builds
`Options` per site (`SvgImageChecks::options`), and inline and SVG-as-image parses then get identical options by
construction.

| Part | Where | Change |
|---|---|---|
| Hook | Blitz `config.rs` | `pub type SvgOptionsFactory = Arc<dyn Fn(usize) -> usvg::Options<'static> + Send + Sync>` replaces `SvgImageResolverFactory`; field `DocumentConfig::svg_options`. |
| Parse | Blitz `util.rs`, `construct.rs:190` | `parse_svg(source: &[u8], options: Option<usvg::Options<'static>>)`; `FONT_DB` touched only for `None` (upstream behaviour, and `net.rs:339`, F177d, unreachable from our crate). `FONT_DB` becomes a `OnceLock` so a test can assert it was never initialised. |
| Font database | fulgur new `svg_fonts.rs` | `pub struct SvgFonts`; `SvgFonts::new(fonts: &[Arc<Vec<u8>>], generics: &BTreeMap<String, String>) -> Arc<Self>`; `fn database(&self) -> Arc<usvg::fontdb::Database>` built lazily once (`OnceLock`), so a document without SVG pays nothing. |
| Contents | `svg_fonts.rs` | Every bundled face as `Source::Binary(Arc::clone(bytes))` in bundle order, no copy, no `load_system_fonts`. `set_serif_family` / `set_sans_serif_family` / `set_monospace_family` / `set_cursive_family` / `set_fantasy_family` from `AssetBundle::generic_families` (the F70 map HTML text uses); a missing `serif` entry falls back to the first bundled family. |
| Face picking | `svg_fonts.rs` | `SvgFonts::resolver(self: &Arc<Self>, site: SvgSite) -> usvg::FontResolver<'static>`: `select_font` queries the listed families; a `Named` keyword that is in the generics map (`system-ui`, `ui-monospace`, …) resolves through the map as our F3b rewrite does for HTML; if nothing matches, record a fallback finding (site, list as written) and query `Family::Serif`. `select_fallback` stays usvg's default (now bundled faces only, deterministic order). |
| Options | fulgur `svg_images.rs` | `SvgImageChecks::new(images, fonts: Arc<SvgFonts>)`, `warning_immediately(fonts)`; `options(site)` fills `fontdb`, `font_resolver` and `font_family` (the serif mapping, so an SVG that names no family gives no finding); `factory()` returns `options(SvgSite::Inline(id))`. State gains `font_fallbacks: BTreeSet<(SvgSite, String)>`; `SvgImageReport` gains `font_fallbacks: Vec<SvgFontFallback>`. `offline_options()` keeps an empty fontdb (validation in `image::validate_detailed` and callers without checks only). |
| Engine | fulgur `engine.rs` (three `SvgImageChecks::new` sites), `MarginDocEnv::new` | One `Arc<SvgFonts>` per render, shared with the margin-box checks. Findings logged; with strict text checking the render returns `Error::SvgFontFallback(Vec<SvgFontFallback>)`. |
| Ours | `crates/htmltopdf/src/lib.rs`, `fonts.rs` | New `Origin::Svg` (Display `SVG text in <site>`) carried in `UnresolvedFamily`; `run()` maps the fulgur error to `Error::UnresolvedFontFamilies`, so `strict_fonts` refuses an SVG list exactly when it refuses an HTML list (no entry bundled or mapped). Non-strict: drawn with the serif mapping and a warning (css-fonts-4 §5 fallback; better than HTML's drop). |

Strict behaviour is a decision: in HTML an unmatched list loses text (F3), so strict refuses it; in SVG the text is no
longer lost, but the author still asked for a face we do not have — the same switch reports the same problem, whatever
element it is written on.

Font sharing: krilla-svg 0.7.0 (`lib.rs` `convert_fontdb`) turns each used face into
`krilla::Font::new(make_shared_face_data(..))` and embeds glyphs as text (`SvgSettings::embed_text` default `true`).
Krilla's `Font` equality is the hash of its `FontInfo` (checksum, length, metrics, PostScript name;
`krilla-0.7.0/src/text/font.rs:210`, `util.rs:61`), so the same bundled bytes used by HTML and SVG text should end up as
one embedded font. Test 5 checks it.

## 4. Acceptance

New `crates/htmltopdf/tests/svg_text_fonts.rs`; font names from `pdffonts` (subset prefix stripped), text from
`pdftotext` (poppler is already required by `fixtures.rs`):

1. Host isolation (child process): the test binary re-runs itself (`current_exe`, `--exact`, env `F152_CHILD`) with
   `HOME` a temp dir and `FONTCONFIG_FILE` a `fonts.conf` whose `<dir>` holds a copy of `Inter-Bold.ttf` (the same copy
   in `$HOME/.fonts`, covering fontdb without fontconfig). Only `Inter-Regular` bundled; an inline SVG with
   `font-weight="bold" font-family="Inter"` text renders byte-identically with and without that host dir, and
   `Inter-Bold` never appears. Red: the host face is embedded.
2. Only bundled faces, every site: inline `<svg>`, `<img src="card.svg">`, `background-image`, `list-style-image`
   (fonts only; marker text is not extractable), four texts each (none, `sans-serif`, `Inter`, `monospace`): every font
   name in {`Inter-Regular`, `JetBrainsMono-Regular`}; `pdftotext` contains all texts for inline, img and background.
   Red: host fonts inline; text missing for img and background.
3. Generic families map like HTML: `monospace` / `ui-monospace` → JetBrainsMono; none / `serif` / `sans-serif` → the
   default family; `font-weight="700"` with Inter-Bold bundled → Inter-Bold.
4. Unbundled family: `font-family="Georgia"` → drawn in Inter, one warning naming the SVG and `Georgia`;
   `strict_fonts(true)` → `Err(UnresolvedFontFamilies)` with `Origin::Svg`.
5. Fonts are shared: the same face used in HTML and an inline SVG appears once in `pdffonts`.
6. Margin box: an `@page` margin box with a running element containing an inline `<svg><text>`: drawn, bundled faces
   only.
7. Determinism and stdout: two renders byte-identical; `htmltopdf-cli/tests/stdout_clean.rs` gains a case with inline
   and `<img>` SVG text, stdout stays empty.

Fork tests: Blitz `construct.rs` tests (the factory's options are used, the fontdb honoured; `FONT_DB` never
initialised); fulgur `svg_fonts.rs` unit tests (generic setters, `Named` keywords via the map, fallback finding, bundle
order, no host face); `render_smoke.rs` one case per site. Fixtures 01-25 have no SVG `<text>` (03, 05, 08 have an
inline `<svg>` without text, which today loads host fonts but uses none): byte-identical, pixel-diff change 0;
`bench.py` should show 03/05/08 faster without the host scan — record the numbers.

## 5. Steps

0. Chrome reference for the probes in `out/scratch/f152/` (expected: all three sites draw text; inline uses the
   inherited Inter); record in §7 with the `pdffonts` evidence above.
1. Ours: tests 1-7, red.
2. Blitz: `SvgOptionsFactory`, `parse_svg(options)`, `OnceLock` `FONT_DB`, lib tests; in the same lock bump fulgur's
   `factory()` returns `offline_options()` + the resolver (output-neutral).
3. fulgur: `SvgFonts` (database, generics, resolver), wired into `SvgImageChecks::new` / `options`, the three engine
   sites and `MarginDocEnv`; unit + smoke tests; VRT unchanged.
4. fulgur: fallback findings, `SvgImageReport.font_fallbacks`, `Error::SvgFontFallback` under strict text checking.
5. Ours: lock bump, `Origin::Svg`, mapping in `run()`; gates (`compare.py`, `bench.py --check`, `vrt-validator`,
   `pdf-reviewer` — fonts in the PDF, merge-policy item 5).
6. Docs: ADR-0007 "SVG text uses the bundled fonts; an unmatched family falls back to the serif mapping; strict refuses
   it"; `docs/upstream.md` (the F152 gap row becomes carried, fork rows); the fulgur fork's `CLAUDE.md` "Font
   determinism caveat"; findings (F152 done, F152b, F152c filed).

## 6. Risks and non-goals

- Breaking change in the Blitz fork hook (F177's type renamed); only fulgur uses it.
- The fontdb is built once per render that has an SVG (parses N bundled faces' name tables): expected well under 1 ms
  for 6 faces; `bench.py` decides; otherwise cache on `AssetBundle` (follow-up).
- Text that rendered in a host face now renders in a bundled one (intended). Non-strict, an unbundled family is
  substituted with only a warning (the spec's fallback; Chrome substitutes too).
- If test 5 shows two font objects: bigger but deterministic PDF; a fulgur follow-up, not a blocker.
- Non-goals: F152b (usvg `colr.rs` `println!` still fires for a caller-bundled COLRv1 face in SVG text; we bundle none;
  needs a usvg fork or excluding COLR faces); F152c (inline SVG text does not inherit `font-family` / `font-size` from
  the HTML); `@font-face` inside an SVG; a glyph missing from every bundled face (F44); F177d; renaming `SvgImageChecks`.

## 7. Evidence

Step 0 (probes `out/scratch/f152/inline.html`, `img.html` + `card.svg`; Chromium via Playwright, A4):

- Inline `<svg>`, four `<text>`: Chrome draws all four. No family → `Inter-Regular` (inherited from the HTML, F152c);
  `font-family="Inter"` → `Inter-Regular`; `sans-serif` → host `LiberationSans`; `monospace` → host `DejaVuSansMono`
  (Chrome's generic mapping = its UA fonts; ours = the bundle's `generic_families`).
- `card.svg` as `<img src>` and `background-image`: Chrome draws the text (`pdftotext`: `Card text Faktura`).
- fulgur on `main` + F177b: inline embeds host FreeSerif, FreeSans, FreeMono and a host CFF `Inter-Regular` next to the
  bundled `Inter-Regular`; `<img>` / background draw no text (`No match for 'sans-serif' font-family` x3).
