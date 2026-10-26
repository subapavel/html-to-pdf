# F99e — three main-thread overheads in the paint path (deflate, krilla fonts, font byte copies)

Date: 2026-10-26. Finding: F99, step 6 (AGENTS.md goal 2). Status: planned. Base: main + F174; fulgur fork `5f831cd2`,
Blitz fork `f4132d5`, Taffy fork `7bbedd5`. Profile: callgrind of `b200-items`, 1 thread, release with line tables,
7.74 G instructions (Ir).

## 1. Problem

On this host (4 vCPU, 2.1 GHz; `bench.py`, median of 7) `b200-items` takes 1377 ms against the 1000 ms goal,
`b200-text` 950 ms, `b200-mixed` 1042 ms. Shaping already runs on rayon, so wall time is set by the main thread: about
5.81 G Ir in about 1.38 s, i.e. about 4.2 G Ir/s (used below to turn Ir into ms; an estimate). Three costs sit on the
main thread outside the parallel shaping, and none is layout:

- **(A)** Every page's content stream is deflated on the main thread: `krilla::Page::drop` → `InternalPage::new` →
  `Deferred::new` runs zlib-rs eagerly because krilla's `rayon` feature is off (`krilla-0.7.0/src/page.rs`,
  `util.rs`). 421 M Ir inclusive, about 100 ms.
- **(B)** A new `krilla::text::Font` for every glyph run: `paragraph.rs` `draw_shaped_lines` calls `Font::new`, which
  re-parses the font (head, OS/2, name, metrics, glyf/loca) every time. 27 355 calls, 317 M Ir, about 75 ms.
- **(C)** A copy of the font bytes per new `ConvertContext`: `convert/mod.rs` `get_or_insert_font` does a `to_vec()`
  of about 400 KB, 230 times (once per margin-box document plus the main one): 94.6 M Ir of memcpy, about 92 MB kept
  alive by `MarginBoxRenderer::render_cache` until the render ends (page faults are not counted by callgrind: about
  20-40 ms, an estimate). Also `blitz_adapter::parse_inner` copies every bundled font once more
  (`(**data).clone()`); `MarginDocEnv::new` already shares the bundle's `Arc`.

No pixels are wrong; `bench/corpus/b200-items.html` shows A, B and C, `b200-text` / `b200-mixed` A and B.

## 2. Spec

Speed only; every PDF must stay byte-identical (AGENTS.md goal 3). Unverified, from memory: ISO 32000-1 §7.3.8 /
§7.4.4 (streams, FlateDecode); RFC 1950/1951 (deflate output is a function of input and level; krilla's level stays
6); krilla identifies a font by its `FontInfo` hash, not by its data address, so reusing one `Font` gives the same
font resources as building one per run.

## 3. Where it belongs

The fulgur fork (our glue) plus one feature line in our crate; nothing upstream, no Blitz / Taffy / Parley change.

| # | Change | Where | Gain (estimate) | Critical path | Risk / size |
|--:|---|---|---|---|---|
| A | Turn on krilla `rayon`: page streams (and images, masks) are deflated on idle rayon workers while the next page is drawn | fulgur `crates/fulgur/Cargo.toml`: feature `parallel-pdf = ["krilla/rayon"]`, off by default like `parallel-construct`; ours: `crates/htmltopdf/Cargo.toml` enables it | items −80…−110 ms, text −70…−100, mixed −50…−70 | yes: moves work off the main thread (no gain on 1 vCPU) | low / XS (rayon already in the lock) |
| B | One krilla `Font` per (font bytes, face index) per render | fulgur `paragraph.rs` `draw_shaped_lines`; a render-wide cache passed through `Canvas` | items −60…−90 ms | yes: removes work | low / S |
| C | No copy of bundled font bytes: `ConvertContext` and the main document reuse the bundle's `Arc<Vec<u8>>` | fulgur `convert/mod.rs` `get_or_insert_font`; the `ConvertContext` sites in `engine.rs` and `render.rs`; `blitz_adapter::parse_inner` | items −20…−40 ms, peak RSS up to −90 MB | yes: removes work | very low / S |

Order: A, C, B (C makes B's keys collapse to the ≤ 5 bundle fonts; B is correct without C). Expected after all three:
`b200-items` about 1150-1200 ms (still over the goal), `b200-text` about 800-850, `b200-mixed` about 880-930.

Signatures (fulgur fork): `pub(crate) struct FontBytes` (keyed by data address, length checked) with
`seeded(bundle: &[Arc<Vec<u8>>]) -> Self` and `get_or_copy(&mut self, font: &parley::FontData) -> Arc<Vec<u8>>`
(copies only unknown blobs); `pub(crate) struct KrillaFonts` (`(usize, u32)` → `(Arc<Vec<u8>>,
Option<krilla::text::Font>)`, each entry holds an `Arc` clone so an address can never be reused by another font;
`misses` counter) with `get(&mut self, data: &Arc<Vec<u8>>, index: u32) -> Option<krilla::text::Font>`; `Canvas` gets
`pub fonts: &'a mut KrillaFonts` (not an `Option`: every construction site passes the cache; a cached `None` keeps
today's behaviour for a font that fails to parse). In `parse_inner` build the blob with `Blob::new(Arc::clone(data)
as _)` as `MarginDocEnv` does.

## 4. Acceptance

1. Bytes, the hard gate after each step: sha256 of every PDF from main's binary and the branch binary — fixtures
   01-25, the 7 corpus documents, every `--scaling` variant — at `RAYON_NUM_THREADS=1` and `4` (A also `2`), all equal;
   for A also `b200-text` and `b200-items` 5 times in a row at 4 threads, all identical. Compare numbers unchanged.
2. The profile confirms the cause (callgrind, `b200-items`, 1 thread): A `Page::drop` inclusive < 10 M Ir; B
   `Font::new_variable` ≤ 10 calls; C `get_or_insert_font` gone from the memcpy callers.
3. Tests: C `FontBytes::seeded(..).get_or_copy` is `Arc::ptr_eq` with the bundle's (red today); B 1000 `get` calls with
   one key give `misses == 1`, invalid bytes give `None` with one miss; A has no red test (same bytes):
   `determinism_threads.rs` and `image_robustness.rs` (images are now decoded on workers) are the guards. Everything
   else green: ours `cargo test --release`, clippy, fulgur `--lib`, `render_smoke`, `gcpm_integration`, `fulgur-vrt`
   with the same failure list as before.
4. Speed: after each step, main vs branch A B A B, median of 7, on `b200-items`, `b200-text`, `b200-mixed`, `02-long`,
   plus 15 interleaved runs per binary with wall and CPU time. A: lower wall, about the same CPU, and under
   `taskset -c 0` no more than +2 %; B and C: lower CPU. `bench.py --check` passes; `--update-baseline` only on the
   baseline's machine label.
5. Reviews: `vrt-validator`; `pdf-reviewer` (A touches the PDF writer path, B and C fonts: merge policy item 5).

## 5. Steps

0. Baseline (½ day): main's binary kept aside; hashes, timings, RSS. 1. A (½ day): feature lines, `cargo tree -e
features -i krilla` shows `rayon`; gates. 2. C (1 day): red test, `FontBytes`, seeding at the production sites, the
`parse_inner` blob; gates, RSS. 3. B (1 day): red test, `KrillaFonts`, the `Canvas` field; gates. 4. Docs (½ day):
`docs/upstream.md`, `docs/benchmarks.md`, findings, §7; reviews.

## 6. Risks and non-goals

- A: a panic in a job on a worker goes to rayon's panic handler (abort; with a host-set handler the render would
  hang in `Deferred::wait` instead), not to our `catch_unwind`. The jobs are zlib (does not panic on input) and
  krilla's image decoding. A top-level `<img>` or background reaches krilla only through `fulgur::image::krilla_image`,
  whose F32a `validate` decodes PNG / GIF fully on the main thread first; **rasters embedded in an SVG skip F32a**
  (krilla-svg calls `Image::from_png` / `jpeg` / `gif` / `webp` directly) and are now decoded on a worker. The
  pdf-reviewer found no panic there in 660 mutated PNG / GIF / WebP / JPEG files embedded as `data:` URLs (exit codes
  and messages identical to main); the remaining risk is a bug in png / gif / image-webp on a crafted SVG-embedded
  image. `image_robustness.rs` is the guard for the top-level path.
- A: the jobs use rayon's global pool (shared with a binding's caller); pages are committed in order and every chunk is
  waited for when visited, object refs are allocated on the main thread, so bytes cannot depend on scheduling (§4.1).
- B: an address as a key: each entry holds its `Arc`, so a freed buffer never aliases; the map is only looked up, never
  iterated, so a `HashMap` is fine.
- The gains are estimates; if each gives less than half its range, write it in §7 and move on.
- Non-goals: the deflate level (C5: changes bytes; A gets most of the gain); building margin documents in parallel
  (C2b: `render_page` is 12.2 %, about 225 ms, the next step F99f, needs a `Sync` margin environment); Stylo
  revalidation (7.3 %); `column_css::walk` (re-profile); `Box<TextLayout>::clone`; promising 1000 ms for
  `b200-items`.

## 7. Result

- Fork `5d370fd7` (A), `b8435360` (C), `6069832d` (B); `crates/htmltopdf` enables `parallel-pdf`. Lock bumped from
  `c41518b5`.
- Bytes: fixtures 01-25 and the 7 corpus documents byte-identical to main's binary at 1 and 4 threads (our gate); the
  implementation's gate also at 2 threads, the 18 scaling-variant documents and 5 repeated renders of `b200-text` /
  `b200-items`; the pdf-reviewer's at 1 / 2 / 4 threads, untagged and tagged, with every Inter weight, a TTC, bundled
  monospace, broken fonts, 30 image kinds, 5000 x 5000 scans and 660 mutated images inside SVG: no difference, exit
  codes and messages identical.
- Profile (callgrind, `b200-items`, 1 thread, 7.33 G Ir, was 7.74 G): `Page::drop` 0.3 M Ir (was 421 M; deflate runs in
  rayon jobs), `Font::new_variable` 8 calls (was 27 355), no font memcpy.
- Speed (dev VM, 15 interleaved runs, median wall / CPU): `b200-items` 1355 -> 1227 ms (CPU 1605 -> 1547),
  `b200-text` 897 -> 773 ms, `b200-mixed` 878 -> 788 ms; A trades a few % of CPU for wall time, B and C remove CPU. B
  gave less than estimated (about −30 ms on items, not −60…−90), C about −5…−15 ms of CPU and −4 MB of peak RSS (not
  −90 MB: the estimate was wrong). `b200-items` is still over 1 s.
- Reviews: pdf-reviewer no defect (found F177, F178 on main); vrt-validator in §7 of the PR.
