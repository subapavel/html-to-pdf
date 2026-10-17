# F78 — A long paragraph loses a line at a page break

Date: 2026-10-17. Finding: F78 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours`, carried via `[patch.crates-io]`.

## 1. Problem

One body-level `<p>` of `word%05d` words (Inter, A4, 15 mm margins) loses one line at the 25th page break and another
near the 48th: 0 words missing at N ≤ 9000, 7 at 10 000–15 000, 14 at 20 000. At N = 12 000 page 25 ends at `word09094`
with a blank band at the bottom and page 26 starts at `word09102`. With Noto Sans a line goes missing about every 7
pages. The dropped-text check cannot see it (the line was laid out and shaped, then never painted). It is not f32
drift but a fixed per-page error from describing the same lines in two ways:

1. The paginator measures lines with Parley's line boxes: `collect_inline_line_metrics` returns `(min_coord, max_coord)`
   (fork `pagination_layout.rs:3290-3305`); a fragment's height is `bottom(last) − top(first)` (`fragment_inline_root`
   `:3805-3806` / `:3829-3831`, `fragment_nested_inline_root` `:3577-3590`).
2. Blitz lays text out with `quantize = true` (`blitz-dom layout/construct.rs:739`). Parley 0.6
   (`layout/line/greedy.rs:608-641`) rounds ascent and descent, clamps negative leading to 0 for min/max, and advances
   `y` by the unrounded `line_height`. With Blitz's `normal` = 1.2 (`stylo_to_parley.rs:115`), an Inter 16 px line
   advances 19.2 px but its box is 16 + 4 = 20 px, so consecutive boxes overlap (Noto Sans: 22 px box, 2.8 px overlap).
3. The renderer, `paragraph_lines_for_page` (`render.rs:3802-3865`), recomputes the page's lines on its own: `consumed`
   = Σ previous fragment heights (`:3815-3819`), then it walks Σ `ShapedLine.height` (Parley's `line_height`,
   `convert/inline_root.rs:638-643`) to choose the first line (`:3822-3831`) and rebases baselines by `consumed`
   (`:3855`).

Every fragment therefore counts ≈ 0.8 px more than its lines advance. For the repro (page 0 holds 51 lines because the
`<p>` margin collapses through body, F29; later pages 52) the fragment heights are 980, then 999, 999, 999, 999, 1000, …,
so `consumed − (top(first_k) − top(0))` = 18.6 px at k = 24 and 20.2 px at k = 25: on page 25 every line is drawn 18.6 px
too high (the blank band); at k = 25 the excess passes one line (19.2 px) and the start index skips planned line 1299 =
`word09095…09101`. Next crossing k ≈ 48. When boxes are shorter than the advance (a fractional `line-height` with positive
leading) the error runs the other way and a line is **painted twice**. Integer line-heights with leading ≥ 0 tile
exactly — why the F30 tests (`line-height:20px`) and most fixtures never show it.

## 2. Spec

css-break-3 §4.1: breaks inside a paragraph are class B, between line boxes; a line box contains no break point
(CSS 2.1 §13.3.3); css-break-3 §2: fragmentation splits the flow's content across fragmentainers. So each line box is
painted in exactly one fragment, at the position the fragmenter gave it. (Derived, not quoted; css-spec-researcher not
run.)

## 3. Design (fulgur fork)

Line ranges are decided once, by the fragmenter, as line indices; the renderer uses them as given (any second
derivation from floats can disagree with the first — this bug is that disagreement).

- `pub struct LineSlice { pub first: u32, pub end: u32, pub offset: Px }`; `PaginationGeometry.line_slices:
  Vec<LineSlice>`, parallel to `fragments`, empty unless the node was split by line. `offset = lines[first].0 −
  lines[0].0` (the fragmenter's own coordinate for the top of that fragment): rebasing by it puts every continuation
  page's first line exactly where line 0 sits on page 0 (page 0 unchanged), and it is the base
  `record_split_inline_root_descendants` already uses for inline boxes, so text and images agree on every page.
- `fragment_inline_root` and `fragment_nested_inline_root` push one slice per fragment;
  `fn line_slices_tile(slices: &[LineSlice], n: usize) -> bool` (contiguous, covers `0..n`) checked with `debug_assert!`.
- `paragraph_lines_for_page(all_lines, fragments, slices: &[LineSlice], page_index, is_split)`: when the slices match the
  fragments and `slices.last().end == all_lines.len()`, return `all_lines[first..end]` rebased by `offset.in_pt()`;
  otherwise fall back to today's height walk with a `log::warn!` naming the node (so `<p>`s split by any other emitter
  don't go silent).
- Unchanged: break decisions, fragment `y`/`height`, cursors — page counts cannot change. When boxes tile,
  `offset == consumed` exactly (quantized coordinates are integers), so that output is byte-identical.

Rejected: fragment heights = Σ line-height (changes breaks and page counts); handing min/max to the renderer (still two
derivations).

## 4. Acceptance

- `crates/htmltopdf/tests/long_paragraph_pages.rs` (A4, default `RenderOptions`, Inter, strict), with the
  `nested_paragraph_split.rs` word/bbox helpers: every word exactly once, in order; none below the content box; no word
  top more than 2 pt above the content top (catches the upward shift); page count unchanged. Cases: body `<p>`
  N = 12 000 and 20 000 (today 11 993 / 19 986); `div > p` N = 12 000 (F30); `<td><p>` N = 12 000 (F30/F30b). All red
  today.
- `long_paragraph.rs`: back to A4 with default margins (drop the one-tall-page workaround); a guard only (too short to
  reach a lost line).
- Fork unit tests: (a) `paragraph_lines_for_page` with synthetic 19.2 px lines and fragments from 20 px boxes, 60
  pages, and (b) the mirror case (box < advance → duplicate) — union over pages is every line once (both red today);
  (c) both fragmenters record tiling slices with the expected offsets; (d) a lib test on a real document with
  `examples/.fonts` Noto Sans, 7000 words: every line on exactly one page; plus one `render_smoke`.
- Fixtures 01–11: byte-identical expected, except possibly 06 and 08 (`leading-snug` at 10pt = 18.33 px lines whose
  boxes don't tile): only continuation-page lines may move, by ≤ 1 px, toward the fragmenter's position; same lines,
  same pages, compare ±0.1. Fulgur VRT/example goldens byte-identical except goldens with a split paragraph whose boxes
  don't tile (UA `normal` with Noto: 2.8 px per page) — name each; its diff must only be such a move.

## 5. Steps

1. Confirm (scratch): for N = 12 000 dump per fragment the planned `[first, end)`, height, `consumed`,
   `top(first) − top(0)` and the renderer's start/end; expect ≈ 0.8 px/page to 18.6 / 20.2 at k = 24 / 25, render start
   = planned + 1 at k = 25, box 20 vs `line_height` 19.2, `entry.lines.len()` = Parley's line count. If not, stop and
   write a postmortem.
2. Ours: red `long_paragraph_pages.rs`, `long_paragraph.rs` back to A4. Fork: red unit tests (a)–(b).
3. Fork: `LineSlice`, recording in both fragmenters, `line_slices_tile` debug check (output-neutral); (c) green.
4. Fork: renderer reads slices, warning fallback, (d) + smoke, clippy, fmt; VRT/examples, re-bless only named goldens.
5. Ours: lock bump, tests, `render-fixtures.sh` + `cmp` 01–11, compare, docs, `vrt-validator`, `pdf-reviewer`
   (`render.rs` is on the writer path; text moves between pages).

## 6. Risks and non-goals

- Convert's `ShapedLine`s come from a different layout pass than the fragmenter (step 1 checks the count; the fallback
  stays). Paragraphs reworked by `recalculate_paragraph_line_boxes` (`convert/inline_root.rs:260`, inline images) keep a
  baseline model that differs from Parley's (already true for their descendants). Rebase conflicts with F30c.
- Non-goals: Blitz `normal` = 1.2 vs Chrome's font metrics (Inter ≈ 1.21), which moves break positions — separate
  finding; the overlapping boxes themselves; Σ-height slicing for blocks and multicol (`render.rs:1543`, `:2722`), where
  slices are exact; body-level widows parity (F30c).

## 7. Result

(Filled in at landing.)
