# F70 — UA-sheet `monospace` (`pre`, `code`, `kbd`, `samp`, `tt`) must resolve to a bundled face

Date: 2026-10-17. Finding: F70 (HIGH) in `2026-10-15-m0-findings.md`. Status: **planned**. Builds on
`2026-10-15-f3-font-fallback.md`, `2026-10-15-f3b-generic-families.md` and ADR-0002.

## 1. Problem

With only Inter bundled, `/tmp/claude-0/vrt-f66/atk/pn5.html` (a bare four-line `<pre>`) paints only its blue marker; in
`pi.html` the `pre style="font-family:Inter"` block is drawn and the bare `<pre>` after it is not. rc 0, no warning, also
with `--strict-fonts`. Chain:

1. Blitz `assets/default.css:287-304` (`listing`, `xmp`, `pre`, `plaintext`) and `:601-606` (`tt`, `code`, `kbd`, `samp`)
   set `font-family: -moz-fixed` (Stylo's alias for `monospace`, F3 §7c); a declaration on the element beats the family
   inherited from our `html {}` prelude (`lib.rs:523-530`).
2. `stylo_to_parley.rs:30-40, 165-167` turn it into `parley::FontFamily::Generic(Monospace)`.
3. fulgur `blitz_adapter.rs:319-336` (`parse_inner`) builds the fontique `Collection` with `system_fonts: false` and only
   `register_fonts`, so the generic-family and fallback maps are empty.
4. Parley 0.6.0 `shape/mod.rs:254-257`: `select_font` returns `None` for the item's first cluster, the
   `while let Some(font)` loop never runs, the item gets no glyph run — layout and paint just have no text.
5. F3b rewrites keywords only in CSS that passes through us (`fonts.rs:742-750`: bundled CSS in `build()`, `<style>` /
   `style=""` in `prepare()`); the UA sheet is compiled into Blitz. `check_fonts` (`lib.rs:281-295`) scans the same
   author sources, so it has nothing to report. JetBrains Mono is registered, but nothing points the generic at it.

Not affected: `<button>`, `<input>`, `<select>`, `<textarea>` (`default.css:50-95` sets no family; they inherit) and
`<var>`. Author `ui-monospace`, `monospace`, `font: 12px monospace` render via F3b. Only sources F3b does not rewrite drop
the same way: `<link>` / `@import` sheets and `style=""` values with character references.

## 2. Spec (section numbers unverified)

- HTML §15.3.3–15.3.4 (Rendering): `pre, listing, plaintext, xmp` and `code, kbd, samp, tt` get
  `font-family: monospace` — Blitz's sheet conforms; the defect is in resolution.
- CSS Fonts 4 §4.2 `<generic-family>`: the UA picks the face. Fonts 3 §3.1.1: generic families "must always result in at
  least one matched font face". §5 font matching: an exhausted list goes to system fallback, then a last-resort glyph —
  text must not vanish.
- CSS Cascade 5 §6.1–6.4: UA declarations lose to every author declaration; re-declaring the UA rule as author CSS would
  change precedence.

## 3. Design

How a generic resolves is engine behaviour → fulgur fork (`subapavel/fulgur` `htmltopdf/srgb-colours`). Which face each
generic gets is our policy and stays in F3b's `fonts::generic_map` (`fonts.rs:724`), unchanged. Rejected: a bundled
`pre, code, … { font-family: <mono> }` rule (fulgur appends bundled CSS after the document's `<style>`,
`lib.rs:257-260`, so it would beat the document's equal-specificity rules; an `@layer` would still outrank the
document's layers); editing Blitz's `default.css` (Blitz does not know our families).

fulgur fork:
- `asset.rs`: `pub generic_families: BTreeMap<String, String>` (CSS keyword → family) and
  `pub fn set_generic_family(&mut self, keyword: &str, family: impl Into<String>)`.
- `blitz_adapter.rs`: `parse_inner(…, generics: &BTreeMap<String, String>, …)`; after `register_fonts`,
  `fn apply_generic_families(c: &mut fontique::Collection, g: &BTreeMap<String, String>)` calls
  `set_generic_families(GenericFamily::parse(k), family_id(f))`, skipping unknown keywords / families. Threaded from
  `Engine` (`engine.rs:103`; `render.rs` ~4177/4212/4284). An empty map behaves exactly as upstream.
- `convert/inline_root.rs` `extract_paragraph` (:444): collect the text ranges of all glyph runs;
  `fn uncovered_text(text: &str, covered: &[Range<usize>]) -> Option<String>` returns an excerpt of non-whitespace,
  non-default-ignorable text no run covers. Record `DroppedText { element: String, excerpt: String }` in `ConvertContext`;
  after conversion `log::warn!` once per entry; with `EngineBuilder::strict_text(true)` return
  `Error::TextWithoutFont(Vec<DroppedText>)`.

htmltopdf:
- `RendererBuilder::build()` (`lib.rs:517`): `assets.set_generic_family(g.keyword(), family)` for every `generics` entry;
  `Renderer::engine` sets `strict_text(self.strict_fonts)`.
- New `Error::TextWithoutFont(Vec<String>)` wrapping fulgur's error (callers never see fulgur types).
- `fonts.rs` `with_generics_unmapped` (:207): keep the six generics Stylo parses (`serif`, `sans-serif`, `monospace`,
  `cursive`, `fantasy`, `system-ui`) as generics (the engine now resolves them from any source); `ui-*`, `math`,
  `emoji`, `fangsong` stay names.

## 4. Acceptance

New `crates/htmltopdf/tests/ua_generic_fonts.rs` (Inter only, `strict_fonts(true)` unless stated):

- A1: bare `<pre>`, `<code>`, `<kbd>`, `<samp>`, `<tt>`, `<xmp>`, `<listing>` and `<p>Částka <code>UACODE8</code> Kč</p>`,
  each with a unique token: `Ok`; every token and the Czech text around it in `pdftotext`; the PDF embeds `JetBrainsMono`.
- A2: with `generic_family(Monospace, "Inter")` the tokens are present and no `JetBrainsMono`. A3: with no caller font
  at all, the tokens are present.
- A4: `:root{--f:'Nope'} .x{--f:Inter} p{font-family:var(--f)}` with `<p>LOST7</p>` passes the static check (F3
  post-review gap); strict → `Err(Error::TextWithoutFont)` naming `LOST7`; non-strict → `Ok` and exactly one warning
  naming `LOST7` (own test binary, global logger, like `page_overflow_warning.rs`).
- A5 (`fonts.rs` unit test): `style="font-family:monospace;content:&#39;x&#39;"` now passes the check; the same with
  `ui-monospace` still fails.
- A6 (probe, not gating): `<button>Odeslat</button>` and `<textarea>` text are drawn; if not, file a finding.
- fulgur fork: a `blitz_adapter.rs` test (`<pre>` with Noto Sans and `monospace → Noto Sans` has glyph runs; without the
  map it has none); `uncovered_text` unit tests (whitespace, ZWSP, inline boxes are not "dropped"); a
  `tests/render_smoke.rs` case.
- Fixtures 01–11 contain none of `pre|code|kbd|samp|tt|xmp|listing|plaintext|textarea|input|button|select` (grep): PDFs
  byte-identical to the current head, strict renders give zero dropped-text warnings, compare numbers unchanged. Probe
  PNGs `pn5` / `pi`: the `pre` lines appear in JetBrains Mono above the blue square.

## 5. Steps

1. Red tests A1–A4, A6 probe.
2. fulgur fork: `AssetBundle::generic_families`, `parse_inner` threading, `apply_generic_families`, fork test.
3. htmltopdf: `build()` passes the map, lock bump, A1–A3 and A5 green.
4. fulgur fork: `uncovered_text`, `DroppedText`, `strict_text`, warning; htmltopdf `Error::TextWithoutFont`; A4 green.
5. `render-fixtures.sh`, `cmp` against the previous head, `compare.py`, PNGs; `docs/upstream.md`; findings;
   `vrt-validator`; `pdf-reviewer` (fonts path).

## 6. Risks and non-goals

- The `parse_inner` signature change in a heavily patched file means rebase conflicts on the next fulgur bump; keep the
  diff to one parameter.
- False positives of the runtime check (collapsed whitespace, default-ignorables, inline-box placeholders) become errors
  in strict mode; A1, A4 and the zero-warning fixture run guard against that.
- Stylo rescales a keyword-derived font size when the computed family is exactly the generic `monospace` (Blitz
  `font_metrics.rs:154`, 13 px): UA `pre` / `code` now get that, as in Chrome; author `monospace` rewritten by F3b does
  not — an existing divergence, file it as a finding.
- Non-goals: a last-resort fallback for named families (`Collection::append_fallbacks`; it would render `'Nope'` in Inter
  instead of failing, which changes F3's contract — separate finding); Chrome's form-control faces; removing F3b's
  rewrite; `ui-*` aliases at engine level (fontique `FontInfoOverride`); text paths outside `extract_paragraph` (GCPM
  margin boxes) for the runtime check; SVG `<text>`.

## 7. Result

(Filled in at landing.)
