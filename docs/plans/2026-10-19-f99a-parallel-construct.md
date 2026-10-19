# F99a — Parallel inline-layout construction (first F99 step)

Date: 2026-10-19. Finding: F99 (speed). Status: **planned**. Profile: `2026-10-19-f99-speed-profile.md` (candidate C1).
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

(Filled in at landing.)
