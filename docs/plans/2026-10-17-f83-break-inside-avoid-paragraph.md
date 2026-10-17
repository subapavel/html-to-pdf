# F83 — A `break-inside: avoid` paragraph taller than a page loses words

Date: 2026-10-17. Finding: F83 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix goes on `subapavel/fulgur`
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

(Filled in at landing: the step 1 numbers and the Chrome probes; red before and green after, per test; the fork commit
and the `Cargo.lock` pin; the check table as in F30d §7; fixture 13 numbers and PNGs; deviations from the plan.)
