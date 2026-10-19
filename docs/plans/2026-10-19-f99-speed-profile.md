# F99 step 1 — where the 200-page render time goes

Date: 2026-10-19. Status: **measurement only** (no engine change; the instrumentation was scratch and is reverted). Finding: F99 in
`2026-10-15-m0-findings.md` (AGENTS.md goal 2: 200 pages under 1 s). Branch `f99-speed` (main `437f00f`), fulgur fork
`htmltopdf/srgb-colours` `7c1389b`, Blitz fork `htmltopdf/blitz-dom-0.2.4` `52b22ef`.

## 0. Result

- `b200-items` 2.44 s, `b200-text` 1.12 s, `b200-mixed` 1.05 s (CLI wall, median of 5, 4 vCPU Xeon 2.1 GHz). The instrumented binary measures the same, so the timers cost nothing visible.
- **The footers are 0.96 s of the 2.40 s render of `b200-items`** (40 %). Cause: every page whose margin-box text differs (`counter(page)`) builds two fresh Blitz documents (a measure document and a
  render document); each one re-parses Blitz's UA stylesheet and the whole 20 KB `tailwind.css` and re-registers the five Inter fonts. `counter(pages)` itself is free (no second pass).
- **Text shaping (Parley `build_into` -> HarfRust OpenType lookups) is the biggest single cost of the other two documents**: 431 of 1095 ms (39 %) in `b200-text`, 285 of 1023 ms (28 %) in `b200-mixed`.
- fulgur's own passes (pagination incl. F83b, F30d multicol, F30g abspos, fixed-position passes) are 1-8 % of any document. They are not where the time goes.
- Scaling is linear in pages for `b200-text` and `b200-mixed` (5.6-6.1 and 4.9-5.5 ms/page from 50 to 400 pages) and for the footers (4.8 ms/page). The only superlinear phases are Taffy table layout and
  Stylo style in **one very long table** (`b200-items` without footers: 6.0 -> 7.6 ms/page from 50 to 400 pages; Taffy x2.4-2.9 per doubling).
- One measured, byte-identical, near-zero-size win exists: blitz-dom's own `parallel-construct` feature (§5, C1) takes `b200-text` to 0.90 s and `b200-mixed` to 0.89 s. `b200-items` needs the footer
  fix as well and still looks like 1.4-1.6 s afterwards; the 1 s goal is reachable for two of the three documents, not (yet) for the 4800-row table (§6).

## 1. How this was measured

- **Build**: scratch copies of the workspace under `/tmp/claude-0/f99-ws-*` (the repo's `Cargo.toml` and `Cargo.lock` were never touched), `CARGO_TARGET_DIR=/tmp/claude-0/f99-prof-target`,
  `CARGO_PROFILE_RELEASE_STRIP=false`, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` (not `true`: 13 GB of disk were free and callgrind needs only symbols and line tables; the target dir is 1.5 GB).
  `target/release` is untouched. For the timers, `[patch.crates-io]` of the scratch copy pointed at the two local fork checkouts.
- **Timers**: `std::time::Instant` scopes in a small accumulator (`blitz_dom::prof`, a new file, scratch) around: `parse_inner` (`HtmlDocument::from_html`, font registration), each stage of
  `BaseDocument::resolve` (style / construct / deferred construct / flush / Taffy layout), Parley `build_into` (shaping) and `break_lines` (line breaking), every step of
  `Engine::layout_to_drawables`, and in `render_v2` the pre-passes, `draw_v2_page`, `MarginBoxRenderer::render_page` (split into measure documents, render documents, painting), the Krilla page
  drop (where the content stream is deflated) and `Document::finish`. Output: one `PROF` line per scope on stderr. The patches were 575 lines and are not committed; the fulgur and Blitz working trees are clean again.
  There was no phase timing to reuse: fulgur has no `tracing` and its `log::debug!` / `FULGUR_DEBUG` print no timings; Blitz has a `log_phase_times` feature, but it prints per `resolve` (400+ blocks here).
- **Numbers**: `htmltopdf render` with the flags of `scripts/bench.py`, 1 warm-up + 5 runs, median per scope. `valgrind --tool=callgrind` on the un-instrumented build of the same commits
  (one run per document; Ir is instructions, not time; 7.49 G / 14.71 G / 6.10 G Ir for text / items / mixed).
- `perf` is not installed. The build is LTO-thin, so small functions are inlined into their callers and do not show up separately in callgrind.

## 2. Phase breakdown (ms, median of 5, wall clock)

| # | Phase | `b200-items` | `b200-text` | `b200-mixed` |
|--:|---|---:|---:|---:|
| | **CLI wall** | **2443** | **1124** | **1053** |
| | `Engine::render` | 2403 | 1095 | 1023 |
| | CLI outside `render` (asset load, PDF write, exit; 7 ms on a 1-page fixture) | 40 | 29 | 30 |
| 1 | Parse: html5ever, Blitz UA stylesheet, `--css` sheet, 5 fonts (`parse_html`) | 98 | 26 | 63 |
| 2 | GCPM extraction from `--css` and inline `<style>`, DOM passes (counter, string-set, running, bookmark: no-ops here) | 10 | 3 | 7 |
| 3 | Stylo style (`resolve_stylist`) | 118 | 13 | 56 |
| 4 | Blitz box construction + `flush_styles_to_layout` | 56 | 11 | 31 |
| 5 | Deferred construction = **Parley shaping** (`resolve_deferred_tasks`) | 255 | 444 | 311 |
| | ... of which `TreeBuilder::build_into` (itemise, select font, HarfRust) | 217 | 431 | 285 |
| 6 | **Taffy layout** (`resolve_layout`, incl. table track sizing and Parley line breaking) | 364 | 149 | 96 |
| | ... of which Parley `break_lines` (x105 718 / x2561 / x40 815 calls) | 64 | 114 | 44 |
| 7 | `relayout_position_fixed` + column styles + F30d `multicol_layout::run_pass` | 33 | 5 | 39 |
| 8 | fulgur `pagination_layout::run_pass_with_multicol` (fragmenter incl. F83b avoid, F78 slices) | 52 | 6 | 30 |
| 9 | fixed / body-direct abspos passes + F30g `fragment_abspos_flows` | 17 | 2 | 10 |
| 10 | convert (`dom_to_drawables`) | 108 | 96 | 97 |
| 11 | rest of `layout_to_drawables` (drop of the Blitz document, map clones) | 42 | 4 | 44 |
| 12 | `draw_v2_page` + repeated table headers (Krilla drawing calls) | 149 | 216 | 162 |
| 13 | **`@page` margin boxes** (`MarginBoxRenderer::render_page`) | **963** | 0 | 0 |
| | ... measure documents (x201) | 418 | | |
| | ... render documents (x215: build + pagination pass + convert) | 467 | | |
| | ... painting (x400) and the rest | 78 | | |
| 14 | Krilla page finish (`Page::drop`, deflate of the content stream) | 44 | 106 | 73 |
| 15 | `krilla::Document::finish` (xref, fonts, write to memory) | 2 | 2 | 3 |
| 16 | rest of `render_v2` | 19 | 5 | 12 |

Notes.

- **`counter(pages)` has no phase of its own.** There is no second pagination pass and no re-layout of the body for the page count: `render_v2` takes `page_count` from the geometry
  (`implied_page_count`) before the first page is painted. The F30d / F30g / F83b passes are rows 7-9 and the fragmenter row 8 (F83b is part of the fragmenter): together 5 % (items), 1 % (text), 8 % (mixed) of the wall.
- Font subsetting and PDF writing are not a cost: `Document::finish` is 2-3 ms (17.7 M Ir). The PDF of `b200-text` is 0.96 MB.
- Rows 1, 3, 5 and 6 (parse, style, shaping, Taffy) are 50-56 % of the wall of every document once the footers are left out.
- Side note, not traced: stderr gets one `ERROR: Unexpected token` line per margin-box document (416 on `b200-items`, none on the other two): a Stylo parse error inside the margin-box wrapper
  (`margin_css` or the box declarations). Cheap, but it means some declaration is dropped and it is noise in CI logs.

### Top 15 inclusive functions per document (callgrind, Ir, process and `main` frames skipped)

`b200-items` (14.71 G Ir):

```text
 98.19%      14446M  fulgur::engine::Engine::render
 52.40%       7709M  fulgur::engine::Engine::layout_to_drawables
 50.40%       7415M  blitz_dom::document::BaseDocument::resolve
 47.31%       6960M  <blitz_dom::document::BaseDocument>::compute_child_layout
 47.15%       6936M  blitz_dom::layout::compute_node_layout
 45.79%       6737M  fulgur::engine::Engine::render_artifacts
 45.54%       6699M  fulgur::render::render_v2
 36.45%       5363M  fulgur::render::MarginBoxRenderer::render_page
 30.36%       4466M  fulgur::blitz_adapter::parse_inner
 23.51%       3458M  blitz_html::html_document::HtmlDocument::from_html
 20.86%       3069M  blitz_dom::document::BaseDocument::resolve_layout
 20.76%       3053M  taffy::compute::block::compute_block_layout
 19.19%       2823M  style::stylesheets::stylesheet::StylesheetContents::from_str
 19.19%       2823M  style::stylesheets::stylesheet::Stylesheet::parse_rules
 16.92%       2490M  taffy::compute::grid::track_sizing::track_sizing_algorithm::<blitz_dom::layout::table::TableTreeWrapper>
```

`b200-text` (7.49 G Ir):

```text
 99.13%       7423M  fulgur::engine::Engine::render
 74.47%       5576M  fulgur::engine::Engine::layout_to_drawables
 66.58%       4985M  blitz_dom::document::BaseDocument::resolve
 52.58%       3937M  blitz_dom::document::BaseDocument::resolve_deferred_tasks
 51.77%       3876M  parley::builder::TreeBuilder::build_into
 51.28%       3839M  parley::builder::build_into_layout::<TextBrush>
 46.98%       3517M  parley::shape::shape_item::<TextBrush>
 32.48%       2432M  harfrust::hb::face::hb_font_t::shape_with_plan
 28.55%       2137M  harfrust::hb::ot_layout::apply_forward
 28.09%       2103M  <blitz_dom::document::BaseDocument>::compute_child_layout
 28.09%       2103M  blitz_dom::layout::compute_node_layout
 24.66%       1847M  fulgur::engine::Engine::render_artifacts
 24.55%       1838M  fulgur::render::render_v2
 17.00%       1273M  harfrust::hb::ot_shape::position_by_plan
 15.94%       1193M  taffy::compute::block::compute_block_layout
```

`b200-mixed` (6.10 G Ir):

```text
 97.44%       5941M  fulgur::engine::Engine::render
 72.40%       4414M  fulgur::engine::Engine::layout_to_drawables
 56.39%       3438M  blitz_dom::document::BaseDocument::resolve
 38.30%       2335M  blitz_dom::document::BaseDocument::resolve_deferred_tasks
 36.81%       2245M  parley::builder::TreeBuilder::build_into
 36.32%       2214M  parley::builder::build_into_layout::<TextBrush>
 34.35%       2095M  <blitz_dom::document::BaseDocument>::compute_child_layout
 34.21%       2086M  blitz_dom::layout::compute_node_layout
 33.42%       2038M  parley::shape::shape_item::<TextBrush>
 25.04%       1527M  fulgur::engine::Engine::render_artifacts
 24.68%       1505M  fulgur::render::render_v2
 23.37%       1425M  fulgur::convert::convert_node
 22.97%       1400M  harfrust::hb::face::hb_font_t::shape_with_plan
 21.02%       1282M  taffy::compute::block::compute_block_layout
 19.81%       1208M  harfrust::hb::ot_layout::apply_forward
```

Reading the lists: `b200-items` is the margin boxes (`render_page` 36 %, of which `parse_inner` / `from_html` 23.5 %, i.e. stylesheet parsing: `Stylesheet::parse_rules` 19.2 %) plus Taffy table track sizing
(`track_sizing_algorithm::<TableTreeWrapper>` 16.9 %, 33 661 calls of `GridItem::minimum_contribution`, each a cell laid out at min-content). `b200-text` is 47 % in `parley::shape::shape_item`, of which
HarfRust `shape_with_plan` 32.5 % (`apply_forward` 28.6 %; GSUB `substitute_pre` 13.6 %, GPOS `position_by_plan` 17.0 %), per-character `FontSelector::select_font` 7.6 % (1.1 M calls) and swash cluster parsing 2.6 %.
The Krilla page finish is zlib-rs (`longest_match`, `deflate_medium`), 9.6 % of `b200-text`.

## 3. Why the footers cost about 1 s on `b200-items`

The footer is two boxes: `@bottom-right { content: "Strana " counter(page) " / " counter(pages) }` and a constant `@bottom-left`.

Call path: the `render_v2` page loop (`render.rs`, per page) -> `MarginBoxRenderer::render_page` (`render.rs` ~L4468) -> per effective box `resolve_content_to_html_with_anchor` (the text with the page numbers
substituted) -> two loops in that function:

1. **Stage 1a** (top / bottom boxes): `self.measure_cache.entry((html, page height, content width)).or_insert_with(...)` -> `blitz_adapter::parse_and_layout_with_generics(measure_html, ...)`.
2. **Stage 3**: `self.render_cache` keyed by `(html, rect width, rect height)`; on a miss `parse_and_layout_with_generics(render_html, ...)`, then `run_pass_with_break_styles` and `dom_to_drawables`.

The key contains the HTML, and the HTML contains the page number, so the right box misses **both** caches on every page (201 measure documents, 215 render documents because the rect width also changes with the
number of digits); the constant left box hits after page 1. Every miss builds a complete Blitz document: `parse_inner` creates a new Parley `FontContext`, **clones and registers all five Inter files**, then
`HtmlDocument::from_html` creates a new Stylo `Stylist`, **parses Blitz's UA stylesheet**, and parses `margin_css`, which is `strip_display_none(gcpm.cleaned_css)` = **the whole 20 KB `tailwind.css`** passed with `--css`,
before the three-word footer is laid out.

Per margin document (instrumented, averages over 201 / 215 documents): 2.08 ms for a measure document, 2.17 ms for a render document, of which `from_html` 1.3 ms, font registration 0.22 ms, Stylo style (stylist flush of
the big sheet) 0.42 ms, deferred construct 0.09 ms, Taffy 0.01 ms, convert 0.12 ms, document drop about 0.07 ms. Callgrind attribution over all 417 documents: UA stylesheet parse 1169 M Ir (8.0 % of the run, 2.8 M per document),
the `--css` sheet parse 1682 M Ir (11.4 %, 4.0 M per document), html5ever and the rest of `from_html` about 3.7 %, document drop 1.3 %.

Experiments (`b200-items` shape, 200 pages, instrumented binary, three runs each, ms):

| Variant | `render_page` total | `Engine::render` |
|---|---:|---:|
| as committed (`counter(page)` + `counter(pages)`) | 962-1037 | 2320-2473 |
| footer `counter(page)` only | 850-1062 | 2105-2505 |
| footer with static text only (`content: "Strana"`) | **13-15** | **1265-1402** |
| `--css` empty instead of `tailwind.css` (143 pages, 300 margin documents) | 387-408 (1.3 ms per document) | |

So the cost is **any page-varying footer text, not `counter(pages)`**: one constant footer costs 14 ms in total (two documents). Without the `--css` sheet a margin document is 1.3 ms instead of 2.1 ms, so the
Tailwind sheet is about 40 % of each document and the UA sheet, the fonts and the stylist the rest. `b200-items` without footers takes 1.33 s in the scaling run (§4), which matches the finding's 1.4 s.

## 4. Scaling (instrumented binary, median of 3 after a warm-up)

Documents are regenerated with the `scripts/bench.py` generators (`gen_text` / `gen_items` / `gen_mixed` with N scaled; at N for 200 pages they reproduce the committed corpus byte for byte).
`items-nofooter` is the `b200-items` shape with `PAGE_PLAIN` instead of the footer boxes. Columns are ms.

| Document | Pages | Wall | per page | Parley shaping | Taffy | Stylo style | pagination | convert | draw | page finish | footers |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| text | 49 | 300 | 6.1 | 117 | 37 | 3 | 1 | 24 | 51 | 25 | |
| text | 98 | 562 | 5.7 | 210 | 73 | 5 | 3 | 45 | 98 | 49 | |
| text | 200 | 1151 | 5.8 | 449 | 147 | 10 | 6 | 96 | 202 | 100 | |
| text | 397 | 2222 | 5.6 | 861 | 301 | 33 | 14 | 238 | 417 | 208 | |
| mixed | 50 | 243 | 4.9 | 65 | 22 | 14 | 5 | 22 | 33 | 17 | |
| mixed | 100 | 536 | 5.4 | 132 | 44 | 28 | 11 | 45 | 70 | 36 | |
| mixed | 201 | 1100 | 5.5 | 311 | 99 | 64 | 28 | 109 | 155 | 79 | |
| mixed | 400 | 2079 | 5.2 | 565 | 178 | 111 | 53 | 195 | 313 | 156 | |
| items-nofooter | 51 | 306 | 6.0 | 52 | 58 | 21 | 8 | 21 | 29 | 8 | |
| items-nofooter | 100 | 640 | 6.4 | 110 | 143 | 47 | 23 | 52 | 62 | 17 | |
| items-nofooter | 200 | 1326 | 6.6 | 197 | 349 | 116 | 41 | 109 | 120 | 33 | |
| items-nofooter | 399 | 3027 | 7.6 | 408 | 1021 | 310 | 101 | 202 | 270 | 74 | |
| items | 51 | 555 | 10.9 | 51 | 53 | 20 | 8 | 21 | 33 | 11 | 255 |
| items | 100 | 1127 | 11.3 | 104 | 146 | 52 | 19 | 44 | 68 | 22 | 475 |
| items | 200 | 2521 | 12.6 | 234 | 415 | 130 | 39 | 116 | 143 | 46 | 962 |
| items | 399 | 5143 | 12.9 | 411 | 1046 | 324 | 102 | 199 | 278 | 87 | 1921 |

- `text` and `mixed` are **linear** (ms/page is flat within 5-10 %). Shaping is 37-39 % of `text` at every size, so no phase grows faster there. (`convert` and `draw` of `text` grow a little more than 2x from 200 to 400 pages, +24 % and +3 %.)
- The footers are linear too (5.0, 4.75, 4.8, 4.8 ms/page): no per-page re-layout of the body, no growth with the document.
- `items` is **superlinear in the table**: Taffy x2.5, x2.4, x2.9 per doubling (58 -> 143 -> 349 -> 1021 ms), Stylo style x2.2, x2.5, x2.7, pagination x2.9, x1.8, x2.5, while shaping and parse double. It is 4800 rows
  (24 000 cells) in one `<table>`; at 400 pages (9600 rows) Taffy + style are 1.33 s of the 3.03 s. Not traced further (Taffy table track sizing and Stylo style invalidation over 10^4 siblings are the suspects).
  At 200 pages the effect is moderate (6.6 vs 6.0 ms/page) and it does not change the ranking in §5.
- Fixed cost: a 1-page fixture is 21 ms wall, of which 14 ms is `render` (7 ms CLI setup and exit).

## 5. Candidate fixes, ranked

Gains are ms off the CLI wall of the three documents (items / text / mixed) on this VM. "Measured" means I built and ran it in the scratch tree; everything else is an estimate from the attribution
above (callgrind Ir shares, per-document timers) and is marked as such. Byte-identical means `cmp` of the PDF.

| # | Fix | Gain items / text / mixed | PDF bytes | Where | Size | Risk |
|--:|---|---|---|---|---|---|
| C1 | **Enable blitz-dom `parallel-construct`**: Parley shaping of the deferred inline layouts runs on rayon (rayon is already in the tree via Stylo and blitz-dom) | **measured: -114 / -226 / -168** (2443 -> 2329, 1124 -> 898, 1053 -> 885). Items also pays about +40 ms because 416 tiny margin documents wake the pool; a "parallel only above N tasks" guard should give -150 to -250 (estimate) | identical: the 3 b200 documents, `b200-text` at `RAYON_NUM_THREADS` 1, 2, 3, 4, and all 16 fixtures | one `features = ["parallel-construct"]` on the `blitz-dom` dependency of the fulgur fork (a standing-decision fork change); optional 3-line guard in the Blitz fork | XS | low: the speed-up saturates at about 2x on this 4-vCPU VM (shaping 444 -> 237-283 ms); a CI runner with 1-2 vCPUs gains less; memory not measured; fixtures within noise (19-61 ms) |
| C2a | **Stop re-parsing the UA and `--css` stylesheets and re-registering fonts per margin document**: one process-wide `SharedRwLock` in Blitz so a parsed `Stylesheet` can be shared between documents (cache keyed by origin and text), and one `FontContext` (cloned collection) instead of `register_fonts` per document | estimate **-600 to -650 / 0 / 0** (UA 0.55 + `--css` 0.7-0.9 + fonts 0.22 of 2.1 ms, x416), and about -0.5 ms on every 1-page invoice | identical by construction (same rules); to be verified with `cmp` | Blitz fork (`document.rs` L228 `SharedRwLock::new()`, `add_user_agent_stylesheet`, `process_style_element`) + fulgur `blitz_adapter::parse_inner` | M (about 150 lines) | medium: a process-global lock serialises stylesheet writers across threads; Stylo asserts that guards belong to the same lock |
| C2b | **Build the margin-box documents in parallel**: first collect, for all pages, the distinct (html, geometry) keys (known before painting), build and measure them on rayon, then paint sequentially | estimate **-450 to -550 / 0 / 0** (2-2.5x scaling as in C1); on top of C2a it acts on fewer, cheaper documents (together about -750) | identical (same documents, same paint order) | fulgur fork (`render.rs`: `MarginBoxRenderer::render_page` split into plan / build / paint) | M (about 150 lines) | low-medium: Blitz is thread-safe per document (fulgur `CLAUDE.md`); the gain depends on the core count |
| C2c | **Reuse one margin document per template** and change only the text node that holds the page numbers, instead of a new document per page | estimate **-850 to -900 / 0 / 0** (about 10 ms left per footer type) | must be shown with `cmp` on every footer in the corpus and fixtures: a mutated document may differ from a fresh one if Taffy caches or the inline layout of the text node are not invalidated | fulgur (`render.rs`) + Blitz mutator API | L | medium-high; the only fix that removes the cost class, hence last |
| C3 | **Taffy table track sizing and Stylo style over thousands of rows**: `minimum_contribution` lays out each of the 24 000 cells at min-content about 1.4 times (33 661 calls, 66 k Ir each); Taffy and style grow superlinearly with the row count | unknown; estimate -150 to -250 on items at 200 pages (the table part of Taffy 364 + style 118), more at 400 pages; 0 / 0 for text and mixed | identical if it is a pure cache; pixels may move if layout results change | Blitz fork `layout/table.rs`, possibly Taffy (not forked) | L | medium-high; needs its own profile step first (what is quadratic?) |
| C4 | **Cheaper font selection in Parley** (`FontSelector::select_font`, 1.1 M calls, 7.6 % of `b200-text` Ir, about 80 ms): cache the result for runs of characters of one script and style | estimate -30 / -80 / -60 | identical if the cache key is exact | Parley (no fork yet; the owner creates `subapavel/parley`) | M | medium; must keep the fallback order |
| C5 | **Krilla page finish**: zlib-rs deflate of the content streams is 9.6 % of `b200-text` (106 ms); a lower compression level, or deflating pages on worker threads | estimate -20 / -50 / -40 with level 1-3; threads similar | **not identical** for a level change (streams and file size change, goldens move); identical for threads | Krilla (not forked) or `SerializeSettings` in fulgur | S-M | low; moves bytes, so it needs its own byte-golden decision |

Not proposed, not measured: dropping OpenType features of Inter (`calt`, `ccmp`) would speed shaping (`apply_forward` is 28 % of `b200-text` Ir) but changes glyph selection and Chrome parity; a word-level
shaping cache changes kerning at word boundaries; subsetting the bundled Inter would renumber glyph ids. All three change output, so they are product decisions.

## 6. Recommendation

1. **First F99 PR: C1** (parallel construct). It is the only candidate that is measured, byte-identical on everything I could run, one line, and it takes two of the three documents under 1 s on the dev VM
   (`b200-text` 0.90 s, `b200-mixed` 0.89 s). Add the "parallel only when there are many tasks" guard in the same PR if `b200-items` or the 1-page fixtures show a loss (items showed +40 ms from the margin
   documents). Acceptance: the 16 fixtures and the three b200 documents `cmp`-identical at `RAYON_NUM_THREADS` 1 and 4, `scripts/bench.py` rows lower, baseline updated in the same PR, hard budgets unchanged.
2. **Second PR: C2a, then C2b if the footer is still over about 300 ms.** The footer is the largest single cost in the table (40 % of `b200-items`, 4.8 ms per page of any multi-page invoice with a page
   counter), and every invoice has one. C2a first, because it also helps every 1-page invoice (the UA sheet is parsed in every document) and does not depend on the core count.
3. **Third: C3**, after its own profile step (what is quadratic in the table) and a `vrt-validator` pass on pixels.
4. **Is 1 s reachable?** `b200-text` and `b200-mixed`: yes, with C1 alone on 4 vCPUs (0.90 s / 0.89 s), about 0.8 s with C4 and C5. `b200-items` (one 4800-row table with a page counter in the footer): after C1 and
   C2a + C2b I expect 1.4-1.5 s (2443 - 150 to 250 - about 750); C2c would give 1.3-1.4 s. Getting under 1 s would need C3 to remove most of Taffy + style (up to -350) on top, and then the floor of parse,
   shaping, convert and draw (rows 1, 5, 10, 12: about 0.6 s on this document) leaves no margin. If the owner wants a number: "200 pages of text and of mixed content under 1 s; one 4800-row table with
   footers under 1.5 s". Choosing the goal per document shape is product scope (AGENTS.md goal 2) and is not decided here.

## 7. Not verified

- Gains for C2a, C2b, C2c, C3, C4 and C5 are estimates; only C1 was built and run. Callgrind Ir shares were turned into ms with the per-document timers, not measured per candidate.
- C1 on other machines (1-2 vCPUs, a CI runner), peak memory with the per-thread font contexts, and rayon inside `render_batch` or the bindings were not measured. Byte-identity was checked on the three
  200-page documents and the 16 fixtures, not on `cargo test` or the bindings.
- The instrumented binary is the same commits plus timers. The VM is shared and the run-to-run spread was 2-8 % (`b200-items` 2266-2580 ms), so differences below about 5 % are noise.
- The cold first render (5.7 s once, `docs/benchmarks.md`) was not reproduced or investigated.
- The `ERROR: Unexpected token` line per margin document was not traced to its declaration.
- Debug info is line tables only, not `debug = true`; function-level inclusive costs are not affected, line-level attribution inside inlined Stylo code was not used.
