# F101 / F44 — a character with no glyph in any bundled font is drawn as `.notdef` without a warning

Date: 2026-11-04. Findings: F101 and F44 (medium, *Pilot bar*) in `2026-10-15-m0-findings.md`. Status: planned. Builds on F70
(`2026-10-17-f70-ua-monospace.md`: `DroppedText`, `strict_text`), F178 / ADR-0008 (`KrillaFonts::skip_run`) and ADR-0002.

## 1. Problem

Parley gives each cluster the first face in its `font-family` list that maps it. When no face maps it (the fontique fallback map is
empty because host fonts are off), the cluster keeps the face of its neighbours and gets glyph id 0. fulgur passes that id
unchanged into `krilla::text::KrillaGlyph` (`paragraph.rs` `draw_shaped_lines`, `LineItem::Text` arm, ~:893). Krilla then writes
CID 0 (`.notdef`). Depending on the viewer that is an empty box or nothing; it has no ToUnicode entry, so `pdftotext` loses the
character. Nothing reports it, with or without `--strict-fonts`. The F70 check (`inline_root.rs` `extract_paragraph`, :669) only
asks whether *a run* covers the text. Only F33's outside-marker path drops `.notdef` (`list_marker.rs` `drop_notdef_marker`, :292).
Repro, a Czech invoice rendered with Inter only:

    <p>Odběratel: 株式会社サクラ, Ōsaka</p>
    <table><tr><td>Cestovné – Trutnov ⇄ Berlin</td></tr></table>
    <p>Děkujeme za nákup 🙂</p>

It exits 0 with no warning, also with `--strict-fonts`. The customer's name is seven boxes and is missing from the text layer.
Fixture 04 has the real case: `⇄` (U+21C4, `index.html:76`) is in neither Inter nor JetBrains Mono (F101). F44's inside markers
(`A◦B▪C` with Noto Sans) are the same defect.

## 2. Spec (cited from memory, section numbers unverified)

- CSS Fonts 4 §5 (font matching; the wording is from Fonts 3 §5.2): when no font can display a character, the UA "should indicate by
  some means that a character is not being displayed" (a missing-glyph box or a last-resort font). Drawing `.notdef` conforms.
  Staying silent breaks our standing decision (warn by default, strict is opt-in): the box looks like a rendering choice, and the
  loss from the text layer cannot be seen at all.
- ISO 32000-1 §9.7.6.3 (handling undefined characters): a CID with no glyph shows `.notdef`. This is legal in plain PDF.
- ISO 19005-2/-3 §6.2.11.8 (veraPDF rule 6.2.11.8-1) and ISO 14289-1 (PDF/UA-1) §7.21.8: no text-showing operator may reference
  `.notdef`, in any rendering mode. PDF/A forbids it outright, so the warning is how a future PDF/A user (F62) learns that the file
  will fail validation. F71 cites §6.2.11.7.2; that is the Unicode-mapping clause (level U), which a `.notdef` also fails.
- Krilla's PDF/A and UA1 validators have a `ContainsNotDefGlyph` error (unverified). This plan does not rely on it.

## 3. Where it belongs and the decision

Parley is correct: it has no face for the character, and glyph 0 is the OpenType signal for that. Parley and Blitz stay untouched.
Reporting what reaches the PDF writer is fulgur's job, so it goes in the fulgur fork (`subapavel/fulgur`, `htmltopdf/srgb-colours`).
The error mapping, docs and strict policy go in `crates/htmltopdf`. No upstream issue or PR (standing decision).

**Detect at draw time, not in `extract_paragraph`** (F101 suggested the latter). The check must see exactly the glyphs Krilla writes:

- hidden runs (F209) and clipped-out lines (ADR-0009) are never written, so they must not fail a strict render;
- margin-box text goes through the same `draw_shaped_lines` with the same `KrillaFonts` (`render.rs` :289, :442), and F70's
  convert-time check misses it.

The cost is the same in both places: one `id == 0` compare inside a loop that already maps every glyph. There is a precedent:
`skip_run` already collects at draw time and reports through `render_v2` → `Engine::report_dropped_text`.

fulgur fork:

- `draw_primitives.rs`: `pub struct MissingGlyph { pub character: char, pub font_family: String }`, displayed as
  `U+21C4 "⇄" in Inter`. `KrillaFonts` gets a `MissingGlyphs` list (an ordered `Vec` plus a `BTreeSet<(char, String)>`, like
  `DroppedTexts`, so repeated content is reported once and the order is deterministic). New method:
  `pub fn note_notdef(&mut self, data: &Arc<Vec<u8>>, index: u32, cluster: &str)`.
- `note_notdef` is the cold path. It reports each character of the cluster that the face's `cmap` does not map
  (skrifa `charmap().map`). It skips whitespace (tabs too, see §6), control characters and default-ignorables (make
  `inline_root::is_default_ignorable` `pub(crate)`). If every character is mapped (a GSUB step produced 0), it reports the first
  visible one. The family name comes from `name` ID 16, else ID 1, else `face N`. It is read once per face and cached in the
  `KrillaFonts` entry.
- `paragraph.rs` Text arm, after the `run.visible` and `fonts.get` checks: for each glyph with `id == 0`, call `note_notdef` on
  `run.text.get(g.text_range)`.
- `render_v2` returns `(Vec<u8>, DrawReport)` with `DrawReport { skipped: Vec<DroppedText>, missing_glyphs: Vec<MissingGlyph> }`.
  `Engine::report_missing_glyphs(&self, Vec<MissingGlyph>) -> Result<()>` runs after `report_dropped_text`.
- `Error::MissingGlyphs(Vec<MissingGlyph>)` under the existing `strict_text`. The message is capped at 20 entries, like
  `TextWithoutFont`.
- Warning: one per (character, family), at most 20, then `... and N more characters with no glyph in their font`. Text:
  `font family "Inter" has no glyph for U+21C4 "⇄"; it is drawn as .notdef (an empty box or nothing) and is missing from the text layer`.

htmltopdf:

- `Error::MissingGlyphs(Vec<MissingGlyph>)`, with our own `pub struct MissingGlyph { pub character: char, pub font_family: String }`
  so callers never see fulgur types. Mapped in `Renderer::run` (`lib.rs` :410). Message:
  `characters with no glyph in their font, drawn as .notdef: …` (`join_capped`).
- **Switch: `strict_fonts`** (fulgur `strict_text`). **CLI: `--strict-fonts`.** No new flag. ADR-0007's rule is that one switch
  reports one class of problem, here text the bundled fonts do not cover, and a strict template must not ship boxes. Rewrite the
  "Not covered: … `.notdef`" sentences in `Error::TextWithoutFont`, `RendererBuilder::strict_fonts` (:730-735), fulgur
  `strict_text` and the CLI help (`main.rs` :58-61).

**F75 uses a different mechanism and is not included.** Its text is already reported (F70 `DroppedText`). What F75 lacks is a
*rendering* change: fontique's fallback map (`Collection::append_fallbacks` in `blitz_adapter::parse_inner`), so that `'Nope'`
draws in a bundled face. That moves the bytes of every non-strict document with such text, changes F3's contract and needs an ADR
(as the finding says). It also does not replace this check: a character no bundled face has is still `.notdef` after F75. F75
comes next as its own plan, and can then put a symbol face on the fallback list (F101b).

## 4. Acceptance (red first)

No new fixture is needed. The check only reads glyph ids (`krilla_glyphs` is unchanged), so every PDF stays byte-identical. Only
warnings and strict exit codes change, and the tests below assert those.

New `crates/htmltopdf/tests/missing_glyph_warning.rs`. It is its own test binary because the logger is global, as in
`dropped_text_warning.rs`. Fonts: Inter Regular and Bold.

- G1, strict: the §1 invoice with `⇄` three times, once in `<b>`. Expect `Err(MissingGlyphs)` with exactly U+682A, U+5F0F, U+4F1A,
  U+793E, U+30B5, U+30AF, U+30E9, U+21C4, U+1F642, each `in Inter`, each once, in drawing order. No Czech letter and no `Ō`.
- G2, not strict: `Ok`, and one warning per G1 entry naming its code point, the character and `Inter`.
- G3, strict `Ok`, nothing reported: whitespace, `\u{200B}`, `\u{00AD}`, `<br>`; a `<pre>` with newlines and a tab; `株` in a
  `visibility:hidden` span; `株` on a line clipped out by `overflow:hidden` (ADR-0009).
- G4: `@page { @bottom-center { content: "株" } }` is reported. F70's check does not cover margin boxes.
- G5: an inside marker `list-style-type: "株 "` is reported (F44). **Not constructible, see *Step 1 result*:** G5 became a guard that
  markers report nothing.
- G6: 25 distinct missing characters give 20 warnings plus `... and 5 more`. The error message names 20; the vector holds 25.
- G7: text in `font-family:'Nope'` gives `TextWithoutFont`, not `MissingGlyphs` (no run, so no glyph).
- CLI (`crates/htmltopdf-cli/tests/`): the G1 HTML with `--strict-fonts` exits 1 and stderr names `U+21C4`. Without the flag it
  exits 0 and writes the PDF.
- fulgur fork: `note_notdef` unit tests (a mapped base with a missing mark, a cluster where every character is mapped,
  ignorables, dedupe across Regular and Bold of one family). Add a `tests/render_smoke.rs` case, because the draw path is not in
  codecov otherwise.
- Fixture 04 (red today): its strict render returns exactly `[U+21C4 in Inter]`. This also proves it passes every earlier strict
  check. Record it in `fixtures/glyph-gaps.txt` (`04-bilingual-vat U+21C4`), read by `fixtures.rs`, `row_borders.rs`, `no_unpainted_content.rs`,
  `determinism_threads.rs` (the four test binaries that render every fixture strictly), `render-fixtures.sh` and `bench.py`. A listed fixture is rendered without `--strict-fonts` and must warn about exactly its listed
  code points, so a new gap still fails. Every other fixture and `bench/corpus/` document stays strict and must report nothing
  (a grep finds only `→` and `−` outside Latin, both in Inter; step 1 confirmed it by rendering, see below).

Byte gate: `cmp` every fixture and corpus PDF against the previous head, 0 bytes differ. `compare.py` numbers identical, no
threshold moves. fulgur-vrt goldens byte-identical. PNGs unchanged: fixture 04's `⇄` stays a box, which is F101b.

## 5. Steps (size: small, about 3 days)

1. Red tests G1–G7, the CLI test, the fixture-04 assertion, `glyph-gaps.txt` and its readers in the two Rust tests. (½ day)
2. fulgur fork: `MissingGlyph`, `KrillaFonts::note_notdef` and the family cache, the `paragraph.rs` call, `DrawReport`,
   `report_missing_glyphs`, `Error::MissingGlyphs`, unit and smoke tests. (1 day)
3. htmltopdf: map `Error::MissingGlyphs`, update docs and CLI help, bump the lock; G1–G7 and the CLI test go green. (½ day)
4. `render-fixtures.sh` and `bench.py` read the gap file. Run the byte gate, `compare.py`, look at the PNGs, run `bench.py --check`.
   Record the fork commit in `docs/upstream.md`. Mark F101 / F44 done; file F101b and F101c. (½ day)
5. `vrt-validator`; `pdf-reviewer`, because this touches the fonts path (merge policy item 5). Fix their findings on the branch. (½ day)

## 6. Risks and non-goals

- False positives turn strict renders into failures. Guards: the whitespace / control / ignorable filter, the `visible` and
  clipped-line skips (G3), and every fixture and corpus document except 04 reporting nothing.
- Cost: one integer compare per written glyph. The `cmap` and `name` lookups run only for `.notdef` clusters, the `name` lookup
  once per face. F70's comparable per-paragraph check was unmeasurable. `bench.py --check` is the guard; any growth beyond noise on
  `b200-items` blocks the PR.
- Strict templates that passed before now fail on a box they were already shipping. This is intended; say it in the PR.
- In a right-to-left paragraph over 64 KiB, cluster ranges wrap (F79). `.get()` then yields no text, or the wrong text, so such a
  glyph may be named wrongly or reported as `?`. It is still reported.
- Changing the `render_v2` return type in a heavily patched file means rebase conflicts on the next fulgur bump. Keep it to one
  struct.
- Non-goals:
  - drawing the character (F75, F101b);
  - PDF/A conformance (F62);
  - SVG `<text>` (F101c);
  - form-control text (F76);
  - F33's outside-marker drop, which stays a warning because nothing is drawn;
  - tabs in `pre` (F71: drawn as `.notdef`, but expanding them is the fix; reporting a tab as a "character" would fail every
    strict `<pre>`).

Filed by this plan:

- F101b: fixture 04's `⇄` prints `.notdef` where Chrome draws an arrow from a host face. It needs a bundled symbol face reachable
  through F75's fallback list. Then remove the gap entry and re-baseline 04 as content that is now rendered.
- F101c (low): `.notdef` in SVG `<text>` (usvg / krilla-svg path) is not reported.

## Step 1 result (2026-11-04)

- **Gap list by rendering.** All 45 fixtures and the 7 `bench/corpus/` documents were rendered with main's binary (`f550a8e`)
  and every text-showing operator was scanned for CID 0 (`qpdf --qdf`, Identity-H two-byte codes). Fixture 04 has exactly one:
  `⇄` U+21C4 (`index.html:76`, `Cestovné – Trutnov ⇄ Berlin`). The other 51 PDFs have none. A second, independent check read
  the `cmap` tables of the five Inter weights and JetBrains Mono and tested every non-ASCII character of every `index.html`,
  corpus document and `tailwind.css`: the same single code point. The scan also finds the tab of a `<pre>` (G3), the one
  `.notdef` that the plan leaves unreported on purpose.
- **G5 cannot be built.** Blitz ignores a string `list-style-type` and `::marker { content }` (the item gets its disc), and every marker
  type fulgur emits (disc, circle, square, disclosure-*, the numbering systems, the `□` it uses for the ones it does not know) is
  mapped by Inter or by the built-in JetBrains Mono, so no inside marker reaches `.notdef` with the bundled faces; F44's
  `A◦B▪C` needed Noto Sans without those glyphs. G5 is therefore a false-positive guard (every list type reports nothing). The
  marker path of `note_notdef` is covered by the fork's unit tests, not by an htmltopdf test.
- **Choice (a).** `Error::MissingGlyphs` and `MissingGlyph` exist in `htmltopdf` but nothing produces them yet; the red tests
  are `#[ignore = "F101: red until the fork reports .notdef"]`. Step 3 removes the `ignore`s.
