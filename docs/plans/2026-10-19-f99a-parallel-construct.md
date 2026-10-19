# F99a — Parallel inline-layout construction (first F99 step)

Date: 2026-10-19. Finding: F99 (speed). Status: **done** (§7). Profile: `2026-10-19-f99-speed-profile.md` (candidate C1).
Fixes go on the Blitz fork (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`) and the fulgur fork (`htmltopdf/srgb-colours`
after `7c1389b`, `crates/fulgur/Cargo.toml`), carried via `[patch.crates-io]`.

## 1. Problem and measurement

The profile puts Parley shaping (`resolve_deferred_tasks`, Blitz `document.rs`) at 255 / 444 / 311 ms of the three b200
documents (items / text / mixed), done on one thread. Blitz has a `parallel-construct` feature that runs these tasks on
rayon. With it on, the profile measured −114 / −226 / −168 ms (text 898 ms, mixed 885 ms: under the 1 s goal) and
byte-identical PDFs on the b200 documents and fixtures 01–16 at 1–4 threads.

## 2. Why the feature cannot be switched on as it is (read in Blitz `document.rs`, main session review)

1. **Font state leaks across documents.** Worker threads keep a `thread_local! FONT_CTX` (`:66-69`). It is created once per
   thread as a clone of the *first* document's font context (`:727-745`) and from then on only gains fonts
   (`register_fonts` on every later `Resource::Font`, of any document). It is never reset. In a long-lived process (our
   library, the bindings, the planned HTTP service) document B is shaped with the fonts of every earlier document, and
   with the generic-family map (F70 `apply_generic_families`) of whichever document initialised the thread — or none.
   Same input → different PDF depending on history: this breaks invariant 3 (determinism) and can bring F70's dropped
   `pre`/`code` text back. The CLI renders one document per process, which is why the profile saw no difference.
2. **stdout.** The same block `println!`s "Initialising FontContext for thread …" (`:734`). With `-o -` this writes into
   the PDF stream (fulgur's `CLAUDE.md`: library crates must not touch fd 1).
3. The thread-local `LAYOUT_CTX` (Parley scratch) carries no document state: safe.

## 3. Design

- **Blitz fork** (`document.rs`): the worker's font context is a per-document snapshot.
  - Each `BaseDocument` gets a `font_generation: u64` (from a process-wide `AtomicU64`, bumped whenever its font context
    changes: font registration, generic families, any `font_ctx` mutation the fork exposes).
  - `FONT_CTX` holds `(u64 generation, Box<FontContext>)`. In the deferred-construction closure (`:1136-1141`): if the
    thread's generation differs from the document's, replace it with `self.font_ctx.lock().clone()` and store the
    document's generation; otherwise reuse it.
  - Remove the `rayon::broadcast` at font registration (`:727-746`), including the `println!`. Registration only touches
    the document's own context; workers pick it up lazily through the generation check.
  - Keep `cfg(feature = "parallel-construct")` gating; non-parallel builds unchanged.
- **fulgur fork**: `blitz-dom = { version = "0.2", features = ["parallel-construct"] }` (plus whatever default features
  it already uses). Check `apply_generic_families` (F70) mutates the document's `font_ctx` before construction and bumps
  the generation (add a Blitz fork accessor if needed).
- **Thread pool**: rayon's global pool (default: number of CPUs). No `RenderOptions` knob in this step; record that the
  library now uses rayon's global pool (bindings and services share it).
- **Small documents**: the profile saw ~40 ms of pool wake-up across `b200-items`' 416 margin documents. If fixtures or
  margin-box documents get slower, use the serial path when a document has fewer than N deferred tasks (N measured,
  e.g. 32); record the threshold.

## 4. Acceptance

- **Determinism tests** (ours, `crates/htmltopdf/tests/determinism_threads.rs`):
  1. One process renders document A (Inter only) then document B (Inter + JetBrains Mono, with `<pre>`/`<code>`), then
     B again: both B renders byte-identical to B rendered first in a fresh `Renderer`; and A after B identical to A first.
  2. The same with the generic-family map changed between documents (`generic_family(Monospace, "Inter")`, F70 A2).
  3. B rendered from 4 threads concurrently (`std::thread::scope`), each result identical.
  4. Fixtures 01–16 rendered under `RAYON_NUM_THREADS=1` and `=4`: identical to each other and to main's binary.
- **No stdout**: `htmltopdf render … -o -` output equals the `-o file` output byte for byte (a CLI test).
- **Speed**: `python3 scripts/bench.py` table before/after in §7; `b200-text` and `b200-mixed` under 1000 ms median on
  the dev VM; no fixture slower than its baseline margin; `--check` passes. Update `bench/baseline.json` (lowering only).
- Fork: Blitz unit test for the generation check (two documents with different fonts shape with their own fonts on one
  worker); fulgur lib/smoke/VRT/examples as usual (byte-identical).
- `vrt-validator`; `pdf-reviewer` (fonts path: shaping and font selection run on worker threads now).

## 5. Steps

1. Red tests: the determinism tests with the feature switched on but the Blitz fix not yet in (expect 1 and 2 red, and the
   stdout test red) — this proves the hazard; record in §7. If they are green, record why (re-read `document.rs`) before
   continuing.
2. Blitz fork fix + unit test; fulgur feature flag; lock bump.
3. All tests, fixtures `cmp` (01–16 vs main, at 1 and 4 threads), bench, docs (`docs/upstream.md` rows for both forks,
   §7, F99 entry updated with the new numbers).

## 6. Risks and non-goals

- A hidden mutable font-context path not covered by the generation bump would leak state again: the tests in §4 cover
  fonts and generics; any other `font_ctx` mutation found while implementing gets a bump and a test.
- Memory: one font-context clone per worker per document (the collection holds `Arc` blobs; measure peak RSS for
  `b200-text` before/after with `/usr/bin/time -v` and record it).
- Non-goals: margin-box documents (C2a/C2b, next F99 step), Taffy/Stylo table scaling (C3), Krilla deflate (C5).

## 7. Result

Blitz fork `b720ad0` (on `52b22ef`), fulgur fork `33f8993` (on `7c1389b`), both pushed; `Cargo.lock` pins them. Machine: 4 vCPU Xeon 2.1 GHz, shared VM
(run-to-run spread 2-8 %, so a difference under about 5 % is noise).

### 7.1 Step 1: the hazard, proved before the fix

`crates/htmltopdf/tests/determinism_threads.rs` (new). References are produced by child processes (the test binary re-run on one scenario), so "rendered first"
is true by construction. Scenarios: A = Inter Regular only, B = Inter Regular + Bold + SemiBold, G1 / G2 = all Inter weights with `monospace` mapped to Inter /
left at the built-in JetBrains Mono; each document has 400 to 1200 inline layouts so that every worker of the pool takes tasks. Blitz at `52b22ef` with
`parallel-construct` switched on in the fulgur fork, each test run alone in its own process:

| Test | Before the fix | After |
|---|---|---|
| `document_does_not_see_the_fonts_of_earlier_documents` (A, B, B, A) | **red**: "B after A" differs from a fresh process (69 160 vs 84 639 bytes): B's bold text was shaped with A's Regular-only context | green |
| `document_does_not_see_the_generic_families_of_earlier_documents` (G1, G2, G1, G2) | **red**: "G2 after G1" has no JetBrains Mono (82 062 vs 84 639 bytes): G2's `<pre>` / `<code>` were shaped with G1's `monospace` = Inter. This is F70's dropped-text hazard in another form | green |
| `concurrent_renders_are_identical` (4 x B, then 2 x A and 2 x B at once) | 4 x B green, **red** on the mixed run ("a next to the other", 42 438 vs 33 627 bytes) | green |
| `fixtures_are_identical_at_one_and_four_rayon_threads` (child processes, `RAYON_NUM_THREADS` 1 and 4) | green (one document per process, so no history) | green |
| `scenarios_differ_from_each_other` (the scenarios can tell a leak) | green | green |
| `crates/htmltopdf-cli/tests/stdout_clean.rs` (CLI stdout empty) | **green** | green |

Run together in one process (default harness) the same three tests fail even earlier ("A first"), because the tests contaminate each other; that is the hazard too, but
the table above is the clean evidence. Blitz unit tests (`font_snapshot_tests`, 3): the worker test fails with "font of the previous document leaked" when the generation
check is disabled (mutation check) and passes with it.

What step 1 did **not** prove: the stdout half of §2 item 2. Reading `document.rs` and the callers: the `println!` sat in the `Resource::Font` arm of `load_resource`,
which only runs when a document fetches an `@font-face` font through the net provider. fulgur hands Blitz its `FontContext` through `DocumentConfig` (never as a
`Resource::Font`), and `htmltopdf` sets no `base_path`, so the provider rejects every fetch: the line cannot be printed through `htmltopdf` or the CLI, before or after. It
was reachable for direct fulgur users with a `base_path` and an `@font-face` (not run). The stdout test is therefore a guard, green on both sides. The CLI has no `-o -`
(it writes a file called `-`), so the test checks that stdout is empty instead of comparing `-o -` with `-o file`. Looking for stdout noise turned up a different line that **is**
printed on stdout today: F100.

### 7.2 Change

- Blitz (`document.rs`): `BaseDocument::font_generation`, taken from a process-wide `AtomicU64` at creation and again whenever a font is registered (`Resource::Font`, the
  only place the font context is changed after creation; `parse_inner` applies the generic families before the document exists). The worker thread-local holds
  `(generation, FontContext)`; `take_worker_font_ctx` reuses it only for an equal generation, else clones the document's context. The `rayon::broadcast` and its `println!` are
  gone. The per-task body is `run_construction_task`, shared by both paths. Fewer than `PARALLEL_CONSTRUCT_MIN_TASKS` = 256 deferred layouts are shaped on the calling thread.
- fulgur: Cargo feature `parallel-construct = ["blitz-dom/parallel-construct"]`, off by default; `crates/htmltopdf/Cargo.toml` enables it. rayon's global pool is used (bindings and
  services built on `htmltopdf` share it); no `RenderOptions` knob.

### 7.3 Speed (release CLI, 4 vCPU, `scripts/bench.py`, median of 7, min/median ms, before and after run back to back)

| Document | Pages | Before (`437f00f` + docs) | After | Change of the median |
|---|---:|---:|---:|---:|
| 01-basic | 1 | 15.3 / 16.3 | 15.4 / 15.9 | noise |
| 02-long | 3 | 48.4 / 49.2 | 43.1 / 49.3 | noise |
| 03-logo-qr | 1 | 18.9 / 20.5 | 19.5 / 22.0 | noise |
| 04-bilingual-vat | 1 | 16.7 / 18.3 | 17.2 / 19.4 | noise |
| 05-fancy | 1 | 20.0 / 21.9 | 19.5 / 22.3 | noise |
| 06-long-notes | 2 | 19.7 / 21.3 | 19.1 / 19.8 | noise |
| 07-heading-margin | 2 | 17.3 / 18.9 | 16.8 / 19.3 | noise |
| 08-terms-grid | 2 | 23.3 / 28.1 | 23.0 / 26.1 | noise |
| 09-fixed-row-height | 3 | 19.3 / 20.8 | 18.2 / 19.5 | noise |
| 10-stacked-items | 2 | 18.0 / 18.9 | 16.3 / 18.7 | noise |
| 11-full-width-rows | 1 | 15.1 / 15.5 | 14.9 / 15.2 | noise |
| 12-terms-columns | 2 | 21.4 / 22.8 | 21.4 / 22.2 | noise |
| 13-terms-keep-together | 3 | 23.6 / 26.3 | 22.2 / 24.0 | noise |
| 14-totals-keep-together | 2 | 17.2 / 18.7 | 16.8 / 19.9 | noise |
| 15-paid-stamp | 2 | 19.1 / 19.9 | 18.1 / 19.3 | noise |
| 16-terms-card | 2 | 20.0 / 25.8 | 20.2 / 21.9 | noise |
| b200-items | 200 | 2232 / 2265 | 2131 / 2293 | none (min -4 %) |
| b200-text | 200 | 1056 / 1126 | 783 / 851 | **-24 %** |
| b200-mixed | 201 | 950 / 1001 | 780 / 853 | **-15 %** |

`b200-text` and `b200-mixed` are under the 1000 ms goal (about 880 and 860 ms in the run that `--check` and the new baseline use; 825 and 877 ms in the `--check` run). The profile's
estimate for `b200-items` (-150 to -250 ms) did not materialise: the roughly 24 000 tasks of its main document shape in parallel, but the gain is inside the spread and its 416 margin
documents (1-2 tasks each) now stay serial, so the +40 ms of pool wake-ups the profile saw are gone; the footers (C2a, C2b) and the table (C3) are what is left there.
Earlier runs on a busier VM gave the same ordering (text 1153 -> 952, mixed 1050 -> 905).

`python3 scripts/bench.py --check`: ok. `bench/baseline.json` lowered for `b200-text` (786.7 / 878.5, was 1114.1 / 1123.0) and `b200-mixed` (811.0 / 861.9, was 1032.3 / 1083.5); nothing raised,
fixtures left as they were.

Threshold: shaping costs 10-30 us per layout in table and card documents (24 641 layouts in all 417 documents of `b200-items`, 11 415 in `b200-mixed`) and about 180 us in the long paragraphs of
`b200-text` (2521). Tasks per document: margin box 1-2, fixtures 23-48, `02-long` 324. Thresholds 1, 32, 128, 512 and "never" were timed on `01-basic`, `02-long`, `05-fancy` (15 runs, twice):
all within 2 ms of each other, no signal. 256 is where the serial stage costs about 5 ms, so the choice rests on the per-task cost, not on a measured optimum. A document with a few very
long paragraphs stays serial (the count is a proxy).

### 7.4 Memory (peak RSS, `resource.getrusage` of the child: `/usr/bin/time` is not installed here)

| Document | Before | After |
|---|---:|---:|
| b200-text | 128 MB | 131 MB |
| b200-mixed | 177 MB | 178 MB |
| b200-items | 243 MB | 243 MB |
| 02-long (3 pages) | 26 MB | 29 MB |

One font-context snapshot per worker (blobs are `Arc`s) costs 1-3 MB. Each worker keeps the snapshot of the last document it shaped until it shapes another.

### 7.5 Checks

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` (ours) | clean |
| `cargo test --release --locked --no-fail-fast` (ours) | 449 passed, 0 failed, 43 test binaries (incl. the 5 new determinism tests, `zz_child_process` and the CLI stdout test) |
| fixtures 01-16 `cmp` against main `d532069`'s binary | 16/16 byte-identical at `RAYON_NUM_THREADS=1`, `=4` and unset; also identical to the pre-change binary of this branch |
| `b200-items`, `b200-text`, `b200-mixed` `cmp` before / after | byte-identical at 4 threads and at 1 thread |
| Blitz fork `cargo test -p blitz-dom --lib` | 38 passed (default features), 40 passed (`--features parallel-construct`, 3 new) |
| fulgur fork `--lib --features parallel-construct` (local Blitz) | 2151 passed |
| fulgur fork `render_smoke` (feature on, local Blitz) | 262 passed |
| fulgur fork `fulgur-vrt` (`FONTCONFIG_FILE=...fonts-noinInter.conf`), with and without the feature | all green, goldens untouched (byte-identical) |
| fulgur fork `examples_determinism` (feature on) | 10 of 11; `committed_svg_matches_rendered` fails as in every earlier item (host fonts, stale `examples/svg/index.pdf`) |
| `bench.py --check` | ok |

### 7.6 Deviations from the plan

- The feature is a fulgur Cargo feature (`parallel-construct`, off by default) that `htmltopdf` turns on, not an unconditional feature on fulgur's `blitz-dom` dependency: fulgur also builds for
  `wasm32-unknown-unknown`, where rayon has no threads.
- Threshold 256 instead of the example 32 (§3), see §7.3.
- No stdout test red (§7.1); the CLI has no `-o -`.
- Scenario A / B differ in the Inter weights they bundle, not in JetBrains Mono: the built-in mono face is always registered by `Renderer::build`.
- The Blitz generation is bumped only at `Resource::Font`: no other code changes the context after creation (checked by grep over blitz-dom and fulgur).
- Commit trailers carry the attribution the session harness prescribes (`Claude Sonnet 5.5`), not the `Claude Opus 5.5` line the task text gave.

### 7.7 Not verified

- A machine with 1-2 vCPUs or a CI runner; `render_batch` and the bindings with the global pool; wasm.
- `vrt-validator` and `pdf-reviewer` (plan §4) were not run in this change; fonts and shaping on worker threads is the thing `pdf-reviewer` should look at. The PDFs are byte-identical on 16 fixtures and
  the 3 b200 documents, which is the evidence here.
- That `println!` was reachable (direct fulgur use with `base_path` + `@font-face`); only read from the code.
- The cold first render (5.7 s once) was not looked at.
- Whether the thread-local `LAYOUT_CTX` (Parley scratch) carries no document state; read, not tested beyond the byte-identity above.
