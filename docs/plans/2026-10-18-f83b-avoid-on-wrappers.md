# F83b — `break-inside: avoid` on block wrappers: the recursion gate and the nested paths ignore it

Date: 2026-10-18. Finding: F83b in `2026-10-15-m0-findings.md`. Status: planned. Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after `659affe`), carried via `[patch.crates-io]`. All fork line numbers are in
`crates/fulgur/src/pagination_layout.rs` at `659affe`.

## 1. Problem

The usual invoice idiom is `<div class="break-inside-avoid">` around totals, payment details and signature. Since F87 the
class reaches the break table, but the fragmenter still splits such a wrapper in place. F83 step 1 confirmed two cases
(A4, 15 mm, Inter, Chromium m141):

- **Probe 1:** `<div style="height:930px"></div><div style="break-inside:avoid"><p>word00000 aaa</p><p>… bbb</p><p>… ccc</p></div>`.
  Chrome moves the div whole to page 2. We put `aaa` and `bbb` on page 1 and `ccc` on page 2.
- **Probe 2:** `<div><p>before</p><p style="break-inside:avoid">` with 2500 words. Chrome starts the paragraph on page 2
  (9 pages). We start it below `before` on page 1.

No text is lost. Causes, from reading:

1. **Body level.** The gate (`:1530-1549`) is `has_forced_break_below || has_page_name_change_below ||
   would_split_block_subtree`.
   - It never reads `break-inside`; the comment at `:1495-1500` says so.
   - `would_split_block_subtree` (`:2083-2151`) is true as soon as the children overflow the rest of the strip.
   - The F28 pre-advance (`:1563-1570`) moves the box only when its *first row* (`leading_block_size`, `:2176-2253`) does
     not fit. So the wrapper is entered and split between its children.
   - Only a box the gate skips reaches the "move whole" advance (`:1620-1623`).
2. **Nested** (`fragment_block_subtree`, `:2585`). Same gate (`:3062-3068`). `avoid_inside_child` (`:3085-3088`) feeds only
   the multicol test.
3. **Too-tall avoid boxes.** At body level a too-tall avoid box starts on a fresh page only as a side effect:
   - an inline root through the line-path pre-advance (`:1418-1421`);
   - a gate-false box through `:1620`.

   A wrapper the gate enters, and every nested avoid box (`first_break_h`, `:3132-3172`), starts in place.
4. **`leading_block_size` knows nothing of `avoid`.** Suppose a parent is entered at a page top because its first row
   "fits", and that first row holds an avoid box. Today the parent then relies on the recursion splitting that box. Once
   the box is kept whole, nothing checks it, so it would overflow. So (1) cannot be fixed in the gate alone.

## 2. Spec (cited from memory, unverified; css-spec-researcher not run)

- **css-break-3 §3.2** `break-inside: avoid`: avoid breaks inside the principal box.
- **§4.1 break points.**
  - Class A: between siblings.
  - Class C: between a container's content edge and its first or last child, *only if there is a non-zero gap*. Border
    and padding are not a gap.
- **§4.3 forced breaks.** Forced breaks are honoured whatever `break-inside` the ancestors have.
- **§4.4 unforced breaks.** Rule 2 says: no break inside a box whose `break-inside` is `avoid`. If the rules leave no way
  to fit the content, Rules 1, 2 and 4 are dropped before Rule 3 (orphans/widows); CSS 2.1 §13.3.3 has the same order.
  So an avoid box taller than a page breaks like `auto`.
- **§4.5 optimal breaks.** Prefer breaks that violate no rule. A break before the avoid box is a perfect class A point,
  so a box that does not fit the rest of the page breaks *before* it. That holds even when it is taller than a page:
  the break inside then happens on the fresh page, where no better point exists.
- **§5.2.** Margins adjoining an unforced break are truncated.
- **Chrome / LayoutNG** (break appeal: a break inside an avoid box scores below a perfect earlier break; unverified,
  agrees with the F83 probes). It does not break at the very start of a fragmentainer, so a box first on its page starts
  in place.

## 3. Where it belongs and design (fulgur fork, `pagination_layout.rs`)

**Where.** This is engine behaviour in fulgur's own fragmenter. The gate and `leading_block_size` are 0.40.0 code, so the
bug is upstream too.

- Blitz and Taffy lay out one tall page (ADR-0001), so pages are none of their business.
- Our crate changes only tests, fixture 14 and docs.
- No upstream PR or issue (standing decision).

**Scope (point 7).** Avoid boxes that are children of block flow: the body walk, and `fragment_block_subtree` when
`block_parent` (`:3084`: not flex/grid, not atomic inline, not orthogonal, not a table).

- The avoid box itself may have any display. Fixture 02's totals is a `flex` section in block flow.
- Flex/grid items, table cells and rows keep today's path (non-goals, §6). Moving one item would desynchronise
  `RowState`/`SplitRow`.

**New helpers.** Signatures only.

- `fn avoid_extent(doc: &BaseDocument, node: &blitz_dom::Node) -> f32`
  - An inline root with ≥ 2 lines: its line extent (F83/F83c rule).
  - Any other box: its border-box height.
- `fn avoid_keeps_whole(avoid_inside: bool, extent: f32, room: f32, fresh: f32, content_above: bool) -> bool`
  - `= avoid_inside_holds(avoid_inside, extent, fresh) && (extent <= room + 1.0 || content_above)`.
  - The box is kept whole when it fits a page, and either fits here or can move. A box first on a short strip (page 0
    below the body offset, under a repeated header band) that does not fit is entered as today. Chrome does not break at
    the start of a fragmentainer either.
- `fn avoid_piece_size(doc, node_id, column_styles, depth) -> f32`
  - Block-size from the top border edge to the first forced break inside: the top of a descendant with
    `break-before: page`, the bottom of one with `break-after: page`.
  - Returns the node's own height when there is none.
  - Walks like `has_forced_break_below` (`:2265`).
- `fn starts_with_relaxed_avoid(doc, node_id, column_styles, fresh, depth) -> bool`
  - True when the node, or its first in-flow descendant reached with no gap (the same walk as `leading_block_size`), has
    `avoid` and `!avoid_inside_holds(…, avoid_extent, fresh)`.
- `leading_block_size` gains `column_styles: Option<&ColumnStyleTable>`. A node (or first-row descendant) whose avoid
  holds counts `avoid_piece_size(..).min(own_h)`, not its first row. Call sites to update: `:1564`, `:2245`, `:3154`,
  `:4167`, tests `:9595`, `:10648`.
- `fragment_block_subtree` gains `content_above: bool`: in-flow content precedes `cursor_in` on `page_in`.
  - For a child: `(page_index == page_in && content_above) || cursor_before > page_start_y + band + 0.5`, where
    `cursor_before` is the cursor before `:3004`.
  - The parent's own border, padding and margin therefore never count as content.

**Body walk** (`fragment_pagination_root`).

- `content_above = cursor_y - gap > 0.5`: the cursor before this child's own margin gap. After a page advance it is 0.
- **(1) Fits a fresh page.**
  - When `avoid_keeps_whole(…, room = strips.bottom(page_index) - cursor_y, …)`, the gate drops its
    `would_split_block_subtree` term.
  - The box then takes the existing advance (`:1620`) and the whole-emit (`:1803`).
  - Not when it is first on its page: `content_above` is false and it fits, or it is entered as today.
- **(2) Taller than a page.**
  - The rule: after the line path (`:1468`) and before the gate, if `content_above && starts_with_relaxed_avoid(child)`,
    advance one page and recompute the gate. The box is then entered from the top, like `auto`.
  - It sits after the line path, so inline roots (F83, fixture 13) keep their path byte for byte.
  - A height-only box (F83c) already advances at `:1620`; it reaches the same strip slicer.
- **(4) Forced break inside an avoid box that holds.** The gate keeps `has_forced_break_below`, so the forced break wins.
  F28's `leading_block_size` now returns the piece before the first forced break: the box moves whole to the next page
  when that piece does not fit here. After the forced break the rest breaks like `auto`, because no better break point
  exists at the top of a page. Step 1 probes this; if Chrome splits in place, `avoid_piece_size` is dropped and the
  holding box counts `own_h` only when it has no forced break.

**Nested** (`fragment_block_subtree`, `block_parent` only).

- **(3) Avoid that holds:** `avoid_keeps_whole(…, room = strips.bottom(page_index) - child_page_y, content_above =
  child_page_y > page_start_y + band)`.
  - It drops the `would_split` term, so `needs_recursion` is false.
  - `first_break_h = child_h` (`:3171`) and the cut (`:3173`) move it whole, as for any unsplittable child.
  - `nested_line_split` (`:4122`) already returns `None` for a fitting avoid paragraph that is not first on its strip, so
    it needs no change.
- **(2) Nested relaxed box:** when `content_above_child && starts_with_relaxed_avoid(child)`, `first_break_h =
  strips.fresh()`. This goes for line-split, multicol-free and recursed children alike (`:3132-3172`). The child starts
  the next page and is then split as today: probe 2 above.
- **Through wrappers.** The avoid-aware `leading_block_size` makes a gapless parent move before it is entered, so no
  parent is entered at a page top with an avoid first row it cannot hold. `starts_with_relaxed_avoid` at the outer level
  breaks before the parent when content precedes it. Chrome uses the class A point there, not the parent's border.

**(5) Table rows.** Blitz gives `tbody`/`tr` zero size; the table's `layout_children` are its cells (`:2031-2041`,
`:3713-3715`). So `tr { break-inside: avoid }` sits on a node the fragmenter never places, and nothing changes.

- Cells of a row that fits already move whole; F87 §7.3 found rows unobservable.
- Chrome also keeps such rows whole.
- Not covered: a row whose cell is entered by the gate (a cell holding several blocks that overflow). Ours splits it;
  Chrome with `avoid` should move the row. Filed as **F83d**, guard test only.

**(6) Margins.** Every move above goes through an existing advance:

- body: `cursor_y = 0` (`:1621`);
- nested: `page_taffy_origin = this_top_in_parent - band` (`:3202`).

The moved box's top margin is truncated (§5.2), and a test pins it.

**(8) F83c.**

- Inline roots measure by line extent (unchanged): a height-only avoid text box holds and keeps the strip path.
- Block wrappers measure by border box. A height-only wrapper after content goes to page 2 either way (`:1620`, then the
  strip slicer); a guard test pins it.

**Byte identity, predicted.**

- **Fixtures 01–12:** only 02 uses `avoid`.
  - Its `tr`s are zero-size.
  - Its two `.keep-together` sections are on page 3 with ample room in ours and in Chrome (`out/compare/02-long-p3.png`).
    They fit, so today's gate is false and the new rule gives the same path.
  - Prediction: 01–12 byte-identical.
- **Fixture 13:** a body-level inline root on the line path, before any new code. Byte-identical.
- **Fulgur VRT:** `avoid` appears only inside multicol containers (`multicol-break-inside-avoid`, `multicol-2`,
  `multicol-table-width`). The gate never enters those, and F30d does not read the children's `break-inside`.
  Byte-identical.
- **`tests/break_inside_avoid.rs`:** unchanged; the `.huge` box is first on its page.
- **Examples:** only `examples/break-inside`'s `.callout`, a `div` with one `<p>`, `padding: 10pt 14pt`.
  - Today F28 already moves it whole when its `<p>` does not fit.
  - It is the only possible mover, and only if just its bottom padding straddles the page end. If so it moves to page 2,
    which is correct (§4.4 counts the border box).
  - Step 3 renders it and names the outcome.
  - Any other re-bless is a regression.

## 4. Acceptance

**Ours: new file `crates/htmltopdf/tests/avoid_wrappers.rs`.**

- Setup: A4, 15 mm, Inter, strict, `css_file(fixtures/tailwind/tailwind.css)`, `line-height: 20px`.
- Helpers copied from `tailwind_breaks.rs` / `break_inside_avoid.rs`: `printed_words` (`pdftotext -bbox`), `page_of`,
  `assert_every_word_once`.
- Each case uses `class="break-inside-avoid"`, with an inline-`style` twin where marked (I).
- Expectations come from the step 1 Chrome probes.

1. Probe 1 (I): `aaa`, `bbb`, `ccc` on page 2, and `aaa`'s top within 1 pt of the content top (margin truncated: the
   wrapper has `mt-6`). **Red.** Control without the class: `aaa` on page 1, which is the in-place split for `auto`
   (guard).
2. The same wrapper first in body: page 1 (guard).
3. `<p>before</p>`, then the wrapper of four 700-word `<p>`: `word00000` on page 2, every word once. **Red.** Without
   `before`: page 1, the existing guard `an_avoid_wrapper_of_four_long_paragraphs_loses_no_word`.
4. Nested relaxed (I): `<div><p>before</p><p avoid>` with 2500 words: `word00000` on page 2, 9 pages like Chrome (±1 for
   F81 under `normal`). **Red.**
5. Nested holds: `<div><div style="height:930px"></div><div avoid>` of three `<p>`: all three on page 2. **Red.**
6. Through a wrapper: `<p>before</p><div class="border-t-2"><div avoid>` with four long `<p>`. `word00000` and the outer
   border are on page 2. **Red.**
7. Forced break: spacer 960 px, then avoid `[p a1, p a2, p a3, p.b style=break-before:page]`.
   - The `a`s on page 2 and `b` on page 3. **Red** if step 1 confirms.
   - Guard with spacer 930 px and one `a`: `a` on page 1, `b` on page 2.
8. No blank page (guards):
   - an avoid box of 980 px first in a body with `margin-top: 40px`: starts on page 1;
   - a relaxed avoid `<div class="mt-8">` first in body: `word00000` on page 1.
9. F83c guard: `<p>before</p><div style="height:1500px" class="break-inside-avoid"><p>a</p><p>b</p></div>` gives pages
   2–3, with `a` on page 2.
10. Table guard: the 02 shape (60 rows, `tr{break-inside:avoid}`). Every row's five cells share a page; the page count
    equals the same table without the rule.

**Fork: lib tests** next to `:8370`, `avoid_fragments` helper (200 px pages, 20 px lines).

- **Red:**
  - `avoid_wrapper_that_fits_a_fresh_page_moves_whole`: spacer 150 px, then `div.av` of three `<p>`; one fragment, page 1,
    y 0.
  - `avoid_wrapper_taller_than_a_page_after_content_starts_on_the_next_page`.
  - `nested_avoid_wrapper_moves_whole`.
  - `nested_relaxed_avoid_paragraph_starts_a_page`.
  - `forced_break_piece_of_an_avoid_box_moves_whole`.
- **Guards:**
  - `avoid_wrapper_first_on_its_page_is_entered`;
  - `avoid_box_on_a_short_first_strip_is_split` (body offset 60 px);
  - the existing F83/F83c tests (`:8411-8515`) unchanged.
- **Unit tests:**
  - `avoid_keeps_whole`: the six combinations;
  - `avoid_piece_size`: none, `before`, `after`, nested;
  - `starts_with_relaxed_avoid`: a gap stops it, padding does not;
  - `leading_block_size` with an avoid first row.
- **`render_smoke.rs`:** `render_smoke_avoid_wrapper_moves_whole_to_the_next_page` (end-to-end; this render path is not
  covered by codecov otherwise, per fulgur's CLAUDE.md).

**Fixture `14-totals-keep-together`** (new).

- Content: the 13 header and party grid, then an items table of 2–3 lines per row, sized so that it ends about 120–160 px
  above the page-1 content bottom in both engines. Then `<div class="break-inside-avoid mt-6">` with:
  - the totals table (Základ daně, DPH 21 %, Celkem k úhradě);
  - payment details: bank account, IBAN, VS, due date;
  - a signature block (Vystavil, razítko a podpis);
  - about 300 px together. Czech content.
- PNG:
  - Page 1 ends with the last item row.
  - Page 2 starts with the totals at the content top, then payment and signature, as in Chrome.
  - Today page 1 shows the totals under the table and page 2 the rest.
- Record compare numbers with main's binary (before) and with the branch (after). Threshold = measured worst page + 1
  point. Both have 2 pages, so not in `PAGE_COUNT_MAY_DIFFER`.
- README: names the class, F83b, and the before/after numbers. Fixture 13's README drops "(F83b)" from "Not exercised".

**Pixel diff.** Fixtures 01–13 are `cmp`-identical to main's binary, and the compare numbers and thresholds are unchanged.

## 5. Steps

1. **Confirm (gate).** Scratch only: our CLI on the local fork, temporary logs removed afterwards.
   - Log the gate decision for probes 1 and 2 (expect `would_split` true, F28 no advance).
   - Chrome probes (Playwright, A4, 15 mm, Inter, `20px` and `normal`) for every §4 case plus:
     - a flex row with an avoid item;
     - a table row whose cell holds two `<p>` and overflows (for F83d);
     - the `.callout` example.

   Then decide:
   - If Chrome keeps cases 3/4/6 on page 1, drop rule (2) and step 4, and record it.
   - If Chrome splits case 7 in place, drop `avoid_piece_size`.
   - If probe 1 or 5 does not move in Chrome, or the gate is not the emitter: stop, write
     `docs/postmortems/2026-10-18-f83b-*.md`.
2. **Red tests.**
   - Ours: `avoid_wrappers.rs`.
   - Fork: the lib and unit tests, and `render_smoke`.
   - Record which are red.
3. **Fork, rule (1)/(3)/(4):**
   - `avoid_extent`, `avoid_keeps_whole`, `avoid_piece_size`;
   - `leading_block_size(column_styles)` and all its callers;
   - the gate changes at `:1530` and `:3062`;
   - comments at `:1495-1500`, `:1612-1619`, module docs `:53-56`, `:742-744`.

   Then:
   - `--lib` on crates.io Blitz (the same 13 failures as at `659affe`) and on the local Blitz (0 failures);
   - all 42 `tests/*.rs` one by one;
   - VRT byte-identical (with the `fonts-noinInter.conf` fontconfig, as in F87);
   - the 21 examples rendered before and after, with the callout outcome named;
   - fmt and `clippy --all-targets -D warnings`.

   Any unpredicted mover means stop and write a postmortem.
4. **Fork, rule (2):** `starts_with_relaxed_avoid`, the `content_above` parameter, the body pre-advance and the nested
   `first_break_h`. Then the step 3 checks again.
5. **Ours:**
   - lock bump;
   - `cargo test --release --locked`;
   - fixtures 01–13 `cmp` against main's binary;
   - compare unchanged;
   - render time of 01 and 02 within noise.
6. **Fixture 14:**
   - HTML and README;
   - Chrome reference (the scratch `render-ref.mjs`, as in F83 §7);
   - tune the row count until Chrome moves the wrapper and main's binary splits it;
   - compare and threshold;
   - look at the PNGs.
7. **Docs:**
   - findings: F83b done, F83d filed;
   - `docs/upstream.md` fork row;
   - §7 here.

   Then run `vrt-validator`. `pdf-reviewer` is not needed: the writer, fonts and metadata are untouched.

## 6. Risks and non-goals

- **The gate change is broad.** Any avoid box that fits a page now moves whole, not only invoice wrappers. That is the
  spec, but customers' multi-page documents with `avoid` on large sections will gain pages. The guards at 8 keep "first
  on page" and "short first strip" from producing blank pages.
- **`content_above` is a new notion.** It differs from the `cursor_y > 0` convention F28 uses (margins count there). A
  mismatch would show up as a blank page or a missed move; tests 8 and the lib guards pin both.
- **Tolerance.** `avoid_keeps_whole` counts the bottom padding and border (§4.4); F28 counted the first row. The
  `.callout` example may move.
- **Rebase conflicts.** `fragment_block_subtree` gains a parameter and `leading_block_size` changes signature: expect
  conflicts with F30/F30b/F30d on a fulgur bump.
- **Non-goals:**
  - **F83d:** avoid on flex/grid items, on table cells, and `tr` rows whose cells are entered.
  - Avoid on multicol containers taller than a page (F30d-b) and inside columns.
  - `avoid-page` / `avoid-column` distinctions.
  - `break-before/after: avoid` scoring (F87c maps them to "no forced break").
  - F83a (`page-break-inside` alias).
  - F30c (body-level `auto` paragraphs move whole).
  - Orphans/widows parsing.
  - `box-decoration-break`.

## 7. Result

(Filled in at landing.)
