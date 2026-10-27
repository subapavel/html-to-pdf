# F178 — a bundled font the PDF writer cannot embed is left out (warned) or refused (strict)

Date: 2026-10-27. Finding: F178 (`2026-10-15-m0-findings.md`). The fix goes on `subapavel/fulgur` `htmltopdf/srgb-colours`
(the check when a font is registered, plus a safety net at draw time); the strict-mode mapping is ours.

## 1. Problem

Parley (fontique, skrifa) and krilla parse a bundled font separately, and they disagree. Repro (F99e pdf-reviewer):
`fixtures/fonts/Inter-Bold.ttf` with the `head` tag in its table directory renamed to `hexd` (directory still sorted).
fontique registers the face and shapes `font-bold` text with it; at draw time `KrillaFonts::get`
(`draw_primitives.rs:672`) gets `None` from `krilla::text::Font::new`, and `paragraph.rs:842` skips the run with a bare
`continue`: exit 0 even with `--strict-fonts`, the bold lines missing from the PDF, no warning. With `glyf` renamed
(`glyx`) instead, krilla accepts the face, `FontInfo::can_be_cid_font()` is false, and the face is embedded as a
non-subset Type 3 font whose glyph procedures are empty (`type3.rs:169`: no outline, no path): `pdftotext` still finds
the text but nothing is painted — the loss looks like success. SVG text has the same skip (`krilla-svg-0.7.0`
`text.rs:34`, `convert_fontdb` drops a `None` face). Bytes that are not a font at all (`font_bytes(b"junk")`) only log
`unknown font magic bytes; passing through` (`asset.rs:89`). Silent data loss on an invoice that bypasses the strict
switch (AGENTS.md goal 1, ADR-0002).

## 2. Spec (from memory, unverified; step 0 checks Chrome)

- OpenType 1.9 "Required tables": `cmap`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2`, `post`, plus outlines
  (`glyf`+`loca`, or `CFF `/`CFF2`). A face without `head` is not a conforming font; a face with no outlines and no
  colour or bitmap glyph tables (`COLR`, `SVG `, `CBDT`, `sbix`) has nothing to draw.
- css-fonts-4 §4.3 (`src`): an invalid font resource is treated as if it failed to load; §5 (font matching) continues
  with the remaining faces, so text is drawn in a fallback face, never dropped. Our UA fonts are the bundle (ADR-0002,
  ADR-0007). §5.2: `font-weight: 700` with only 400 available matches 400; we do not synthesise bold (ADR-0002), so the
  fallback is Inter-Regular.
- Chrome runs OTS on web fonts: a face with a required table missing fails, and for a collection the whole file fails
  (unverified; step 0 checks the fallback with an `@font-face` `data:` URL).
- PDF 32000-1 §9.6.5 (Type 3): glyphs are content streams; an empty stream paints nothing but still maps to Unicode —
  the invisible-but-extractable text.

## 3. Where it belongs

krilla's `Option` is the right API for its caller; skipping the run silently is fulgur's choice. The check must run on
the bytes fulgur keeps (WOFF2 is decoded inside `AssetBundle`) and use krilla's own parser (it decides what gets
embedded). Our crate imports only fulgur and does not depend on krilla, so the check goes in the fulgur fork and
reaches us through a fulgur error.

| Part | Where | Change |
|---|---|---|
| Check | fulgur new `font_check.rs` | `pub fn check_font(bytes: &[u8]) -> Result<(), FontRejection>`: for each face `0..n` (skrifa `FileRef` count): `FontRef::from_index` (else "not an OpenType font: {e}"); `krilla::text::Font::new(Arc::clone(..).into(), i)`, `None` diagnosed as "no `head` table" / "metrics are not finite" / "unitsPerEm is 0"; then glyphs: outlines present (skrifa `outline_glyphs()`, exact API checked in step 2) or a colour / bitmap table, else "no glyph outlines". `FontRejection { faces: Vec<RejectedFace> }`, `RejectedFace { index: u32, name: Option<String> /* name ID 4 */, reason: String }`. |
| Registration | fulgur `asset.rs` `add_font_bytes` | After the WOFF2 decode, `check_font(&decoded)`; on failure `Err(Error::FontRejected(r))`, bytes not pushed. Unknown magic is no longer passed through. Parley (`parse_inner`, `MarginDocEnv::new`) and `SvgFonts` all read `fonts`, so a rejected file is missing from all three by construction. |
| Error | fulgur `error.rs` | `FontRejected(FontRejection)`; the message names each face (index, name, reason). |
| Safety net | fulgur `KrillaFonts`, `paragraph.rs:842`, `render.rs:190`, `engine.rs` | At the `continue`, record a `DroppedText { element: "glyph run in a face the PDF writer cannot read", excerpt }` into `KrillaFonts::skipped`; `render_v2` hands the list back and the engine reports it like the dropped text at line ~739 (`Error::TextWithoutFont` under `strict_text`, otherwise `warn!`). Only reachable when `AssetBundle::fonts` (pub) is filled directly, or with system fonts. |
| Ours | `crates/htmltopdf/src/lib.rs` `add_font`, `build`, `render`, `render_to_file` | `Err(fulgur::Error::FontRejected(r))` → `RejectedFont { source: String, faces: Vec<String> }` (`source`: the path for `font_file`, `font_bytes #N` otherwise); the file's families are not added to `registered` and never become `default_family`. `build()` non-strict: one `warn!` per file ("font `<source>` is left out: face 1 (Inter Bold): no `head` table; its text uses the other bundled faces"). `render*` with `strict_fonts`: `Err(Error::RejectedFonts(Vec<RejectedFont>))` before `check_fonts`. |

Decisions:

- Strict refuses at render time, not in the builder: builder methods come in any order (`dropped_text_warning.rs` calls
  `strict_fonts` after `font_bytes`) and `build()` cannot fail; this is where `check_fonts` refuses too.
- Non-strict warns and leaves the font out instead of failing: today the document renders minus that face's text; a
  hard error would produce no PDF at all, against "warn by default".
- Existing font errors are unchanged: WOFF1, a WOFF2 decode failure and an I/O error still return `Err` from the
  builder at once.
- A collection with one bad face is left out whole: fontique registers a blob with all its faces and `unregister_font`
  matches by attributes (could remove a good face of another file); OTS also rejects a whole collection.
- A face with no outlines is refused like a face without `head` (empty glyphs on a tax document are data loss);
  colour-only faces are accepted (krilla draws them as Type 3).

## 4. Acceptance

New `crates/htmltopdf/tests/font_rejected.rs` with helpers `rename_table(font, from, to)` (keeps the directory sorted)
and `ttc_of(&[&[u8]])` (collection header + rebased table offsets); fonts from `pdffonts` (subset prefix stripped), text
from `pdftotext` as in `svg_text_fonts.rs`; the logger is captured in one test per process.

1. `head` renamed, non-strict: Inter-Regular + the bad Bold, `<p>Normální</p><p class="font-bold">TUČNÝ ŘÁDEK</p>`:
   `Ok`; both texts in `pdftotext`; `pdffonts` = {Inter-Regular}; exactly one warning naming `face 0 (Inter Bold)`, the
   source and `head`; no "text has no font face" warning. Red: TUČNÝ missing, no warning.
2. Strict, `strict_fonts` both before and after `font_bytes`: `Err(RejectedFonts)` naming the same; `htmltopdf-cli
   --strict-fonts --font <bad>`: exit 1 with that message, stdout empty.
3. `glyf` renamed: as 1 and 2, and `pdffonts` lists no `Type 3`. Red: a Type 3 Inter-Bold.
4. A collection of (Inter-SemiBold, `head`-renamed Bold) next to Inter-Regular: the warning names `face 1`;
   Inter-SemiBold is not in `pdffonts` (whole file left out); strict `Err`.
5. Junk bytes registered first, then Inter: warning "not an OpenType font"; the default family is `Inter`; strict `Err`.
6. SVG (F152 path): inline `<svg><text font-family="Inter" font-weight="bold">TUČNĚ</text>` with the `glyf`-renamed
   Bold: drawn, only Inter-Regular embedded, no Type 3. Red: Type 3.
7. Fork: `render_smoke.rs` — an `AssetBundle` whose `fonts` gets the bad bytes pushed directly: `strict_text` →
   `Err(TextWithoutFont)` with the excerpt, non-strict → one warning. `font_check.rs` unit tests: Inter ok, built-in
   mono ok, `head` renamed, `glyf` renamed, junk, collection index.
8. Neutral: fixtures 01-25 byte-identical, `compare.py` change 0, `bench.py --check` ok. Registration cost measured in
   step 0 and recorded in §7 (expected microseconds per face: `Font::new` reads the table directory, metrics, `name` and
   outline headers, never the glyph data); target under 1 ms for five Inter faces + JetBrains Mono, under 5 ms for a
   17 MB TTC (host `wqy-zenhei.ttc` in a scratch probe only, never in a committed test).

## 5. Steps

0. Bad fonts in `out/scratch/f178/`; Chrome reference (`@font-face` `data:` URL of each bad Bold, expected to fall back
   to Regular with the text drawn); confirm the empty Type 3 glyphs with `pdftoppm`; measure the check cost.
1. Ours: tests 1-6, red.
2. fulgur: `font_check.rs` + unit tests.
3. fulgur: `add_font_bytes` calls the check; `Error::FontRejected`; invert `test_add_font_bytes_unknown_passthrough`;
   VRT unchanged.
4. fulgur: the draw-time safety net + smoke test.
5. Ours: lock bump; `RejectedFont`, `Error::RejectedFonts`, the warning in `build`, the strict check in `render*`; CLI
   test. Gates: `compare.py`, `bench.py --check`, `vrt-validator`, `pdf-reviewer` (fonts, merge-policy item 5).
6. Docs: ADR-0008 "A bundled font file the PDF writer cannot embed is left out whole; strict refuses it";
   `docs/upstream.md` fork row (behaviour change); `RendererBuilder::font_file` / `strict_fonts` doc comments; findings
   (F178 done, F178b, F178c filed).

## 6. Risks and non-goals

- Fork behaviour change: fulgur callers passing bytes with unknown magic now get `Err` (those bytes never drew
  anything); recorded in `docs/upstream.md`.
- A face krilla accepts but whose subset fails at write time already fails the whole render (`PdfGeneration`): loud,
  out of scope.
- The check parses each face once per `Renderer` and the render again (`KrillaFonts`); sharing is a follow-up only if
  `bench.py` shows a cost.
- Non-goals: leaving out only the bad face of a collection (F178b); deeper sanitising (broken `loca` / `cmap`, all-empty
  outlines; F178c, OTS-like); WOFF1 / WOFF2 decode errors as warnings; synthetic bold; `@font-face` in documents.
