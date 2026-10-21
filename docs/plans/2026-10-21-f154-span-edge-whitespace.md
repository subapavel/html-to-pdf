# F154: whitespace at the inner edges of inline elements is dropped (collapse across element boundaries)

Date: 2026-10-21. Finding: F154 (`2026-10-15-m0-findings.md:848-850`). It also closes F159(f), the trailing space in `content: "Pred "`. Status: done (§7).

**Owner action: none.** The fix goes in the existing Blitz fork only (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`). No Parley fork is needed (§3). Our crate gets tests, a fixture and the lock bump. Line numbers refer to `/home/user/blitz` on that branch and to Parley 0.6.0 (`Cargo.lock:1897`, `~/.cargo/registry/src/index.crates.io-*/parley-0.6.0`).

## 1. Problem

Whitespace at the inner edge of an inline element is deleted instead of collapsing with its neighbours. On main, `<p>a <span>b </span>d</p>` prints `a bd`, `<p>a<span> b</span> d</p>` prints `ab d`, and `<b>Splatnost: </b>2. 11. 2026` prints `Splatnost:2. 11. 2026`. Chrome keeps one space in each. No committed fixture shows the bug today: we grepped fixtures 01-21, `bench/`, fulgur VRT and the fulgur examples for inline elements with whitespace at an inner edge, both single-line and multi-line, and found nothing outside inline-block roots. That is why step 0 adds fixture 22.

Mechanism, verified in source:

- Blitz passes Parley the root's `white-space` mode (`construct.rs:919-924`) and resets it for every node (`:1083-1089`).
- Parley's `TreeStyleBuilder::push_uncommitted_text` collapses each span's text on its own. It runs `trim_start()` when `is_span_first` (`tree.rs:96-105`) and `trim_end()` when `is_span_last` (`:106-108`).
- `push_style_span` (`:155-164`) sets `is_span_first`, and `pop_style_span` (`:177-178`) commits with `is_span_last = true`. So the edges of every span are trimmed, not only the edges of the paragraph.
- The trimming at the root (`begin` `:75`, `finish` `:198`) is Parley's stand-in for removing spaces at line start and line end. The same code applied to nested spans is the bug.
- F150 worked around it for unstyled `display: contents` wrappers (`contents_needs_span`, `construct.rs:1021-1048`). The F150 fuzz generator avoids whitespace at element edges for the same reason.

## 2. Spec

There is no `css-spec-researcher` output. Every item below is **unverified** and quoted from memory.

- **CSS Text 3 §4.1.1, Phase I** (`#white-space-phase-1`), for `collapse` and `preserve-breaks`:
  - Spaces and tabs around a segment break are removed.
  - Segment breaks are transformed (§4.1.2): to a space under `collapse`, to a forced break under `preserve-breaks`.
  - Tabs become spaces.
  - "Any collapsible space immediately following another collapsible space — even one outside the boundary of the inline containing that space, provided both spaces are within the same inline formatting context — is collapsed." So the **first** space survives, together with its element's font and decoration.
- **CSS Text 3 §4.1.3, Phase II** (`#white-space-phase-2`):
  - A sequence of collapsible spaces at the beginning of a line is removed.
  - A sequence of collapsible spaces at the end of a line is removed.
  - Preserved spaces at the end of a line hang (`pre-wrap`).
- **CSS Text 3 §3 / Text 4 `white-space-collapse`.** `preserve` and `break-spaces` spaces are not collapsible, so a collapsible space after a preserved space is kept. NBSP (U+00A0) is never collapsible.
- **CSS 2.1 §16.6.1.** This is the older form of the same rules; it already collapses across inline boundaries.
- **`<br>`** is a forced line break (HTML rendering §15.3.x). Spaces before it are at a line end and spaces after it are at a line start.
- **CSS Lists 3.** The UA style `::marker { white-space: pre }` (unverified) bears on the inside-marker case W16.
- **WPT:** `css/css-text/white-space/`. Specific files were not checked.

## 3. Where it belongs

**Decision: (A), the Blitz fork only.** Blitz does Phase I itself, plus the Phase II trimming at the start and end of the inline formatting context and next to forced breaks. It then hands Parley the finished text in `WhiteSpaceCollapse::Preserve`. Parley passes `Preserve` text through unchanged (`tree.rs:92`), and nothing else in Parley 0.6 reads the mode (grep: `tree.rs` only).

Why (A):

- **The rule is defined over the inline formatting context.** Element boundaries, per-element `white-space`, atomic inlines, `<br>` and `pre-line` are Blitz's knowledge. Parley has a single builder-global mode, read at commit time, so mixed modes are already approximate.
- **Blitz already emulates part of it.** It handles `pre-line` itself (F66, `:1218-1235`).
- **Line-end hanging needs no change.** Parley's line breaker already hangs trailing whitespace at soft breaks (`greedy.rs:425-453`, `:547`; `alignment.rs:80-87`; min/max-content in `data.rs:537-561`).
- **No owner action.** The fork already exists.

Why not (B), a Parley fork: it would need the owner to create `subapavel/parley`. It would still leave `pre-line` and per-element modes in Blitz. It would mean rewriting text that has already been committed (a trailing space whose owning span has closed) inside Parley's flattened ranges. Drop our workaround when Parley's builder handles spaces across spans.

Changes in `packages/blitz-dom/src/layout/` use a **record, collapse, replay** design:

- **New `whitespace.rs`**, signatures only:
  - `pub(crate) enum InlineOp<'a> { PushSpan(parley::TextStyle<'static, TextBrush>), PopSpan, Text { text: Cow<'a, str>, collapse: stylo::WhiteSpaceCollapse, case: Option<TextTransformCase> }, Marker(String), InlineBox(u64), ForcedBreak }`
  - `pub(crate) fn collapse_white_space(ops: Vec<InlineOp<'a>>) -> Vec<InlineOp<'a>>`. This is a pure function.
  - `pub(crate) fn replay(ops: Vec<InlineOp<'_>>, builder: &mut TreeBuilder<'_, TextBrush>)`
- **State of `collapse_white_space`:** `prev ∈ {LineStart, CollapsibleSpace, Other}`, starting at `LineStart`, plus the position of the last emitted collapsible space.
  - Collapsible means `is_ascii_whitespace`, which is what Parley used. It never includes NBSP, so `2 000 Kč` survives.
  - `collapse` / `preserve-breaks` text: a space is dropped when `prev` is not `Other`. `preserve-breaks` splits at `\n` into `ForcedBreak`s; this replaces `:1218-1235`.
  - Preserved text: emitted verbatim. It sets `prev` to `LineStart` if it ends in `\n`, otherwise `Other`.
  - `InlineBox` sets `prev` to `Other`.
  - `ForcedBreak` and the end of the context delete the last collapsible space and set `LineStart`.
  - Spans are transparent.
  - `Marker` (inside list marker) keeps today's effect: its trailing space suppresses the next one (W16 checks this).
- **`construct.rs`:**
  - `build_inline_layout_into` (`:886`) and `build_inline_layout_recursive` (`:1070`) push `InlineOp`s instead of calling the builder, including the contents chain at `:950-1006`.
  - The builder is set to `Preserve` once. The per-node `set_white_space_mode` calls (`:924`, `:1089`) go.
  - `transform_text` moves into `replay` (collapse first, then transform, which is the spec order).
  - `push_forced_break` (`:1247`) keeps its empty span, so the style runs stay the same for unaffected documents.
  - Only the doc comment of `contents_needs_span` (`:1021-1024`) changes; it no longer cites trimming.

## 4. Acceptance

New file `crates/htmltopdf/tests/inline_whitespace.rs`. It uses the `display_contents.rs` helpers (bundled Inter, strict fonts, `pdftotext -bbox` `words()`).

There are two oracles:

- **Flat oracle.** An unstyled `<span>` must not move anything: word boxes equal those of the same text without tags, within 0.05 pt.
- **Chrome.** Word gaps (next `xMin` minus `xMax`) within 0.3 pt of the constants from step 0. Constants are relative, because the host Inter differs.

| ID | Shape | Main | Must show / assert |
|---|---|---|---|
| W1-W3 | `a <span>b </span>d`; `a<span> b</span> d`; `a <span>b <i>c </i></span>d` | `a bd`, `ab d`, `a b cd` | words separated; flat oracle |
| W4 | `<b>Splatnost: </b>2. 11. 2026`; `<strong>Celkem k úhradě: </strong>12 100 Kč` | glued | gap = Chrome (a bold space) |
| W5 | `a <span> b</span>`; `a<span style="font-size:30px"> </span> b` | | one space, not two; in the second, gap = Chrome's 30 px space (the first space owns it) |
| W6 | `<p style="width:200px;text-align:right"><span> a</span> b <span>c </span></p>`, also `center` | | right edge of `c` / centre = Chrome, so the context's start and end spaces are removed |
| W7 | `a <br> <span> b</span>`; `pre-line`: `a <span>b </span>\n  c` | | `b` / `c` start at the left edge; 2 lines |
| W8 | `a <span style="white-space:pre">  b  </span> c` | measure | gaps = Chrome |
| W9 | `a <span style="display:inline-block">X</span> b` | fine | guard: both gaps = Chrome |
| W10 | `.x::before{content:"Pred "}` / `::after{content:" Po"}` | glued | separated (F159f) |
| W11 | `a <span style="display:contents;color:red">b </span>d` | glued | separated |
| W12 | `Viz <a href="https://x.cz">web </a>firmy` | glued | separated; link annotation present |
| W13 | body as inline root (F111): `Faktura <b>č. </b>2026` | glued | separated |
| W14 | W1 inside `white-space:nowrap` (F66 `break_lines`) | glued | one line, separated |
| W15 | `text-transform:capitalize` on `a <span>b </span>d` | | `A B D` |
| W16 | `list-style-position:inside` + `<li> <span> x</span></li>` | measure | gap = Chrome |
| W17 | NBSP guard: `<b>12&nbsp;</b>&nbsp;000` | fine | two NBSP advances kept |

**Blitz tests:**

- `whitespace.rs` unit tests on `collapse_white_space` for each W shape, as op lists.
- `construct.rs` `whitespace_tests`, in the `contents_tests` style: `text_layout.text` is `"a b d"` and so on.

**Fixtures and bytes:**

- The grep predicts that fixtures 01-21, the bench corpus and the scaling variants stay **byte-identical** against main's binary. This is the gate. Every file that changes must be explained by markup with edge whitespace, or fixed.
- New `fixtures/invoices/22-inline-spacing/`: a Czech invoice with labels in `<b>…: </b>`, a link, a `::before` label and a `pre-line` note. Chrome references are committed. Record main's number and the fix's number; the fix must be lower. The threshold is the fix's number plus 1 point. No other fixture's diff may grow (merge policy item 3).
- fulgur `cargo test -p fulgur --lib` and the `fulgur-vrt` goldens are run against the fork. The only multi-line hits sit in inline-block roots (`inline-block-nested.html`, `review_card_inline_block.html`), where the trimming is unchanged, so no golden is expected to move. Re-bless only goldens whose diff is a restored space, and list them in the PR.
- `bench.py --check` must pass, because the record pass adds one `Vec` per inline context.
- The F150 fuzz generator stops avoiding whitespace at inline edges. It guards this fix with its wrapped-vs-plain oracle.

## 5. Steps

0. **Measure (½ day).**
   - Chrome constants for W1-W17 with `/tmp/claude-0/-home-user/a456d4dd-daaf-5a66-be03-3af9ac054072/scratchpad/chrome-pdf.mjs` (single probe files) or `render-ref.mjs` (fixture 22).
   - Main's output for each probe.
   - Re-run the fixture grep and add a Python `html.parser` scan for edge whitespace in `fixtures/`, `bench/` and the fulgur HTML.
   - Write the red tests: ours, plus the Blitz unit tests against a `collapse_white_space` stub that returns its input.
   - Draft fixture 22.
1. **Collapser (≤1 day).** `whitespace.rs`: the ops and `collapse_white_space`. Green: the Blitz unit tests.
2. **Record and replay in `construct.rs` (1 day).** Single `Preserve` mode, `pre-line` folded in, transform at replay, `whitespace_tests`. Green: `cargo test -p blitz-dom`.
3. **Wire up and measure (1 day).**
   - Use a local `[patch]` path to `/home/user/blitz`.
   - Green: `inline_whitespace.rs`, `white_space.rs`, `text_transform.rs`, `display_contents.rs` (including the fuzz test with edge whitespace), `body_inline_split.rs`.
   - Byte gate, `compare.py`, then look at `out/compare/22-*` and the PNGs of every changed file.
   - fulgur lib tests and VRT, `bench.py --check`.
   - Push the fork and bump the lock.
4. **Records (½ day).**
   - Fixture 22 references and threshold.
   - `docs/upstream.md`: a gap row for Parley's span-edge trimming (known, not reported, worked around in the Blitz fork) and a line on the fork branch row.
   - Mark F154 and F159(f) done.
   - Run `vrt-validator`.

## 6. Risks and non-goals

**Risks**

- **Shaping and kerning.** A kept space now sits at a style-run boundary. Kerning pairs that involve a space can shift words by hundredths of a point; the flat oracle's 0.05 pt allows for it. If it fails, relax it and record the number.
- **Line breaking.** Parley may break after an inline box (`greedy.rs:224-225`), so a space after an `<img>` can start a soft-wrapped line. Today does the same; this is not a regression.
- **Decoration over a hanging space.** `<u>word </u>` that ends at a soft wrap would draw its underline under the hanging space, where Chrome may not. Measure in step 0 next to W12. If they differ, file a new finding; do not fix it here.
- **Bidi.** Edge spaces are neutrals, resolved as in flat text. RTL is not exercised by invoices.
- **Marker space (W16)** is unverified against Chrome. Keep today's behaviour unless step 0 shows otherwise.
- **Rebase cost.** All of this is in `construct.rs`, which is already heavily patched (F4, F66, F150), so expect conflicts on a Blitz bump.

**Non-goals**

- A Parley fork.
- East Asian segment-break removal (§4.1.2).
- `break-spaces` wrapping after spaces (`stylo_to_parley.rs:103` TODO).
- Out-of-flow boxes being transparent to collapsing. They stay atoms, as today.
- Removing collapsible spaces at the start of a soft-wrapped line after an atomic inline.
- `nowrap` per span (F66b).
- Simplifying `contents_needs_span`. It still saves style runs and keeps bytes stable; only its comment changes.

## 7. Result

Implemented as planned, in the Blitz fork only (`d04479a`, after `b4cbc77`; new `layout/whitespace.rs`: `InlineOp`, `collapse_white_space`, `replay`; `construct.rs` records ops, one `Preserve` mode, `pre-line` and `text-transform` folded into the pass). fulgur fork `945667d`: one snapshot re-blessed (a test commit). Our crate: tests, fixture 22, the lock bump.

**Step 0, main `87d18ff` against Chrome 148.** The probes (`inline_whitespace.rs` W1-W17) were rendered by Chrome with the two bundled Inter files loaded through `@font-face`, so the constants are exact, not offsets between two Inter versions (tolerance 0.3 pt on gaps, one `xMax`-to-`xMin` distance per pair; flat text `a b d` has a 3.38 pt gap in both engines). Main's output is in the second column.

| Probe | Main | Chrome / fixed |
|---|---|---|
| W1, W2, W3 | `a bd`, `ab d`, `a b cd` | gaps 3.38 pt |
| W4a, W4b | `Splatnost:2.`, `úhradě:12` | bold space 2.85 / 2.84 pt, the rest 3.38 |
| W5a / W5b | one space / `ab` | one space / 6.33 pt (the 30 px space) |
| W6 right, centre | fine (guard) | `x` of the first word 165.06 / 103.90 pt (ours 0.23 pt left) |
| W7 (`br`, `pre-line`) | fine (guard) | `a` and `b` on separate lines at the left edge; `a b` then `c` |
| W8 | second gap 6.75 pt | both gaps 10.12 pt (three spaces) |
| W9, W16 | fine (guards) | 3.38 pt; `x` after an inside marker at the same place with and without the leading spaces |
| W10a, W10b | `PredmidPo`, `PrediPo` | gaps 3.38 pt |
| W11, W12, W14, W15 | glued | gaps 3.38 pt (`A B D` in W15); link annotation present |
| W13 | `č.2026` | 3.38 and 2.48 pt (kerning around the bold `.` and space; ours matches within 0.3) |
| W17 | gap 3.38 pt (one NBSP deleted) | 6.22 pt (two NBSPs) |
| X1 `a <br>b` centred | `b` 1.38 pt right of `a` | `b` 0.30 pt left of `a` |
| X3 `<u>aaa bbb </u>ccc` at 60 px | `aaa`, `bbbccc` | three lines |

**Red / green.** Ours `inline_whitespace.rs` (19 tests): on Blitz `b4cbc77` 15 fail (the 4 guards pass: W6, W7 break, W9, W16), on `d04479a` 19 pass. Blitz `whitespace.rs` unit tests (17), run against a `collapse_white_space` stub that returns its input: 8 fail, the other 9 hold on a stub because their input already holds the spaces; with the collapser 17 pass. Blitz `construct.rs` `whitespace_tests` (11): on the old `construct.rs` 8 fail, 11 pass now; `cargo test -p blitz-dom` 111 passed, 0 failed. F150 fuzz, generator changed (below): on `b4cbc77` 161 failure records in the 64 CI seeds (words glued), on the fix 0; `fuzz_long` (2000 seeds, 92 s release): 1 failure, seed 2767, which is not about white space (F166).

**Bytes.** Fixtures 01-21, the bench corpus (`b200-*`, `slow-*`) and the scaling variants against main's binary, 1 and 4 threads: 94 of 94 identical. No file changed, so there is nothing to explain; the scan (`html.parser` over `fixtures/`, `bench/`, fulgur's 94 HTML files) found edge whitespace only in fixture 19 (a newline-only text between two `<br>`, which was already removed at a line start) and in two fulgur VRT files whose spans are inline-block roots. `compare.py` numbers of 01-21 unchanged (all 22 pass); no threshold lowered.

**Fixture 22** (`fixtures/invoices/22-inline-spacing`, Czech invoice: `<b>…: </b>` labels, `<strong>`, `<em>`, `<a>` with trailing spaces, `::before` / `::after` labels, a `pre-line` note, NBSP amounts, spaces before `<br>`). 1 page, Chrome 1, main 1. Chrome diff: main's binary 4.0 % (3.96), fix 3.5 % (3.54); threshold 0.046. I looked at the side-by-side of both: on main every label touches its value (`Datum vystavení:21. 10. 2026`, `portálu zákazníkanebo`), with the fix the lines match Chrome's, and the link's underline runs under the space after `zákazníka` as in Chrome's. The rest is the text drift every fixture has and the synthetic italic of `<em>`.

**Bench.** `bench.py --check` ok. Medians, main -> fix (noisy machine, 4 cores): `01-basic` 22.0 -> 20.6 ms, `b200-text` 950 -> 916 ms, `b200-mixed` 971 -> 1060 ms, `b200-items` 1470 -> 1527 ms, `slow-paragraph` 3480 -> 3661 ms (one run each of seven): within the noise between two runs of the same binary, no measurable cost for the extra `Vec` per inline context.

**fulgur against the fork.** `cargo test -p fulgur` (every test target, the lib 2223): 2730 passed, 1 failed: `gcpm_snapshot::gcpm_counters_function_nested_ol_snapshot`, whose `li::before { content: counters(item, ".") ". " }` lost its trailing space on main (F159f); the PDF now holds the space glyph (`ToUnicode <0003> <0020>`, 19 instead of 18 glyphs); re-blessed in the fulgur fork `945667d` (only the glyph table and the stream change). `fulgur-vrt`: 26 of 64 goldens differ byte-wise on `b4cbc77` and on `d04479a` alike (the goldens are stale for our forks, not touched by this change); the same 26, two of them (`gcpm/toc-target-counter*.html`) 2-3 bytes larger: the restored space. Not re-blessed.

**Deviations from the plan.**

- W17 was marked "fine" in §4; it was not: Parley's `trim_start` / `trim_end` are Unicode trims and deleted an NBSP at an edge (Chrome 6.22 pt, main 3.38 pt). Fixed by the same change. W6, W7, W9 and W16 already matched Chrome on main and stay guards.
- A space before `<br>` was kept on main (the text is committed before the break without trimming): a centred `a <br>b` was shifted by 1.7 pt (X1/X4). Removed now, as §4.1.3 says.
- The old fuzz oracle did not see glue: wrapped and plain documents glue the same words, and the 97 % word floor tolerates a few. The generator now puts white space at element edges, sometimes *instead of* the separator (`w0001<b> w0002</b>`), and the test fails when one extracted word holds two tokens.
- `Marker` is its own op (it must not move the `capitalize` word start), and `stylo_to_parley::white_space_collapse` was removed (dead after the change). `push_forced_break` keeps its empty span but no longer switches modes.
- The comment of `contents_needs_span` changed as planned; the function stays (it saves style runs and keeps bytes stable).
- Chrome constants use `@font-face` with the bundled fonts instead of "relative offsets"; `FLAT_PT` 0.05 held with no relaxation (kerning risk did not materialise).

**New findings** (in the findings file): F166 (nested fixed `display: table` loses words, found by the long fuzz), F167 (inside list marker narrower than Chrome's, about 6.6 pt).

**Not verified.** The spec statements of §2 (still from memory). Underline over a space that hangs at a soft wrap (X3 checks the words only; fixture 22's link has the underline over the trailing space on one line, like Chrome). Right-to-left text. `break-spaces` wrapping after preserved spaces (non-goal). The fuzz test in the debug profile (only `--release` was run). `fulgur-vrt` goldens: they are stale at the base, so only "the same 26 fail" was compared, not their content.
