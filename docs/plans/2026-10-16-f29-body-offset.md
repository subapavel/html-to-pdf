# F29 — The fragmenter ignores the body's own offset on page 0

Date: 2026-10-16. Finding: F29 in `2026-10-15-m0-findings.md` (+ the body-level remainder recorded in F30 §7). Status:
in progress. Fix goes on `subapavel/fulgur` `htmltopdf/srgb-colours` (HEAD `6c23d12`), carried via `[patch.crates-io]`.

## 1. Problem

Page 0 is painted lower than it is paginated. Render places page-0 content at `margin.top + body_offset + frag.y`
(`render.rs:283-287`, also `:110-114` link destinations, `:264-268` body background), where `body_offset` =
`body.final_layout.location.y` (body margin + a first child's `margin-top` collapsing through `<body>` + `html`
padding/border). The fragmenter starts `cursor_y` at 0 (`pagination_layout.rs:494`) and checks against the full page
height, so up to `body_offset` px of page 0 is drawn below the content box (or below the MediaBox and lost).

Probes: `/tmp/claude-0/vrt-f28/ct_m_top_big.html`, `c_m_top_big.html`; 3 × `<p style="margin:500px 0 0">` (second `<p>`
at 1033 px on a 1009 px page); `out/scratch/val2/k3-bm100-sp950.html` (Line0001 at y = −13.6 pt, lost),
`h2b-spacer980-bm40.html`, `k4-wrap-bm100-sp930.html`, `j1-bodyp-spacer980-bm40.html`, `c3-longtable-bm40.html`.

F30 (`099f2a7`) fixed only nested split paragraphs via a local `page_limit_of` (`:1996-2005`, used at `:2366`, `:2398`,
`:2461`, `:2536`, `:2635`, `:2763`, `:2893`). Still full-page: body level `:775`, `:787` → `fragment_inline_root`
`:3614`, `:887`, `:902-906`, `:955`, `:1010`; subtree `:2354`, `:2369`, `:2465`, `:2423` (`:2398` works around it).

## 2. Spec

CSS 2.1 §8.3.1 (a first child's top margin collapses through body; root margins don't). CSS 2.1 §13.3.3 /
css-break-3 adjoining margins at breaks → 0; body padding/border not repeated (`box-decoration-break: slice`), so the
offset applies to page 0 only. css-break-3 §4.1: no break point before the first content of the flow.

## 3. Design (fulgur fork, `pagination_layout.rs` only)

The paginator stays in body coordinates; page 0's bottom moves up by the offset (fragment `y` meaning unchanged, so
render / paragraph / fixed-position code stay byte-identical). Rejected: starting the cursor at the offset (changes
every page-0 `y`, reassociates floats, breaks the `cursor_y > 0.0` "nothing placed yet" tests → blank page 0).

`PageStrips { page_h, first_offset }` with `bottom(page)` (page_h − first_offset on page 0) and `fresh()` (page_h),
built once in `PaginationLayoutTree::new`, passed instead of the bare `page_height_px` (the type forces every site to
choose). Delete `page_limit_of`. Sites combining a cursor with the page end → `bottom(page)`; size-only tests ("taller
than a page", header ≤ ¼ page, continuation slices) → `fresh()`; `:2398` → plain `leading`. F12 unaffected (bands only
on continuation pages). `log::warn!` when `bottom(0) <= 0`.

## 4. Acceptance

`crates/htmltopdf/tests/body_offset.rs` (inline HTML): every token once, in order, none below the content box, page
count = Chrome's, for (a) 3 × `margin:500px 0 0`, (b) `ct_m_top_big`, (c) `c_m_top_big`, (d) `h1.mt-8` + 200 `<p>`,
(e) `body{margin:40px}` + body-level `<p>` of 120 lines, `j1`, (f) `k3`, `h2b`, `k4`, (g) `c3` (thead once per page),
(h) body margin 40 + two 500 px divs → 2 pages, (i) `html{padding-top:50px}` + 60 `<p>`; guards (j) body padding +
border, (k) no offset unchanged. Fork unit tests (`PageStrips`, monolithic pair, body-level paragraph, table, collapsed
margin, 500 px div first slice) + one `render_smoke`. Fixtures 01–06 byte-identical (preflight body margin 0, no
collapsing first-child margins). New fixture `07-heading-margin` (`h1.mt-8` first child, 2 pages, a row that today
hangs below the content box moves to page 2 like Chrome) with Chrome reference and threshold.

## 5. Steps

1. Ours: red tests, Chrome page counts, fixture 07 + reference. 2. Fork: `PageStrips` threaded, output-neutral.
3. Fork: body-level sites → `bottom()`. 4. Fork: subtree sites, warning, smoke, clippy, fmt, goldens (re-bless only
goldens whose page-0 content was below the content box; name them). 5. Ours: lock, tests, compare + `cmp`, 07
threshold, docs, `vrt-validator`.

## 6. Risks and non-goals

Fulgur goldens/tests use the UA `body{margin:8px}` — multi-page ones may move content that hung up to 8 px into the
margin (valid only if it was below the content box). Real documents may gain a page (expected, matches Chrome).
Non-goals: offset ≥ page height (warning only); first monolithic content taller than page 0's room (F29b); parent
frame end on the `!split_below` branch; `html` margin; body bottom margin at document end; `@page :first` margins
(F6/F31); negative body margin.
