# F99f step 1 - fresh speed profile (2026-10-27, measurement only)

Base: main `ab643e7` (branch `f99f-speed-profile`), release CLI `d686c1a0...` (sha256 of `target/release/htmltopdf`, unchanged at the end). fulgur fork `3fa6f44`,
Blitz fork `fd50eee`, Taffy fork `7bbedd5`, krilla 0.7.0, parley 0.6.0, harfrust 0.3.2. Host: 4 vCPU Xeon @ 2.10 GHz, run-to-run spread 5-10 %.
No tracked file was edited, nothing committed, /home/user/fulgur and /home/user/blitz untouched.

## 0. Result in six lines

- `b200-items` 1276 ms (bench median of three runs; baseline 1190), `b200-text` 812, `b200-mixed` 808. AGENTS.md goal 1000 ms: met by text and mixed, missed by items by ~275 ms.
- `b200-items` without its two `@page` margin boxes takes **1008 ms** (A/B interleaved, 15 pairs, median 1232 vs 1008): **the margin boxes cost 224-247 ms wall**, i.e. 18-20 % of the document. That is the largest single item.
- Wall is set by the **main thread**: 5.05 G of 7.41 G Ir (68 %) run on it (callgrind, 1 rayon thread), the rest is shaping and deflate on workers. Wall barely changes with rayon threads (items 1 thread 1450 / 2 threads 1292 / 4 threads 1354 ms; text 994 / 796 / 824).
- Main-thread wall of `b200-items` (gdb stack sampling, 841 samples): layout (Taffy + Parley line breaking) 23 %, margin boxes 17 %, convert 12 %, paint 9 %, parse 7 %, waiting for shaping 7 %, Stylo 6 %, whole-tree walks/drops 8 %.
- Things added since F99e (F178 `check_font`, F186 borders, F184 canvas, F37 body slices, F180 patterns, F152 SVG fonts) are **not visible**: each <= 1.9 M Ir of 7 400 M (<0.03 %) on these corpus documents (they are not in the corpus shapes; see §5).
- New visible costs that predate or are independent of today's changes: five full-document scans in the `htmltopdf` crate before the engine starts (263 M Ir, 3.5 % of Ir), per-glyph `Arc<Font>` clone/drop in krilla (`b200-text`), ~490 M Ir of repeated inline-layout clone + line breaking in table cells.

## 1. Commands and raw numbers

```text
cargo build --release --locked --offline -p htmltopdf-cli          # no-op: up to date
python3 scripts/bench.py b200-items b200-text b200-mixed 01-basic 02-long   # x3, 7 runs each (outputs: bench-run{1,2,3}.txt)
cp target/release/htmltopdf scratchpad/htmltopdf.normal            # safety copy
CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --release --locked --offline -p htmltopdf-cli   # 4m22s, same target dir
cp target/release/htmltopdf scratchpad/htmltopdf.prof              # 117 MB, copy kept outside target/
cargo build --release --locked --offline -p htmltopdf-cli          # 0.2 s: normal binary back, sha256 d686c1a0... identical to before
RAYON_NUM_THREADS=1 valgrind --tool=callgrind htmltopdf.prof render <doc> ...   # b200-items, b200-text, b200-items-nobox
gdb -batch -x sampler.py (stack sampling of every thread, one-shot SIGUSR1 timer 20 ms after each resume)   # 15 + 10 + 8 runs
```

Tools: `valgrind` 3.22 yes; `perf` no. The profiling binary produced the byte-identical PDF to the normal binary (sha256 abd74076...). Disk was 3.6 GB free before and 2.0 GB after (profile deps live in the existing target dir, 8.1 GB; the
normal artifacts were kept, so the final rebuild was 0.2 s). `b200-items-nobox.html` = `bench.gen_items(boxes=False)` (the corpus table with a plain `@page`; `gen_items()` reproduces the committed corpus byte for byte).
Helper scripts (scratchpad): `run.sh`, `tm.py` (wall + rusage), `cg/cgparse.py`, `cg/q.py` (callee tables from callgrind files), `cg/clean.py`, `classify.py`, `sampler.py`, `pinger.py`, `samp.sh`.

### 1.1 bench.py, three runs (ms; median of 7 per run; the middle column is median of the three medians)

| document | pages | run 1 | run 2 | run 3 | median of 3 | baseline.json |
|---|--:|--:|--:|--:|--:|--:|
| 01-basic | 1 | 18.1 | 18.4 | 17.3 | 18.1 | 15.0 |
| 02-long | 3 | 33.2 | 30.3 | 34.0 | 33.2 | 29.4 |
| b200-items | 210 | 1360.8 | 1276.1 | 1271.8 | **1276** | 1190.1 |
| b200-text | 200 | 785.7 | 811.6 | 837.9 | **812** | 787.0 |
| b200-mixed | 203 | 786.2 | 816.1 | 807.8 | **808** | 785.0 |

Mins: items 1160 / 1154 / 1230; text 754 / 763 / 766; mixed 739 / 729 / 768. All `ok` against the gate. Today the machine is ~3-8 % slower than when the baseline was written (01-basic 18 vs 15 ms) - treat the baseline as the reference for the gate, these numbers for this profile.
Start-up `htmltopdf --version` 2.3-2.7 ms.

### 1.2 Rayon threads (normal binary, median of 7, rusage CPU = user+sys)

| document | threads | wall med (min-max) | CPU | peak RSS |
|---|--:|---|--:|--:|
| b200-items | 1 | 1450 (1233-1655) | 1500 | 247 MB |
| b200-items | 2 | 1292 (1237-1399) | 1483 | 247 |
| b200-items | 4 | 1354 (1248-1679) | 1676 | 247 |
| b200-items-nobox | 1 | 1163 (1121-1237) | 1204 | 245 |
| b200-items-nobox | 4 | 999 (952-1382) | 1295 | 245 |
| b200-text | 1 | 994 (969-1012) | 1097 | 126 |
| b200-text | 2 | 796 (793-835) | 1160 | 127 |
| b200-text | 4 | 824 (771-861) | 1663 | 128 |
| b200-mixed | 1 | 989 (886-1024) | 1041 | 177 |
| b200-mixed | 4 | 804 (751-899) | 1279 | 178 |

Two threads are as good as four on this VM; four buys 40 % more CPU for no wall time.

### 1.3 Margin-box cost, A/B interleaved (15 pairs, 1 run each, 4 threads)

`b200-items` median 1232 / min 1183; `b200-items-nobox` median 1008 / min 936. Difference **224 ms (median) / 247 ms (min)**. Callgrind agrees: 7 409 M Ir with, 6 543 M without (866 M, 11.7 %); sampling says 17 % of the main thread.

## 2. Where the time goes - `b200-items`

### 2.1 Callgrind (1 rayon thread; Ir, not time; 7 409 M Ir, was 14 710 M in F99 and 7 740 M before F99e)

Thread split: main thread 5 049 M (68.2 %), the single rayon worker 2 358 M (31.8 %: shaping 1 931 M + page deflate 426 M). `Engine::render` 4 786 M; the rest of `main` is the `htmltopdf` crate (263 M).

Top 15 inclusive functions (merged threads; percentages of total Ir; "recursion" entries are the inner `'2` frames):

| # | function | Ir | % | note |
|--:|---|--:|--:|---|
| 1 | `Engine::render` | 4786M | 64.6 | main thread only |
| 2 | `Engine::layout_to_drawables` | 3229M | 43.6 | |
| 3 | `BaseDocument::resolve` (x438 calls) | 2642M | 35.7 | 1969 M main document + 672 M of 437 margin documents |
| 4 | `run_construction_task` (Parley shaping, worker) | 2072M | 28.0 | `TreeBuilder::build_into` 1907 M (25.7), `shape_item` 1766 M (23.8), `hb_font_t::shape_with_plan` 1235 M (16.7), `apply_forward` 978 M (13.2) |
| 5 | `Engine::render_artifacts` | 1557M | 21.0 | |
| 6 | `render_v2` | 1517M | 20.5 | `render_page` 844 M + `draw_v2_page` 590 M |
| 7 | `resolve_stylist` (x438) | 1193M | 16.1 | 686 M main + ~507 M margin documents |
| 8 | `resolve_layout` (x438) | 1138M | 15.4 | main doc 1122 M, margin documents ~16 M |
| 9 | `style::parallel::style_trees` | 1019M | 13.7 | |
| 10 | `TableTreeWrapper::compute_child_layout` | 869M | 11.7 | 105 706 calls for 24 000 cells (4.4 per cell) |
| 11 | `MarginBoxRenderer::render_page` (x210) | 844M | 11.4 | 437 `resolve`s 672 M, `MarginDocEnv::parse` 86 M, doc drop 40 M |
| 12 | `draw_v2_page` | 609M | 8.2 | `dispatch_page_node` 588 M, krilla `draw_glyphs` 456 M |
| 13 | `track_sizing_algorithm::<TableTreeWrapper>` | 556M | 7.5 | `minimum_contribution` 293 M |
| 14 | `dom_to_drawables` | 517M | 7.0 | table cells 399 M (`extract_paragraph` 247 M, `extract_block_style` 95 M) |
| 15 | `HtmlDocument::from_html` | 488M | 6.6 | main 400 M + margin documents 86 M |

Others worth knowing: `blitz_dom::layout::inline::break_lines` 447 M (6.0 %; 105 718 calls: `break_next` 218 M, `finish` 209 M), `krilla::stream::deflate_encode` 430 M (5.8 %, worker), `Box<TextLayout>::clone` 66.5 M + drop 75.1 M
(81 654 times each, inside `compute_node_layout`), `column_css::walk` 85 M, GridItem `sort_by_key` ~150 M, `abspos_boxes_in_tree_order::walk` 45 M, `drop_glue<HtmlDocument>` 94 M, `drop_glue<Drawables>` 35 M, `CascadeData::add_stylesheet` 167 M (all in margin documents; main document 0.4 M).

### 2.2 Phases of `b200-items` (wall, main thread, 4 rayon threads)

gdb stack sampling (`sampler.py`): 15 runs, 841 samples of the main thread, uniform in run time (one-shot 20 ms timer after each resume); shares x the 1300 ms median. Caveats: stops perturb caches, the first samples after
symbol loading are dropped by construction, a signal may skid by one instruction, +-1 sample = 0.12 % = 1.5 ms; treat each ms figure as +-30 %. The `Ir` column is callgrind (1 thread), for cross-checking.

| phase (main thread) | samples | share | ~ms of 1300 | Ir share |
|---|--:|--:|--:|--:|
| layout: Taffy + Parley `break_lines` (main doc) | 192 | 22.8 % | 297 | 15.2 % |
| convert (`dom_to_drawables`) | 98 | 11.7 % | 151 | 7.0 % |
| paint pages (`draw_v2_page`) | 75 | 8.9 % | 116 | 8.2 % |
| parse: html5ever + UA/`--css` sheets (main doc) | 62 | 7.4 % | 96 | 5.4 % |
| waiting for Parley shaping on rayon (main doc) | 60 | 7.1 % | 93 | (workers) |
| Stylo style (main doc) | 54 | 6.4 % | 83 | 9.3 % |
| `layout_to_drawables` rest: inline-style walk, memcpy/free, doc drop | 48 | 5.7 % | 74 | ~4 % |
| fulgur pagination + abspos passes | 22 | 2.6 % | 34 | 1.6 % |
| `render_v2` rest (krilla page drop/finish, waits for deflate) | 22 | 2.6 % | 34 | 0.3 % + wait |
| `column_css::walk` + DOM passes | 12 | 1.4 % | 19 | 1.2 % |
| `render_artifacts` rest (drop of Drawables) | 11 | 1.3 % | 17 | 0.5 % |
| `htmltopdf` pre-scans (`prepare`, `check_images`, `check_fonts`, ...) | 10 | 1.2 % | 15 | 3.5 % (263 M) |
| Blitz box construction | 10 | 1.2 % | 15 | 1.7 % |
| process start-up (ld.so, first page faults) | 16 | 1.9 % | 25 | - |
| cli rest | 3 | 0.4 % | 5 | - |
| **margin boxes, total** | 146 | **17.4 %** | **226** | 11.4 % |
| - Stylo (stylist build of the 20 KB sheet + restyle) | 67 | 8.0 % | 104 | 6.8 % (507 M) |
| - Parley shaping, serial (below the parallel threshold) | 24 | 2.9 % | 37 | |
| - parse (`MarginDocEnv::parse`, html5ever) | 22 | 2.6 % | 34 | 1.2 % |
| - document drop | 12 | 1.4 % | 19 | 0.5 % |
| - other | 10 | 1.2 % | 15 | |
| - paint, convert, layout | 11 | 1.3 % | 17 | |

Margin-box total agrees with the A/B wall difference (224-247 ms). `b200-items-nobox` (8 runs, 347 samples, 1000 ms): layout 28.5 % (285 ms), convert 12.7 % (127), paint 12.4 % (124), parse 9.5 % (95), shaping wait 8.9 % (89), Stylo 7.5 % (75).

### 2.3 `b200-text` for comparison (10 runs, 346 samples, x 800 ms; callgrind 7 288 M Ir, was 7 490 M in F99, main thread 2 653 M / worker 4 634 M)

| phase | share | ~ms | note |
|---|--:|--:|---|
| waiting for Parley shaping on rayon | 27.5 % | 220 | worker Ir 3 916 M: `shape_item` 3 517 M (48 % of all Ir), `shape_with_plan` 2 432 M, `select_font` 567 M, GPOS 17 %, GSUB 14 % |
| paint pages | 26.0 % | 208 | krilla `draw_glyphs` 889 M Ir for 1 118 381 glyphs (`fill_stroke_glyph_span` 578 M, `check_text_span_prop` 253 M); **48 of 90 paint samples are in `Arc<Font>` clone/drop** (see C3) |
| layout (Taffy + `break_lines` 744 M Ir) | 20.5 % | 164 | |
| convert | 13.6 % | 109 | |
| parse + Stylo | 4.9 % | 39 | |
| page deflate (worker, 725 M Ir) | - | - | only the wait at the end shows: `render_v2` rest 2.0 % = 16 ms |

Font subsetting / PDF write (`krilla::Document::finish`) is 18-20 M Ir (~4 ms) in both: not a cost.

## 3. Comparison with the F99 profile (2026-10-19)

| F99 phase (items, ms of 2443) | F99 | now (est. ms of ~1300) | what moved |
|---|--:|--:|---|
| 1 parse | 98 | 96 (+34 in margin docs) | unchanged for the main doc; margin-doc parse was 215 x 1.3 ms, now 437 x ~0.08 ms (F99b shared sheets) |
| 3 Stylo style | 118 | 83 (+104 in margin docs) | main unchanged; margin docs: UA/`--css` parse gone (F99b), stylist build + restyle remain |
| 4 box construction | 56 | 15 | |
| 5 Parley shaping | 255 | 93 waiting (+37 serial in margin docs) | F99a rayon |
| 6 Taffy layout | 364 | 297 | F99d `minimum_contribution`; table track sizing 17 % -> 7.5 % of Ir; line breaking now the dominant part |
| 7-9 passes (multicol, pagination, abspos) | 102 | 53 + 19 | F144 scans gone |
| 10 convert | 108 | 151 (sampled; Ir 517 M) | not comparable ms-for-ms (timers vs sampling); Ir share 7.0 % |
| 12 draw_v2_page | 149 | 116 | |
| 13 **margin boxes** | **963** | **226** | F99b: -77 %; still the biggest single item |
| 14 krilla page finish (deflate) | 44 | 0 on main (34 wait) | F99e A |
| 15 Document::finish | 2 | ~4 | |
| **wall** | **2443** | **~1276** | -48 % |

Ir: items 14 710 M -> 7 409 M, text 7 490 M -> 7 288 M, so text/mixed gained almost nothing in work since F99; their wall gain (1124 -> 812) is F99a/F99e parallelism.
F99's top-15 list items that are gone: `Stylesheet::parse_rules` 19.2 % -> 0.2 %, `parse_inner` 30.4 % -> `from_html` 6.6 %, `render_page` 36.5 % -> 11.4 %.

### 3.1 Cost of today's changes (callgrind, Ir; "visible?" = anything above noise)

| change | where | Ir on items / text | visible? |
|---|---|---|---|
| F178 `check_font` per registered font | `font_check::check_font` | 1.9 M (6 calls) / 2.6 M | no (0.03 %, ~0.4 ms) |
| F186 border side colours | `draw_primitives::draw_border_line`, `table_borders::*` | `draw_border_line` 1.9 M; `table_borders::paint_table` 28.6 M (the F22 collapsed-border paint, 0.4 %, `page_rows` 11.6 M) | no |
| F184 canvas background | no symbol survives (inlined) | < 1 M | no (the corpus documents have no root background) |
| F37 body slices | `body_box_slices` | 0.01 M | no |
| F180 background patterns | `draw_background` | 0.7 M | no |
| F187 abspos frame | `abspos_boxes_in_tree_order::walk` + `fragment_abspos_flows` | 45 M + 47 M (1.2 %; one full-DOM walk, known F149) | small |
| F152 SVG fonts | fontdb per SVG parse | 0 (no SVG in the corpus) | not measured; only matters with SVGs |
| `htmltopdf` 5 pre-scans (`prepare` x3 closures, `check_fonts`, `check_images`) | `crates/htmltopdf/src/lib.rs`, `fonts.rs` | 5 x ~40 M = 200 M + 63 M other = 263 M (3.5 % of Ir; sampled 1.2 % = ~15 ms) | **yes, small**: each scans the 1.9 MB HTML with `CharSearcher` and allocates 57 686 times; text 26 M + 3 x 6.6 M |

The corpus does not exercise F186/F184/F37/F180/F152 shapes, so "not visible" means "no cost on documents that do not use them"; their cost on documents that do was not measured (fixtures are 15-35 ms each and dominated by start-up).

## 4. Candidates (next F99 steps), ranked

Estimates are mine (Ir shares converted with the sampled ms/Ir of the phase), marked est. Nothing was built.

| rank | id | change | where | est. saving items / text / mixed | PDF bytes | risk | size |
|--:|---|---|---|---|---|---|---|
| 1 | **M1** | **Margin boxes off the main thread**: first collect the distinct (html, geometry) keys of all pages, build + measure + convert the 437 documents on rayon (needs a `Sync` margin environment, F99e §6 note), paint sequentially. Alternative with less concurrency: one document per page for both the measure and the render stage (210 + 227 documents today). | fulgur fork `render.rs` `MarginBoxRenderer::render_page`, `blitz_adapter::MarginDocEnv` | **-100 to -170 ms** / 0 / 0 (margin boxes cost 224-247 ms; the work is independent per page and can overlap with the main thread's paint and with the idle 2-3 cores) | identical by construction (same documents, same paint order); verify with `cmp` on the 16+ fixtures and corpus at 1/2/4 threads | low-medium: per-document Blitz is thread-safe, one shared stylesheet lock (F99b) must become `Sync`; gain depends on core count (0 on 1 vCPU) | M |
| 2 | **L1** | **Memoise ComputeSize inline measurement**: `compute_inline_layout` clones the whole `TextLayout` and runs `break_lines` for every `ComputeSize` request (81 654 clones + 105 718 line breaks for 24 000 cells; the code has a `FIXME: avoid the need to clone`). Key a one-entry cache on the node by the resolved `width` (after the min/max-content clamp, which makes all `Definite(limit >= max)` queries equal), `nowrap`, `scale` and the inline-box sizes (skip the cache when the layout has inline boxes). | Blitz fork `layout/inline.rs` (+ a field on `TextLayout`) | **-60 to -120 ms** / -10 to -30 (text has 2 561 breaks, 744 M Ir) / -10 to -30 | identical if the key is exact (pure function of layout, width, nowrap, inline box sizes); verify with `cmp` + the 7 corpus documents + the 16 scaling variants | low-medium: invalidation when the layout is rebuilt (new `TextLayout` = fresh cache) and when inline boxes change size | S-M |
| 3 | **S1** | **Main-thread leftovers bundle** (all byte-identical, no fork needed for the first two): (a) the 5 full-document scans in `htmltopdf` fused into one pass without per-tag allocation (-15 to -40 ms; 263 M Ir); (b) drop the Blitz document and the `Drawables` on a rayon worker (or `mem::forget` in the CLI path) while the first page is painted (-25 to -35 ms: `drop_glue<HtmlDocument>` 94 M + frees + `drop_glue<Drawables>` 35 M); (c) fuse `column_css::walk` (85 M) and `walk_for_inline_styles` into the existing DOM pass (-10 to -15 ms); (d) Taffy fork: sort `GridItem` indices instead of the 250-byte items, or skip the sort when already ordered (~150 M Ir, -15 to -25 ms) | `crates/htmltopdf` (a); fulgur fork (b, c); Taffy fork (d) | **-65 to -115 ms** / -20 to -40 / -20 to -40 | identical | very low (a, c), low (b: `Send`), low (d) | S each, one PR per finding |
| 4 | **K1** | krilla per-glyph overhead: `CIDFont::font()` is called 3 x per glyph (3.36 M calls on text) and clones/drops an `Arc<Font>` (atomic RMWs; 48 of 90 sampled paint stops on text), plus `check_text_span_prop`/`add_glyph`/`get_codepoints` per glyph (253 M Ir). Hoist per run. Needs a `subapavel/krilla` fork (owner creates it, standing decision). | krilla 0.7.0 `text/` + `content.rs` | -20 to -40 / **-60 to -110** / -40 to -70 (text paint is 26 % of its wall) | identical (same content stream) | low; touches the PDF writer path -> `pdf-reviewer` | S-M |
| 5 | **M2/C2c** | one margin document per template, mutate the page-number text node, re-resolve (removes parse, stylist build, drop: ~170 of the 226 ms) | fulgur + Blitz mutation API | -150 to -190 / 0 / 0 | must be proven with `cmp`; Taffy/Parley caches | medium-high | L |
| 6 | **C4/F99 C4** | cheaper `FontSelector::select_font` in Parley (567 M Ir = 7.8 % on text, 205 M on items) | Parley (needs `subapavel/parley`) | -10 / -50 / -40 | identical if exact | medium | M |

Reaching 1000 ms on `b200-items` needs about -275 ms (median) from 1276: M1 + L1 + S1 give an estimated -225 to -405 ms, so it is within reach but not certain; the nobox
document is already at 1008 ms, so any plan that leaves the margin cost at 224 ms cannot get there from the layout/convert side alone. text and mixed already meet the goal; K1 (+ C4) would give them ~50-100 ms of margin.
Recommended order: **S1(a,b)** first as a quick safe win in our own repo, **L1** (largest main-thread item left, pure memo), then **M1**; K1 once the owner has created the krilla fork.

## 5. Not verified / caveats

- ms figures for phases are sampled shares x median wall; gdb stops change cache state and the per-phase error is about +-30 %. The only direct wall measurements are §1.1-1.3 (bench, thread sweep, margin-box A/B).
- Callgrind ran with `RAYON_NUM_THREADS=1`, so the worker/main split differs from 4 threads (shaping spread over 4 workers); Ir is not time (shaping has high IPC, layout/convert/drops lower), which is why Ir shares and sampled shares differ (e.g. convert 7 % Ir vs 12 % wall).
- Candidate savings are estimates; none was built. L1 hit rate (how many of the 81 654 ComputeSize requests repeat a width) was not measured.
- Not profiled: `b200-mixed` (only wall), the 1-page invoice start-up path (18 ms; F179 pool start-up not re-measured), slow-* documents, other machines, peak memory under the candidates.
- Today's changes were quantified from the three corpus shapes only; fixtures using backgrounds/borders/SVG/canvas were not profiled. F152 per-SVG fontdb cost is unmeasured.
- Sampling attribution of `Arc<Font>` clone/drop (K1) may include skid from the following instruction; the callgrind call counts (3.36 M `CIDFont::font` calls for 1.12 M glyphs) support it but atomics cost is invisible in Ir.
- Side observation: no `ERROR: Unexpected token` lines were checked this time (stderr discarded in the profile runs).
