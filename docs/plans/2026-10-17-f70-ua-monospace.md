# F70 — UA-sheet `monospace` (`pre`, `code`, `kbd`, `samp`, `tt`) must resolve to a bundled face

Date: 2026-10-17. Finding: F70 (HIGH) in `2026-10-15-m0-findings.md`. Status: **done**. Builds on
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

Landed on branch `f70-ua-monospace`; fulgur fork `htmltopdf/srgb-colours` `78677ad` (generic map, dropped-text check) and
`c9230dc` (review fix below); the Blitz fork is unchanged (`52b22ef`).

What it does, as planned: `AssetBundle::generic_families` / `set_generic_family` (fulgur) reaches fontique's `Collection`
through `parse_inner` (one extra parameter) after the fonts are registered; `RendererBuilder::build()` hands over every entry
of `fonts::generic_map`; the engine therefore resolves `monospace` (and the five other generics Stylo parses) for the UA
sheet, `<link>` sheets and attributes we do not rewrite. An empty map leaves the collection untouched. F3b's rewrite stays;
`with_generics_unmapped` keeps the six engine generics as generics (A5).

| Check | Result |
|---|---|
| `pn5.html` (bare `<pre>`) | the four lines are drawn in JetBrains Mono above the blue square (was: only the square); `pdffonts`: JetBrainsMono only |
| `pi.html` | Inter `pre` and the bare `pre` ("mono default / second") are both drawn |
| A1 / A2 / A3 (`ua_generic_fonts.rs`) | green; all eight tokens and "Částka UACODE8 Kč" in `pdftotext`; JetBrainsMono embedded; none with `generic_family(Monospace, "Inter")`; works with no caller font |
| A4 (`dropped_text_warning.rs`) | strict: `Err(TextWithoutFont)` naming `LOST7` only; non-strict: `Ok` and exactly one warning; a document with whitespace, ZWSP and an inline box says nothing |
| A5 (`fonts.rs`) | `monospace` and the other five engine generics pass in an attribute with a character reference; `ui-monospace`, `ui-serif`, `ui-sans-serif`, `ui-rounded`, `math`, `emoji`, `fangsong` still fail |
| A6 probe | `<button>` and `<select>` text is drawn; `<textarea>` and `<input value>` are not, with any font (including `style="font-family:Inter"`): not a font problem, filed as F76 |
| Fixtures 01-11 | `cmp` byte-identical to the previous head (0 bytes differ, 11 of 11); zero warnings in the strict render; `compare.py` output identical; none contains `pre|code|kbd|samp|tt|xmp|listing|plaintext|textarea|input|button|select` |
| fulgur fork | `--lib` 2026, `render_smoke` 245, `fulgur-vrt` all green with goldens byte-identical (host-font path, empty map), `examples_determinism` 10 of 11 (the 11th, `committed_svg_matches_rendered`, is an environment failure of this host; not re-run on the old head) |
| Timing (A/B, alternating binaries, median of 25 / 11 wall-clock runs, CLI start-up included) | fixture 02: 48.4 / 48.3 ms and 46.6 / 44.1 ms before / after; 2000-row table (43 pages): 298 / 280 ms and 263 / 290 ms. The spread between rounds is larger than any difference: the per-paragraph coverage check costs nothing measurable |

Deviations from the plan:

- **Coverage is the run's `text_range()`, not the glyphs' ranges.** The first version (`78677ad`) collected each glyph's
  cluster range and reported ligature continuations (`office`), combining marks (`e` + U+0301), ZWJ emoji sequences,
  regional-indicator pairs, decomposed Hangul and Thai SARA AM as dropped text: a cluster's own range stops at its first
  character although the rest is shaped and drawn. Found by probing about 30 small documents under strict mode; `render_smoke` has the
  regression case. No false positive remains in those (whitespace, `font-size:0`, `visibility:hidden`, `opacity:0`,
  `text-overflow`, `-webkit-line-clamp`, `::before` / `::after`, lists, RTL, emoji, CJK, `<br>`, `<wbr>`, flex, multicol,
  `<details>`, off-screen text, `<ruby>`, inline SVG, Czech text), but only those were tried.
- `DroppedText` lives in `ConvertContext` as planned; the element is the *inline root* (a `<code>` inside a `<p>` is reported as
  `p`), recorded once per distinct (element, excerpt). The engine logs at most 20 lines per render and one more with the count.
  Pass 2 of a two-pass render does not report again.
- `parse_and_layout` and `parse_html_with_local_resources` keep their signatures (30 and 13 test callers) and delegate to
  `parse_and_layout_with_generics` / `parse_html_with_local_resources_and_generics`; only `parse_inner` and
  `render_v2` / `MarginBoxRenderer::new` gained a parameter (`parse_inner` needs `#[allow(clippy::too_many_arguments)]`).
- `strict_text` is wired to `strict_fonts` (no separate public switch in `htmltopdf`); `Error::TextWithoutFont` carries
  `Vec<String>` (`<element> "excerpt"`), never fulgur types.
- A6 is a committed test for button and select only (see the table).

Not done / not verified: margin boxes (`@page`) are not checked by the runtime check and their text did not get a new test;
the 13 px keyword-size rule is observed (UA `pre` / `code` 58.5 pt for ten characters, author `monospace` 72 pt) but Chrome was
not re-rendered for it (F74); bold `monospace` is still Regular (ADR-0002); `vrt-validator` and `pdf-reviewer` were not run by
the author. Filed: F74 (author `monospace` size), F75 (named-family last-resort fallback), F76 (`<textarea>` / `<input>` text).
