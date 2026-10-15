# F3 — Bundled fallback font, generic-family mapping, loud error on unresolved `font-family`

Date: 2026-10-15

## 1. Problem

If no bundled face matches any entry in an element's `font-family` list, the text silently disappears from the PDF. Fixture `fixtures/invoices/01-basic/index.html` lines 106–108 put the payment identifiers in `<span class="font-mono">`. In the committed `tailwind.css` that class resolves to `--font-mono: "JetBrains Mono", ui-monospace, monospace`. Here is why nothing matches today:
- `"JetBrains Mono"` is not bundled. Only Inter is, fetched by `scripts/fetch-fonts.sh`.
- `Renderer::engine` sets `system_fonts(false)`, so no generic family maps to anything.

The result is that `123456789/0300`, `CZ65 0300 0000 0001 2345 6789` and `20260001` are missing. `pdftotext` confirms it (M0 findings, F3). Fixtures 02 (`20260002`), 03 (`20260003`, `48 400,00 CZK`) and 04 (`CEKOCZPP`, `20260004`) lose their account, IBAN and VS values the same way. `fixtures/probe.html` row G (`font-family:monospace`) shows the generic-only case. This is data loss on a tax document. The existing test `invoice_identifiers_survive_into_pdf_text` in `crates/htmltopdf/tests/fixtures.rs` does not check text, so it passes.

## 2. Spec

No css-spec-researcher output was available. These are cited by the planner; **section numbers are unverified**.
- **CSS Fonts 4 §4.2, `font-family`, generic families.** `serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`, `system-ui`, `ui-serif`, `ui-sans-serif`, `ui-monospace`, `ui-rounded`, `math`, `emoji` and `fangsong` are generic keywords, and the user agent chooses which face each one maps to. That lets us map them to bundled faces.
- **CSS Fonts 4 §5, Font Matching Algorithm.** When the list is exhausted, the UA does system font fallback. A character that cannot be displayed *should* be shown somehow (a last-resort or missing-glyph glyph). Silently dropping a run breaks this. Turning an unresolved list into an error is our own invoice policy, which the spec does not forbid.
- **CSS Fonts 4 §5.1.** Family names match ASCII case-insensitively.
- **CSS Custom Properties 1 §3, `var()` substitution.** Tailwind reaches the family list through `var(--font-mono)` and `var(--default-mono-font-family, …)`.
- **ISO 32000-1 §9.10, ToUnicode.** `pdftotext` can only recover the text if fulgur/Krilla emit ToUnicode, which they already do for Inter.

## 3. Where it belongs

- **Upstream (file issues, do not wait):**
  - fulgur: a glyph run with no resolved font is dropped without a warning.
  - Parley/fontique: with system fonts disabled, the generic and fallback maps are empty, and the finding asks Parley to fall back to any available face rather than drop the run.
  - Record both in `docs/upstream.md`.
- **Local, `crates/htmltopdf` only.** This keeps fulgur imported in one place, and the policy (which face, error vs warn) is ours.
  - New `crates/htmltopdf/fonts/JetBrainsMono-Regular.ttf` plus `OFL.txt`, committed to git.
    - Why commit it: an *always*-bundled face has to be in the crate. Fetching it at build time would break offline and deterministic builds. Fetching it by script would make the guarantee false for library users.
    - Why JetBrains Mono: the fixture CSS already names it first, it is SIL OFL 1.1, and it covers Latin Extended-A (Czech).
  - New `crates/htmltopdf/src/fonts.rs`: built-in bytes, family-name extraction, the generic map, and a static family check.
  - `crates/htmltopdf/src/lib.rs`:
    - `RendererBuilder::build()` registers the built-in face *after* the caller's fonts, so the caller's faces win.
    - `Renderer::render` and `render_to_file` run the check before rendering.
    - New `Error` variant.
  - **Unverified:** the fulgur 0.40.0 API for generic families. Only `AssetBundle::{new, add_css, add_font_bytes, add_image}` and `Engine::builder().system_fonts(..)` are confirmed, by our own `lib.rs`. Step 1 settles it.
- **Signatures (sketch):**
```rust
pub enum GenericFamily { Serif, SansSerif, Monospace, Cursive, Fantasy, SystemUi,
                         UiSerif, UiSansSerif, UiMonospace, UiRounded, Math, Emoji, FangSong }
impl RendererBuilder {
    pub fn generic_family(self, g: GenericFamily, family: impl Into<String>) -> Self;
    pub fn strict_fonts(self, strict: bool) -> Self;            // default true
}
pub enum Error { /* … */ UnresolvedFontFamilies(Vec<UnresolvedFamily>) } // list text + origin
pub(crate) fn family_names(font: &[u8]) -> Vec<String>;
pub(crate) fn check_font_families(css_lists: &[FamilyList], html: &str,
    registered: &FamilySet, generics: &GenericMap) -> Vec<UnresolvedFamily>;
```
- `GenericFamily` is our own enum, so callers never see fulgur or fontique types.
- **Default mapping:**
  - `monospace` and `ui-monospace` go to the built-in face.
  - Every other generic goes to the family of the first font the caller added, or to the built-in face if the caller added none.
  - `generic_family` overrides either rule.
  - Reason: deterministic, explicit, and text never has nowhere to go.
- **The check:**
  - Sources: `font-family` and the `font` shorthand in bundled CSS (parsed once in `build()`, to protect the low-tens-of-ms budget) and in the document's `<style>` blocks and `style=""` attributes.
  - `var()` is resolved against the union of all declarations of that property.
  - CSS-wide keywords are skipped.
  - A list passes if any entry is a registered family or a mapped generic.
  - Output keeps document order (no HashMap iteration), so error text is deterministic.
  - Use the `cssparser` crate (already in the dependency tree).
  - With `strict_fonts(false)` the check only logs a `log::warn!`, as an escape hatch for false positives.

## 4. Acceptance

- **A1, test `mono_identifiers_survive_into_pdf_text` in `tests/fixtures.rs`.** Render each fixture and run `pdftotext <pdf> -`, failing loudly if `pdftotext` is missing. Collapse whitespace, then assert these substrings:
  - 01: `123456789/0300`, `CZ65 0300 0000 0001 2345 6789`, `20260001`, `IBAN: CZ65 0300 0000 0001 2345 6789`
  - 02: `20260002`
  - 03: `20260003`, `48 400,00 CZK`
  - 04: `CEKOCZPP`, `20260004`
  - This replaces the body of `invoice_identifiers_survive_into_pdf_text`.
- **A2, render tests with inline HTML and no caller fonts.** Each of `monospace`, `ui-monospace`, `sans-serif`, `serif` and `system-ui` renders a unique token (e.g. `GEN-MONO-0042`) that `pdftotext` finds. Repeat with Inter registered.
- **A3.** `<p style="font-family:'Nope'">x</p>` returns `Err(UnresolvedFontFamilies)` naming `'Nope'`. `'Nope', Inter` renders. `font: 12px/1.2 "Nope"` errors. All five invoice fixtures render with no error, so `tailwind.css` gives no false positives. The CLI exits non-zero on the error.
- **A4.** `cargo test --release --workspace` is green. The existing determinism tests still pass.
- **A5, PNG and number.**
  - In `out/compare/01-basic-p1.png`, the three values appear in a monospace face on the same baselines as their labels, and nothing else changes.
  - Run `python3 scripts/compare.py 01-basic` and record the number in the PR (baseline 5.6 %).
  - Expect |Δ| ≤ 0.3 pp, most likely down. A shift above 0.5 pp means the line box moved (CSS 2.1 §10.8) and the PR must explain it.
  - Do not change the 5 % threshold here; F1 is what brings 01 under it.

## 5. Steps

1. **Spike, ≤ 0.5 day, read-only.** Run `cargo fetch`, then read `fulgur-0.40.0/src` (grep `system_fonts`, `register_fonts`, `FontContext`, `Collection`, `GenericFamily`, `set_generic_families`, `append_fallbacks`). Answer:
   - (a) Does fulgur expose generic-family mapping, or a handle to the parley/fontique collection?
   - (b) Which name IDs does fontique use as the family name (1 or 16)?
   - (c) Does Stylo in servo mode parse `ui-*` as generic keywords or as family names?
   - (d) Are fonts that are registered but unused embedded in the PDF?
   - Append the answers to this plan as "Verified API".
2. **Built-in face, ≤ 0.5 day.**
   - Commit the TTF and `OFL.txt`.
   - Add `include_bytes!` and register the face in `build()`.
   - Write ADR-0002 "Ship a built-in last-resort font" (irreversible public behaviour, goes against the "fonts are fetched" convention).
   - Sort the font list in `tests/fixtures.rs::renderer()`, because `read_dir` order is not deterministic.
   - Red test A1 turns green.
3. **Static check, 1 day.** Add `fonts.rs` (parser, `family_names`, the check), the error variant and `strict_fonts`. Unit tests cover quoted vs unquoted names, case, `var()` with fallback, `font` shorthand and `inherit`. A3 turns green.
   - Until Step 4 lands, generics count as *unresolved*.
4. **Generic mapping, 1 day.**
   - If Step 1 found (a): call it from `Renderer::engine` and teach the check about mapped generics.
   - Otherwise: open a fulgur PR adding `AssetBundle::set_generic_family(GenericFamily, &str)` wrapping fontique `set_generic_families`, and carry it via `[patch.crates-io]` as ADR-0001 allows; track it in `docs/upstream.md`.
   - A2 turns green.
5. **Measure and document, ≤ 0.5 day.** Run `scripts/render-fixtures.sh`, then `compare.py`, and look at the PNGs. Record the numbers for 01 to 04. Update `fixtures/fonts/README.md` (mono is built in) and `docs/upstream.md` (both issues).

## 6. Risks and non-goals

**Risks:**
- If fulgur has no hook, Step 4 becomes an upstream PR plus a carried patch. Steps 2 and 3 still fix fixtures 01–04 and make every other case fail loudly.
- The static scan gets some cases wrong (false positives for unused declarations; false negatives for `<link>`/`@import` sheets). Mitigations: `strict_fonts(false)`, and A1 as a runtime-independent net.
- If our family-name rule differs from fontique's, the check could pass a list that still drops text. Step 1(b) settles the rule.
- The repo grows by about 300 KB (unverified). OFL 1.1 requires `OFL.txt` to ship with the font.
- Only the Regular face is bundled, so 03's `font-mono font-semibold` will be Regular or synthesised bold.
- Pre-existing, outside F3: the `.gitignore` rules `*.png` and `*.pdf` exclude `fixtures/invoices/*/reference/`, so a fresh clone has no Chrome references and `compare.py` compares against blank pages. The 01 number is only meaningful after running `scripts/reference/render.mjs`.

**Non-goals:**
- F1 (colour), F2 (`@property`), F4 (`text-transform`), F5 (table layout), F6.
- Per-glyph coverage fallback for scripts Inter lacks, and runtime detection of `.notdef` glyphs (upstream issue).
- Serif, CJK or emoji faces; SVG `<text>` host-font use; CLI flags for generic mapping; changing compare thresholds.
- Adding an `@font-face` to `input.css`, rebuilding `tailwind.css` or regenerating Chrome references (needs node; dropped from this plan).

## 7. Verified API (Step 1 spike, 2026-10-15, fulgur 0.40.0 / blitz-dom 0.2.4 / fontique 0.6.0 / stylo 0.8.0)

- **(a) Generic-family mapping: not exposed.** `AssetBundle` only has `add_css*`, `add_font_file/bytes`, `add_image*`,
  `set_base_url`; `fonts` is a public `Vec<Arc<Vec<u8>>>`. `blitz_adapter::parse_inner` builds the
  `parley::FontContext` privately and only calls `collection.register_fonts(blob, None)`. fontique does have
  `Collection::set_generic_families` / `append_generic_families`, but nothing hands us the collection, and
  `CollectionOptions::shared` only shares between clones of one collection (not global), so it is no back door.
  Consequence: Step 4 needs an engine change (upstream PR carried via `[patch.crates-io]`). See "Step 4 status".
- **(b) Family name rule.** fontique `register_fonts` uses name ID 16 (typographic family), else ID 1; the US-English
  name if present, else the first. `fonts::family_names` implements the same rule via `ttf-parser`.
- **(c) `ui-*` keywords.** Stylo in servo mode parses only `serif`, `sans-serif`, `monospace` (alias `-moz-fixed`),
  `cursive`, `fantasy` and `system-ui` (pref `layout.css.system-ui.enabled` is `true`) as generics. `ui-serif`,
  `ui-sans-serif`, `ui-monospace`, `ui-rounded`, `math`, `emoji` and `fangsong` are plain `FamilyName`s, so a mapping
  would have to register aliases under those names, not set a generic.
- **(d) Unused registered faces are not embedded.** Verified with `pdffonts`: with Inter and the built-in face
  registered, a document using only Inter embeds only Inter. The built-in face costs binary size, not PDF size.

## 8. Implementation status (what landed, what did not)

- Landed: Steps 1, 2, 3 and 5. The identifiers of fixtures 01-04 are in the PDF text; `font-family` lists that nothing
  can satisfy are an error.
- **Step 4 (generic mapping) did not land.** The engine has no hook (a), and the carried-patch route needs a copy of
  fulgur 0.40.0 (about 4 MB) vendored into the repo as a `[patch.crates-io]` path (or a fork branch). That was not done
  without a decision on where the patch lives. Until it lands, generics count as *unresolved* in the check, so
  `font-family: monospace` alone fails loudly instead of rendering. A2 is therefore not met; A3 is.
- Sketch of the patch (for the upstream PR): `AssetBundle::set_generic_family(GenericFamily, family)` and
  `add_font_alias(alias, family)` (for the `ui-*` names), applied in `parse_inner` after the `register_fonts` loop via
  `Collection::set_generic_families`; the mapping has to be threaded through `parse_inner`'s callers in `engine.rs`
  and `render.rs` (about 15 sites, all taking `fonts()` today). `htmltopdf` then fills `GenericMap` in
  `RendererBuilder::build()` and adds the `generic_family()` override from section 3.
- Measured (01-04 vs a blank reference, see the caveat in section 6): 01 3.1 % before, 3.3 % after (+0.2 pp of ink from
  the three mono values). These are not Chrome diffs.


## Post-review: what the static scan does and does not catch (vrt-validator, PR #1)

Catches: a named family no bundled font satisfies, in bundled CSS, `<style>` and `style=""`.
Does not catch (text still dropped, no warning): `font-family: var(--x)` with `--x` undefined or defined
differently in several places; an empty list. False positives in strict mode: unused rules, overridden rules,
`@font-face` with no use, numeric HTML entities in names. The scan is a safety net, not a cascade; the real
fix for the dropped-text class of bugs is upstream fallback behaviour (F3b).
Fixed in review: default is warn, not error; the CLI shows warnings without `RUST_LOG`; elements with no
`font-family` anywhere get the first bundled font through an `html {}` prelude.
