# F78 — A long paragraph loses a line at a page break

Date: 2026-10-17. Finding: F78 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
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

Landed on branch `f78-lost-line-at-page-break`; fulgur fork `htmltopdf/srgb-colours` `8993bd5` (fragmenters record
`LineSlice`s, `line_slices_tile`, output-neutral) and `412f902` (renderer paints the slices, fallback warning); the Blitz
fork is unchanged (`52b22ef`). `Cargo.lock` pins `412f902` (`e073b6b` after §8).

**Step 1, the model held** (`fulgur` CLI with a scratch dump in `fragment_inline_root` and `paragraph_lines_for_page`,
removed again; N = 12 000, Inter, A4, `--margin 15`): 33 pages, 1709 lines both in the fragmenter's metrics and in
`entry.lines`; fragment heights 980, 999, 999, 999, 1000, ...; fragment tops `top(first_k) - top(0)` are integers (979,
1978, 2976, ...); `consumed` (Σ fragment heights) minus Σ `ShapedLine.height` of the planned lines = 0.8 px at k = 1,
8.2 at k = 10, 18.6 at k = 24, 20.2 at k = 25 (the plan's numbers); the renderer's start index equals the planned one up
to k = 24 and is planned + 1 at k = 25 (1300 against 1299); line box (-1, 19) = 20 px against `line_height` 14.4 pt =
19.2 px; `pdftotext`: 11 993 of 12 000 words, which is what `htmltopdf` printed too.

**Red before** (the same tests, fork at `20cb647`): `long_paragraph_pages.rs` 4 of 4 red (`body > p` 12 000 and 20 000
words: 11 993 and 19 986 printed, first missing `word09095`; `div > p`: 11 993; `td > p`: 11 993; page counts 33, 55,
33, 33 are asserted after the words); fork unit tests (a) and (b) red (the first line of page 1 sits 0.75 pt above where
line 0 sits on page 0; for the mirror case the page holds a different number of lines than planned); (d) red: 698 lines
painted, fewer than the paragraph has (the `render_smoke` twin: `pdftotext` finds 6980 of 7000 words, the loss F70 saw); (c) did not compile
(no `LineSlice`).

**After:**

| Check | Result |
|---|---|
| `long_paragraph_pages.rs` | 4/4: every word once, in order, none below the content box, none above its top, page counts 33 / 55 / 33 / 33 unchanged |
| `long_paragraph.rs` | back on A4 with the default margins (guard only), green |
| fork `--lib` | 2037 passed (new: 2 tiling/recording tests, `line_slices_tile`, nested slices, (a), (b), fallback, (d)) |
| fork `render_smoke` | 249 passed (new: 7000 words over A4 in Noto Sans, 6980 -> 7000 words) |
| fork other test binaries | every `crates/fulgur/tests/*.rs` run one by one, all green |
| fork `fulgur-vrt` | all green, goldens byte-identical (none has a split paragraph with non-tiling boxes) |
| fork `examples_determinism` | 10 of 11; `committed_svg_matches_rendered` fails as before (environment, stale `examples/svg/index.pdf`) |
| fork examples | all 21 example PDFs rendered with the debug CLI before and after (`mise` recipe, bundled fontconfig): byte-identical, nothing re-blessed |
| fork fmt, `clippy --workspace --all-targets -D warnings` | clean |
| ours `cargo test --release --locked` | green except the two `nested_paragraph_split` tests below, which now pass with the adjusted slack; `fmt`, `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-11 | `cmp` byte-identical to the render before the lock bump (11 of 11), so 06 and 08 did not move; `compare.py` numbers identical: 4.4, 10.2/11.4/9.8, 5.7, 5.7, 21.6, 15.9/11.8, 7.2/7.1, 13.2/12.1, 5.1/4.9/0.7, 5.9/1.5, 6.9 %; page counts unchanged |

The repro in pictures (`out/scratch/f78/sbs-p25-26-before-before-after-after.png`, git-ignored; input `p12000.html` next to it): pages 25 and 26 of the 12 000-word document, before (page 25 ends
at `word09094` after a blank band, page 26 starts at `word09102`) and after (page 25 is full, page 26 starts at
`word09095`); 12 000 of 12 000 words, 33 pages.

Timing (wall clock, CLI start-up included, median of 7 runs, the two binaries run one after the other, not alternated):

| Document | before | after |
|---|---|---|
| fixture 02 | 56 ms | 53 ms |
| 800 paragraphs of 60 words, 160 pages (byte-identical output) | 368 ms | 354 ms |
| one `<p>` of 20 000 words, 55 pages | 2517 ms | 2446 ms |
| one `<p>` of 52 700 words, 145 pages | 16 187 ms | 16 456 ms |

No change beyond noise. The last row is slow before and after (F82).

Deviations from the plan:

- **`line_slices` is recorded for every paragraph the two fragmenters emit**, also when it ends up as one fragment (the
  plan said "empty unless the node was split by line"); the renderer ignores slices unless the node is split. The fixed /
  absolute emitters that replace a node's fragments clear its slices too. `fragment_inline_root` became `pub(crate)` so the
  render tests can drive it.
- **The fallback warning is per paragraph and page, not once**, and names the paragraph by its first 40 characters
  (`paragraph_lines_for_page` knows no node id). It did not fire in any fixture, nor in probes with inline-block,
  multicol, direct-text `<td>` and inline-image paragraphs; only the unit test reaches it.
- **`nested_paragraph_split.rs`: `SLACK_PT` 5.0 -> 6.0.** Two tests (direct text in a bordered div, in a flex-column item;
  `.box` has `line-height: normal`, so its boxes do not tile) failed by 0.75 pt: `Line0104` ends at 805.1 pt, the limit was
  804.4. The first page of that box is unchanged and already ends at 804.4 (the 5 pt of slack is exactly that overflow,
  F80); the continuation pages used to drift up by 1 to 2 px per page (first line at 54.3, 52.8, 51.3, 49.8 pt ...) and
  now start at 54.3 pt on every page like the first, their last lines 0 or 1 px below the first page's. That is the
  "toward the fragmenter's position" move the plan allowed for non-tiling goldens, here in a test limit, not a golden.
- (d) reads each line's text from its glyphs' `text_range`s: a run's `text` is the whole paragraph.
- Not in the plan, found on the way: F80 (box overflow), F81 (`normal` = 1.2 against Chrome), F82 (superlinear single
  paragraph); see the findings.

Not verified here: `vrt-validator` and `pdf-reviewer` (run by the owner's agent); veraPDF; Chrome's positions for the 12 000-word
document (it breaks lines differently because of F81, so there is no line-for-line reference); paragraphs reworked by
`recalculate_paragraph_line_boxes` over many pages beyond one probe (a `<p>` and a `div > p` of 1500 words each with 37
inline 14 px images, 9 pages: the same words on every page before and after, 148 images; the PDF differs, the first line
of every continuation page is at 42.1 pt like the others where it drifted between 39.9 and 41.4 pt before, and the images
stay in their lines on the pages looked at (page 4, top and bottom)).

## 8. After the review of PR #30 (`vrt-validator`, `pdf-reviewer`: merge)

- **M2 (introduced by F78), fixed:** the fallback warning fired for every one-line inline root inside a block that the block
  fragmenter splits (`<div style="height:1200px">text</div>` after a `<p>`), where the PDF is right. Fork `e073b6b`:
  `paragraph_lines_for_page` warns only when the paragraph has more than one line and the measured page holds some of them
  (`measured_cut_is_partial`). Red first: `crates/htmltopdf/tests/fallback_warning.rs` (one-line and three-line leaf blocks must not warn,
  a `break-inside: avoid` paragraph of 2500 words must, naming `word00000`). `leaf.html`, `near1020.html`, `wr_abs.html`, `fixh_short.html`,
  `nb1040.html`, `j7-inline-block.html`: 0 warnings (were 2 each); `pbi_normal.html`: still 7 (F83). Fixtures 01-11 `cmp` byte-identical.
- **L2** (image-to-text offset varies by up to 2.1 pt across the pages of a paragraph with inline images, `im/middle_1.3.html`) is the
  `recalculate_paragraph_line_boxes` risk of §6; filed as F84 together with the image 1.8 pt above the content top on continuation pages.
- Findings filed or extended from the reviews: F30d (multicol, HIGH), F83 (`break-inside: avoid`, HIGH), F30g (abspos), F30f (inline-block / overflow),
  F30c (widows), F84, a note under F20.
