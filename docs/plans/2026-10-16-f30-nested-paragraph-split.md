# F30 — A paragraph taller than a page inside a block loses its tail

Date: 2026-10-16. Finding: F30 (paragraph sub-case) in `2026-10-15-m0-findings.md`. Status: in progress. Fix goes on
`subapavel/fulgur` `htmltopdf/srgb-colours` (0.40.0 + F1, F11, F12, F19, F28, F9), carried via `[patch.crates-io]`.

## 1. Problem

A paragraph (inline formatting context: `<p>`, `<li>`, `<pre>`, or a `div` whose content is text) taller than a page
inside a block (`div`, `section`, `li`, `<td><p>`, a Tailwind `rounded-lg border p-4` notes box) prints only the lines
that fit on its first page; the rest is drawn below the page box and no page is added. At body level the same `<p>`
splits at line boxes.

Root cause (fork `pagination_layout.rs`, from reading):
1. Body level detects inline roots before the recursion gate (`:742-805`) → `fragment_inline_root` (`:2860`), one
   fragment per page.
2. `fragment_block_subtree` (`:1776`) doesn't ("Nested inline roots are not split at line edges", `:1731-1735`); the
   gate is false for a paragraph child (`would_split_block_subtree` `:1387` skips zero-height text nodes).
3. The child takes the leaf path (`first_break_h = child_h`), emitted as one full-height fragment; the cursor ends
   past the page.
4. F28's `leading_block_size` counts an inline root whole, so a wrapper whose first child is such a paragraph is
   entered in place and never cut.
5. Render (`render.rs:3710` `paragraph_lines_for_page`) draws all lines from one origin; `implied_page_count` adds no
   page.

## 2. Spec

css-break-3 §4.1 / CSS 2.1 §13.3.3: breaks between line boxes are class **B**; ≥ `orphans` lines before and ≥ `widows`
after (initial 2), relaxed when no break avoids overflow; a single line box is monolithic. List markers belong to the
first line box.

## 3. Design (fulgur fork)

Split only what cannot fit any page whole (or is first on its strip and overflows); shorter paragraphs keep today's
whole move, so existing output stays byte-identical.

- `nested_line_split(...) -> Option<LineSplit>`: > 1 line, parent in block flow (not flex/grid, not atomic inline, not
  orthogonal, not clipping), child not multicol / transformed; and (i) `inset_top + line span > page_h + 1` or (ii)
  first on its strip and overflowing.
- `plan_line_breaks(lines, first_room, page_room)` (pure): greedy, ≥ 1 line per strip, widows pull-back to n − 2,
  relaxed otherwise; `ORPHANS = WIDOWS = 2` at module level.
- `fragment_nested_inline_root(...)`: one fragment per range (first at `y_in`, others at 0), height = line span,
  `inset_top` reserved on every strip; records descendants (inline-blocks, images) on the fragment holding their line.
- `fragment_block_subtree`: `first_break_h = inset_top + bottom of line 2` for a split child (so it moves when < 2
  lines fit); recursion branch calls `fragment_nested_inline_root`; previous-page parent fragment uses the strip
  height.
- `leading_block_size(.., page_h)`: a split inline root counts `inset_top +` 2 lines.
- `render.rs`: `split_slice_height_pt` for split inline-root blocks at the three slice sites; list marker only on the
  first fragment.
- `warn_unprinted_inline_lines(...)`: `log::warn!` for unsplit inline roots whose lines end below the page (F30b
  cases), so remaining loss is not silent.

Interaction: F28 tests stay green; F12 band 0 on pages reached through `<td><p>`; F13/F19 unchanged.

## 4. Acceptance

`crates/htmltopdf/tests/nested_paragraph_split.rs` (`body{margin:0} p{margin:0;line-height:20px}`): every `LineNNNN`
once, in order, none below the content box, ≥ 2 pages for (a) `rounded-lg border p-4` + `<p>` × 400 lines, (b) direct
text in a bordered div, (c) `<ul><li>` (bullet once), (d) `<section><article>`, (e) `<td><p>`, (f) `<pre>` /
`pre-wrap` in a div, (g) 200 short `<li>` (guard); `<div><p>` page count = body-level page count; fill / orphans /
widows cases; `#[ignore = "F30b"]` for `<td>` direct text. Fork unit tests + one `render_smoke`. Fixtures 01–05
byte-identical. New fixture `06-long-notes` (invoice + ≈ 1.4 pages of Czech terms in a `rounded-lg border p-4` box and
a `<ul>`; only classes in the compiled `tailwind.css`) with a committed Chrome reference and a threshold.

## 5. Steps

1. Probes + red tests. 2. `plan_line_breaks`. 3. `nested_line_split` / `fragment_nested_inline_root` / wiring /
`leading_block_size`. 4. Render slice height + marker, smoke, clippy, fmt, goldens. 5. Warning. 6. Ours: lock, tests,
fixture 06 + reference, compare, docs, `vrt-validator`, `pdf-reviewer`.

## 6. Risks and non-goals

Rebase conflicts in `fragment_block_subtree`; `break-inside: avoid` on a paragraph taller than a page is relaxed.
Non-goals: `box-decoration-break`, splitting paragraphs shorter than a page (we move them whole), reading
`orphans`/`widows`. Follow-ups: F30b (flex/grid items, direct-text cells), F30c (body-level parity: pre-advance
pushes giant paragraphs, widows not relaxed, padded inline roots), F30d columns, F30e `display:table` wrappers,
F30f inline-block wrappers, F30g abspos.
