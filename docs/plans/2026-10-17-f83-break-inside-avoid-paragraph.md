# F83 — A `break-inside: avoid` paragraph taller than a page loses words

Date: 2026-10-17. Finding: F83 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after `ab9fe07`), carried via `[patch.crates-io]`. All fork line numbers are at `ab9fe07`.

## 1. Problem

`<p style="break-inside:avoid">` of 2500 `wordNNNNN` words (A4, 15 mm, Inter; `/tmp/claude-0/vrt-f78/ab/pbi_normal.html`,
`pbi_1.3.html`) prints 2478 words on 7 pages. The renderer warns once per page: "... recorded no matching line ranges
(0 slices for 7 fragments ...)". `crates/htmltopdf/tests/fallback_warning.rs:69-78` pins that warning today. Invoice
case: Tailwind `break-inside-avoid` on a terms paragraph that grows past one page. The bundled CSS is injected as a
`<style>` (fork `engine.rs:126`), so the fork's own CSS reader (it fills `column_styles`) sees the class too
(`engine.rs:492`). Root cause, from reading:

1. Body walk (`fragment_pagination_root`, `pagination_layout.rs:960`): `avoid_inside` (`:1290-1293`) makes
   `line_metrics` empty (`:1365-1372`, fulgur-k0g0). So the paragraph skips the line path
   (`fragment_inline_root`, `:1373-1436`), the only body-level emitter of `LineSlice`s besides the nested one.
2. The recursion gate (`:1495-1514`) is false: a `<p>`'s only children are text nodes of height 0, so
   `would_split_block_subtree` skips them (`:2094-2098`). The paragraph moves to a fresh page if something precedes it
   (`:1583-1586`). Then, being taller than `strips.fresh() + 1` (`:1637`), it reaches the fulgur-sbw2 strip slicer
   (`:1638-1744`). That slicer cuts it into strips at fixed pixel heights (`bottom − cursor`, then `fresh()`), records
   no `line_slices`, and records descendants on the first page only (`:1650-1658`).
3. `paragraph_lines_for_page` (`render.rs:3873-3951`) finds no slices (`:3887`) and measures lines from the strip
   heights (`:3900-3931`). A strip edge falls mid-line. From reading: each page starts at the line that straddles the
   edge, drawn up to one line above the content top, and keeps only the whole lines that fit. So up to one line per
   break is drawn on neither page. Step 1 checks this against the 22 lost words.

The nested paths are not affected, from reading. `nested_line_split` (`:4085-4100`) never reads `break-inside`, and
`avoid_inside_child` (`:3048-3051`) feeds only the multicol test (`:3052-3064`). So `div > p{avoid}` and
`div{avoid} > p` (gate → `fragment_block_subtree` → `fragment_nested_inline_root`, `:4159-4236`) already record slices.

## 2. Spec (cited from memory, unverified; css-spec-researcher not run)

- css-break-3 §3.2 `break-inside: avoid`: avoid a break inside the principal box. §3.4: `page-break-inside` is a
  legacy alias (`avoid` → `avoid`). §3.3 `orphans` / `widows`: initial value 2.
- css-break-3 §4.4 Unforced Breaks: Rules 1–2 (avoid at class A points, `avoid` on an ancestor), Rule 3 (orphans /
  widows), Rule 4 (class B/C points only inside `break-inside: auto`). When these leave no way to keep content from
  overflowing the fragmentainer, Rules 1, 2 and 4 are dropped first and Rule 3 only after that. CSS 2.1 §13.3.3 has the
  same order (A, B, D, then C). So an `avoid` paragraph taller than a page breaks between its line boxes like an `auto`
  one, with orphans/widows still in force. §4.1: a line box is never split.
- css-break-3 §4.5: the UA should break as little as possible and prefer breaks that violate no rule. §5.2: margins
  next to an unforced break are truncated. §5.3: the box decorations are sliced (`box-decoration-break: slice`).
- Chrome (unverified, probed in step 1): LayoutNG scores a break inside an `avoid` box below a break in front of it. If
  the box does not fit the rest of the page, Chrome breaks before it, so it starts on a fresh page whether or not it
  fits there. If it does not fit there either, Chrome breaks it between lines on that page, with orphans/widows 2. A box
  that is first on its page starts in place.

## 3. Design (fulgur fork)

Rule: `break-inside: avoid` holds while the box fits a page. Past that it is relaxed and the box breaks as if it were
`auto`. `multicol_wants_split` (`:394-400`) already does this for multicol containers.

- `fn avoid_inside_holds(avoid_inside: bool, height: f32, fresh: f32) -> bool`, next to `multicol_wants_split`:
  `avoid_inside && height <= fresh + 1.0`. The 1 px tolerance is the one the strip slicer uses (`:1614-1624`, `:1637`).
  So the inputs that change are exactly the avoid inline roots taller than a page.
- Body call site `:1368`: `if avoid_inside_holds(avoid_inside, child_h, self.strips.fresh()) { Vec::new() } else
  { collect_inline_line_metrics(child) }`. Rewrite the comments at `:1365-1367`, `:1463-1465` and `:1577-1582`, and the
  docs at `:53` and `:730-731`.
- A relaxed paragraph then takes the `auto` line path. The pre-advance at `:1382-1389` still moves it to a fresh page
  when something precedes it, the same start the strip path gives today (`:1583`). So placement does not change, only
  where the cuts fall. `fragment_inline_root` records tiling `LineSlice`s (`:4441-4445`, `:4472-4481`) and keeps
  ORPHANS/WIDOWS = 2 (`:4390-4393`, `:4411-4425`), as css-break-3 §4.4 requires. Inline boxes go to the page of their
  line (`record_split_inline_root_descendants`, `:1405-1414`) instead of page 1 only.
- An avoid paragraph that fits a page: `avoid_inside_holds` is true and the code path is today's. It moves whole to the
  next page (`:1583`). Byte-identical.
- Nested and wrapper paths, multicol, the renderer: no change. `paragraph_lines_for_page` takes the slice branch
  (`render.rs:3887-3898`) and stays silent.
- Byte identity. Fixtures 01–12: only 02 uses `avoid` (on `tr`, and on two `.keep-together` sections that fit a page and
  are not inline roots), so all expected byte-identical. Fulgur: `avoid` appears only on text-free boxes in the multicol
  VRT fixtures and on the `examples/break-inside` callout (a `div` holding a `<p>` that fits). So every VRT golden and
  all 21 examples are expected byte-identical. Any re-bless is a regression.
- Filed, not fixed here:
  - **F83a**: `page-break-inside` is not aliased. Fulgur's CSS reader (`column_css.rs:606-628`) maps only
    `page-break-before` and `page-break-after`. So `page-break-inside: avoid` is ignored: no loss, but nothing is kept
    together. It must land after F83, which makes it safe.
  - **F83b**: the recursion gate (`:1495-1514`, `:3025-3031`) and `nested_line_split` ignore `avoid`. If the step 1
    probe confirms, a block with several children that fits a fresh page is split in place. A too-tall avoid box below
    content starts in place in the nested and wrapper paths, where Chrome (unverified) moves it.

## 4. Acceptance

New file `crates/htmltopdf/tests/break_inside_avoid.rs` (A4, default options, Inter, strict). It uses the
`long_paragraph_pages.rs` helpers (`words`, `render`, `printed_words`, `page_count`, `assert_every_word_once`):

1. Body-level `<p style="break-inside:avoid">` with 2500 words: every word once, in order, inside the content box. The
   page count and the first word of each page equal those of the same `<p>` without the style (a relaxed avoid is
   `auto`). **Red** (2478 printed).
2. `<p>before</p>`, then case 1: `word00000` on page 2, every word once. **Red.**
3. Body-level `<div style="break-inside:avoid">` with 2500 words of direct text (an inline root that is not a `p`).
   **Red.**
4. Guards, green today, must stay green:
   - `<p style="page-break-inside:avoid">` with 2500 words (stays green when F83a lands).
   - `<div><p>before</p><p style="break-inside:avoid">…</p></div>`.
   - `<div style="break-inside:avoid"><p>…</p></div>`.
   - `<div style="break-inside:avoid">` holding four `<p>` of 700 words each (≈ 2.5 pages together): every word once.
5. Guard that it still moves whole: `<div style="height:900px"></div><p style="break-inside:avoid">` of 200 words.
   Every word on page 2, and the same inside a `div`.

`fallback_warning.rs`: `fallback_warnings` returns the PDF as well. The third case asserts no warning and all 2500
words (`pdftotext`). Update the module doc: the warn path is now covered only by the fork unit test (`render.rs:5679`).

Fork tests:

- Unit test of `avoid_inside_holds`: auto → false; fits → true; `fresh + 0.5` → true; `fresh + 1.5` → false.
- Lib tests (`parse`, `extract_column_style_table`, `run_pass_with_break_styles`, 200 px page):
  - A body-level avoid `<p>` of 40 lines at `line-height:20px`: `line_slices.len() == fragments.len()`,
    `line_slices_tile`, and fragments and slices equal to the same `<p>` without avoid. **Red.**
  - A 5-line avoid `<p>` after a 150 px spacer: one fragment, on page 1.
  - `div > p{avoid}` and `div{avoid} > p`: slices tile.
- `render_smoke`: 7000 words in Noto Sans, `<p style="break-inside:avoid">`, every word once. Twin of
  `render_smoke.rs:7371`. **Red.**
- Keep `tests/break_inside_avoid.rs:47-81` green.

Pixel diff: fixtures 01–12 `cmp` byte-identical. Compare numbers unchanged (as in F30d §8). New fixture
**`13-terms-keep-together`**:

- Content: the 06 header, items and total, then a body-level `<p class="break-inside-avoid text-xs">` of Czech terms
  ≈ 1.3 pages long (`text-xs` = 16 px lines, which tile, F78).
- Chrome reference committed; threshold = measured + 1 point.
- The PNG must show the terms starting at the top of page 2, every line once, and page 3 ending where Chrome's does. If
  Chrome starts the terms on page 1 (step 1), list the fixture in `PAGE_COUNT_MAY_DIFFER` naming F83b/F30c.

## 5. Steps

1. **Confirm** (scratch: our CLI on the local fork, with a temporary log in the strip slicer, removed afterwards):
   - `pbi_normal.html` gives 7 fragments from `:1638-1744`, 0 slices, and the skipped lines match the 22 words.
   - `page-break-inside`, `div > p{avoid}`, `div{avoid} > p` and the four-paragraph wrapper: all words, 0 warnings.
   - F83b probe: `<p>`s filling most of the page, then a `div{avoid}` of three short `<p>`s that fits a fresh page.
   - `<div style="height:1500px;background:#eee;break-inside:avoid">a<br>b<br>c</div>` with and without avoid (§6
     risk).
   - Chrome probes (Playwright, A4, 15 mm, Inter): cases 1–3 with and without `<p>before</p>`, giving the page of
     `word00000`, the page count, and the first word per page.

   If the fragments come from another emitter, stop and write `docs/postmortems/2026-10-17-f83-*.md`.
2. Ours: red `break_inside_avoid.rs` and the flipped `fallback_warning.rs`. Fork: red lib test and `render_smoke`.
3. Fork: `avoid_inside_holds`, its unit test, the call site and the comments. Then:
   - lib/smoke green;
   - `--lib` (the same 13 environment failures as at `e073b6b`) and every `tests/*.rs` run one by one;
   - VRT and the 21 examples byte-identical;
   - fmt and `clippy --all-targets -D warnings`.
4. Ours: lock bump, `cargo test --release --locked` green, fixtures 01–12 `cmp` and compare unchanged.
5. Fixture 13: HTML, README, Chrome reference (`scripts/reference/render.mjs`), compare, threshold, look at the PNGs.
6. Docs:
   - findings: F83 done, F83a and F83b filed;
   - `docs/upstream.md`;
   - §7.

   Then `vrt-validator`. `pdf-reviewer` is not required: the PDF writer path, fonts and metadata are not touched
   (`render.rs` unchanged).

## 6. Risks and non-goals

- An avoid text box taller than a page only because of `height` or padding now takes the `auto` line path, so its
  fragments are its lines. Step 1 compares; a difference from `auto`'s own handling is filed, not fixed here.
- A transformed avoid paragraph taller than a page used to be one fragment (`:1634-1637`). It now splits like a
  transformed `auto` paragraph does today at body level.
- The page count of a relaxed paragraph may differ by ±1 from the strip path. Not a fixture.
- Placement differs between paths: the body-level path moves to a fresh page; the nested and wrapper paths start in
  place (F83b).
- Non-goals:
  - F83a and F83b;
  - body-level placement of `auto` paragraphs (F30c);
  - `avoid-page` / `avoid-column` distinctions;
  - parsing `orphans` / `widows` (they stay at 2);
  - `box-decoration-break`;
  - F81 (`normal` = 1.2) and F82 (very long paragraphs are slow).

## 7. Result

Landed on branch `f83-break-inside-avoid`; fulgur fork `htmltopdf/srgb-colours` **`ebd78f5`** (on top of `ab9fe07`); the Blitz fork is unchanged (`52b22ef`). `Cargo.lock` pins `ebd78f5`.
Status: **done**.

**Step 1, the model held** (our CLI built against the local fork with a temporary log in the strip slicer, removed again; `pbi_normal.html`, 2500 words, Inter, A4, 15 mm,
`line-height: normal`): the `<p>` goes through the strip slicer at `:1638` (child 6816 px, fresh page 1009.1 px, 355 lines, edges at 993 / 2002 / 3011 / 4020 / 5030 / 6039 px),
7 fragments, 0 line slices, 7 renderer warnings. The edges straddle lines 51, 104, 156, 209, 261 and 314. Against the same paragraph without `avoid` (identical line wrapping), the 22
missing words are lines 103, 208 and 313 (7 words each, the line just above the straddled one at three of the six cuts) plus the one word of the last line (355): 21 + 1 = 22. 42 words
were drawn above the content top (the straddling line starting the next page). `pbi_1.3.html`: 8 pages, 7 edges, 21 missing. So the emitter is the strip slicer, as planned; no stop.
The other cases (before the fix, all green): `page-break-inside`, `div > p{avoid}`, `div{avoid} > p`, four 700-word paragraphs in an `avoid` div: all words once, 0 warnings.
F83b probe: confirmed (see findings). The `height:1500px` probe: the §6 risk is real, filed as **F83c** (below).

Chrome probes (Chromium Skia/PDF m141, A4, 15 mm, Inter; `line-height: normal` and `20px` give the same picture; 2500 words):

| Case | Chrome: page of `word00000`, pages | Chrome without `avoid` |
|---|---|---|
| 1 `<p avoid>` first | page 1, 8 pages; first word per page 0, 344, 694, 1045, 1409, 1761, 2113, 2464 | identical |
| 2 `<p>before</p><p avoid>` | **page 2**, 9 pages | page 1, 8 pages (the paragraph starts below `before`) |
| 3 `<div avoid>` direct text, first / after `<p>before</p>` | page 1, 8 pages / page 2, 9 pages | page 1 / page 1 |
| `<div><p>before</p><p avoid>` | page 2, 9 pages (ours: page 1, F83b) | |
| four-paragraph `avoid` wrapper | starts in place, same as without | |
| spacer 930 px + `div{avoid}` of three short `<p>` (F83b) | moves whole to page 2 (ours: splits in place) | splits in place |

So Chrome breaks a too-tall `avoid` box between lines like an `auto` one and moves it to a fresh page when something precedes it, as §2 said. With `line-height: 20px` our new first-word-per-page
list is Chrome's up to one line on the later pages (case 1: 0, 344, 694, **1044**, **1408**, 1761, 2113, 2464 against 1045 and 1409; case 2: 0, 351, 701, **1051**, **1415**, 1768, 2121, 2471 against 1052 and
1417: widows / orphans placement differs by one line at two breaks). With `normal` the page counts differ from Chrome's (7 / 8 against 8 / 9): F81, not this finding.

**Red before** (main at `c6fdadb`, fork `ab9fe07`):
- ours `break_inside_avoid.rs`: 3 of 8 red: `a_body_paragraph_taller_than_a_page_loses_no_word` (printed 2473 of 2500), `a_paragraph_taller_than_a_page_after_content_starts_on_the_next_page`
  (2473 of 2500; `word00000` was already on page 2), `a_body_div_with_direct_text_taller_than_a_page_loses_no_word` (2480 of 2500); the other 5 are guards, green before and after;
- ours `fallback_warning.rs`, flipped: red (the warning fired);
- fork lib: `avoid_paragraph_taller_than_a_page_splits_like_an_auto_one` and `avoid_div_with_direct_text_taller_than_a_page_splits_like_an_auto_one` (0 slices for 4 fragments); the guards
  `avoid_paragraph_that_fits_a_page_moves_whole`, `avoid_paragraphs_in_a_div_keep_their_tiling_slices` and the unit test of `avoid_inside_holds` green;
- fork `render_smoke`: `render_smoke_an_avoid_paragraph_over_many_a4_pages_loses_no_line` red (6920 of 7000 words).

**Green after:** all of them.

**After:**

| Check | Result |
|---|---|
| fork `--lib` | 2041 passed, 13 failed on crates.io Blitz: the same 13 as on `e073b6b` / `ab9fe07`; with the local Blitz fork path (`/home/user/blitz`, `52b22ef`) 2054 passed, 0 failed (the 5 new tests included) |
| fork `render_smoke` | crates.io Blitz: 250 passed, 3 failed (the same `outside_*_markers`, `table_parts_with_a_non_table_display`); local Blitz: 253 passed, 0 failed |
| fork other test binaries | all 42 `tests/*.rs` run one by one (with the local Blitz): green; `break_inside_avoid.rs` 3 passed |
| fork `fulgur-vrt` | green, goldens byte-identical, nothing re-blessed |
| fork examples | all 21 example PDFs rendered with the debug CLI before (`ab9fe07` plus stash) and after: byte-identical |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked` | 324 passed, 0 failed (8 new `break_inside_avoid.rs`, `fallback_warning.rs` flipped, `fixtures.rs` renders 13 strict); `cargo fmt --check`, `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-12 | `cmp` byte-identical to the render of main's binary (`/tmp/claude-0/vrt-f83/htmltopdf-base`, main `c6fdadb`, fulgur `ab9fe07`): 12 of 12; `compare.py` numbers unchanged: 4.4, 10.2/11.4/9.8, 5.7, 5.7, 21.6, 15.9/11.8, 7.2/7.1, 13.2/12.1, 5.1/4.9/0.7, 5.9/1.5, 6.9, 14.6/7.2 %; page counts unchanged |
| fixture 13 | new: **3 pages like Chrome, 1.9 / 11.7 / 1.9 %**, threshold 0.127 in `compare.py` (measured 11.7 % + 1 point) |

Fixture 13 (`out/compare/13-terms-keep-together-p1.png` .. `-p3.png`, git-ignored): page 1 is the invoice head, items and total (1.9 %); the terms start at the top of page 2 like Chrome's; page 2 breaks after the
same word as Chrome's ("... proti pohledávkám") and page 3 holds the same nine lines; every line once. The 11.7 % of page 2 is text drift (Chrome and we put the same words on every line; a line of vertical
drift part-way down a dense page). It is not in `PAGE_COUNT_MAY_DIFFER`: Chrome also starts the terms on page 2.

Deviations from the plan:

1. **Acceptance 4, the wrapper test**: "four `<p>` of 700 words each (about 2.5 pages together)": 2800 words are about 8 pages together; the test uses four paragraphs of 700 words, each a few pages long.
2. **Fixture 13 text**: the 27 clauses of fixture 12 plus ten new clauses (37, 8276 characters), one `<p>` with the clause numbers inline (no `<h3>`, so no heading is left alone on page 1).
3. **`avoid_paragraph_that_fits_a_page_moves_whole`** asserts page index 1: "one fragment, on page 1" in §4 means the second page.
4. **Chrome reference** rendered with the scratch copy of `render.mjs` (it hard-codes Playwright's install path); `scripts/reference/render.mjs` cannot resolve `playwright` from the repo in this environment. Output format is the same.
5. **Fork tests run against the local Blitz fork** as well as crates.io Blitz (the 13 + 3 failures are Blitz-fork tests that crates.io Blitz fails); the counts above give both.
6. **Comments rewritten** at the gate (`:1463`) say what is true (the gate does not read `break-inside`, F83b), not what the old comment claimed.

Filed: **F83a**, **F83b**, **F83c** in `2026-10-15-m0-findings.md`. **F83c is a visible regression for one shape**: an `avoid` box whose height exceeds a page only through `height` and that holds two or more
lines (`<div style="height:1500px;background:#eee;break-inside:avoid">a<br>b<br>c</div>` after content) used to start on page 2 and span pages 2-3 like Chrome; it now takes the `auto` line path (as §6 allowed)
and is cut at the bottom of page 1 without its remainder (one page; no text lost; an `auto` box of that shape is drawn the same way). Not fixed: the plan says file it.

Not verified here: `vrt-validator` (not run); `pdf-reviewer` (not required, `render.rs` unchanged); the spec citations (§2, from memory, `css-spec-researcher` not run); a transformed avoid paragraph taller than a page
(§6 risk, no probe); `avoid` paragraphs with inline boxes (`<img>`, `inline-block`) across the cut (the code path is `fragment_inline_root`'s F32a one, no avoid-specific test); right-to-left text; timing (not measured after the change: the line path is the one an `auto` paragraph already takes; `pbi_normal.html` took 55 ms before).
