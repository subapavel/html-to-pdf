# F161 + F165 — multicol loses words at a column end, around `column-span: all`, and before a `display: contents` sibling

Date: 2026-10-21 (rewritten 2026-10-26 after the first copy was lost with its container). Findings: F161 (HIGH,
silent) and F165 (medium-high) in `2026-10-15-m0-findings.md`. Status: **done** in
`subapavel/fulgur` `htmltopdf/srgb-colours` `5aa953f6` + `1c1c164b` (on `182ffb73`); our part (lock, tests, fixture 24, fuzz
floor) on branch `f161-multicol-lost-words`.

## 1. Problem

A two-column terms block can print a fraction of its text, with no warning:

- **Wrapped paragraph after a short one.** `<div style="columns:2"><p>w0009</p><p>w0014 … w0020</p></div>` prints 4
  of 8 words (2 of 8 in the fuzz test's CSS). A paragraph that starts in a column with less than one line of room is
  cut after its first line by `slice_lines_with_first_col_remainder`; the distribution in `layout_column_group` then
  moves that slice to the next column, where the slice holding the rest overwrites it in `by_col`.
- **`column-span: all` across pages.** `<p>top</p>` + 120 `<p>` + `<h2 style="column-span:all">` + 120 `<p>` in
  `columns:2` prints 180 of 243 words (171 of 241 in the finding's CSS). A container with a spanner never reached
  the F30d fragmenter (F30d-b), so it kept the strip path.
- **F165.** `<div><div style="columns:3">…12 p…</div><div style="display:contents"><p>Q</p></div></div>` puts Q at
  295 pt instead of 109 pt. `propagate_height_delta` shifts the boxes of the later siblings, and a contents sibling
  has none.

The F150 fuzz test skips its 97 % floor for documents with a multicol container because of this. Chrome prints every
word of every shape above.

## 2. Spec (from memory, unverified)

- css-break-3 §4.1 / §4.4: breaks inside a paragraph fall between line boxes; a line that does not fit what is left
  of a partly filled column goes to the next column. Only an empty fragmentainer must take an oversized line.
- css-multicol-1 §6.1 `column-span: all`: the content before a spanner is balanced in the column row above it, the
  content after it starts a new row below it. §7.1 / §8.2: in a fragmented context only the last fragment is
  balanced; columns that do not fit continue on the next page.
- css-display-3 §2.5: a `display: contents` element generates no box; its children are boxes of its parent.

## 3. Where it belongs

fulgur (Blitz and Taffy do not lay out multicol; fulgur does), our fork, branch `htmltopdf/srgb-colours`. No
upstream PR or issue (standing decision).

| Fork file | What changed |
|---|---|
| `multicol_layout.rs` | `layout_column_group`, `slice_lines_with_first_col_remainder`, new `merge_line_slices`; `MulticolGeometry.segments` / `SegmentGeom`; `propagate_height_delta` calls the new `shift_box_roots` |
| `pagination_layout.rs` | `McPage.{group, y}`, `McContainer`, `multicol_container`, `fragment_multicol_container`, new `McEnv`, `McFragment`, `record_multicol_row` |
| `convert/mod.rs` | `convert_multicol_paragraph_slices`, `convert_paged_multicol_slices` |

Our side: `Cargo.lock`, tests, fixture 24, `compare.py`. No library code.

## 4. Acceptance

**(a) Fixture `fixtures/invoices/24-columns-span/`** (`index.html`, `README.md`, `reference/` from
`scripts/reference/render.mjs`). A4, 15 mm, `lang="cs"`, "Faktura 2026-0024"; header, parties, a short items table
and the total as in fixture 12 (classes only); every multicol rule inline or in the fixture's `<style>`, no
Tailwind rebuild. Under the total a `<section class="text-xs" style="columns:2;column-gap:24px">` of Czech clauses
(`<h3 style="margin:0">` + `<p style="margin:0">`):

- group A: clauses 1–6, one of them a one-line paragraph followed by a 50-word one (the F161 shape), balanced above
- spanner 1: `<h2 style="column-span:all;margin:0;padding:6px 0;border-bottom:1px solid #d1d5db">Reklamační řád</h2>`
  on page 1 (spacing as padding: spanner margins are dropped, risk 3);
- group B: clauses 7–23, crossing the page break (the multi-page span-all path);
- spanner 2 "Ochrana osobních údajů" on page 2 (a continuation page with two column rows, `McPage.y > 0`);
- group C: clauses 24–27, balanced as the last group;
- after the section, the F165 shape: `<div style="display:contents"><p class="text-sm mt-4">…signature…</p></div>`.

No bare text or `<div>` of text in the section (that sends the container to the old path, review point 3). Tune
clause lengths against Chrome: exactly 2 pages, spanner 1 on page 1, group B crossing the break, spanner 2 on page 2;
`PAGE_COUNT_MAY_DIFFER` stays empty. A test in `fixtures.rs` asserts every clause heading and both spanners in
source order, the last word of every clause, the signature, and 2 pages. `compare.py`: threshold = worst page + 1.

**(b) Red tests, `crates/htmltopdf/tests/multicol_lost_words.rs`** (own Inter / strict render and `pdftotext -bbox`
helpers, like `multicol.rs`). Measured with the CLI on main's binary (`182ffb73`) and on the branch:

| Test | Main | Branch |
|---|---|---|
| `a_paragraph_after_a_one_word_paragraph_keeps_its_words` | 4 of 8 | 8 |
| `the_column_break_may_fall_after_any_line_of_a_paragraph` (k = 0..16) | k = 7: 29 of 32 | all |
| `a_column_span_all_heading_in_a_container_over_two_pages_keeps_every_word` | 180 of 243 | 243 |
| `a_column_span_all_heading_on_one_page_keeps_every_word` | guard | guard |
| `a_contents_wrapper_after_a_multicol_container_is_placed_under_it` | 295 pt vs 109 pt | equal |
| `a_long_multicol_container_before_a_contents_wrapper_keeps_its_words` | guard (228, the finding's CSS lost 120) | 228 |

**(c) Fuzz floor.** `display_contents.rs`: the `!has_columns &&` exemption of the 97 % floor and its comment go;
`has_columns` still gates the container. Green at the CI's 64 seeds, once at `DISPLAY_CONTENTS_FUZZ_SEEDS=200` (the
finding's seeds 141 and 157), and `--ignored fuzz_long` with only the known F166 seeds failing.

**Pixels.** Fixtures 01–23 byte-identical to main's binary (only 12 has multicol, its plan is unchanged).

## 5. Steps

Steps 1–5 are **implemented in fulgur 5aa953f6**.

1. **One column decision.** The first line is exempt from the column budget only in an empty column;
   `DistItem::InlineRootSlice.col` carries the slicer's column to the distribution; a second slice of one paragraph
   in a column is merged (`merge_line_slices`) instead of overwriting `by_col`. Fork tests:
   `slice_lines_with_remainder_partly_filled_column_does_not_force_a_line`, `layout_column_group_never_drops_lines`,
   render_smoke `multicol_paragraph_at_a_column_end_loses_no_words`.
2. **Segments.** `MulticolGeometry.segments: Vec<SegmentGeom>` (`Group(usize)`, `SpanAll { node }`) in source order.
3. **Fragmenter over segments.** `multicol_container` accepts spanners when every group has a non-empty flow;
   `fragment_multicol_container` plans each group from the room under the previous segment (balanced when it is not
   the last), places a spanner whole, moves it to the next page or cuts it like a block; `record_multicol_row`
   records `McPage.group` and `McPage.y`. Fork tests: `multicol_span_all_is_cut_over_pages`,
   `multicol_span_all_taller_than_a_page_is_cut`, render_smoke `multicol_span_all_over_pages_loses_no_words`.
4. **Convert across groups.** `convert_paged_multicol_slices` looks up `groups[mc_page.group]` per row, `top = y`.
5. **F165.** `shift_box_roots` is called from `propagate_height_delta` for every later sibling; for a
   `display: contents` sibling it shifts the children. Fork test:
   `propagate_delta_shifts_children_of_a_contents_sibling`.
6. **Ours:** lock at `5aa953f6`; (b) and (c); fixture 24 (Chrome first); fixtures 01–23 `cmp` against main's
   binary, `compare.py`, `bench.py --check`; fork `--lib` and `render_smoke`; findings and `docs/upstream.md`;
   `vrt-validator`, `pdf-reviewer` (convert is on the paint path; check fixture 24 tagged).

## 6. Risks and non-goals

1. Every multicol container that fits its page runs the new slicer rule: a paragraph starting within a line of a
   column end may move. Watch fixture 12 and the fork's multicol goldens.
2. Containers with a spanner now take the fragmenter's F30d-b limits: a block taller than a column is placed whole,
   no margins inside columns, `column-rule` approximate. Before, the fulgur-916y block-recursion path could split a
   tall `<div>` in such a container: run `display_contents.rs` C17 and the 200-seed fuzz; if words are lost, have
   `multicol_container` return `None` when a group holds a block taller than a page.
3. Spanner margins are dropped (the height is the border box in both passes); Chrome keeps them. File under F30d-b.

Non-goals: F30d-b (other than the spanner across pages), F30d-c, F30d-d, nested multicol, multicol in flex / grid /
table cells.

## Review points on 5aa953f6

1. `merge_line_slices` runs behind `debug_assert!(false)` and our tests run `--release`: a merge there is silent,
   and the union range is right for contiguous ranges only. Same for `debug_assert!(*col >= col_idx)`.
2. Bare text plus a spanner: `multicol_container` rejects the container when a group's flow is empty, and the old
   path loses words over pages. A container of spanners only (`groups.is_empty()`) also takes the old path.
3. F165 edge cases: `shift_box_roots` follows DOM children; a multicol that is itself inside a contents wrapper
   (`<w><div columns>…</div></w><p>Q</p>`) and siblings in anonymous blocks are untested.
4. Determinism: only `BTreeMap` / `BTreeSet` and source-order `Vec`s are added, fixed tolerances.

## 7. Result

- Fork `5aa953f6` (steps 1–5) and `1c1c164b` (review fixes, below); lock bumped from `182ffb73`.
- `cargo test` found the risk 2 regression: `display_contents.rs` C17 ("span-all in a wrapper, then a long
  paragraph", a `<div>` a page and a half tall under a spanner) lost 44 of 300 words. A 2000-seed comparison of
  the `fuzz_long` documents against main's binary found one more (seed 2434: a `<div>` shorter than a page in the
  last group under a spanner was put whole in the second column and ran six lines past the page bottom, 257 of 263
  words; main 263). `1c1c164b`: `fill_multicol_columns` lets the first atom of a column overflow only on a fresh
  page or as the page's first atom (on a partial page it ends the page), and a container with spanners whose flow
  holds a box taller than a page keeps the old path. Fork tests: a planner unit test, the tiny-room test updated
  to the rule, a `render_smoke` case.
- After the fix, all 2200 fuzz documents (the 2000 of `fuzz_long` and the 200 of the CI generator) against main's
  binary: **0 print fewer words, 33 print more**. Still lossy on both binaries alike (pre-existing, filed under
  F173 (e)): seeds 134 and 153 of the CI generator, 1590, 1726, 1823 (F166) and 1908 of `fuzz_long`.
- Shapes of §4 (b) on main's binary / the branch: 4 / 8, 29 / 32 (k = 7), 180 / 243, 295 pt / 109 pt.
- Fixtures 01–23 byte-identical to main's binary; fixture 24 prints 948 of 948 words (main 862), 8.6 % worst
  page, 2 pages like Chrome. Fork: 2232 lib tests, 283 `render_smoke`, clippy clean; of the fork's 64 VRT goldens
  the same 26 differ byte-wise on `182ffb73`, `5aa953f6` and `1c1c164b` in this container (environment), and their
  output is byte-identical between `5aa953f6` and `1c1c164b`.
- Fork tests are run with the Blitz and Taffy forks patched in on the command line
  (`cargo test --config 'patch.crates-io.blitz-dom.git="https://github.com/subapavel/blitz"' --config
  'patch.crates-io.blitz-dom.branch="htmltopdf/blitz-dom-0.2.4"' …`, the same for `blitz-html` and `taffy`); the
  fork's own `Cargo.toml` and `Cargo.lock` are not changed.
