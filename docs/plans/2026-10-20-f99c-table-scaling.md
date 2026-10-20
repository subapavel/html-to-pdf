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

(to be filled)
