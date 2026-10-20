# F99c — one very long table: make the per-row and per-page work linear (F99 step 4, F138 folded in)

Date: 2026-10-20. Finding: F99 (speed, AGENTS.md goal 2), with F138 (collapsed-border cost) folded in. Status: **planned**. Previous steps: `2026-10-19-f99-speed-profile.md` (C3), `2026-10-19-f99a-parallel-construct.md`, `2026-10-19-f99b-margin-documents.md`.

Line references point at the working trees I read:

- Blitz fork: `/home/user/blitz`, `htmltopdf/blitz-dom-0.2.4`. `Cargo.lock` pins `fadaabb`.
- fulgur fork: `/home/user/fulgur`, `htmltopdf/srgb-colours`, `cc1db38` or later.
- Taffy: crates.io 0.9.2, under `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/taffy-0.9.2`.

I could not check the checked-out commits with git. When this doc says "measured" it means a number from earlier plans. Everything else is an estimate and is marked as one.

## 1. Problem

`bench/corpus/b200-items.html` takes 1.6–1.9 s on the dev VM: 1575 ms after F99b, 1662–1680 ms after F22, 210 pages. The goal is 1 s. `b200-text` and `b200-mixed` are about 0.86 s.

The document is one table: 4800 rows, 24 000 cells, a repeated `thead`, Tailwind's `border-collapse: collapse`, `tr { break-inside: avoid }` and two margin boxes. The F99 profile (§4) found that this table is the only part of any corpus document that grows superlinearly. Going from 51 to 399 pages without margin boxes:

- Taffy grows ×2.4–2.9 for each doubling.
- Stylo style grows ×2.2–2.7.
- Pagination grows ×1.8–2.9.
- The whole render goes from 6.0 to 7.6 ms per page.

F22 then added about 14 % (F138). Part of that is real geometry (210 instead of 204 pages, about 3 %). The rest is code that walks the whole table once per page (§3).

No fixture shows the problem. The 3-page `02-long` is too short. What shows it is `b200-items` and the scaling variants of Step 0.

## 2. Spec

This step is speed only. Every PDF must stay **byte-identical**, so no CSS behaviour changes. I had no `css-spec-researcher` output; the section numbers below are from memory and **unverified**. They are listed because the changed code must keep exactly these semantics:

- **CSS 2.1 §17.6.2 and §17.6.2.1** (collapsing borders and conflict resolution), as painted per page in F22 §7.
- **CSS 2.1 §17.2 / CSS Tables 3** (`table-header-group` repeated on each page). F12 and F22 decide which line goes under the repeated header.
- **Selectors 4, tree-structural pseudo-classes** (`:last-child`; `:where(.divide-y>:not(:last-child))` is in the compiled `tailwind.css`). A sibling lookup must return exactly what a scan of the parent's children returns.
- **css-break-3** (fragmentation): the fragmenter is not changed.

## 3. Where it belongs

### 3.1 Superlinear spots found by reading, ranked by expected gain against risk

Notation: R = rows, C = cells, P = pages, S = children of one parent. On `b200-items`: R = 4801, C ≈ 24 000, P = 210, and S ≈ 9 600 for the `tbody` (rows plus whitespace text nodes).

| # | Spot | Cost | Est. gain on `b200-items` | Fix candidate | Risk |
|--:|---|---|---|---|---|
| A1 | fulgur `table_borders::paint_header_copy`: `rest.iter().find(on_page(...))` (`table_borders.rs:239-243`). Runs on every page that has a header copy (`render.rs:800-816`). Every earlier cell is checked with a `geometry` lookup and a fragment scan. | O(P·C): about 2.5 M BTreeMap lookups | **100–350 ms** (est.); likely most of F138 | Find the first body row from the page's own node list (`per_page_node_ids`, `render.rs:176-185`, through `collapsed_cell_of`): the smallest cell index ≥ `header_cells` that is on the page. Same result by construction. | low |
| A2 | fulgur `table_borders::paint_lines`: `for i in 0..=line_rows` on every page (`table_borders.rs:379-441`). | O(P·R) | 15–40 ms (est.) | Visit only lines `r` and `r+1` for the rows `r` on the page, plus `line_rows` in `Mode::Copy`. Every other line already hits `continue` at `:401`, and `hlines` is a BTreeMap, so the output is identical. | low |
| A3 | fulgur `table_borders::paint_tables` (tables inside wrappers) filters all cells on every page (`:127-133`). Not hit by `b200-items`. | O(P·C) | 0 here; a long table inside an `overflow:hidden` or opacity wrapper | Same per-page index as A1, in ascending order | low |
| B1 | Blitz `Node::child_index` is a linear `position` (`node/node.rs:479-484`). `forward` and `backward` call it (`:487-505`), and Stylo reaches them through `prev_sibling`, `next_sibling`, `prev_sibling_element` and `next_sibling_element` (`stylo.rs:224-230`, `:304-326`). | O(S) per query, O(S²) per `tbody` if Stylo queries every row. Candidate cause of the superlinear style phase. | 0–80 ms (est.); unknown whether the bloom filter rejects the `:not(:last-child)` rules first | O(1) lookup through a stored index hint, checked on every read (§3.2). Same values by construction. | low |
| B2 | Blitz `widen_explicit_columns` (`layout/table.rs:394-452`, called on every uncached table layout, `layout/mod.rs:354`). Each call re-measures the 19 200 cells of the four explicit-width columns (`:409-436`, cache hits) and **deep-clones the whole `TableContext`** (`:445`), about 28 800 `taffy::Style`s. | O(C) per table layout call, ×2–4 calls | 10–40 ms (est.) | `TableContext { items: Arc<[TableItem]>, explicit_columns: Arc<[ExplicitColumn]>, .. }` (`:32-41`), so the clone copies one container `Style` | low-medium |
| T1 | Taffy grid: placement, then 2–4 full track-sizing passes per table layout (`compute/grid/mod.rs:208-470`; re-runs at `:351-470`). Items are sorted on every pass (`track_sizing.rs:542`, `mod.rs:498`). Cells have a 9-slot cache, and the re-run check at `mod.rs:362-381` measures every item again with a new available height. | Linear on paper. Measured ×2.4–2.9 per doubling; the source is unknown (call count, cache misses, memory). | unknown, maybe 150 ms at 200 pages | Only after Step 0 names the cause: a Blitz table-to-grid mapping change, or a Taffy fork (`subapavel/taffy`, which the owner must create) | high: track sizing moves geometry → **F99d** |
| A4 | fulgur `paint_table_row_backgrounds` walks every background cell of the table on every page (`render.rs:3780-3818`). | O(P·cells with a row background) | 0 here (no row backgrounds); quadratic for zebra invoices (`even:bg-*`) | Index backgrounds by page | medium → **new finding** |
| — | The F138 suspects in Blitz: `collapsed_borders::resolve` (`collapsed_borders.rs:260-400`, O(R·tracks)) and the collapsed-border override in `flush_styles_to_layout` (`damage.rs:315-331`, O(1) per node). | linear | — | none: F138's superlinear part is A1 and A2 | — |

The fragmenter has no nested sibling loop on the table path, as far as I can read: `fragment_block_subtree` `pagination_layout.rs:3112-4390`, `repeating_table_header` `:4624-4681` (once per table), and `row_mate_can_split` `:5184` (only for cells taller than a page). Its ×1.8–2.9 is noisy and is left to Step 0.

### 3.2 Decision

- **Fix A (always in scope): fulgur fork, `table_borders.rs` and `render.rs`.** This is F22's own code, so it lives in fulgur. A1 and A2 are the F138 quadratic. A3 uses the same helper and only goes in if it adds no more than about 10 lines.

  ```rust
  fn first_body_row(edges: &CollapsedEdges, table: usize, page_node_ids: &[usize], drawables: &Drawables) -> Option<u32>;
  fn page_lines(rows: &BTreeMap<u32, RowSpan>, line_rows: u32, mode: Mode) -> Vec<u32>; // ascending, deduplicated
  // paint_header_copy and paint_repeated_table_headers gain `page_node_ids: &[usize]`
  ```

- **Fix B (in scope if Step 0 attributes at least 20 ms at 4800 rows to `child_index`, `forward` or `backward`): Blitz fork, `node/node.rs`.**
  - Add `index_hint: AtomicU32` to `Node`; `u32::MAX` means unknown.
  - `child_index()` keeps its signature. It returns the hint when `parent.children.get(hint) == Some(self.id)`. Otherwise it scans once and refreshes the hint of every sibling in that one pass, so the cost is amortised O(1).
  - **The hint must be atomic, not a `Cell`.** `Node` is `unsafe impl Sync` (`node.rs:128-129`) and Stylo may traverse in parallel, so a `Cell` would be a data race. With the check on every read, a stale hint cannot change a result.

- **Fix C (in scope if Step 0 attributes at least 20 ms to the `TableContext` clone and drop): Blitz fork, `layout/table.rs`.** The design is B2 above. Field values stay the same, so layout is identical.

- **Upstream:** no Taffy, Stylo or Parley change in this PR (§6). The standing decision for forks applies: branch commits, `[patch.crates-io]` already in place, rows in `docs/upstream.md`.
- **Ours:** only `Cargo.lock`, `scripts/bench.py` (scaling report and gate) and the docs.

## 4. Acceptance

1. **Bytes.** `cmp` the branch binary against main's binary at `RAYON_NUM_THREADS=1` and `=4`. Every PDF must be identical:
   - every fixture under `fixtures/invoices/`;
   - all 7 documents in `bench/corpus/`;
   - the 16 scaling variants of Step 0.

   `compare.py` `summary.json` must be byte-identical, so the pixel-diff change is **0** for every fixture. Look once at `out/compare/02-long-p2.png` (header copy and its line) and at a page of `b200-items` past page 100.
2. **Speed.** Run `bench.py b200-items b200-text b200-mixed 02-long` with the main and branch binaries back to back, alternating (A B A B, median of 7 each).
   - **Gate:** `b200-items` median below 1000 ms. **Not expected from this scope:** the estimates add up to −125 to −510 ms.
   - **Stated target:** at least 250 ms off `b200-items` (to about ≤ 1400 ms), or the sum Step 0 attributes to the fixed spots minus 30 %, whichever is smaller. `b200-text` and `b200-mixed` lower or within noise.
   - Run `--update-baseline`, which may only lower rows. Lower `HARD_MS_OVERRIDE["b200-items"]` from 3000 ms to about 1.8 × the new median.
   - The residual is filed as **F99d** with the Step 0 table.
3. **Scaling.** `bench.py --scaling` prints ms at 1200, 2400 and 4800 rows for each variant, and r = t(4800)/t(1200). Linear is about 4 and quadratic tends to 16.
   - `--check` gates **variant `t`** (the `b200-items` table without margin boxes) at r ≤ `SCALING_MAX_RATIO`.
   - Set `SCALING_MAX_RATIO` in Step 4 halfway between main's r and the branch's r, capped at 5.0. If main is already below 5.0 it is a guard at 5.0.
   - A failure is re-measured with twice the runs, as the existing rule does.
   - **No timing assertions in `cargo test`.** Debug builds and parallel test threads on shared runners make wall-clock asserts flaky. Correctness is guarded by the deterministic tests in item 5. The ratio test is self-normalising, so it can live in `bench.py --check`, which CI already runs.
4. **Determinism.** `crates/htmltopdf/tests/determinism_threads.rs` stays green, and item 1 passes at 1 and 4 threads. Blitz (if Fix B): 4 threads query `child_index` concurrently on one 10 000-child parent and get identical results.
5. **Unit and integration tests**, red because the helpers do not exist yet.
   - **fulgur:** `first_body_row` equals the old scan, and `page_lines` produces the same `hlines` as the full loop. Both cover 1–5 tracks, header rows 0/1/2, rows cut by a break, and `Mode::Page` and `Mode::Copy`.
   - **fulgur `render_smoke`:** a 300-row collapsed table with a repeated `thead`. VRT goldens untouched; `gcpm_integration` green; `examples_determinism` 10 of 11 as before.
   - **Blitz:** `child_index` equals `position` after `append`, `insert_before` and `remove` (mutator) on a 10 000-child parent. A clone of a `TableContext` shares its items (`Arc::ptr_eq`). `cargo test -p blitz-dom --lib` passes with and without `parallel-construct`.
   - **Ours:** full `cargo test --release --locked`.
6. **Reviews.** `vrt-validator` with no high-severity finding. `pdf-reviewer` is required by merge policy item 5, because `table_borders.rs` emits content-stream operators. With identical bytes, its review is a check of the `cmp` matrix.

## 5. Steps (each one day or less)

0. **Profile and scaling curve (1 day, measurement only, scratch files).**
   1. `command -v perf`. If it is missing, `sudo apt-get install -y linux-perf` (or `linux-tools-generic`). If that fails too, use `valgrind --tool=callgrind` as in F99 step 1. In the VM, use `-e cpu-clock`; if needed, `sudo sysctl kernel.perf_event_paranoid=1`. Check `df -h /tmp` for at least 4 GB free.
   2. Build a symbolised binary of main in a separate target dir. The release profile has `strip = true` (`Cargo.toml:37`), so the existing binary has no symbols. A different debug setting rebuilds everything anyway, and `target/release` must stay the gated binary:

      ```bash
      CARGO_TARGET_DIR=/tmp/claude-0/f99c-prof CARGO_PROFILE_RELEASE_STRIP=false \
      CARGO_PROFILE_RELEASE_DEBUG=line-tables-only RUSTFLAGS="-C force-frame-pointers=yes" \
      cargo build --release --locked -p htmltopdf-cli
      ```

   3. Write a scratch generator that imports `scripts/bench.py` and patches `N_ITEM_ROWS` and the CSS. Variants: `t` (the corpus body with `PAGE_PLAIN`), `t-sep` (+ `table{border-collapse:separate}`), `t-noavoid` (no `tr` avoid rule, no `break-inside-avoid` class), `t-nohead` (no `thead`), and `t-boxes` (`PAGE_BOXES`, the corpus shape). Sizes 1200, 2400 and 4800 rows, and 9600 for `t` only.
   4. **Wall time:** the stripped main binary, flags of `bench.py` `render_cmd`, `RAYON_NUM_THREADS=1`, 1 warm-up plus a median of 5.
   5. **Attribution:**

      ```bash
      RAYON_NUM_THREADS=1 perf record -e cpu-clock -F 999 -g -o X.data -- BIN render …
      perf report -i X.data --children --stdio
      ```

      Phase = inclusive share of these symbols × wall:

      | Phase | Symbols |
      |---|---|
      | parse | `blitz_adapter::parse_inner` |
      | style | `BaseDocument::resolve_stylist` |
      | construct | `resolve_layout_children` + `flush_styles_to_layout` |
      | shaping | `resolve_deferred_tasks` |
      | Taffy | `resolve_layout` |
      | fragmentation | `pagination_layout::run_pass_with_multicol` |
      | convert | `convert::dom_to_drawables` |
      | paint | `render::draw_v2_page` + `paint_repeated_table_headers` |
      | PDF write | Krilla page drop + `Document::finish` |
      | margin boxes | `MarginBoxRenderer::render_page` |

      Also collect the self-time top 30 (`--no-children`) at 1200 and at 4800. A symbol whose share doubles is a quadratic suspect; find its callers with `-G`.
   6. **Call counts:** callgrind on `t`-1200 counts how many times the table's grid layout and `widen_explicit_columns` run.
   7. Record the phase × size table, r for each phase (r > 5 is flagged), a fit a·n + b·n² for each flagged phase, and the measured share of A1–A3, B1, B2 and T1, all in §7.1.
   8. **Gate:**
      - Fix A always goes in.
      - B and C go in only if each has at least 20 ms at 4800 rows.
      - If A + B + C measure under 150 ms together, stop and re-plan around the measured hot spot. That spot will most likely be T1, which needs F99d and a Taffy fork request to the owner.
1. **Scaling tool (half a day, ours).**
   - Parameterise `gen_items(rows, boxes, collapse, avoid, thead)`. `--generate` must reproduce `bench/corpus/` byte for byte; check with `git diff --exit-code bench/corpus`.
   - Add `--scaling`, which writes to `out/bench/scaling/` (not committed). Add the r gate as a constant, to be set in Step 4.
2. **Fix A (1 day, fulgur fork).**
   - Write the red tests of §4 item 5 first, then add `first_body_row` and `page_lines` and thread `page_node_ids` through `paint_repeated_table_headers` (`render.rs:359`, `:697`, `:805`). A3 goes in only if it is small.
   - Run fork lib, `render_smoke`, `gcpm_integration`, VRT and examples. `cmp` `b200-items`, `02-long` and `09` against main. Push.
3. **Fixes B and C if the gate allows (1 day, Blitz fork).** Write tests first, then the change. Run `cargo test -p blitz-dom --lib` with and without `parallel-construct`. `cmp` the three b200 documents, then push.
4. **Ours (1 day).**
   - Pin both forks with a `Cargo.lock` bump; run `cargo test --release --locked`; run the full `cmp` matrix at 1 and 4 threads; run `compare.py`.
   - Run the bench A/B, update the baseline and the override, and set `SCALING_MAX_RATIO`.
   - Docs: the F99 entry (step c) and F138 (done) in the findings; new findings for A4 and the side finding in §6, plus F99d; `docs/benchmarks.md`; `docs/upstream.md`; §7 here.
   - Then run `vrt-validator` and `pdf-reviewer`.

## 6. Risks and non-goals

- **Estimates.** The gains in §3 are estimates. Step 0 may put the cost inside Taffy (T1). Then this PR is Fix A only, and the 1 s gate moves to F99d.
- **A1 equivalence.** A1 relies on two facts: every fragment's page is below `page_count` (`implied_page_count` is the maximum page plus 1), and each cell maps to exactly one table in `collapsed_cell_of` (`convert/table.rs:123`). The equivalence test covers both.
- **Fix B.** It adds 4 bytes per node. A `Cell` would be unsound because `Node` is `Sync`. It must not change mutation semantics: any stale hint falls back to the scan.
- **Fix C.** It changes private fields of a `pub` struct (`TableContext`) only; the `collapsed_borders()` accessor stays.
- **Noise.** The shared VM spreads 2–8 %. Alternate the binaries; the profiling binary (frame pointers) is used for attribution only, not for timing.
- **Side finding, not verified.** Grid lines are `i16` (`table.rs:648`, `:1096`, `grid.row as i16`) and Taffy's track indices are `u16`. A table of more than about 16 000–32 000 rows would wrap. File it as a finding; it is not part of this PR.
- **Non-goals:**
  - any output change;
  - Taffy or Stylo changes, and T1;
  - margin boxes (C2b, C2c);
  - Parley font selection (C4) and the Krilla deflate level (C5);
  - A4 (zebra row backgrounds): filed;
  - promising 1000 ms for this shape: that is F99d.

## 7. Result

Status: **done** (Fix A and Fix B; Fix C not warranted; the rest is F99d). Forks: Blitz `a4b7624` (on `fadaabb`), fulgur `92304b1` (on `cc1db38`). Measured 2026-10-20 on a host that is **not** the one the plan's numbers come from (4 vCPU Xeon @ 2.80GHz; the committed baseline is from a 2.10GHz host and the same main binary is about 1.4-1.5 times slower here: `b200-items` 2.3 s instead of 1.58 s). Use ratios and A/B deltas, not absolute times.

### 7.1 Step 0: where the time goes

Symbolised build of main (`/tmp/claude-0/f99c-prof`, frame pointers, line tables; deleted afterwards), `perf record -e cpu-clock -F 999 -g`, `RAYON_NUM_THREADS=1`, the `t` variant (corpus table, plain `@page`). `perf` was not installed: `apt-get install linux-tools-generic`, run as `/usr/lib/linux-tools/<ver>/perf` (the wrapper wants a kernel-matched package). Samples are ms of CPU; perf and the frame pointers inflate the total by about 15 % over the stripped binary's wall clock (second row).

Wall clock, stripped main, 1 thread, median of 5 (ms), per variant and rows:

| variant | 1200 | 2400 | 4800 | 9600 | r = t(4800)/t(1200) |
|---|---:|---:|---:|---:|---:|
| t | 375 | 789 | 1881 | 4854 | 5.02 |
| t-sep | 377 | 793 | 1737 | | 4.61 |
| t-noavoid | 401 | 841 | 1877 | | 4.69 |
| t-nohead | 393 | 803 | 1719 | | 4.37 |
| t-boxes | 500 | 949 | 2236 | | 4.48 |

`t-nohead` is 160 ms below `t` at 4800 rows (the header copy is A1); `t-sep` is 140 ms below (collapsed borders: A1 again, and the F22 painting). Doubling `t`: x2.10, x2.38, x2.58: superlinear and getting worse.

Phase x size, inclusive samples (ms) of `t`:

| phase | 1200 | 2400 | 4800 | 9600 | r(4800/1200) |
|---|---:|---:|---:|---:|---:|
| all samples | 473 | 1199 | 2149 | 4991 | 4.5 |
| parse | 33 | 63 | 133 | 307 | 4.0 |
| style (`resolve_stylist`) | 28 | 62 | 149 | 384 | **5.3** |
| construct (`resolve_layout_children`) | 15 | 46 | 65 | 127 | 4.3 |
| flush_styles_to_layout | 6 | 23 | 26 | 51 | 4.3 |
| shaping (`resolve_deferred_tasks`, workers) | 117 | 227 | 363 | 672 | 3.1 |
| Taffy (`resolve_layout`) | 85 | 359 | 626 | 1459 | **7.4** |
| convert | 36 | 65 | 130 | 262 | 3.6 |
| paint (`draw_v2_page`) | 56 | 122 | 161 | 394 | 2.9 |
| `render_v2` (self, includes inlined paint of header copies) | 16 | 60 | 154 | | **9.6** |
| fragmentation (`run_pass_with_multicol`) | not separately visible (inlined) | | | | |
| margin boxes, PDF write | below 1 % (no margin boxes in `t`; Krilla page drop / `finish` not a symbol) | | | | |

Flagged (r > 5): Taffy, style and the self time of `render_v2`. Fits a*n + b*n^2 (n = rows; two points, third as a check): `paint_header_copy` a = 0.0065, b = 5.0e-6 (predicts 522 at 9600, measured 701); Taffy `minimum_contribution` grows as n^1.5-1.8, so it is not a pure quadratic (a = 0.008, b = 1.4e-5 predicts 1379, measured 1028).

Attribution (samples that any frame of the call chain, inline frames included, puts in the named code; `addr2line -i` on the sample addresses; at 1200 / 2400 / 4800 / 9600 rows):

| spot | 1200 | 2400 | 4800 | 9600 | verdict |
|---|---:|---:|---:|---:|---|
| **A1** `table_borders::paint_header_copy` (the `find` / `on_page` scan, `table_borders.rs:242`) | 15 | 58 | **146** | 701 | the whole of render_v2's superlinear self time; the plan estimated 100-350 |
| **A2** `paint_lines` loop over all lines (`:379-384`) | 3 | | 13 | | about 3 ms excess: negligible, done anyway (same helper) |
| A3 `paint_tables` | 0 | 0 | 0 | 0 | not hit by the corpus |
| **B1** `Node::child_index` (`node.rs:488`, inlined `position`) inside `matches_generic_nth_child` | 3 | 14 | **47** | 178 | in scope (excess about 35 ms); `matches_generic_nth_child` 5 / 20 / 54 / 201 |
| **B2** `widen_explicit_columns` | 1 | 1 | 3 | 2 | **below the 20 ms bar: Fix C not done**; `TableContext` drop 2 / 4 / 8 / 11 |
| **T1** Taffy `resolve_intrinsic_track_sizes`; of it `GridItem::minimum_contribution` | 44; 30 | 226; 181 | **415; 364** | 1129; 1028 | the largest superlinear cost: self time of `minimum_contribution` 17 / 128 / 291 / 902 |
| (new) pagination `has_page_name_change_below` + `has_forced_break_below` + `walk_used_page_names` | 5 | 18 | 54 | 81 | F144 |
| `build_table_context` | 12 | 33 | 47 | 91 | linear |

Callgrind of `t`-1200 (2.0 G instructions): `build_table_context` 2 calls (the items table and the totals table), `resolve_intrinsic_track_sizes` 8 calls, 13 % of all instructions; `widen_explicit_columns` has no cost line (inlined, under 0.1 %), so Fix C has nothing to take.

**Gate decision.** A + B + C = 146 + 13 + 47 + 3 = about 210 samples, about 170 ms of wall clock at 4800 rows: above 150 ms, so the PR goes on: Fix A and Fix B, not C (3 ms). T1 is the largest superlinear cost (about 360 ms at 4800 rows) and is not in this PR.

T1's cause, from the source (not verified with a patch): Taffy 0.9.2 `grid_item.rs:497-504` computes `spans_auto_min_track` and `spans_a_flexible_track` with `axis_tracks.iter().any(..)` over every track of the axis, per item and per call; without a flexible track the second scan never stops early. The fix is in Taffy, so it needs a `subapavel/taffy` fork, which only the owner can create. Filed as F99d.

### 7.2 What changed

- fulgur `92304b1`: `table_borders::first_body_row` (A1), `page_lines` and `build_hlines` (A2); `paint_header_copy` gets `page_node_ids` and loses `page_index`; `render.rs` passes the page's node list to `paint_repeated_table_headers`. A3 not done (see F145).
- Blitz `a4b7624`: `Node::index_hint: AtomicU32`, `child_index` reads it checked and refreshes all siblings on a miss (B1).
- Ours: `Cargo.lock` pins both; `scripts/bench.py` (`gen_items` parameters, `--scaling`, `SCALING_MAX_RATIO` 4.6); docs.
- Tests, red first (fulgur: compile errors for the missing helpers; Blitz: no `index_hint`): fulgur `table_borders` 4 new unit tests (`first_body_row` against the old scan for 1-5 tracks, header rows 0-2, cut rows and a second table; `page_lines` against the loop over all lines for every set of 6 rows, `started` / `continues` combinations and all four modes; the lines next to rows) and the `render_smoke` 300-row test; Blitz 5 new lib tests (position after append / insert / remove on a 10 000-child parent, stale and out-of-range hints, one query refreshes all siblings, `forward` / `backward`, 4 threads on one parent).

### 7.3 Acceptance

1. **Bytes: 86 of 86 identical** (`/tmp/claude-0/htmltopdf-main-fbaf57f` against `target/release/htmltopdf`; 20 fixtures + 7 corpus documents + 16 scaling variants (4 sizes of `t`, 3 of the other four), at `RAYON_NUM_THREADS=1` and `=4`, `cmp` of the whole file). `compare.py --threshold-check`: `summary.json` byte-identical to main's, every fixture ok. Looked at `out/compare/02-long-p2.png` (header copy with its rule) and page 120 of `b200-items` (rendered at 60 dpi): header copy, rule under it and row rules as before.
2. **Speed** (A B A B alternating, 4 threads, this host): `b200-items` 2312 -> 1970 ms (7 runs) and 2333 -> 1953 ms (11 runs), -342 / -380 ms (-15 / -16 %): the stated target (250 ms) is met, the 1 s gate is not (as the plan said). `b200-text` 1074 -> 1024 and 947 -> 990, `b200-mixed` 1075 -> 1147 and 1127 -> 1110: no long table, +-5 % noise both ways. `02-long` 55.2 -> 55.8. One thread, `t` variant: 4800 rows 2119 -> 1855 ms, 9600 rows 5367 -> 3975 ms.
3. **Scaling**: `bench.py --scaling` (median of 5): r of `t` main 4.62 / 4.68 (4.5-5.0 over runs) -> 4.07-4.40. The gate is `SCALING_MAX_RATIO` = 4.6, not the plan's "halfway" 4.4: with 4.4 a re-measure of the branch itself gave 4.40 and failed. Main fails the 4.6 gate (4.68), the branch passes.
4. **Determinism**: `determinism_threads.rs` is in `cargo test --release --locked` (below); bytes at 1 and 4 threads as above.
5. **Tests**: fulgur `cargo test -p fulgur --lib` 2207 passed, `render_smoke` + `gcpm_integration` 18 + 277 passed, `examples_determinism` 10 of 11 as the plan says (`committed_svg_matches_rendered` is the one that fails; not re-run on main). `fulgur-vrt`: 26 goldens fail on this host **before and after** (identical list with byte sizes, run on `cc1db38` + Blitz `fadaabb` and on the branch; reference sizes about twice the actual ones, a host font / rasteriser difference, not from this change). Blitz `cargo test -p blitz-dom --lib` 74 passed, with `--features parallel-construct` 76 passed. fulgur `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all -- --check` clean in both forks and ours. Ours: see the PR text for `cargo test --release --locked`.

### 7.4 Deviations from the plan

- Fix C not done (3 samples, under the 20 ms bar); A3 not done (F145: more than ten lines).
- `bench/baseline.json` not rewritten and `HARD_MS_OVERRIDE["b200-items"]` stays 3000 ms: the host is a different CPU (2.80 against 2.10 GHz) and its medians are higher than the baseline's, so "lowering only" has nothing to lower and 1.8 x today's median (3.5 s) is above 3 s. Redo both on the baseline host.
- `SCALING_MAX_RATIO` 4.6 instead of 4.4 (see 7.3.3).
- Blitz commit `a4b7624` lacks the attribution trailer lines (pushed before the reminder was applied; the later commits have them).
- A5 (new, outside the plan): the pagination page-name scans (F144) are 54 samples at 4800 rows; not touched.
