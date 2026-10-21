# F154: whitespace at the inner edges of inline elements is dropped (collapse across element boundaries)

Date: 2026-10-21. Finding: F154 (`2026-10-15-m0-findings.md:848-850`). It also closes F159(f), the trailing space in `content: "Pred "`. Status: planned.

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

(to be filled)
