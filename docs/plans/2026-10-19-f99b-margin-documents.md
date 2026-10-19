# F99b — `@page` margin-box documents (second F99 step)

Date: 2026-10-19. Finding: F99 (speed). Status: **planned**. Profile: `2026-10-19-f99-speed-profile.md` (C2a, C2b, C2c, §3). Previous step: `2026-10-19-f99a-parallel-construct.md`.
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

(Filled in at landing: commits, the bench table before/after, RSS, the `cmp` matrix, the mutation-check result, which of 1b and A was taken and why, deviations, what was not verified.)
