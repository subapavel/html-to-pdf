# F99d: one very long table, made linear in Taffy's grid minimum contribution

Date: 2026-10-20. Finding: F99d (speed, AGENTS.md goal 2), step 5 of F99. Status: **planned**. Previous steps: `2026-10-20-f99c-table-scaling.md` (§7.1 has the Step 0 profile this plan uses).

Line references are to the working trees I read. I could not check the commits with git.

- Taffy fork: `/home/user/taffy`, `subapavel/taffy` `htmltopdf/minimum-contribution`, on the v0.9.2 release commit `8f30e39`.
- Blitz fork: `/home/user/blitz`.
- fulgur fork: `/home/user/fulgur`.

## 1. Problem

`bench/corpus/b200-items.html` is one table with 4800 rows and 24 000 cells. After F99c it still takes 1953–1970 ms on the 2.80 GHz host. That is about 1.3 s on the baseline host (an estimate), and the goal is 1 s. Its largest cost that grows faster than the document is Taffy's grid track sizing. In the F99c perf profile (1 thread, `--scaling` variant `t`, at 1200 / 2400 / 4800 / 9600 rows):

- `GridItem::minimum_contribution` inclusive samples: 30 / 181 / 364 / 1028.
- Its self time: 17 / 128 / 291 / 902, which grows as about n^1.5–1.8.
- At 4800 rows that self time is 291 of 2149 samples.

No invoice fixture shows the problem: `02-long` has 60 rows. What shows it is `b200-items` and the `t` variant of `bench.py --scaling`.

**Cause, confirmed by reading (§3.1); not yet measured with a patch.** For every grid item, `minimum_contribution` checks two facts about **every** track of the axis. One of the two checks never stops early, so one block-axis pass costs items × row tracks.

## 2. Spec

This step changes speed only, and every PDF must stay byte-identical. The references below are only the semantics the changed code must keep. They are cited from memory and are **unverified**; I had no `css-spec-researcher` output.

- **css-grid-1 §6.6, Automatic Minimum Size of Grid Items.** The content-based minimum applies if the item "spans at least one track … whose min track sizing function is auto" and, "if it spans more than one track …, none of those tracks are flexible". The Taffy source quotes this text at `grid_item.rs:489-502`.
  - Taffy 0.9.2 checks **all** tracks of the axis, not the spanned ones. That deviates from the spec, and we **keep it**: changing it would move pixels (§6).
- **css-grid-1 §11.5, Resolve Intrinsic Track Sizes**, step 2 (non-spanning items) and steps 3–4 (spanning and flexible items). These steps call the minimum contribution. Their order and their calls into the layout tree must stay the same.
- **CSS 2.1 §17 / CSS Tables 3.** Blitz lays tables out as a grid: `blitz-dom` `layout/table.rs:182-192`. That mapping is not touched.

## 3. Where it belongs

### 3.1 The cause, read in Taffy 0.9.2

1. **Call path.**
   - `resolve_intrinsic_track_sizes` (`track_sizing.rs:523-934`) calls `IntrinsicSizeMeasurer::minimum_contribution` (`:145-151`) in two places:
     - at `:600` and `:607`, for span-1 items in a track whose min function is `auto`;
     - at `:695` and `:702`, for spanning batches and flexible batches.
   - That calls `GridItem::minimum_contribution_cached` (`grid_item.rs:541-554`), which calls `minimum_contribution` (`:459-537`).
   - The automatic-minimum closure (`:486-528`) runs when the item's `size` and `min-size` in the axis are `auto` and it is not a scroll container. That is every table cell.
   - The closure computes `spans_auto_min_track = axis_tracks.iter().any(min is auto)` (`:497-500`) and `spans_a_flexible_track = axis_tracks.iter().any(max is fr)` (`:504`), both over the whole axis. `item_axis_tracks` (`:491`) is used only for its length (`:503`).
2. **Size of the row axis.**
   - Blitz makes the rows **explicit** `auto` tracks, not implicit ones: `grid_template_rows = vec![auto(); row]`, Blitz `table.rs:192`.
   - Taffy puts a gutter between tracks, so the row vector has 2R+1 entries, about 9603 at 4801 rows. The first and last gutters are collapsed to a fixed zero (`explicit_grid.rs:298-299`).
   - So `any(is_auto)` stops at index 1, and `any(is_fr)` never stops, because a table has no `fr` row: **2R+1 checks per item**.
   - In the inline axis the vector has about 11 entries, which is negligible.
3. **How often it runs.**
   - The result is cached per item, axis and pass (`minimum_contribution_cache`).
   - `items` is rebuilt on every `compute_grid_layout` (`mod.rs:198`), and the cache is cleared when a pass is re-run (`mod.rs:378`, `:389`, `:438`, `:449`).
   - So the scan runs once per item per block pass. Each `compute_grid_layout` has one block pass (`mod.rs:278`), plus one more when the re-run branch runs (`:455`, which only happens inside the column re-run at `:393`).
   - The table node's Taffy cache (9 slots) decides how many `compute_grid_layout` calls happen. F99c's callgrind run counted 8 `resolve_intrinsic_track_sizes` calls for the two tables at 1200 rows.
   - Cost per block pass is about C × (2R+1) with C ≈ 5R, so it is quadratic. It measures as n^1.5–1.8 because it runs next to linear work.
4. **Other per-item loops in the same path.** None of them is items × tracks:
   - `determine_if_item_crosses_flexible_or_intrinsic_tracks` (`track_sizing.rs:249-264`), `spanned_track_limit` and `spanned_fixed_track_limit` (`grid_item.rs:191-239`), and `available_space` (`:317-337`) look only at the tracks the item spans.
   - `ItemBatcher::next` (`track_sizing.rs:49-54`) runs `position` from index 0 once per batch: items × number of distinct spans. That is linear for tables.
   - The loops over all tracks (`:659-672`, `:737`, `:873`) run once per batch, not once per item.
   - `resolve_item_baselines` (`:456-518`) and the re-run checks (`mod.rs:362-381`, `:422-441`) are linear.
   - Side note, not touched: the row re-run check at `mod.rs:423` filters on `crosses_intrinsic_column`.

### 3.2 Decision

**The fix goes in the Taffy fork only** (`subapavel/taffy` `htmltopdf/minimum-contribution`).

- It has to be in Taffy because the scan runs over Taffy's private track vector. Blitz cannot avoid it without changing how tables map to the grid, and that would move geometry.
- No Blitz or fulgur change.

**Change.** Compute the two booleans once per call of `resolve_intrinsic_track_sizes` and pass them in. Signatures:

```rust
// src/compute/grid/types/grid_track.rs (re-exported pub(super) from types/mod.rs)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct AxisTrackFlags { pub any_auto_min: bool, pub any_flexible_max: bool }
impl AxisTrackFlags { pub fn of(axis_tracks: &[GridTrack]) -> Self; } // the two `any` expressions of grid_item.rs:497-504, verbatim

// src/compute/grid/types/grid_item.rs: one new parameter, after `axis_tracks`
pub fn minimum_contribution(&mut self, tree: &mut impl LayoutPartialTree, axis: AbstractAxis,
    axis_tracks: &[GridTrack], axis_flags: AxisTrackFlags,
    known_dimensions: Size<Option<f32>>, inner_node_size: Size<Option<f32>>) -> f32;
pub fn minimum_contribution_cached(/* same parameters */) -> f32;

// src/compute/grid/track_sizing.rs
struct IntrinsicSizeMeasurer { /* … */ axis_track_flags: AxisTrackFlags }
// resolve_intrinsic_track_sizes: `let axis_track_flags = AxisTrackFlags::of(axis_tracks);` after :556;
// `debug_assert_eq!(AxisTrackFlags::of(axis_tracks), axis_track_flags);` once at the end (O(tracks) per pass).
```

The four call sites (`:600`, `:607`, `:695`, `:702`) do not change: `IntrinsicSizeMeasurer::minimum_contribution` passes `self.axis_track_flags` on.

**Why the output is identical:**

- (a) Both booleans are pure functions of the sizing functions of every element of `axis_tracks`. `is_auto` and `is_fr` are tag compares (`compact_length.rs:360`, `:423`).
- (b) Nothing in `track_sizing_algorithm` writes those fields. The only writer is `GridTrack::collapse` (`grid_track.rs:104-108`), called only from `initialize_grid_tracks` (`explicit_grid.rs:270-271`, `:298-299`) before sizing. The new `debug_assert` re-checks this on every pass of Taffy's test suite.
- (c) Every call passes the same full `axis_tracks` slice the flags come from.
- (d) Computing the flags earlier has no side effects. The order and arguments of all `tree` calls are unchanged, so Blitz's caches see the same sequence.
- (e) No `f32` expression changes. Rust does not contract floating point or use fast-math, so inlining cannot change results.

**Wiring (ours):**

- `Cargo.toml` `[patch.crates-io]` gets:

  ```toml
  taffy = { git = "https://github.com/subapavel/taffy", branch = "htmltopdf/minimum-contribution" }
  ```

- Then `cargo update -p taffy`. The lock diff must be the source line of the one `taffy` entry, still version 0.9.2.
- Cargo unifies on one taffy. `blitz-dom`, `fulgur` (`taffy = "0.9"`, default features) and `stylo_taffy` all ask for `^0.9`, and a workspace `[patch]` applies to git dependents too.
- **The fork must keep `version = "0.9.2"`.** A different version would not match the locked 0.9.2. Cargo then prints only a "patch was not used" warning and builds crates.io Taffy, so the fix would silently not apply.
- **The fulgur and Blitz forks need no commit.** Their tests use the patch only through a local `--config` (§5 step 3). fulgur's lock pins crates.io 0.9.2; Blitz's lock pins 0.9.1.

## 4. Acceptance

1. **Bytes (hard gate).** `cmp` main's binary against the branch binary at `RAYON_NUM_THREADS=1` and `=4`: 20 fixtures + 7 `bench/corpus` documents + 16 `--scaling` variants (F99c §7.3 matrix), **86 of 86 identical**.
   - `compare.py --threshold-check`: `summary.json` byte-identical, so the pixel-diff change is **0** for every fixture.
   - Look once at `out/compare/02-long-p2.png` and at one `b200-items` page past 100: unchanged.
2. **The cause is gone (verifies §3.1).** Repeat the F99c §7.1 perf run (symbolised build in a scratch target dir, `t` variant, 1 thread):
   - `GridItem::minimum_contribution` self time at 4800 rows **≤ 20 samples** (was 291).
   - `resolve_intrinsic_track_sizes` inclusive r(4800/1200) **≤ 5** (was 415/44 ≈ 9.4).
3. **Taffy suite on the fork**, run first on unchanged `8f30e39`, then on the branch, so any failure that was already there is known (taffy's CI matrix, `.github/workflows/ci.yml`):
   - `cargo test --tests` (includes the generated gentests under `tests/generated/`);
   - `--all-features`;
   - `--no-default-features --features grid,std,taffy_tree`;
   - `--no-default-features --features alloc,grid,flexbox,block_layout,taffy_tree` (the `no_std` + alloc path);
   - `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`;
   - `cargo +1.65 build`, if the toolchain installs. Otherwise record it as not run: our workspace needs 1.85, so the MSRV only matters for rebasing.
4. **Unit test in the fork, red first** (it does not compile without `AxisTrackFlags`). `AxisTrackFlags::of` must equal the two literal `any` expressions on hand-built vectors:
   - empty; gutters only; all `auto`;
   - fixed + `auto`; one `fr`; `minmax(auto, 1fr)`; `fit-content`;
   - collapsed tracks; and an `auto` track only at the last index.
5. **Consumers, with the local patch:**
   - fulgur `cargo test -p fulgur --lib`, `render_smoke`, `gcpm_integration` green;
   - `fulgur-vrt`: the same failing list as before (F99c §7.3.5: 26 host-font failures, before and after);
   - Blitz `cargo test -p blitz-dom --lib`, with and without `parallel-construct`;
   - ours: `cargo test --release --locked`, including `determinism_threads.rs`, and `cargo clippy --workspace --all-targets --locked -- -D warnings`;
   - `cargo tree -i taffy` shows one taffy, from the git source.
6. **Speed.** `bench.py b200-items b200-text b200-mixed 02-long`, main and branch alternating (A B A B), median of 7.
   - Stated target: `b200-items` at least −150 ms (estimate −200 to −300). Others unchanged within ±5 %.
   - Goal: under 1000 ms, if reachable. Not expected on the 2.80 GHz host (about 1.7 s, estimate); report the number either way.
   - `bench.py --check` passes. Report r of `t` (F99c: 4.07–4.40).
   - `SCALING_MAX_RATIO` stays 6.0, because the branch's own spread (±0.25) would make a tighter gate flaky.
   - Only if the machine label equals `bench/baseline.json`'s (F99c §7.4): `--update-baseline` (it may only lower rows), and lower `HARD_MS_OVERRIDE["b200-items"]` from 3000 to about 1.8 × the new median. On any other host, leave both and record why.
   - No timing asserts in `cargo test`: F99c §4 item 3 explains why.
7. **Reviews.**
   - `vrt-validator`: no high-severity finding.
   - `pdf-reviewer` is not required: merge policy item 5 covers the PDF writer, fonts and metadata, and this PR touches none of them; the bytes are identical.

## 5. Steps (each ≤ 1 day)

0. **Baseline (½ day, no commits).**
   - Diff `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/taffy-0.9.2/src` against `/home/user/taffy/src`. They must be identical. If they are not, stop: the patch would change output on its own.
   - Run the §4.3 Taffy matrix on unchanged `8f30e39` and record any failures.
   - Keep main's release binary as `/tmp/claude-0/htmltopdf-main-<sha>`.
1. **Fork change (1 day).**
   - Write the red unit test of §4.4 first.
   - Then add `AxisTrackFlags`, the new parameter, the `IntrinsicSizeMeasurer` field and the end-of-pass `debug_assert`.
   - Use only `core` types and no new dependency, so MSRV 1.65 and `no_std` hold. The code sits under `compute/grid`, which is already behind the `grid` feature.
   - Run the §4.3 matrix. Commit with the trailer lines and push.
   - If the push is denied: write a postmortem, per the standing decision.
2. **Local verification before wiring (½ day, scratch target dir, nothing committed).**
   - Build ours with `--config 'patch.crates-io.taffy.path="/home/user/taffy"'`.
   - Run the §4.1 `cmp` matrix and the §4.2 perf check.
   - If any byte differs: stop, write a postmortem, and do not wire.
3. **Wire and measure (1 day).**
   - `[patch.crates-io]` entry, `cargo update -p taffy`, `cargo tree -i taffy`.
   - The full §4.1, §4.5 and §4.6 runs.
   - fulgur and Blitz tests with `--config` patches for `blitz-dom` and `taffy`. In Blitz, also run `cargo update -p taffy` under that config, because its lock pins 0.9.1. Restore both lockfiles afterwards and never commit them.
   - Docs:
     - `docs/upstream.md`: the Taffy gap row becomes **carried**; a new "Our patches" row (`taffy 0.9.2`, `subapavel/taffy` `htmltopdf/minimum-contribution`, base `8f30e39`, fix, "known, not reported"); the bump paragraph gains Taffy (drop the branch when Blitz moves to a Taffy release without the scan).
     - AGENTS.md "Upstream" names `subapavel/taffy`.
     - The F99 (step d) and F99d entries in the findings.
     - `docs/benchmarks.md`.
     - §7 here.
   - Then run `vrt-validator`.

## 6. Risks and non-goals

- **The patch is silently not used.** A version or source mismatch only prints a warning. The guards are §4.5 `cargo tree -i taffy` and the §4.2 perf check: with no gain there, the patch is not active.
- **The git source differs from the crates.io tarball.** Step 0's diff guards against it.
- **Floating point.** No `f32` expression changes, and the sequence of tree calls is the same (§3.2 d–e). The byte matrix is the proof.
- **The gain is smaller than the self time.** The remaining 73 inclusive samples are measurements Blitz needs anyway. The 1 s goal probably still fails on the 2.80 GHz host. What remains for F99 is F144 (about 35 ms) and then the style and shaping phases.
- **Fork branch upkeep.** One source per crate in `[patch]`, so later Taffy fixes go on this branch despite its name. Any Taffy bump in Blitz means a rebase. I did not check whether upstream Taffy `main` has already changed this code.
- **Non-goals:**
  - checking only the spanned tracks, as css-grid-1 §6.6 says: this moves column sizes when a fixed-width column sits next to an `auto` one, so it needs a fixture → new finding, "known, not reported";
  - the `crosses_intrinsic_column` filter at `mod.rs:423` → new finding;
  - `ItemBatcher`'s scan from index 0;
  - F143 (`i16` grid lines);
  - F144: one finding per PR;
  - any Blitz change to the table-to-grid mapping;
  - tightening `SCALING_MAX_RATIO`;
  - promising 1000 ms.

## 7. Result

To be filled in after Step 3.
