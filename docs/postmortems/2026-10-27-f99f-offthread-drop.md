# 2026-10-27 — F99f (b): dropping the Blitz document / drawables on a rayon worker is slower, not faster

**What was tried.** Plan `docs/plans/2026-10-27-f99f-main-thread-leftovers.md` §3 (b): in the fulgur fork move the Blitz
`HtmlDocument` (after `dom_to_drawables`) and the `Drawables` + pagination geometry (after `render_v2`) into
`rayon::spawn(move || drop(x))` when `parallel-construct` / `parallel-pdf` are on. Fork commit `42b3355e` (reverted in
`6d4eb20c`; the tree is identical to `3fa6f44`, which html-to-pdf's `Cargo.lock` still pins). `HtmlDocument` is not `Send`
(three provider trait objects without a bound), so the commit used a documented `unsafe impl Send` wrapper. Lib tests
(2351), `render_smoke` (320), VRT (35 pre-existing failures, actual PDFs) and fixtures 01-26 + corpus were byte-identical.

**What was observed.** Interleaved A/B on `b200-items`, release CLI, `bench.py` flags, 4 vCPU (median of 11-15 pairs; two
identical binaries differ by up to 12 ms):

| variant (one experimental binary, env switch) | diff vs drop in place |
|---|---:|
| document on `rayon::spawn` | **+80 ms** (+64 ms with the committed build against the step-(a) binary) |
| document on `std::thread::spawn` | +20 ms |
| drawables + geometry on `rayon::spawn` | -1 ms |
| document leaked (`mem::forget`) | -41 ms |
| drawables + geometry leaked | -57 ms |
| same binary vs itself | -12 ms |

So the frees do cost 40-55 ms on the calling thread, but freeing the same allocations from another thread costs more than
freeing them in place. Fixture 01 does not move.

**Hypothesis (not verified; no `perf` here).** glibc malloc: memory allocated on the main thread belongs to the main arena;
a worker's frees return chunks to that arena under its lock while the main thread paints and allocates from it, and the
tcache is useless for cross-thread frees. Millions of small chunks make that a contention and cache-line ping-pong cost larger
than the free work itself. The drawables drop is neutral because the main thread has little left to do by then. A
oversubscription effect (main + 4 workers on 4 vCPUs) may add to it.

**What to try next, if the 40-55 ms matter.** Not another thread: skip the frees. The CLI exits right after writing the PDF,
so a process-level "do not free at the end" (leak the document and drawables when the caller is the CLI, e.g. an
`Engine` builder switch the CLI sets) removes the cost; the library default must keep freeing. That is an API decision in the
fulgur fork, not part of this finding. Or a different allocator (mimalloc / jemalloc: a new dependency, owner's call) which
frees cross-thread cheaply.
