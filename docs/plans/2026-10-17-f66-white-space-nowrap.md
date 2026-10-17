# F66 — `white-space: pre` / `nowrap` still wrap

Date: 2026-10-17. Finding: F66 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix goes on `subapavel/blitz`
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

## 7. Result

(Filled in at landing.)
