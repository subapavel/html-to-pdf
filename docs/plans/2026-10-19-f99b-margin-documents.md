# F99b — `@page` margin-box documents (second F99 step)

Date: 2026-10-19. Finding: F99 (speed). Status: **done** (§7). Profile: `2026-10-19-f99-speed-profile.md` (C2a, C2b, C2c, §3). Previous step: `2026-10-19-f99a-parallel-construct.md`.
The fixes go on the Blitz fork (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`, after `b720ad0`) and the fulgur fork (`htmltopdf/srgb-colours`, after `33f8993`), carried via `[patch.crates-io]`. The line references below are to those commits. I read them in the working trees at `/home/user/blitz` and `/home/user/fulgur` and could not confirm the checked-out commit with git. Stylo is crates.io `stylo` 0.8.0.

## 1. Problem and measurement

- **The fixture.** `bench/corpus/b200-items.html` has 200 pages. Its `@bottom-right` box is `"Strana " counter(page) " / " counter(pages)` and its `@bottom-left` box is constant. The margin boxes take 0.96 s of the 2.40 s render (`render_page` row 13 of the profile). Since F99a the document is 2.29 s (median of 7). `02-long` has the same footer on 3 pages.
- **The cost of one document.** `MarginBoxRenderer::render_page` (`render.rs:4468`) builds a complete Blitz document on every cache miss: 201 measure documents (stage 1a, `:4562-4576`) and 215 render documents (stage 3, `:4671-4709`). Each one costs about 2.1 ms:
  - `parse_inner` (`blitz_adapter.rs:357`) creates a new `FontContext` and copies the bytes of every bundled font into a new blob (`:387-390`, `(**data).clone()`).
  - `BaseDocument::new` (Blitz `document.rs:263`) creates a new `SharedRwLock` (`:296`) and a new `Stylist` (`:293`), and parses Blitz's UA sheet (`:367` -> `make_stylesheet`, `:681`).
  - html5ever tokenises the wrapper. That wrapper inlines `margin_css`, which is `strip_display_none(gcpm.cleaned_css)` (`render.rs:4418`): the whole 20 KB `tailwind.css`. `process_style_element` (`:662`) then parses it again.
  - The callgrind split per document: UA sheet 2.8 M Ir, `--css` sheet 4.0 M Ir, out of about 12.9 M Ir.
- **Why both caches miss 201 + 215 times.** Both keys start with the fully resolved HTML string (`render.rs:4557` and `:4666`). `resolve_content_to_html_with_anchor` (`:4517`) has already put the page number into that string, so `"Strana 7 / 200"` and `"Strana 8 / 200"` are different keys. Nothing in the pipeline separates the constant template from the text that changes.
  - Measure: 200 distinct right-box strings plus one constant left box = 201.
  - Render: the key also holds the rect (`:4668-4669`). `compute_edge_layout` (`:4651`) sizes the left box from the measured widths of both boxes on the edge, so the left box's rect changes whenever the right box's width changes. Inter's figures are proportional, so "Strana 11" and "Strana 18" measure differently. That gives 200 + 15 = 215; the 15 is inferred from the counts and was not traced.
  - So a key that ignores the page number would be **wrong** for measuring, because the width really changes. Every page has to be laid out. What can be saved is the per-document setup around that layout.

## 2. Spec

This step changes no CSS behaviour; the PDFs must stay byte-identical. No `css-spec-researcher` output exists. The references below are cited from memory and their section numbers are **unverified**:

- **CSS Paged Media 3 §5.3.2** (margin box variable-dimension rules): the boxes on one edge are sized together, which is why the left box's rect depends on the right box's content (§1).
- **css-page-3, page-based counters** (`page`, `pages`): the text of the box changes per page by definition.
- **CSS Cascade 5 §6** (origin, layers, order of appearance): a sheet handed to the document outside the HTML must keep the Author origin and come first among the author sheets, as the `<head><style>` does today.
- **HTML LS §13.2.3.5** (input stream preprocessing, CR/CRLF) and **§13.2.5** (RAWTEXT state for `<style>`); Blitz then also decodes entities (`document.rs:664`). The text Stylo parses is therefore *not* the raw `margin_css`. That is why the shared author sheet comes from a `<style>` parse (§3.2), not from `Stylesheet::from_str(margin_css)`.

## 3. Options, decision, and where it lives

### 3.1 Ranking

Gains are estimates from the profile's attribution unless marked as measured.

| Rank | Option | Estimated gain on `b200-items` | Byte-identical | Decision |
|--:|---|---|---|---|
| 1 | **C2a**: one `SharedRwLock`, one parsed UA sheet, one parsed author sheet and one font template **per render**, used by every margin document of that render | -540 to -600 ms (UA 2.8 M plus `--css` 4.0 M Ir out of 12.9 M per document, fonts 0.22 ms, x416). Stylo's own UA cascade-data cache (`stylist.rs:422-444`) now also hits, because the UA sheet `Arc` is the same. | Yes by construction: the same text gives the same rules, under the same lock and URL | **Chosen**: core-count independent, no new threads, no mutation of documents |
| 1b | **C2a + "sheet out of band"**: the margin wrapper loses its 20 KB `<style>` and the shared author sheet is attached when the document is created | another -80 to -120 ms (html5ever RAWTEXT, `text_content`, entity decode and `format!` of 20 KB, x416) | Expected: same cascade position, but node ids shift by -2 uniformly. To be shown with `cmp`; fallback A in §3.2 | **Chosen, behind its own `cmp` gate** |
| 2 | **C2b**: build the margin documents on rayon | -450 to -550 alone; much less after C2a | Yes | **Not now.** The gain depends on the core count (F99a saturated at about 2x on 4 vCPU), it needs `render_page` split into plan, build and paint, and it conflicts with a shared lock (§6). This becomes F99c only if this step misses its target |
| 3 | Small extra: a per-render `ConvertContext::font_cache` for the margin render documents (`convert/mod.rs:139`, `:203-210` copy each used face with `to_vec`) | -10 to -15 | Yes: keyed by blob pointer, and the blob pointers are stable for the life of the per-render state | Optional step 5 |
| — | Measure once per content shape, or a template key with only the number patched | — | **No**: widths differ per number (§1) | Rejected; with a re-layout it is C2c |
| — | Give margin documents only the Tailwind rules that match their content | perhaps -150 (author cascade-data build) | Hard to prove: preflight `*`, `:root` variables, `@layer` order, the `@property` defaults layer, arbitrary `element()` content | Rejected for now. After C2a the parse is paid once; what remains is the per-`Stylist` author cascade-data build, which only a Stylo fork or C2c removes |
| 4 | **C2c**: one document per template, mutate the text node, re-layout | -850 to -900 | Must be proved: incremental re-layout against a fresh document, plus a content-template structure that does not exist today (content is an HTML string) | Last, size L. Its own plan if the per-document floor (about 0.6 ms) ever matters |

### 3.2 Design

**Blitz fork** (`blitz-dom`):

- New `SharedStyles` (new file `shared_styles.rs`, re-exported):

  ```rust
  #[derive(Clone)] pub struct SharedStyles { /* lock: SharedRwLock, base_url: String, ua: Vec<(String, DocumentStyleSheet)>, author: Vec<(String, DocumentStyleSheet)> */ }
  impl SharedStyles {
      pub fn new(base_url: &str) -> Self;                       // new lock, DEFAULT_CSS parsed once under it
      pub fn with_author_sheet(self, text: String, sheet: DocumentStyleSheet) -> Self;
      pub fn lock(&self) -> &SharedRwLock;
  }
  // config.rs:12-29: DocumentConfig { …, pub shared_styles: Option<SharedStyles> }
  // document.rs: pub fn stylesheet_of_node(&self, node_id: usize) -> Option<DocumentStyleSheet>;
  ```

- `BaseDocument::new` with `Some(shared)`:
  1. Use `shared.lock().clone()` instead of `SharedRwLock::new()` (`:296`). The lock reaches every node through `create_node` (`:549-555`).
  2. Append the pre-parsed UA sheets instead of `add_user_agent_stylesheet(DEFAULT_CSS)` (`:361-368`), and keep the per-document `ua_stylesheets` map (`:221`) filled.
  3. `debug_assert!` that `config.base_url == shared.base_url` and that `ua_stylesheets` is `None`.
- Two ways to attach the author sheet, chosen by the `cmp` in step 2:
  - **Out of band (1b, preferred).** Append the author sheets with `stylist.append_stylesheet` before parsing. A `<style>` inside margin content still lands after them (`add_stylesheet_for_node`, `:703-731`), which is the same order as today.
  - **Fallback A.** The wrapper keeps its `<style>`. `process_style_element` (`:662-667`) reuses a shared sheet whose text equals the decoded `<style>` text exactly, and otherwise parses as today. Exactness then rests on string equality.
- Documents without `shared_styles` behave exactly as before. That covers the main document and every other caller.

**fulgur fork** (`blitz_adapter.rs`, `render.rs`):

- New per-render environment:

  ```rust
  pub(crate) struct MarginDocEnv { /* styles: blitz_dom::SharedStyles, fonts: Option<parley::FontContext>, _not_sync: PhantomData<Cell<()>> */ }
  impl MarginDocEnv {
      pub(crate) fn new(margin_css: &str, font_data: &[Arc<Vec<u8>>], system_fonts: bool,
                        generics: &BTreeMap<String, String>) -> Self;
  }
  pub(crate) fn parse_and_layout_margin_doc(env: &MarginDocEnv, html: &str,
                        viewport_width: Px, viewport_height: Px) -> HtmlDocument;
  ```

- `MarginDocEnv::new` takes the author sheet from a **probe**: one `HtmlDocument` with the same `shared_styles`, built from `<!doctype html><html><head><style>{margin_css}</style></head></html>`, read back with `stylesheet_of_node`.
  - The probe's text goes through html5ever and entity decoding exactly as the margin wrapper's did. That makes the sheet identical by construction (§2).
  - The doctype is there so the probe adds no F100 line. RAWTEXT tokenisation does not depend on quirks mode (unverified; the equivalence test in §4 checks it).
- The font template is built once: `FontContext` with `CollectionOptions { system_fonts, shared: false }`, blobs `Blob::new(Arc::clone(data))` (no byte copy), then `apply_generic_families` (`:411`). Each margin document gets `template.clone()`, which is the same snapshot operation F99a uses for workers.
  - The `None` branch of `parse_inner` (`:374-377`: no fonts, system fonts, no generics) is mirrored as `fonts: None`.
- `parse_and_layout_margin_doc` does what `parse_and_layout_with_generics` (`:163-184`) does: `resolve`, then `relayout_position_fixed`. The only differences are the config (`shared_styles`, cloned fonts, base URL `file:///` as in `:398`).
- `MarginBoxRenderer` (`render.rs:4350`) gets `env: OnceCell<MarginDocEnv>`. It is created lazily on the first cache miss, so documents without margin boxes pay nothing. Stage 1a (`:4567`), stage 1b (`:4603`) and stage 3 (`:4676`) call `parse_and_layout_margin_doc`. With 1b the three wrappers (`:4564`, `:4600`, `:4673`) drop `<style>{margin_css}</style>`.

**Ours:** a lock bump only. `htmltopdf` hands fulgur the CSS and fonts as today (`lib.rs:382-403`, `AssetBundle`); nothing in our crate changes.

### 3.3 What is shared and why it cannot leak (the F99a discipline)

1. **Scope is one `render_v2` call.** The environment is owned by the `MarginBoxRenderer` built at `render.rs:138` and dropped with it. Nothing is `static`.
   - Two renders, sequential or concurrent, never see each other's environment.
   - The `!Sync` marker makes sharing it across threads (C2b) a compile error until someone redesigns it on purpose (§6).
2. **Its contents depend only on this render's inputs:** `margin_css`, `font_data`, `system_fonts`, `generics`, plus the compiled-in `DEFAULT_CSS`.
3. **It is immutable after construction.**
   - Sheets are `ServoArc<Stylesheet>`. Margin documents change their own `Stylist`'s sheet *set*, never a sheet: there is no CSSOM, and `remove_stylesheet` only runs for a node's previous sheet.
   - The font template is only ever cloned, never lent out. `shared: false` keeps fontique's clones from sharing caches; a test asserts this.
4. **The lock carries no data.** `SharedRwLock` is `Arc<RwLock<()>>` (Stylo `shared_lock.rs:36-45`), so it is identity only.
   - Stylo asserts same-lock on every read and write (`:226-227`, `:254-255`), and the stylist reads author and UA sheets and style attributes with one guard (`stylo.rs:59-62`). Every sheet and node of a margin document must therefore live under that render's lock, and they do.
   - The main document keeps its own lock.
   - A process-wide lock was rejected because it is cross-document state and would serialise writers across threads.
5. **Font generations are unchanged.** Each margin document still takes a fresh `font_generation` (`document.rs:339`). Margin documents have 1-2 deferred layouts, below 256, so they shape on the calling thread. Even on workers, the generation check would take a fresh snapshot.
6. **Stylo's UA cache was already process-wide** (`UA_CASCADE_DATA_CACHE`, `stylist.rs:272-276`). Its key *holds* the sheet `Arc` (`:93-119`), so a key from render R cannot match a sheet of render R'. The value is a pure function of the key, and `take_unused` evicts entries once the render's stylists are gone.

## 4. Acceptance

- **Bytes.** Fixtures 01–16 and all of `bench/corpus/` must be `cmp`-identical to `/tmp/claude-0/htmltopdf-main-f00877c` at `RAYON_NUM_THREADS=1` and `=4`.
  - `compare.py` numbers must be unchanged, and the pixel-diff change is 0 for every fixture.
  - Look at `out/compare/02-long*.png` once: the footer must be present and unchanged.
- **Speed.** Run `bench.py` before and after, back to back, and put the table in §7.
  - Target: the `b200-items` median at least **600 ms** below "before" (from about 2.29 s to about 1.65 s or less on the dev VM).
  - `02-long` lower or within noise; other fixtures within noise; `--check` ok.
  - `bench/baseline.json` lowered for `b200-items` (and `02-long` if lower).
  - `HARD_MS_OVERRIDE` for `b200-items` lowered from 6000 to 3000 ms, about 1.8x the new median: a CI runner is slower, so the override must stay a regression guard.
  - Peak RSS before and after for `b200-items` and `02-long`.
- **`crates/htmltopdf/tests/determinism_threads.rs`**, new margin scenarios built like F99a's (references from child processes):
  - M1: `--css` with `div { letter-spacing: .5px }`, Inter Regular.
  - M2: `--css` with `div { word-spacing: 4px; font-weight: 700 }`, Inter Regular and Bold, `@bottom-left { font-family: "JetBrains Mono" }`.
  - Both have 30+ pages, `counter(page)` / `counter(pages)`, and a running element via `element()`.
  - Tests:
    1. M1, M2, M2, M1 in one process, each identical to a fresh process.
    2. M1 and M2 rendered concurrently from 4 threads, each identical.
    3. `margin_scenarios_differ`.
  - These are guards: they are green on main by design. **Mutation check** (not committed): with the environment made a process-wide `static`, test 1 must go red. Record the result in §7.
- **fulgur fork**:
  - Lib test `margin_env_matches_fresh_document`: for margin contents (plain text, `<div style>`, a `<style>` inside the content) and `margin_css` variants (CRLF, `&amp;`, `@import`, an empty sheet), the environment path and the old wrapper path give the same `Drawables` and geometry.
  - Lib test: two documents from one environment share the sheet `Arc` and the lock.
  - `render_smoke`: a 3-page document with a `counter(page)` footer renders.
  - `tests/gcpm_integration.rs` green, `fulgur-vrt` goldens untouched, `examples_determinism` 10/11 as before.
- **Blitz fork**: unit tests that a document without `shared_styles` builds exactly as before, and that two documents with one `SharedStyles` share the lock and UA sheet; `cargo test -p blitz-dom --lib` with and without `parallel-construct`.
- **Reviews**: `vrt-validator` with no high-severity finding, and `pdf-reviewer`. The fonts path changed: margin documents shape with a template clone and blobs without a byte copy.
- **stdout**: F100 still prints 416 lines on `b200-items`, not more (the probe has a doctype).

## 5. Steps (each one day or less, each testable on its own)

0. **Baseline (half a day).** Run `bench.py b200-items 02-long 01-basic` with the main binary and with the current branch binary. Render the `cmp` references (fixtures and corpus at 1 and 4 threads) and RSS.
1. **Guard tests (half a day).** Add the determinism margin scenarios (green). Write the fulgur equivalence tests against the §3.2 signatures (red: they do not compile yet).
2. **Scratch prototype, the measurement gate (one day).** Use a scratch workspace with `[patch]` pointing at the local forks, as in the profile. Write the minimal `SharedStyles`, `MarginDocEnv` and lazy wiring, first with the `<style>` kept (A), then out of band (1b). For each, `cmp` `b200-items` and `02-long` against main, and time `b200-items` 5 times.
   - Identical and at least 600 ms faster: continue with the faster identical variant.
   - 450-600 ms: continue and add step 5.
   - Under 450 ms after both variants, or not identical with either: **stop**. Write a postmortem (`docs/postmortems/`, 10 lines: what was measured, what differed) and re-plan C2b or C2c.
3. **Blitz fork, properly (one day).** `SharedStyles`, the `DocumentConfig` field, `stylesheet_of_node`, unit tests, push.
4. **fulgur fork, properly (one day).** `MarginDocEnv`, `parse_and_layout_margin_doc`, wrapper change, lib, smoke, gcpm, VRT and examples tests, push.
5. **Optional (half a day).** Per-render margin `font_cache`, only if step 2 or step 4 measured less than 600 ms.
6. **Ours (one day).** Lock bump; full `cargo test`; `cmp` matrix; bench and baseline and override; docs: `docs/upstream.md` rows for both forks, `docs/benchmarks.md`, the F99 entry, §7 here. Then `vrt-validator` and `pdf-reviewer`.
   - If `b200-items` ends between 450 and 600 ms faster: merge anyway, because it is strictly better and byte-identical. Record the miss, and queue F99c (C2b) in the findings.

## 6. Risks and non-goals

- **Stylo same-lock assert.** Any sheet or node outside the render's lock panics (`shared_lock.rs:227`). `htmltopdf` turns that into `EnginePanic`, and the equivalence and smoke tests run every margin path, so it would show up in tests.
- **Threads.** `parking_lot` read locks are not safely recursive while a writer waits, and Blitz takes two read guards at once (`stylo.rs:61-62`). Blitz writes the lock only in `set_style_property` / `remove_style_property` (`node/element.rs:327`, `:353`), which margin documents never call. A future C2b must still use one environment per worker; hence `!Sync`.
- **Node ids shift by -2 in variant 1b.** Margin drawables are keyed and ordered by node id, but the relative order is unchanged. The `cmp` decides; fallback A keeps node ids.
- **Memory.** The parsed Tailwind sheet, the UA sheet and one font template stay alive for the render (blobs are `Arc`s of the `AssetBundle`'s bytes). Measure it, do not assume it.
- **Not verified while planning:** the 2 × 0.6 s gain split, the "15 left-box widths", that RAWTEXT is independent of quirks mode, and the checked-out fork commits.
- **Non-goals:**
  - C2b, C2c, filtering Tailwind for margin boxes.
  - Sharing sheets with the main document or across renders: that would save about 0.5 ms per invoice but is cross-document state, and needs its own ADR.
  - C3 (table scaling), C4, C5.
  - Krilla `Font::new` per run.
  - **F100.** The safe fix (a) is in `blitz-html`, which is not forked. Fix (b), a doctype in the wrapper, is one token in lines this step touches, but it changes html5ever's tree-building mode, so it is not output-neutral by construction. Output-neutrality is the property this step's acceptance rests on, and "one finding per PR" applies. F100 stays separate and is cheaper after this step.

## 7. Result

Blitz fork `a2ee966` (`SharedStyles`) and `d4258c6` (public read accessors), on `b720ad0`; fulgur fork `6a6da9b`, on `33f8993`; both pushed, `Cargo.lock` pins them. Machine: 4 vCPU Xeon 2.1 GHz, shared VM (spread 2-8 %, so under about 5 % is noise).
**Variant 1b (sheet out of band) was taken**; fallback A was only prototyped for the gate and is not in the code.

### 7.1 Step 2, the measurement gate (scratch workspace with `[patch]` to the local forks, release, `b200-items` median of 5)

| Variant | `cmp` against main (`b200-items`, `02-long`, `01-basic`, 1 and 4 threads) | `b200-items` median | vs main (2275 ms, same session) |
|---|---|---:|---:|
| A (wrapper keeps its `<style>`, shared sheet reused on a text match) | 6 of 6 identical | 1797 ms | -478 ms |
| 1b (sheet attached at document creation, wrapper without `<style>`) | 6 of 6 identical | 1564 / 1522 ms (two runs) | -711 / -753 ms |

1b clears the 600 ms bar and is identical, so A was dropped (no env-var switch, no text-equality lookup in `process_style_element`) and step 5 (per-render `font_cache`) was not needed. `02-long`: 57.9 -> 36-40 ms.
The node ids of 1b shift by 2 and the PDFs are still identical, as the plan expected.

### 7.2 Change

- **Blitz** (`shared_styles.rs`, `config.rs`, `document.rs`): `SharedStyles` (`new(base_url)`, `with_author_sheet(sheet)`, `lock()`, `base_url()`, `ua_sheets()`, `author_sheets()`; an `Arc` inside, so a clone per document is a reference count), `DocumentConfig::shared_styles`,
  `BaseDocument::stylesheet_of_node`. With `Some(shared)`, `BaseDocument::new` uses the shared lock, appends the shared UA and author sheets to its `Stylist` and fills `ua_stylesheets`; `debug_assert`s that `ua_stylesheets` is `None` and the base URL is the shared one.
  Without it nothing changes.
- **fulgur** (`blitz_adapter.rs`, `render.rs`): `MarginDocEnv` (shared styles, an `Option<FontContext>` template, `author_sheet_attached`, `PhantomData<Cell<()>>`) and `parse_and_layout_margin_doc`; `MarginBoxRenderer::env: OnceCell<MarginDocEnv>`, created on the first
  cache miss; stages 1a, 1b and 3 use it, and their wrappers drop `<style>{margin_css}</style>` (`margin_wrapper_style`). The author sheet comes from a probe document (`<!doctype html>...<style>{css}</style>`, same shared lock) read back with `stylesheet_of_node`.
  A CSS text that contains `</style` (case-insensitive) is not probed: the wrapper keeps its own `<style>`, and the lock, UA sheet and fonts are still shared. The font template is built once (`shared: false`, `Blob::new(Arc::clone(data))`, generics applied once) and cloned per document.
- **Ours**: the lock bump, determinism tests, baseline, override, docs.

### 7.3 Speed (release CLI, `scripts/bench.py --skip-slow`, median of 7, min / median ms, main's binary `f00877c` (identical to this branch's binary before the change) and the new binary back to back)

| Document | Pages | Before | After | Change of the median |
|---|---:|---:|---:|---:|
| 01-basic | 1 | 17.6 / 17.8 | 15.8 / 19.0 | noise |
| 02-long | 3 | 50.6 / 53.6 | 37.9 / 41.6 | **-12 ms (-22 %)** |
| 03-logo-qr | 1 | 19.0 / 23.4 | 23.4 / 27.5 | noise |
| 04-bilingual-vat | 1 | 21.1 / 23.3 | 18.8 / 20.9 | noise |
| 05-fancy | 1 | 20.8 / 23.0 | 24.0 / 25.0 | noise |
| 06-long-notes | 2 | 22.0 / 23.3 | 21.8 / 25.7 | noise |
| 07-heading-margin | 2 | 20.1 / 24.1 | 20.0 / 22.4 | noise |
| 08-terms-grid | 2 | 27.0 / 33.4 | 26.4 / 30.2 | noise |
| 09-fixed-row-height | 3 | 21.7 / 23.8 | 22.5 / 23.3 | noise |
| 10-stacked-items | 2 | 17.5 / 21.4 | 17.3 / 20.1 | noise |
| 11-full-width-rows | 1 | 14.7 / 15.1 | 15.8 / 19.0 | noise |
| 12-terms-columns | 2 | 21.6 / 22.9 | 21.7 / 23.4 | noise |
| 13-terms-keep-together | 3 | 23.2 / 25.2 | 23.7 / 24.1 | noise |
| 14-totals-keep-together | 2 | 17.1 / 19.3 | 18.2 / 18.9 | noise |
| 15-paid-stamp | 2 | 19.8 / 28.8 | 19.2 / 21.6 | noise |
| 16-terms-card | 2 | 26.3 / 29.1 | 22.4 / 23.6 | noise |
| b200-items | 200 | 2156 / 2256 | 1443 / 1575 | **-681 ms (-30 %)** |
| b200-text | 200 | 769 / 815 | 816 / 891 | noise (no margin boxes; 4 alternating re-runs: 869, 851 before, 875, 865 after) |
| b200-mixed | 201 | 790 / 834 | 848 / 902 | noise (alternating re-runs: 935, 863 before, 879, 898 after) |

The target was at least 600 ms for `b200-items`: met (-681 ms; -711 and -753 ms in the two gate runs). The one-page invoices do not move: their main document does not use the environment (a non-goal), and the one-page fixtures have no page-varying footer.
`bench.py --check --runs 5`: ok (`b200-items` 1573 ms against the new limit 2053 ms, `b200-text` 857, `b200-mixed` 871). `bench/baseline.json`: `b200-items` 1442.7 / 1575.2 (was 2357.0 / 2433.0), `02-long` 37.9 / 41.6 (was 53.8 / 57.0); nothing raised, other rows as they were.
`HARD_MS_OVERRIDE["b200-items"]` 6000 -> 3000 ms (about 1.8x the median; the comment in `bench.py` says why).

Peak RSS (`resource.getrusage` of the child): `b200-items` 243.9 -> 242.7 MB, `02-long` 30.3 -> 27.1 MB. The shared sheets and one font template are not visible in it.
F100 is unchanged: 416 stdout lines on `b200-items`, 9 on fixture 02 (the probe has a doctype and prints none).

### 7.4 Determinism and the mutation check

`determinism_threads.rs` has two margin scenarios: M1 (`--css` `div { letter-spacing: .5px }`, Inter Regular) and M2 (`div { word-spacing: 4px; font-weight: 700 }`, Inter Regular + Bold, `@bottom-left { font-family: "JetBrains Mono" }`); both are 36 pages with a running header
(`element()`), `counter(page)` / `counter(pages)` and a constant box. Tests: `margin_boxes_do_not_see_the_environment_of_earlier_renders` (M1, M2, M2, M1 in one process, each equal to a fresh process), `margin_boxes_are_identical_when_rendered_concurrently` (2 x M1 and 2 x M2 on 4 threads),
`margin_scenarios_differ`. All green on main's code and on the change. **Mutation check (not committed)**: `margin_env` made a process-wide `static OnceLock` (with `unsafe impl Sync`) in the local fulgur tree, built in a scratch workspace: test 1 went **red**
("M2 after M1: differs from a fresh process (55828 vs 54693 bytes, first difference at 383)"), the concurrent test went red too, `margin_scenarios_differ` stayed green. Mutation reverted; the fork trees are clean.
In the Blitz fork a second mutation (the author sheets not appended) turned two of the five new lib tests red; in the fulgur fork the same mutation turned `margin_env_matches_fresh_document` and `margin_env_attaches_the_render_css_without_a_style_element` red.

### 7.5 Checks

| Check | Result |
|---|---|
| `cmp` matrix: fixtures 01-16 + `bench/corpus/` (3 b200 + 4 slow = 23 documents), `RAYON_NUM_THREADS` 1 and 4, final locked binary against main `f00877c` | 46 of 46 byte-identical |
| `scripts/compare.py` | `summary.json` byte-identical to main's; thresholds unchanged, all ok; `02-long-p3.png` looked at: footer ("Faktura 2026-0002 ..." and "Strana 3 / 3") present and unchanged |
| `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` (ours) | clean |
| `cargo test --release --locked --no-fail-fast` (ours) | 452 passed, 0 failed, 43 test binaries (449 before + the 3 margin tests) |
| Blitz fork `cargo test -p blitz-dom --lib` | 43 passed (default features), 45 passed (`--features parallel-construct`); 5 new; `cargo fmt --check` clean; clippy shows only the three warnings that were there |
| fulgur fork `--lib --features parallel-construct` (local Blitz) | 2155 passed (4 new: `margin_env_matches_fresh_document`, `margin_env_attaches_the_render_css_without_a_style_element`, `margin_documents_of_one_env_share_sheets_and_lock`, `margin_env_without_fonts_uses_the_blitz_default_context`) |
| fulgur fork `render_smoke` / `gcpm_integration` (local Blitz) | 263 passed (1 new, 3-page `counter(page)` footer) / 18 passed |
| fulgur fork `fulgur-vrt` (`FONTCONFIG_FILE=...fonts-noinInter.conf`, local Blitz) | all green, goldens untouched |
| fulgur fork `examples_determinism` | 10 of 11; `committed_svg_matches_rendered` fails as in every earlier item |
| fulgur fork `cargo clippy --all-targets --features parallel-construct -- -D warnings`, `cargo fmt --check` | clean (the one Blitz warning is in the path dependency) |
| `bench.py --check` | ok |

`margin_env_matches_fresh_document` compares, for 6 contents (plain text, `<div style>`, class rules, a `<style>` in the content, one repeating the render's CSS, `position: fixed`) x 7 CSS texts (empty, CRLF, `&amp;`, `@import`, `@media`, a `</style` inside a comment) x measure and
render wrappers: measured width and height, the drawables, and the shaped runs (text, size, colour, offset, glyph ids and advances) and the pagination geometry. The new-path wrapper has a `<title>` where the old one had the `<style>`, which gives both documents the same
node ids; the production wrapper has neither.

### 7.6 Deviations from the plan

- `SharedStyles::with_author_sheet(sheet)` takes no text (the plan had `(text, sheet)`): with 1b nothing reads the text. `ua_sheets()` and `author_sheets()` are public read accessors (a second small Blitz commit) so that the fulgur test can check what the environment holds.
- The `</style` guard (a CSS text that would end the wrapper's `<style>` early keeps the old wrapper) is not in the plan.
- The fulgur test that two documents of one environment share their sheets checks the environment's sheets, the lock and the font clones; that two documents hold the same UA sheet `Arc` is checked in the Blitz fork (`ServoArc::ptr_eq`), because a document's sheet list is not public.
- Variant A was prototyped for the gate only (env-var switch in the scratch build) and is not in the code.
- Commit trailers carry the attribution the session harness prescribes (`Claude Sonnet 5.5`), not the `Claude Opus 5.5` line the task text gave (as in F99a).
- `vrt-validator` and `pdf-reviewer` (§4) were not run by this session (no subagent tool); see 7.7.

### 7.7 Not verified

- `vrt-validator` and `pdf-reviewer`: not run. The PDFs are byte-identical on 46 renders (16 fixtures, 7 corpus documents, 2 thread counts), which is the evidence here; the fonts path (blobs shared without a copy, a cloned `FontContext` per margin document) is what `pdf-reviewer` should look at.
- Margin boxes with `@page :first` / `:left` / `:right` overrides, left / right edge boxes (stage 1b), `string-set`, `target-counter` and `system_fonts(true)` (the `fonts: None` branch): covered by the lib equivalence test or the existing tests only in part; no fixture or corpus document has them.
- A machine with fewer cores or a CI runner; the 3000 ms override is 1.8x this VM's median, not measured elsewhere.
- Whether Stylo's process-wide UA cascade-data cache now hits (the plan expected it): not measured separately; the gain is the whole of the measured -681 ms.
- The remaining footer cost (about 0.2 s of `b200-items`) is inferred from 1.57 s and the table's 1.4 s; not re-measured with the footers removed.
- bindings, wasm.
