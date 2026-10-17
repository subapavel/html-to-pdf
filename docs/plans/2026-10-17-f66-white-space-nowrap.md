# F66 — `white-space: pre` / `nowrap` still wrap

Date: 2026-10-17. Finding: F66 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` (`layout/inline.rs`, `layout/construct.rs`, `node/element.rs`, `stylo_to_parley.rs`,
`assets/default.css`). No fulgur, Parley or Stylo change.

## 1. Problem

Probe `/tmp/claude-0/vrt-f32a/p/a9.html` (A4, 15 mm). Ours (`a9.new-1.png`): the `white-space:pre` paragraph soft-wraps
after eight "pre long line"s but "second" starts its own line — **preserved newlines already work, only soft wrapping is
wrong**; the `nowrap` paragraph wraps into four lines; `pre-wrap` wraps (correct). Chrome (`a9.chrome-1.png`) keeps `pre`
and `nowrap` on single lines, but its whole page is shrunk because the line is wider than the page (Blink print
shrink-to-fit, F32a §7), so a9 is not a pixel reference. Root cause, from the code:

1. Stylo 0.8.0 computes the `white-space` shorthand (`shorthands/inherited_text.mako.rs:109-115`) into
   `white-space-collapse` and `text-wrap-mode` (`longhands/inherited_text.mako.rs:145-155`, `:393-401`). Blitz reads only
   the collapse mode (`stylo_to_parley.rs:94-103`; `construct.rs:802-807`, `:870-876`); nothing in `blitz-dom` reads
   `text_wrap_mode`.
2. Parley 0.6.0 has no wrap-mode style property (`style/mod.rs:22-44`: `WhiteSpaceCollapse {Collapse, Preserve}`,
   `OverflowWrap`, `WordBreakStrength`).
3. Blitz `layout/inline.rs:149,158` calls `break_lines(layout, width)` (`:21-32`, F32a) with the box width, so lines break
   at every soft wrap opportunity; min-content (`:106-114`) is Parley's `calculate_content_widths().min`
   (`data.rs:509-568`), which breaks at every `Boundary::Line` — so a `nowrap` table cell shrinks to its longest word and
   the IBAN / amount wraps.
4. `PreserveBreaks` (`pre-line`) and `BreakSpaces` map to `Preserve` (`stylo_to_parley.rs:99-101`), so `pre-line` keeps
   spaces and indentation instead of collapsing them.
5. `text-overflow` is gated by `servo_pref="layout.unimplemented"` (`longhands/text.mako.rs:8-17`), false in
   `stylo_static_prefs` 0.8.0, so it is never parsed; the `text-wrap` shorthand (Tailwind v4 `text-nowrap`) is
   `engines="gecko"` (`shorthands/inherited_text.mako.rs:50-52`), dropped at parse time.
6. `<td nowrap>` is ignored: Blitz `assets/default.css` has no `td[nowrap]` rule.
7. fulgur draws Parley's lines as Blitz positioned them; nothing in `crates/fulgur/src` breaks lines, and
   `overflow:hidden` already clips (`convert/style/overflow.rs:19`, `render.rs:982-999`).

## 2. Spec (from memory, wording unverified; `css-spec-researcher` not run)

- CSS Text 3 §3 (`white-space`) and CSS Text 4 `text-wrap-mode`: `nowrap` and `pre` have no soft wrap opportunities;
  forced breaks (preserved segment breaks, `<br>`) still break. CSS 2.1 §16.6 gives the same table.
- CSS Text 3 §4.1: `pre-line` collapses spaces and tabs, keeps segment breaks, removes spaces around them;
  `break-spaces` wraps after preserved spaces.
- CSS Text 3 §7.1: a line too long for its box is start-aligned and overflows its end edge.
- CSS Sizing 3 §5: min-content takes every soft wrap opportunity, so under `nowrap` it equals max-content. CSS 2.1
  §17.5.2.2 (non-normative): a column's minimum is its cells' min-content.
- HTML §15.3.8: the UA sheet has `td[nowrap], th[nowrap] { white-space: nowrap }`.
- css-overflow-3 §3: `overflow: hidden` clips at the padding box; css-overflow-4 / css-ui-3 define `text-overflow`.

## 3. Design

| Item | Where | Change |
|---|---|---|
| nowrap flag | Blitz `node/element.rs` `TextLayout` | `pub nowrap: bool` (default `false`); only constructor `TextLayout::new` (`construct.rs:282`). |
| set it | `construct.rs` `build_inline_layout_into` | Read `text_wrap_mode` from the inline root's style with the parent fallback of `:784-788`: `fn text_wrap_nowrap(style: &ComputedValues) -> bool` in `stylo_to_parley.rs`. Per IFC root (§1 item 2). |
| break | `layout/inline.rs` `compute_inline_layout` | Break width `f32::MAX` when `nowrap` (as Parley's `break_all_lines(None)`, `layout/mod.rs:168-170`); `width` still used for sizing and `align(Some(width), …)`, so an overflowing line is start-aligned (§7.1). F32a's revert check never fires. |
| min-content | same, `:106-114` | `nowrap` → `min = max` (widens `nowrap` table columns, inline-blocks, flex items). |
| `pre-line` | `construct.rs` text-node push (`:1004-1013`) | Pass the raw Stylo collapse value down; for `PreserveBreaks` split on `'\n'` and push segments with `Collapse` and `"\n"` with `Preserve` between them (as the `<br>` path `:923-932`); Parley's collapse trims spaces after a newline (`resolve/tree.rs:96-105`). |
| `td[nowrap]` | `assets/default.css` | `td[nowrap], th[nowrap] { white-space: nowrap; }` |

## 4. Acceptance

`crates/htmltopdf/tests/white_space.rs` (pdftoppm at 96 dpi, Inter 14 px, `line-height:20px`, text in a
`<div style="width:300px">` so it overflows inside the page box and Chrome does not shrink; "rows" are clusters of dark
pixels; a 10 px blue marker last). Declarations are Tailwind v4's own written inline (`whitespace-nowrap`,
`whitespace-pre`, `whitespace-pre-line`, `truncate`), as the compiled `tailwind.css` contains none of them.

1. `nowrap_paragraph_overflows_on_one_line`: ≈ 450 px of text → 1 row, right edge > 57+300+100. **Red** (3 rows).
2. `nowrap_cell_widens_its_column`: `table{width:400px}`, column 1 a long sentence, column 2
   `CZ65 0800 0000 1920 0014 5399` with `nowrap`: the IBAN is 1 row, inside the table, x ±2 of Chrome's (step 0). **Red.**
3. `pre_keeps_newlines_and_does_not_wrap`: `<pre>` with a 450 px line, `\n`, a short line → 2 rows; row 1 right edge
   > 357. **Red** (3 rows).
4. `pre_wrap_wraps_and_keeps_newlines`: same text → 3 rows, all right edges ≤ 357. Guard.
5. `pre_line_collapses_indentation_and_keeps_newlines`: `"\n    Firma s.r.o.\n    Ulice 1\n"` → rows with left edges
   57 ±1. **Red expected** (else a guard).
6. `truncate_clips_at_the_box`: 1 row, no dark pixel right of 357. **Red** (wraps). Ellipsis not asserted (F66c).
7. `td_nowrap_attribute_keeps_the_cell_on_one_line`. **Red.**
8. `nowrap_still_breaks_at_br_and_preserved_newlines`: `a<br>b` → 2 rows. Guard.

Fork tests (fulgur `blitz_adapter.rs` `mod tests`, next to F32a's `lines_around_img`):
`nowrap_root_breaks_only_at_forced_breaks` (1 / 2 / 2 lines for long text, `<br>`, `pre` with `\n` at 400 px);
`nowrap_min_content_is_max_content` (an `inline-block` in a 100 px box is as wide as its text);
`pre_line_text_has_no_indentation` (IFC text `"Firma s.r.o.\nUlice 1"`). `fulgur-vrt` goldens byte-identical.

Fixtures 01–11 byte-identical (`cmp`); compare numbers unchanged: `fixtures/invoices/*/index.html` contain no
`whitespace-`, `truncate`, `nowrap`, `<pre` or `white-space`. No new fixture (none moves; one would need a Tailwind
rebuild).

## 5. Steps

0. Chrome 141 probes (A4, `@page{margin:15mm}`) in `out/scratch/f66/`: P1–P8 = tests 1–8 standalone; P9
   `text-align:right; white-space:nowrap` overflowing (start-aligned?); P10 `truncate` on a `<td>` of an auto-layout
   table (widen or clip?); P11 `text-nowrap`. Record in §7.
1. Ours: `white_space.rs` tests 1–8, red / green as listed.
2. Fork tests (fulgur `blitz_adapter.rs`), red.
3. Blitz: `nowrap` flag, break width, min-content → tests 1, 2, 3, 6 and the first two fork tests green.
4. Blitz: `pre-line` → test 5 and its fork test green.
5. Blitz: `td[nowrap]` → test 7 green.
6. Ours: lock bump, `scripts/render-fixtures.sh`, `cmp` 01–11, compare, PNGs, `vrt-validator`; `docs/upstream.md`;
   findings F66b–F66e; §7.

## 6. Risks and non-goals

- The flag applies to the whole IFC: a `white-space:normal` span inside a `nowrap` root will not wrap (rare).
- A very wide `nowrap` / `pre` line paints past the page edge instead of shrinking the page (deliberate, F32a §7).
- Taffy grid gives an `overflow:hidden` item an automatic minimum of 0, so a `truncate` cell may clip where Chrome widens
  the column (P10); if so, record it as F66e. F66 does not change table sizing otherwise.
- Non-goals, each a finding: **F66b** `nowrap` on a span inside a wrapping paragraph (needs a wrap mode in Parley — a
  later release or a Parley fork the owner must create; NBSP / word-joiner substitution rejected, it changes the
  extracted text); **F66c** `text-overflow: ellipsis` (Stylo pref off; fulgur would have to draw it); **F66d** the
  `text-wrap` shorthand (Tailwind `text-nowrap`; polyfillable in `htmltopdf` like F8 by rewriting to `text-wrap-mode`);
  `break-spaces` hanging rules; the trailing space before a `pre-line` newline.

## 7. As landed

### Chrome probes (step 0), A4, `@page { size: A4; margin: 15mm }`, Inter 14 px / 20 px, `out/scratch/f66/p*.html`

(The first run without `size: A4` gave Letter pages; the numbers below are A4.) Rows are clusters of dark pixels at 96 dpi,
x in px from the page edge; the content box starts at 57. Ours is the final binary.

| Probe | Chrome 141 | Ours before | Ours after |
|---|---|---|---|
| P1 `nowrap` 300 px box, 418 px text | 1 row, x 57-475 | 2 rows, 57-352 / 57-173 | 1 row, 57-475 |
| P2 table 400 px, long sentence + `nowrap` IBAN (red) | IBAN 1 row, 233-455 | 2 rows, 277-425 / 277-345 | 1 row, 232-454 |
| P3 `<pre>` long line + `short` | 2 rows, 57-475 / 58-90 | 3 rows | 2 rows, 57-475 / 57-89 |
| P4 `pre-wrap` | 3 rows, first ends 352 | same | same (guard) |
| P5 `pre-line` `"\n    Firma s.r.o.\n    Ulice 1\n"` | 2 rows, left 58 / 58 (the leading newline is an empty line) | left 73 / 73 | left 58 / 58; marker 20 px lower than Chrome (F66f) |
| P6 `truncate` | 1 row, ends 347 with `…` | 2 rows | 1 row, ends 352, no ellipsis (F66c) |
| P7 `<td nowrap>` | as P2 | as P2 before | as P2 after |
| P8 `a<br>b` nowrap / `a\nb` pre | 2 rows | 2 rows | 2 rows |
| P9 `text-align: right` + `nowrap`, overflowing | 1 row, 57-475 (start-aligned, CSS Text 3 §7.1) | wraps | 1 row, 57-475 |
| P10 `truncate` + `max-width: 100px` on a `td` of an auto table | clipped at the cell, ellipsis | clipped, wrapped | clipped, no ellipsis |
| P11 `text-wrap: nowrap` | 1 row | 2 rows | 2 rows (F66d) |
| P12 `overflow: hidden` + `nowrap` | 1 row, ends 352 | 2 rows | 1 row, ends 352 |
| P13 plain `truncate` cell (no `max-width`), auto table | the column widens to the text: red 319-736 | clipped | still clipped at about 295 px (F66e) |
| P14 same, `width: 400px` | the table grows to about 500 px, red 134-552 | clipped | clipped at 160 px (F66e) |
| P15 `pre` `"a\nb\n"`, P16 `a<br>b<br>` | 2 rows, marker at y 97 | marker at y 117 | marker at y 117 (F66f, pre-existing) |

P10 and P13 answer the plan's question (clip or widen): Chrome widens a `truncate` cell unless it has a `max-width`; we clip
(F66e), as predicted in §6. The pre-line left edge in Chrome is 58, not 57 (the bearing of `F` and `U`), so test 5 asserts
58 +- 1 instead of the plan's 57 +- 1.

### Commits

| Repo | Commit | What |
|---|---|---|
| `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` | `aebe118` | `TextLayout::nowrap`, unbounded break width, min = max under `nowrap`, `pre-line` collapse with explicit newlines, `td[nowrap]`; one commit (steps 3-5 share `construct.rs`) |
| `subapavel/fulgur` `htmltopdf/srgb-colours` | `f7e3104` | tests only: 4 in `blitz_adapter.rs` |
| ours | this branch | `white_space.rs`, lock bump to both, docs |

### Design as built

- `nowrap` is `TextLayout::nowrap` (`node/element.rs`), set in `build_inline_layout_into` from the inline root's computed
  `text-wrap-mode` (`stylo_to_parley::text_wrap_nowrap`), before `collapse_mode` consumes the style. `TextLayout` derives
  `Default`, so `TextLayout::new` needed no change; the flag is rebuilt on every construction, so a cached layout cannot go stale.
- `compute_inline_layout`: `break_lines(layout, width, nowrap)` uses `f32::MAX` when set (the width still sizes the box and
  aligns with `align_when_overflowing: false`, so an overflowing line is start-aligned, P9). F32a's revert check never
  fires then. `calculate_content_widths().min` is replaced by `.max` under `nowrap`; `TextLayout::content_widths()` (unused
  by layout, kept consistent) does the same.
- `pre-line`: `white_space_collapse(PreserveBreaks)` now maps to Collapse. A text node whose parent is `PreserveBreaks` is
  split at `'\n'`; each newline goes through `push_forced_break` (the `<br>` path, now shared). A trailing space before the
  newline is kept (the plan's non-goal); the indentation after it is trimmed by Parley because the text before ends in
  whitespace. `BreakSpaces` is still Preserve.
- Plan deviation: nothing in `crates/htmltopdf` or fulgur proper changed; as the plan said.

### Tests

- Ours `white_space.rs`: 9 tests, **7 red before** (against `ee5477b`/`f5ef832`): 1, 2, 3, 5, 6, 7 and the added
  `nowrap_long_lines_on_both_sides_of_a_br_stay_two_rows` (the plan's test 8 asserts only `a<br>b` and `a\nb`, which already
  passed, so the two-long-lines case is a separate red test); **green guards** 4 (`pre-wrap`) and 8. The test file builds its own
  Inter renderer from `fixtures/fonts` (as `font_family.rs`) and rasterises with `pdftoppm`.
- Fork (`blitz_adapter.rs`): lib 2010 -> 2014; all four new tests red without the Blitz commit (the three planned plus
  `nowrap_attribute_on_a_cell_does_not_wrap`, which pins the UA rule: red with only `default.css` reverted). They use bundled Noto Sans
  (`examples/.fonts`), not host fonts. `render_smoke` 241, `fulgur-vrt` green with **no golden changed**, `clippy --workspace
  --all-targets -D warnings` and `fmt` clean. Blitz: `cargo test -p blitz-dom` 37 passed, fmt clean, clippy has only its 3
  existing warnings (none in touched lines).
- Ours: `cargo test --release --locked --no-fail-fast` green (every suite ok; 16 min wall, mostly the cold test build),
  `cargo fmt --check`, `cargo clippy --release --locked --all-targets -- -D warnings` clean.

### Numbers

- Fixtures 01-11: `scripts/render-fixtures.sh`, every PDF `cmp`-identical to the one rendered by the previous binary.
  `compare.py` unchanged: 4.4, 10.2/11.4/9.8, 5.7, 5.7, 21.6, 15.9/11.8, 7.2/7.1, 13.2/12.1, 5.1/4.9/0.7, 5.9/1.5, 6.9 %;
  pages 1,3,1,1,1,2,2,2,3,2,1 as before. No threshold touched.
- Time (release CLI, 12 interleaved runs, sorted, ms): fixture 02 before 42-67 (median 52), after 43-68 (median 51); a
  144-page text document (660 paragraphs) before 372-426 (median 408), after 343-434 (median 410), the two PDFs byte-identical.
  The `nowrap` check costs nothing measurable.

### Not verified

`vrt-validator` and `pdf-reviewer` (the caller runs them). A `nowrap` inline-block / flex item / grid item beyond the
`inline-block` unit test; `nowrap` inside list items, `::before` / `::after` content and `display: contents`; a very wide
`nowrap` line past the page edge (it paints into the margin, as designed); pagination of a `nowrap` line wider than the
page. The `white-space` keywords other than the ones probed (`break-spaces`). `css-spec-researcher` was not run; the spec
section references are from memory. The Stylo parse noise `ERROR: Unexpected token` printed by the fork tests is not from
these changes (not investigated).

### Review follow-up (`vrt-validator` on PR #28)

**H1, an explicit cell `width` below the min-content width (fixed).** Two causes in the Blitz fork, `layout/table.rs`: the grid
got the explicit length as a *fixed* track, and the cell's own `width` was also the size Taffy's leaf layout reported for its
min-content, so even a minmax track could not widen. Fix (`204c4c2`): (1) `TableTreeWrapper::compute_child_layout` hides a cell's
length `width` while the cell is measured (percentages stay, see F73), (2) `widen_explicit_columns` (called where the table's grid
is laid out) measures, in automatic layout, the min-content width of every cell that starts in and spans only a column with an
explicit length, and gives the column a fixed track of `max(width, that)`. The width remains the column's *preferred* width
(a 300 px column with `Short` stays 300 px), the fixed algorithm is untouched. A first attempt, `minmax(min-content, width)`
tracks, was dropped: Taffy shares free space equally among tracks below their limit, so the explicit columns no longer reached
their width and fixtures 02 (4 pages), 09 and 11 moved. A second attempt that hid the width of percentage cells too moved the
`multicol-table-width` golden (F73), so it hides length widths only.
- Chrome vs ours (`out/scratch/f66/h1a-h1f`, A4, Inter 14 px; x of the red `b` after the first cell): `<td nowrap width=80>` and
  `style="white-space:nowrap;width:80px"` 222 vs 221, long unbreakable word in `width:80px` 275 vs 274, 300 px column with
  short text 358 vs 357, `table-layout:fixed` 138 vs 137 (the text overflows in both), the later-row price below a header
  `width:96px`: no overlap (ours `x` 329, Chrome 407 because Chrome also spreads the spare width of the 400 px table, F15).
  `atk/uw1`, `uw2`, `r2` look like Chrome's; `atk/c5`, `vrt-f5/r1` no longer spill but the table stays narrower than Chrome's (spare width, F15).
- Tests, **red before** (against `aebe118`): ours `white_space.rs` +5, three red (`explicit_width_below_nowrap_text_widens_the_column`,
  `explicit_width_below_an_unbreakable_word_widens_the_column`, `nowrap_text_in_a_later_row_widens_the_column_of_a_header_width`)
  and two guards (`explicit_width_above_the_min_content_is_kept`, `fixed_layout_keeps_the_explicit_width_and_the_text_overflows`);
  fork `explicit_cell_width_never_goes_below_the_min_content` (red without the Blitz commit: a 477 px text in an 82 px cell).
- **Fixtures:** 01, 02, 04-11 byte-identical. **03 moved**: the `w-20` "Množství" header column was 80 px with its semibold text spilling into the
  padding, it is now 88 px (the cell's min-content), which is Chrome's width; the columns right of it shift by 8 px and the
  first column is 8 px narrower. Compare 03: 5.6837 % -> 5.6788 % (displayed 5.7 % both times, threshold 7 % unchanged), pages 1/1.
  No committed golden exists for the invoice fixtures. `fulgur-vrt` goldens: byte-identical (after the second attempt above).
- Time, release CLI: a 2000-row table with five explicit-width columns (34 pages): 367-419 ms (median 398) against 371-477 ms
  (median 425) before F66; no measurable cost. Tables without explicit lengths do no extra work.

**H2, content past the page edge (warned, F66g).** Decision and scope in the findings file. fulgur `140000c`: `Canvas::paint_extent`
(`PaintExtent`: rightmost x of glyphs, inline images and inline boxes via `paragraph::lines_right_edge` (trailing whitespace is
not ink), and boxes with a visible background or border via `render::note_painted_box`; nothing is recorded between
`push_clip_path` / `push_transform` and their pops, where x is not a page coordinate or the content is clipped anyway),
`render_v2` warns per page when the extent exceeds the page width by more than 0.5 pt. PDF bytes are unchanged (`fulgur-vrt`
goldens identical, fixtures `cmp`-identical, no fixture warns). Test `page_overflow_warning.rs` (red before: no warning; one
test, the logger is global): a 1500 pt `nowrap` line, a 1200 px box with a background, the second page only, identical
message on a repeat, and silence for wrapping text, short `nowrap`, a clipped line, a wide box in a clip, a line that overflows
the margin but not the page, a transparent box; strict images do not turn it into an error. Fork unit tests `PaintExtent`
(2). Not covered: inline-block content (drawn under a transform) and tables' own frame.

**Findings filed:** F66g, F70 (HIGH, `pre` / `code` text dropped), F71, F72, F73; notes under F66 (RTL, `<wbr>`), F66b (`<nobr>`,
`normal` span in a `nowrap` root) and F15. `<code>`, `<kbd>`, `<samp>`, `<tt>` and `<pre>` were all checked with the release CLI.

**Commits after the first landing:** Blitz `204c4c2`, fulgur `140000c`, ours: lock pins, tests, docs. Fork tests: lib 2014 -> 2017,
`render_smoke` 241, `fulgur-vrt` green, `clippy --workspace --all-targets -D warnings` clean; Blitz `cargo test -p blitz-dom` 37 passed.

**Not verified (follow-up):** `<table>` inside flex / grid items with explicit widths; a colspan cell wider than its column
(not measured, as designed); cells below the first row that have a length `width` in a column past the first row's width (their `width`
is now ignored while measuring); whether the page warning fires for content under `position: fixed` / `absolute`.
