# F99f — main-thread leftovers: one pre-scan of the HTML, drops off the critical path

Date: 2026-10-27. Finding: F99 (speed; AGENTS.md goal 2: 200 pages under 1 s). Profile: `2026-10-27-f99f-speed-profile.md`
(step 1, measurement only). This step is the profile's candidate **S1 (a, b)**: byte-identical, small, mostly in our own
crate. L1 (inline measurement memo, Blitz fork) and M1 (margin documents on rayon, fulgur fork) are the next steps.

## 1. Problem

`b200-items` takes 1276 ms (goal 1000; `b200-text` 812, `b200-mixed` 808). The wall is set by the main thread (5.05 of
7.41 G Ir; 1450 / 1292 / 1354 ms at 1 / 2 / 4 threads). Two main-thread costs need no layout change:

- (a) `crates/htmltopdf` scans the whole HTML five times before the engine starts — `prepare` (three rewrite closures over
  `walk_html`), `check_fonts` (`scan_html`), `check_images` (`img_srcs`) — each a `CharSearcher` pass over 1.9 MB that
  allocates per tag (57 686 allocations): 263 M Ir, ~15-40 ms.
- (b) the Blitz `HtmlDocument` (`drop_glue` 94 M Ir + frees) and `Drawables` (35 M) are dropped on the main thread after
  layout / after paint: ~25-35 ms.

## 2. Spec

None (no output change). AGENTS.md goal 3: output byte-identical.

## 3. Where it belongs

| Part | Where | Change |
|---|---|---|
| (a) one pass | `crates/htmltopdf/src/fonts.rs` `walk_html` and its callers (`scan_html`, `img_srcs`, `style_element_ranges`, `rewrite_sites`), `lib.rs` `prepare` / `check_fonts` / `check_images` | Walk the HTML once into a `HtmlSites` list (the `HtmlSite` ranges + `in_svg`, as today's callback reports them) and derive the scan, the image srcs, the style ranges and the rewrites from it; rewrites that change the text re-walk only if they changed it (offsets shift). No per-tag `String` allocation in the walker (tag names compared case-insensitively in place). Behaviour identical: every existing `fonts.rs` / `media.rs` unit test stays green unchanged. |
| (b) drops | fulgur fork, where the Blitz document and `Drawables` go out of scope in `engine.rs` / `render.rs` | Move them into a rayon task (`rayon::spawn(move \|\| drop(x))`) once they are no longer needed (document after `dom_to_drawables`; `Drawables` after the last page), or join before returning if a drop must finish (no `'static` problem: both are owned). Only when the `parallel-*` features are on (else unchanged). |

## 4. Acceptance

- Byte identity: fixtures 01-26 and `bench/corpus` `cmp`-identical to main; fulgur VRT failing set and actual PDFs unchanged.
- `bench.py` (median of three runs, interleaved with main's binary on the same machine): `b200-items` faster by the sum of
  (a) and (b) within noise (target -40 to -75 ms); no corpus document slower; `bench.py --check` ok; record in §7 and
  `docs/benchmarks.md`.
- Unit tests: a fused-pass test asserting the single walk yields the same sites as the old separate walks on a mixed HTML
  (styles, `style=""`, `<img>`, `<svg>`, comments, raw text, `template`); all existing tests green.

## 5. Steps

1. Ours: (a) with tests; measure (a) alone (interleaved A/B, 15 pairs on `b200-items` and `01-basic`).
2. Fork: (b); measure; `cargo test -p fulgur`; VRT.
3. Ours: lock bump; gates; `vrt-validator` light (byte identity + timing re-check); docs.

## 6. Risks and non-goals

- A rewrite (generic mapping, gradient strip, close-open-blocks) changes offsets; the fused pass must re-walk after a
  change or apply rewrites back to front — the existing rewrite tests guard it.
- A background drop that panics (it should not) would abort; drops are of plain data.
- Non-goals: L1, M1, K1 (krilla per-glyph `Arc<Font>`, needs a krilla fork), S1 (c) `column_css::walk` fusion and (d)
  Taffy `GridItem` sort — next steps if (a) + (b) land as estimated.

## 7. Evidence

(A/B numbers, bench table.)
